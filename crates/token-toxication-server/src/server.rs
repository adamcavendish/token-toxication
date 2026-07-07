use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::Router;
use axum_server::{Handle, tls_rustls::RustlsConfig};
use tokio::{net::TcpListener, sync::watch};

use crate::{
    acme::{AcmeManager, ChallengeStore, http01_router},
    config::{AcmeHttp01Config, Config, HttpsConfig},
    error::ServerError,
};

pub async fn serve(
    config: Arc<Config>,
    https_config: HttpsConfig,
    app: Router,
) -> Result<(), ServerError> {
    match https_config {
        HttpsConfig::Off => serve_http(config, app).await,
        HttpsConfig::CertFiles {
            cert_path,
            key_path,
        } => serve_cert_files(config, cert_path, key_path, app).await,
        HttpsConfig::AcmeHttp01(acme_config) => serve_acme_http01(config, acme_config, app).await,
    }
}

async fn serve_http(config: Arc<Config>, app: Router) -> Result<(), ServerError> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|source| ServerError::BindHttp {
            addr: config.bind_addr,
            source,
        })?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|source| ServerError::ServeHttp { source })?;
    Ok(())
}

async fn serve_cert_files(
    config: Arc<Config>,
    cert_path: PathBuf,
    key_path: PathBuf,
    app: Router,
) -> Result<(), ServerError> {
    let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .map_err(|source| ServerError::LoadTlsCertificate {
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            source,
        })?;
    serve_rustls(config.bind_addr, tls_config, app).await
}

async fn serve_acme_http01(
    config: Arc<Config>,
    acme_config: AcmeHttp01Config,
    app: Router,
) -> Result<(), ServerError> {
    let (shutdown_tx, shutdown_rx) = shutdown_channel();
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
    let http01_shutdown = shutdown_rx.clone();
    let http01_task = tokio::spawn(async move {
        axum::serve(
            http01_listener,
            http01_app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(wait_for_shutdown(http01_shutdown))
        .await
    });

    let acme_http = build_http_client()?;
    let manager = AcmeManager::new(acme_config, challenge_store, acme_http);
    let certificate = match manager.prepare_certificate().await {
        Ok(certificate) => certificate,
        Err(error) => {
            let _ = shutdown_tx.send(true);
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
        .spawn_renewal(tls_config.clone(), shutdown_rx.clone());
    let mut https_task = tokio::spawn(serve_rustls_with_shutdown(
        config.bind_addr,
        tls_config,
        app,
        shutdown_rx.clone(),
    ));
    let mut http01_task = http01_task;

    tokio::select! {
        result = &mut http01_task => {
            let _ = shutdown_tx.send(true);
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
            let _ = shutdown_tx.send(true);
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
) -> Result<(), ServerError> {
    let (_shutdown_tx, shutdown_rx) = shutdown_channel();
    serve_rustls_with_shutdown(bind_addr, tls_config, app, shutdown_rx).await
}

async fn serve_rustls_with_shutdown(
    bind_addr: SocketAddr,
    tls_config: RustlsConfig,
    app: Router,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<(), ServerError> {
    let handle = Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        wait_for_shutdown(shutdown_rx).await;
        shutdown_handle.graceful_shutdown(Some(Duration::from_secs(10)));
    });

    tracing::info!("listening on https://{}", bind_addr);
    axum_server::bind_rustls(bind_addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .map_err(|source| ServerError::ServeHttps { source })?;
    Ok(())
}

fn build_http_client() -> Result<aioduct::TokioClient, ServerError> {
    aioduct::TokioClient::builder()
        .tls(aioduct::tls::RustlsConnector::with_webpki_roots())
        .user_agent("token-toxication-acme/0.1")
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|source| ServerError::BuildAcmeHttpClient { source })
}

fn shutdown_channel() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = signal_tx.send(true);
    });
    (shutdown_tx, shutdown_rx)
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
