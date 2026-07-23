use std::{net::SocketAddr, path::PathBuf, time::Duration};

use clap::{Args, ValueEnum};

use crate::error::ConfigError;

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

    #[arg(long, env = "TT_RELAY_STREAM_IDLE_TIMEOUT_SECS", default_value_t = 60)]
    pub relay_stream_idle_timeout_secs: u64,

    #[arg(long, env = "TT_RELAY_STREAM_MAX_DURATION_SECS", default_value_t = 900)]
    pub relay_stream_max_duration_secs: u64,

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
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.https_config().map(|_| ())
    }

    pub fn https_config(&self) -> Result<HttpsConfig, ConfigError> {
        self.validate_relay_timeouts()?;
        match self.https_mode {
            HttpsMode::Off => Ok(HttpsConfig::Off),
            HttpsMode::CertFiles => {
                let cert_path = self
                    .tls_cert_path
                    .as_ref()
                    .ok_or(ConfigError::MissingTlsCertPath)?;
                let key_path = self
                    .tls_key_path
                    .as_ref()
                    .ok_or(ConfigError::MissingTlsKeyPath)?;
                if !cert_path.exists() {
                    return Err(ConfigError::MissingTlsCertFile {
                        path: cert_path.clone(),
                    });
                }
                if !key_path.exists() {
                    return Err(ConfigError::MissingTlsKeyFile {
                        path: key_path.clone(),
                    });
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
                    .ok_or(ConfigError::MissingAcmeIdentifier)?;
                if identifier.trim().is_empty() {
                    return Err(ConfigError::EmptyAcmeIdentifier);
                }
                let email = self
                    .acme_email
                    .as_deref()
                    .ok_or(ConfigError::MissingAcmeEmail)?;
                if email.trim().is_empty() {
                    return Err(ConfigError::EmptyAcmeEmail);
                }
                if self.bind_addr == self.acme_http_bind_addr {
                    return Err(ConfigError::DuplicateAcmeAndServiceListener);
                }
                if self.acme_http_bind_addr.port() != 80 && !self.acme_allow_nonstandard_http_port {
                    return Err(ConfigError::NonstandardAcmeHttpPort);
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

    pub fn relay_stream_idle_timeout(&self) -> Duration {
        Duration::from_secs(self.relay_stream_idle_timeout_secs)
    }

    pub fn relay_stream_max_duration(&self) -> Duration {
        Duration::from_secs(self.relay_stream_max_duration_secs)
    }

    fn validate_relay_timeouts(&self) -> Result<(), ConfigError> {
        if self.relay_stream_idle_timeout_secs == 0 {
            return Err(ConfigError::ZeroRelayStreamIdleTimeout);
        }
        if self.relay_stream_max_duration_secs == 0 {
            return Err(ConfigError::ZeroRelayStreamMaxDuration);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, path::PathBuf};

    use super::{Config, HttpsMode, LETS_ENCRYPT_PRODUCTION_DIRECTORY};
    use crate::error::ConfigError;

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
            relay_stream_idle_timeout_secs: 60,
            relay_stream_max_duration_secs: 900,
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

    #[test]
    fn relay_timeouts_must_be_positive() {
        let mut config = test_config();
        config.relay_stream_idle_timeout_secs = 0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::ZeroRelayStreamIdleTimeout)
        ));

        let mut config = test_config();
        config.relay_stream_max_duration_secs = 0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::ZeroRelayStreamMaxDuration)
        ));
    }
}
