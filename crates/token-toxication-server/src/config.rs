use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, bail};
use clap::{Args, ValueEnum};

pub const LETS_ENCRYPT_PRODUCTION_DIRECTORY: &str =
    "https://acme-v02.api.letsencrypt.org/directory";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HttpsMode {
    /// Serve plain HTTP on bind-addr.
    Off,
    /// Serve HTTPS on bind-addr from certificate and private-key files.
    #[value(name = "cert-files")]
    CertFiles,
    /// Serve HTTPS on bind-addr with ACME HTTP-01 managed certificates.
    #[value(name = "acme-http-01")]
    AcmeHttp01,
}

#[derive(Debug, Clone)]
pub enum HttpsConfig {
    Off,
    CertFiles {
        cert_path: PathBuf,
        key_path: PathBuf,
    },
    AcmeHttp01(AcmeHttp01Config),
}

#[derive(Debug, Clone)]
pub struct AcmeHttp01Config {
    pub identifier: String,
    pub email: String,
    pub http_bind_addr: SocketAddr,
    pub cert_dir: PathBuf,
    pub directory_url: String,
}

#[derive(Debug, Clone, Args)]
pub struct Config {
    #[arg(long, env = "TT_BIND_ADDR", default_value = "0.0.0.0:3000")]
    pub bind_addr: SocketAddr,

    #[arg(long, env = "TT_HTTPS_MODE", value_enum, default_value = "off")]
    pub https_mode: HttpsMode,

    #[arg(long, env = "TT_TLS_CERT_PATH")]
    pub tls_cert_path: Option<PathBuf>,

    #[arg(long, env = "TT_TLS_KEY_PATH")]
    pub tls_key_path: Option<PathBuf>,

    #[arg(long, env = "TT_ACME_IDENTIFIER")]
    pub acme_identifier: Option<String>,

    #[arg(long, env = "TT_ACME_EMAIL")]
    pub acme_email: Option<String>,

    #[arg(long, env = "TT_ACME_HTTP_BIND_ADDR", default_value = "0.0.0.0:80")]
    pub acme_http_bind_addr: SocketAddr,

    #[arg(
        long,
        env = "TT_ACME_ALLOW_NONSTANDARD_HTTP_PORT",
        default_value_t = false,
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub acme_allow_nonstandard_http_port: bool,

    #[arg(long, env = "TT_ACME_CERT_DIR", default_value = "data/acme")]
    pub acme_cert_dir: PathBuf,

