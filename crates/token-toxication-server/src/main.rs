use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use chrono::Utc;
use clap::{Parser, Subcommand};
use token_toxication_server::{AppState, app, config::Config, db::Db, error::MainError, server};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi as _;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    config: Config,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate the OpenAPI JSON document and exit.
    GenerateOpenapi {
        #[arg(short, long, default_value = "openapi/token-toxication.openapi.json")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let cli = Cli::parse();

    if let Some(Command::GenerateOpenapi { output }) = cli.command {
        return generate_openapi(output);
    }

    run_server(cli.config).await
}

async fn run_server(config: Config) -> Result<(), MainError> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "token_toxication_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let https_config = config.https_config()?;
    config.warn_if_default_admin_password();
    let relay_stream_idle_timeout = config.relay_stream_idle_timeout();
    let relay_stream_max_duration = config.relay_stream_max_duration();
    let config = Arc::new(config);
    let db = Db::open(&config.database_path)
        .await
        .map_err(|source| MainError::OpenDatabase {
            path: config.database_path.clone(),
            source,
        })?;
    let http = build_http_client(
        "token-toxication/0.1",
        Duration::from_secs(300),
        relay_stream_idle_timeout,
        false,
    )
    .map_err(|source| MainError::BuildHttpClient { source })?;
    let gemini_http = build_http_client(
        "token-toxication/0.1",
        Duration::from_secs(300),
        relay_stream_idle_timeout,
        true,
    )
    .map_err(|source| MainError::BuildHttpClient { source })?;
    let shutdown = server::ShutdownSignal::new();

    let state = AppState {
        config: config.clone(),
        db,
        http,
        gemini_http,
        antigravity_oauth: Default::default(),
        relay_stream_idle_timeout,
        relay_stream_max_duration,
        shutdown: shutdown.clone(),
        started_at: Utc::now(),
    };

    let app = app(state, config.static_dir.clone());
    server::serve(config, https_config, app, shutdown).await?;
    Ok(())
}

fn build_http_client(
    user_agent: &str,
    timeout: Duration,
    read_timeout: Duration,
    http1_only: bool,
) -> Result<aioduct::TokioClient, aioduct::Error> {
    let mut tls = aioduct::tls::RustlsConnector::with_webpki_roots();
    if http1_only {
        tls.config_mut().alpn_protocols = vec![b"http/1.1".to_vec()];
    }
    aioduct::TokioClient::builder()
        .tls(tls)
        .user_agent(user_agent)
        .timeout(timeout)
        .read_timeout(read_timeout)
        .build()
}

fn generate_openapi(output: PathBuf) -> Result<(), MainError> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|source| MainError::CreateOpenApiOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let spec = token_toxication_server::openapi::ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&spec)
        .map_err(|source| MainError::SerializeOpenApi { source })?;
    fs::write(&output, json).map_err(|source| MainError::WriteOpenApi {
        path: output.clone(),
        source,
    })?;
    println!("wrote {}", output.display());
    Ok(())
}
