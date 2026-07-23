use std::{io, net::SocketAddr, path::PathBuf};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use instant_acme::{AuthorizationStatus, OrderStatus};
use serde_json::json;
use tokio::task::JoinError;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("--tls-cert-path is required when --https-mode=cert-files")]
    MissingTlsCertPath,
    #[error("--tls-key-path is required when --https-mode=cert-files")]
    MissingTlsKeyPath,
    #[error("TLS certificate file does not exist: {}", path.display())]
    MissingTlsCertFile { path: PathBuf },
    #[error("TLS private key file does not exist: {}", path.display())]
    MissingTlsKeyFile { path: PathBuf },
    #[error("--acme-identifier is required when --https-mode=acme-http-01")]
    MissingAcmeIdentifier,
    #[error("--acme-identifier must not be empty")]
    EmptyAcmeIdentifier,
    #[error("--acme-email is required when --https-mode=acme-http-01")]
    MissingAcmeEmail,
    #[error("--acme-email must not be empty")]
    EmptyAcmeEmail,
    #[error("--bind-addr and --acme-http-bind-addr must be different")]
    DuplicateAcmeAndServiceListener,
    #[error(
        "--acme-http-bind-addr must listen on port 80 for HTTP-01; set \
         --acme-allow-nonstandard-http-port only for local ACME tests or \
         explicit public-port forwarding"
    )]
    NonstandardAcmeHttpPort,
    #[error("--relay-stream-idle-timeout-secs must be greater than zero")]
    ZeroRelayStreamIdleTimeout,
    #[error("--relay-stream-max-duration-secs must be greater than zero")]
    ZeroRelayStreamMaxDuration,
}

#[derive(Debug, thiserror::Error)]
pub enum AcmeError {
    #[error("ACME identifier must not be empty")]
    EmptyIdentifier,
    #[error("ACME identifier must be a domain name or IP address, not a URL")]
    IdentifierIsUrl,
    #[error("create ACME order: {source}")]
    CreateOrder {
        #[source]
        source: instant_acme::Error,
    },
    #[error("poll ACME order readiness: {source}")]
    PollOrderReady {
        #[source]
        source: instant_acme::Error,
    },
    #[error("ACME order did not become ready: {status:?}")]
    OrderNotReady { status: OrderStatus },
    #[error("finalize ACME order: {source}")]
    FinalizeOrder {
        #[source]
        source: instant_acme::Error,
    },
    #[error("poll ACME certificate: {source}")]
    PollCertificate {
        #[source]
        source: instant_acme::Error,
    },
    #[error("issued ACME certificate was not readable")]
    IssuedCertificateNotReadable,
    #[error("load ACME authorization: {source}")]
    LoadAuthorization {
        #[source]
        source: instant_acme::Error,
    },
    #[error("ACME authorization is not pending: {status:?}")]
    AuthorizationNotPending { status: AuthorizationStatus },
    #[error("ACME authorization did not include http-01 challenge")]
    MissingHttp01Challenge,
    #[error("mark ACME http-01 challenge ready: {source}")]
    MarkChallengeReady {
        #[source]
        source: instant_acme::Error,
    },
    #[error("restore ACME account: {source}")]
    RestoreAccount {
        #[source]
        source: instant_acme::Error,
    },
    #[error("create ACME account: {source}")]
    CreateAccount {
        #[source]
        source: instant_acme::Error,
    },
    #[error("read {}: {source}", path.display())]
    ReadFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("create {}: {source}", path.display())]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("write {}: {source}", path.display())]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("rename {} to {}: {source}", from.display(), to.display())]
    RenameFile {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("set permissions on {}: {source}", path.display())]
    SetPermissions {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} has no parent directory", path.display())]
    MissingParentDirectory { path: PathBuf },
    #[error("parse ACME account credentials: {source}")]
    ParseAccountCredentials {
        #[source]
        source: serde_json::Error,
    },
    #[error("serialize ACME account: {source}")]
    SerializeAccount {
        #[source]
        source: serde_json::Error,
    },
    #[error("serialize ACME account metadata: {source}")]
    SerializeAccountMetadata {
        #[source]
        source: serde_json::Error,
    },
    #[error("serialize ACME metadata: {source}")]
    SerializeCertificateMetadata {
        #[source]
        source: serde_json::Error,
    },
    #[error("parse ACME certificate metadata: {source}")]
    ParseCertificateMetadata {
        #[source]
        source: serde_json::Error,
    },
    #[error("parse ACME account metadata: {source}")]
    ParseAccountMetadata {
        #[source]
        source: serde_json::Error,
    },
    #[error("parse certificate PEM: {message}")]
    ParseCertificatePem { message: String },
    #[error("parse X.509 certificate: {message}")]
    ParseX509Certificate { message: String },
    #[error("certificate not_before is out of range")]
    CertificateNotBeforeOutOfRange,
    #[error("certificate not_after is out of range")]
    CertificateNotAfterOutOfRange,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("bind HTTP listener at {addr}: {source}")]
    BindHttp {
        addr: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("serve HTTP listener: {source}")]
    ServeHttp {
        #[source]
        source: io::Error,
    },
    #[error("load TLS certificate {} and key {}: {source}", cert_path.display(), key_path.display())]
    LoadTlsCertificate {
        cert_path: PathBuf,
        key_path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("bind ACME HTTP-01 listener at {addr}: {source}")]
    BindAcmeHttp01 {
        addr: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("build ACME HTTP client: {source}")]
    BuildAcmeHttpClient {
        #[source]
        source: aioduct::Error,
    },
    #[error(transparent)]
    Acme(#[from] AcmeError),
    #[error("join HTTPS task: {source}")]
    JoinHttps {
        #[source]
        source: JoinError,
    },
    #[error("join ACME HTTP-01 task: {source}")]
    JoinAcmeHttp01 {
        #[source]
        source: JoinError,
    },
    #[error("serve ACME HTTP-01 listener: {source}")]
    ServeAcmeHttp01 {
        #[source]
        source: io::Error,
    },
    #[error("serve HTTPS listener: {source}")]
    ServeHttps {
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum MainError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("open database at {}: {source}", path.display())]
    OpenDatabase {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("build HTTP client: {source}")]
    BuildHttpClient {
        #[source]
        source: aioduct::Error,
    },
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error("create OpenAPI output dir {}: {source}", path.display())]
    CreateOpenApiOutputDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("serialize OpenAPI document: {source}")]
    SerializeOpenApi {
        #[source]
        source: serde_json::Error,
    },
    #[error("write {}: {source}", path.display())]
    WriteOpenApi {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("upstream error: {0}")]
    Upstream(#[from] aioduct::Error),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Database(_) | Self::Upstream(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = Json(json!({
            "error": {
                "type": status.as_u16(),
                "message": self.to_string(),
            }
        }));
        (status, body).into_response()
    }
}
