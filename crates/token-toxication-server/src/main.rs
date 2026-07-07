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
    let config = Arc::new(config);
    let db = Db::open(&config.database_path)
        .await
        .map_err(|source| MainError::OpenDatabase {
            path: config.database_path.clone(),
            source,
        })?;
    let http = aioduct::TokioClient::builder()
        .tls(aioduct::tls::RustlsConnector::with_webpki_roots())
        .user_agent("token-toxication/0.1")
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|source| MainError::BuildHttpClient { source })?;

    let state = AppState {
        config: config.clone(),
        db,
        http,
        started_at: Utc::now(),
    };

    let app = app(state, config.static_dir.clone());
    server::serve(config, https_config, app).await?;
    Ok(())
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