    #[arg(
        long,
        env = "TT_ACME_DIRECTORY_URL",
        default_value = LETS_ENCRYPT_PRODUCTION_DIRECTORY
    )]
    pub acme_directory_url: String,

    #[arg(
        long,
        env = "TT_DATABASE_PATH",
        default_value = "data/token-toxication.sqlite3"
    )]
    pub database_path: PathBuf,

    #[arg(long, env = "TT_STATIC_DIR", default_value = "apps/admin/dist")]
    pub static_dir: PathBuf,

    #[arg(long, env = "TT_ADMIN_USERNAME", default_value = "admin")]
    pub admin_username: String,

    #[arg(
        long,
        env = "TT_ADMIN_PASSWORD",
        default_value = "change-this-password"
    )]
    pub admin_password: String,

    #[arg(long, env = "TT_API_KEY_PREFIX", default_value = "tokentoxication-")]
    pub api_key_prefix: String,

    #[arg(
        long,
        env = "TT_ALLOW_DEFAULT_ADMIN_PASSWORD",
        default_value_t = false,
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub allow_default_admin_password: bool,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.https_config().map(|_| ())
    }

    pub fn https_config(&self) -> anyhow::Result<HttpsConfig> {
        match self.https_mode {
            HttpsMode::Off => Ok(HttpsConfig::Off),
            HttpsMode::CertFiles => {
                let cert_path = self
                    .tls_cert_path
                    .as_ref()
                    .context("--tls-cert-path is required when --https-mode=cert-files")?;
                let key_path = self
                    .tls_key_path
                    .as_ref()
                    .context("--tls-key-path is required when --https-mode=cert-files")?;
                if !cert_path.exists() {
                    bail!(
                        "TLS certificate file does not exist: {}",
                        cert_path.display()
                    );
                }
                if !key_path.exists() {
                    bail!(
                        "TLS private key file does not exist: {}",
                        key_path.display()
                    );
                }
                Ok(HttpsConfig::CertFiles {
                    cert_path: cert_path.clone(),
                    key_path: key_path.clone(),
                })
            }
            HttpsMode::AcmeHttp01 => {
                let identifier = self
                    .acme_identifier
                    .as_deref()
                    .context("--acme-identifier is required when --https-mode=acme-http-01")?;
                if identifier.trim().is_empty() {
                    bail!("--acme-identifier must not be empty");
                }
                let email = self
                    .acme_email
                    .as_deref()
                    .context("--acme-email is required when --https-mode=acme-http-01")?;
                if email.trim().is_empty() {
                    bail!("--acme-email must not be empty");
                }
                if self.bind_addr == self.acme_http_bind_addr {
                    bail!("--bind-addr and --acme-http-bind-addr must be different");
                }
                if self.acme_http_bind_addr.port() != 80 && !self.acme_allow_nonstandard_http_port {
                    bail!(
                        "--acme-http-bind-addr must listen on port 80 for HTTP-01; \
                         set --acme-allow-nonstandard-http-port only for local ACME \
                         tests or explicit public-port forwarding"
                    );
                }
                Ok(HttpsConfig::AcmeHttp01(AcmeHttp01Config {
                    identifier: identifier.trim().to_string(),
                    email: email.trim().to_string(),
                    http_bind_addr: self.acme_http_bind_addr,
                    cert_dir: self.acme_cert_dir.clone(),
                    directory_url: self.acme_directory_url.clone(),
                }))
            }
        }
    }

    pub fn warn_if_default_admin_password(&self) {
        if self.admin_password == "change-this-password" && !self.allow_default_admin_password {
            tracing::warn!(
                "using default admin password; set TT_ADMIN_PASSWORD before exposing this service"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, path::PathBuf};

    use super::{Config, HttpsMode, LETS_ENCRYPT_PRODUCTION_DIRECTORY};

    fn test_config() -> Config {
        Config {
            bind_addr: "127.0.0.1:3000".parse().unwrap(),
            https_mode: HttpsMode::Off,
            tls_cert_path: None,
            tls_key_path: None,
            acme_identifier: None,
            acme_email: None,
            acme_http_bind_addr: "127.0.0.1:80".parse::<SocketAddr>().unwrap(),
            acme_allow_nonstandard_http_port: false,
            acme_cert_dir: PathBuf::from("data/acme"),
            acme_directory_url: LETS_ENCRYPT_PRODUCTION_DIRECTORY.to_string(),
            database_path: PathBuf::from("data/token-toxication.sqlite3"),
            static_dir: PathBuf::from("apps/admin/dist"),
            admin_username: "admin".to_string(),
            admin_password: "change-this-password".to_string(),
            api_key_prefix: "tokentoxication-".to_string(),
            allow_default_admin_password: false,
        }
    }

    #[test]
    fn off_mode_allows_empty_tls_config() {
        assert!(test_config().validate().is_ok());
    }

    #[test]
    fn cert_files_mode_requires_cert_and_key_paths() {
        let mut config = test_config();
        config.https_mode = HttpsMode::CertFiles;
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("--tls-cert-path is required"));
    }

    #[test]
    fn acme_http01_mode_requires_identifier_and_email() {
        let mut config = test_config();
        config.https_mode = HttpsMode::AcmeHttp01;
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("--acme-identifier is required"));

        config.acme_identifier = Some("127.0.0.1".to_string());
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("--acme-email is required"));
    }

    #[test]
    fn acme_http01_mode_rejects_same_listener() {
        let mut config = test_config();
        config.https_mode = HttpsMode::AcmeHttp01;
        config.acme_identifier = Some("127.0.0.1".to_string());
        config.acme_email = Some("ops@example.com".to_string());
        config.acme_http_bind_addr = config.bind_addr;
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("must be different"));
    }

    #[test]
    fn acme_http01_mode_rejects_nonstandard_http_port_by_default() {
        let mut config = test_config();
        config.https_mode = HttpsMode::AcmeHttp01;
        config.acme_identifier = Some("127.0.0.1".to_string());
        config.acme_email = Some("ops@example.com".to_string());
        config.acme_http_bind_addr = "127.0.0.1:8080".parse().unwrap();
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("must listen on port 80"));
    }

    #[test]
    fn acme_http01_mode_allows_nonstandard_http_port_with_escape_hatch() {
        let mut config = test_config();
        config.https_mode = HttpsMode::AcmeHttp01;
        config.acme_identifier = Some("127.0.0.1".to_string());
        config.acme_email = Some("ops@example.com".to_string());
        config.acme_http_bind_addr = "127.0.0.1:8080".parse().unwrap();
        config.acme_allow_nonstandard_http_port = true;
        assert!(config.validate().is_ok());
    }
}
