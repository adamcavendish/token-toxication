use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use chrono::Utc;
use clap::{Parser, Subcommand};
use token_toxication_server::{AppState, app, config::Config, db::Db, server};
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
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    if let Some(Command::GenerateOpenapi { output }) = cli.command {
        return generate_openapi(output);
    }

    run_server(cli.config).await
}

async fn run_server(config: Config) -> anyhow::Result<()> {
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
        .with_context(|| format!("open database at {}", config.database_path.display()))?;
    let http = aioduct::TokioClient::builder()
        .tls(aioduct::tls::RustlsConnector::with_webpki_roots())
        .user_agent("token-toxication/0.1")
        .timeout(Duration::from_secs(300))
        .build()
        .context("build HTTP client")?;

    let state = AppState {
        config: config.clone(),
        db,
        http,
        started_at: Utc::now(),
    };

    let app = app(state, config.static_dir.clone());
    server::serve(config, https_config, app).await
}

fn generate_openapi(output: PathBuf) -> anyhow::Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create OpenAPI output dir {}", parent.display()))?;
    }

    let spec = token_toxication_server::openapi::ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&spec).context("serialize OpenAPI document")?;
    fs::write(&output, json).with_context(|| format!("write {}", output.display()))?;
    println!("wrote {}", output.display());
    Ok(())
}
