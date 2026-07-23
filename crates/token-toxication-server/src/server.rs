use std::{
    future::{Future, IntoFuture},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use axum::Router;
use axum_server::{Handle, tls_rustls::RustlsConfig};
use tokio::{net::TcpListener, sync::watch};

use crate::{
    acme::{AcmeManager, ChallengeStore, http01_router},
    config::{AcmeHttp01Config, Config, HttpsConfig},
    error::ServerError,
};

const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct ShutdownSignal {
    sender: watch::Sender<bool>,
    receiver: watch::Receiver<bool>,
}

impl ShutdownSignal {
    pub fn new() -> Self {
        let signal = Self::channel();
        let signal_handler = signal.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            signal_handler.cancel();
        });
        signal
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::channel()
    }

    pub(crate) fn cancel(&self) {
        let _ = self.sender.send(true);
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.receiver.clone()
    }

    fn channel() -> Self {
        let (sender, receiver) = watch::channel(false);
        Self { sender, receiver }
    }
}

impl Default for ShutdownSignal {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn serve(
    config: Arc<Config>,
    https_config: HttpsConfig,
    app: Router,
    shutdown: ShutdownSignal,
) -> Result<(), ServerError> {
    match https_config {
        HttpsConfig::Off => serve_http(config, app, shutdown).await,
        HttpsConfig::CertFiles {
            cert_path,
            key_path,
        } => serve_cert_files(config, cert_path, key_path, app, shutdown).await,
        HttpsConfig::AcmeHttp01(acme_config) => {
            serve_acme_http01(config, acme_config, app, shutdown).await
        }
    }
}

async fn serve_http(
    config: Arc<Config>,
    app: Router,
    shutdown: ShutdownSignal,
) -> Result<(), ServerError> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|source| ServerError::BindHttp {
            addr: config.bind_addr,
            source,
        })?;
    tracing::info!("listening on http://{}", config.bind_addr);
    let graceful_shutdown = shutdown.subscribe();
    serve_until_shutdown_deadline(
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(wait_for_shutdown(graceful_shutdown))
        .into_future(),
        shutdown,
        "HTTP",
    )
    .await
    .map_err(|source| ServerError::ServeHttp { source })?;
    Ok(())
}

async fn serve_cert_files(
    config: Arc<Config>,
    cert_path: PathBuf,
    key_path: PathBuf,
    app: Router,
    shutdown: ShutdownSignal,
) -> Result<(), ServerError> {
    let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .map_err(|source| ServerError::LoadTlsCertificate {
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            source,
        })?;
    serve_rustls(config.bind_addr, tls_config, app, shutdown).await
}

