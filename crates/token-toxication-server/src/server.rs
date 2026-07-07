use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use axum::Router;
use axum_server::{Handle, tls_rustls::RustlsConfig};
use tokio::{net::TcpListener, sync::watch};

use crate::{
    acme::{AcmeManager, ChallengeStore, http01_router},
    config::{AcmeHttp01Config, Config, HttpsConfig},
};

pub async fn serve(
    config: Arc<Config>,
    https_config: HttpsConfig,
    app: Router,
) -> anyhow::Result<()> {
    match https_config {
        HttpsConfig::Off => serve_http(config, app).await,
        HttpsConfig::CertFiles {
            cert_path,
            key_path,
        } => serve_cert_files(config, cert_path, key_path, app).await,
        HttpsConfig::AcmeHttp01(acme_config) => serve_acme_http01(config, acme_config, app).await,
    }
}

async fn serve_http(config: Arc<Config>, app: Router) -> anyhow::Result<()> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("bind HTTP listener at {}", config.bind_addr))?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("serve HTTP listener")?;
    Ok(())
}

async fn serve_cert_files(
    config: Arc<Config>,
    cert_path: PathBuf,
    key_path: PathBuf,
    app: Router,
) -> anyhow::Result<()> {
    let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .with_context(|| {
            format!(
                "load TLS certificate {} and key {}",
                cert_path.display(),
                key_path.display()
            )
        })?;
    serve_rustls(config.bind_addr, tls_config, app).await
}

async fn serve_acme_http01(
    config: Arc<Config>,
    acme_config: AcmeHttp01Config,
    app: Router,
) -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = shutdown_channel();
    let acme_config = Arc::new(acme_config);
    let challenge_store = ChallengeStore::default();
    let http01_app = http01_router(challenge_store.clone());
    let http01_listener = TcpListener::bind(acme_config.http_bind_addr)
        .await
        .with_context(|| {
            format!(
                "bind ACME HTTP-01 listener at {}",
                acme_config.http_bind_addr
            )
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

    let acme_http = build_http_client().context("build ACME HTTP client")?;
    let manager = AcmeManager::new(acme_config, challenge_store, acme_http);
    let certificate = match manager.prepare_certificate().await {
        Ok(certificate) => certificate,
        Err(error) => {
            let _ = shutdown_tx.send(true);
            let _ = http01_task.await;
            return Err(error);
        }
    };
    let tls_config = RustlsConfig::from_pem_file(&certificate.cert_path, &certificate.key_path)
        .await
        .with_context(|| {
            format!(
                "load ACME certificate {} and key {}",
                certificate.cert_path.display(),
                certificate.key_path.display()
            )
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
            let https_result = (&mut https_task).await.context("join HTTPS task")?;
            renewal_task.abort();
            result.context("join ACME HTTP-01 task")?.context("serve ACME HTTP-01 listener")?;
            https_result
        }
        result = &mut https_task => {
            let _ = shutdown_tx.send(true);
            let http01_result = (&mut http01_task).await.context("join ACME HTTP-01 task")?;
            renewal_task.abort();
            http01_result.context("serve ACME HTTP-01 listener")?;
            result.context("join HTTPS task")?
        }
    }
}

async fn serve_rustls(
    bind_addr: SocketAddr,
    tls_config: RustlsConfig,
    app: Router,
) -> anyhow::Result<()> {
    let (_shutdown_tx, shutdown_rx) = shutdown_channel();
    serve_rustls_with_shutdown(bind_addr, tls_config, app, shutdown_rx).await
}

async fn serve_rustls_with_shutdown(
    bind_addr: SocketAddr,
    tls_config: RustlsConfig,
    app: Router,
    shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
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
        .context("serve HTTPS listener")?;
    Ok(())
}

fn build_http_client() -> anyhow::Result<aioduct::TokioClient> {
    aioduct::TokioClient::builder()
        .tls(aioduct::tls::RustlsConnector::with_webpki_roots())
        .user_agent("token-toxication-acme/0.1")
        .timeout(Duration::from_secs(120))
        .build()
        .context("build aioduct HTTP client")
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