async fn serve_acme_http01(
    config: Arc<Config>,
    acme_config: AcmeHttp01Config,
    app: Router,
    shutdown: ShutdownSignal,
) -> Result<(), ServerError> {
    let acme_config = Arc::new(acme_config);
    let challenge_store = ChallengeStore::default();
    let http01_app = http01_router(challenge_store.clone());
    let http01_listener = TcpListener::bind(acme_config.http_bind_addr)
        .await
        .map_err(|source| ServerError::BindAcmeHttp01 {
            addr: acme_config.http_bind_addr,
            source,
        })?;
    tracing::info!(
        "listening for ACME HTTP-01 challenges on http://{}",
        acme_config.http_bind_addr
    );
    let http01_shutdown = shutdown.clone();
    let http01_task = tokio::spawn(async move {
        let graceful_shutdown = http01_shutdown.subscribe();
        serve_until_shutdown_deadline(
            axum::serve(
                http01_listener,
                http01_app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(wait_for_shutdown(graceful_shutdown))
            .into_future(),
            http01_shutdown,
            "ACME HTTP-01",
        )
        .await
    });

    let acme_http = build_http_client()?;
    let manager = AcmeManager::new(acme_config, challenge_store, acme_http);
    let certificate = match manager.prepare_certificate().await {
        Ok(certificate) => certificate,
        Err(error) => {
            shutdown.cancel();
            let _ = http01_task.await;
            return Err(ServerError::Acme(error));
        }
    };
    let tls_config = RustlsConfig::from_pem_file(&certificate.cert_path, &certificate.key_path)
        .await
        .map_err(|source| ServerError::LoadTlsCertificate {
            cert_path: certificate.cert_path.clone(),
            key_path: certificate.key_path.clone(),
            source,
        })?;
    let renewal_task = manager
        .clone()
        .spawn_renewal(tls_config.clone(), shutdown.subscribe());
    let mut https_task = tokio::spawn(serve_rustls_with_shutdown(
        config.bind_addr,
        tls_config,
        app,
        shutdown.clone(),
    ));
    let mut http01_task = http01_task;

    tokio::select! {
        result = &mut http01_task => {
            shutdown.cancel();
            let https_result = (&mut https_task)
                .await
                .map_err(|source| ServerError::JoinHttps { source })?;
            renewal_task.abort();
            result
                .map_err(|source| ServerError::JoinAcmeHttp01 { source })?
                .map_err(|source| ServerError::ServeAcmeHttp01 { source })?;
            https_result
        }
        result = &mut https_task => {
            shutdown.cancel();
            let http01_result = (&mut http01_task)
                .await
                .map_err(|source| ServerError::JoinAcmeHttp01 { source })?;
            renewal_task.abort();
            http01_result.map_err(|source| ServerError::ServeAcmeHttp01 { source })?;
            result.map_err(|source| ServerError::JoinHttps { source })?
        }
    }
}

async fn serve_rustls(
    bind_addr: SocketAddr,
    tls_config: RustlsConfig,
    app: Router,
    shutdown: ShutdownSignal,
) -> Result<(), ServerError> {
    serve_rustls_with_shutdown(bind_addr, tls_config, app, shutdown).await
}

async fn serve_rustls_with_shutdown(
    bind_addr: SocketAddr,
    tls_config: RustlsConfig,
    app: Router,
    shutdown: ShutdownSignal,
) -> Result<(), ServerError> {
    let handle = Handle::new();
    let shutdown_handle = handle.clone();
    let handle_shutdown = shutdown.clone();
    tokio::spawn(async move {
        wait_for_shutdown(handle_shutdown.subscribe()).await;
        shutdown_handle.graceful_shutdown(Some(Duration::from_secs(10)));
    });

    tracing::info!("listening on https://{}", bind_addr);
    serve_until_shutdown_deadline(
        axum_server::bind_rustls(bind_addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>()),
        shutdown,
        "HTTPS",
    )
    .await
    .map_err(|source| ServerError::ServeHttps { source })?;
    Ok(())
}

fn build_http_client() -> Result<aioduct::TokioClient, ServerError> {
    aioduct::TokioClient::builder()
        .tls(aioduct::tls::RustlsConnector::with_webpki_roots())
        .user_agent("token-toxication-acme/0.1")
        .timeout(Duration::from_secs(120))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|source| ServerError::BuildAcmeHttpClient { source })
}

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    if *shutdown_rx.borrow() {
        return;
    }
    while shutdown_rx.changed().await.is_ok() {
        if *shutdown_rx.borrow() {
            return;
        }
    }
}

async fn serve_until_shutdown_deadline<F>(
    server: F,
    shutdown: ShutdownSignal,
    listener: &'static str,
) -> Result<(), std::io::Error>
where
    F: Future<Output = Result<(), std::io::Error>>,
{
    serve_until_shutdown_deadline_with_timeout(
        server,
        shutdown,
        listener,
        GRACEFUL_SHUTDOWN_TIMEOUT,
    )
    .await
}

async fn serve_until_shutdown_deadline_with_timeout<F>(
    server: F,
    shutdown: ShutdownSignal,
    listener: &'static str,
    graceful_shutdown_timeout: Duration,
) -> Result<(), std::io::Error>
where
    F: Future<Output = Result<(), std::io::Error>>,
{
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result,
        () = wait_for_shutdown(shutdown.subscribe()) => {
            match tokio::time::timeout(graceful_shutdown_timeout, &mut server).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::warn!(
                        listener,
                        timeout_secs = graceful_shutdown_timeout.as_secs(),
                        "graceful shutdown deadline elapsed; ending listener"
                    );
                    Ok(())
                }
            }
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use std::{io, time::Duration};

    use super::{ShutdownSignal, serve_until_shutdown_deadline_with_timeout};

    #[tokio::test]
    async fn shutdown_deadline_ends_an_unresponsive_listener() {
        let shutdown = ShutdownSignal::for_test();
        shutdown.cancel();

        let result = tokio::time::timeout(
            Duration::from_millis(100),
            serve_until_shutdown_deadline_with_timeout(
                std::future::pending::<Result<(), io::Error>>(),
                shutdown,
                "test",
                Duration::from_millis(10),
            ),
        )
        .await
        .expect("shutdown deadline must end the listener promptly");

        assert!(result.is_ok());
    }
}
