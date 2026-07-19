use std::time::Instant;

use aioduct::{
    RequestBuilderSend, Response,
    runtime::{ConnectorSend, RuntimePoll},
};
use axum::http::{HeaderMap, StatusCode, Uri};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    AppState,
    auth::extract_api_key,
    db::ProviderRouteSelection,
    error::AppError,
    models::{ApiKeyRecord, RequestLog, RequestSummary},
    routing::{RouteFailure, classify_response_failure, classify_transport_failure},
};

#[derive(Clone)]
pub(crate) struct RelayAttemptLog {
    pub path: String,
    pub upstream_url: Option<String>,
    pub request_summary: Option<RequestSummary>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

const CLIENT_CLOSED_REQUEST_STATUS: u16 = 499;

pub(crate) struct RelayAttempt {
    state: AppState,
    api_key_id: String,
    selection: ProviderRouteSelection,
    started: Instant,
}

pub(crate) struct AuthenticatedRelayAttempt {
    state: AppState,
    api_key_id: String,
    started: Instant,
}

impl RelayAttempt {
    pub(crate) async fn authenticate(
        state: &AppState,
        headers: &HeaderMap,
        query: Option<&str>,
    ) -> Result<AuthenticatedRelayAttempt, AppError> {
        let started = Instant::now();
        let api_key_id = authenticate_relay_api_key(state, headers, query)
            .await?
            .view
            .id;

        Ok(AuthenticatedRelayAttempt {
            state: state.clone(),
            api_key_id,
            started,
        })
    }

    pub(crate) fn selection(&self) -> &ProviderRouteSelection {
        &self.selection
    }

    pub(crate) async fn record_failure(
        &self,
        log: &RelayAttemptLog,
        failure: RouteFailure,
    ) -> Result<(), AppError> {
        self.record_failure_with_usage(log, failure, TokenUsage::default())
            .await
    }

    pub(crate) async fn record_failure_with_usage(
        &self,
        log: &RelayAttemptLog,
        failure: RouteFailure,
        usage: TokenUsage,
    ) -> Result<(), AppError> {
        record_route_failure_state(
            &self.state,
            &self.selection.account.account.id,
            &self.selection.route_id,
            &failure,
        )
        .await?;
        self.insert_request_log(
            log,
            failure
                .status_code
                .unwrap_or(StatusCode::BAD_GATEWAY.as_u16()),
            usage,
            Some(failure.error),
        )
        .await
    }

    pub(crate) async fn record_application_failure(
        &self,
        log: &RelayAttemptLog,
        status: StatusCode,
        usage: TokenUsage,
        error: String,
    ) -> Result<(), AppError> {
        self.insert_request_log(log, status.as_u16(), usage, Some(error))
            .await
    }

    pub(crate) async fn send<'request, R, C>(
        &self,
        request: RequestBuilderSend<'request, R, C>,
        log: &RelayAttemptLog,
    ) -> Result<Response, AppError>
    where
        R: RuntimePoll,
        C: ConnectorSend,
    {
        match request.send().await {
            Ok(response) => Ok(response),
            Err(error) => {
                self.record_failure(
                    log,
                    classify_transport_failure(error.to_string(), Utc::now()),
                )
                .await?;
                Err(AppError::Upstream(error.into()))
            }
        }
    }

    pub(crate) async fn record_response(
        &self,
        log: &RelayAttemptLog,
        status: StatusCode,
        headers: &HeaderMap,
        body: &[u8],
        usage: TokenUsage,
    ) -> Result<(), AppError> {
        let error = record_upstream_response_result(
            &self.state,
            &self.selection.account.account.id,
            &self.selection.route_id,
            &self.selection.account.account.provider,
            status,
            headers,
            body,
        )
        .await?;
        self.insert_request_log(log, status.as_u16(), usage, error)
            .await?;
        Ok(())
    }

    pub(crate) async fn record_client_disconnect(
        &self,
        log: &RelayAttemptLog,
        usage: TokenUsage,
    ) -> Result<(), AppError> {
        self.insert_request_log(
            log,
            CLIENT_CLOSED_REQUEST_STATUS,
            usage,
            Some("client disconnected before the upstream stream completed".to_string()),
        )
        .await
    }

    async fn insert_request_log(
        &self,
        log: &RelayAttemptLog,
        status_code: u16,
        usage: TokenUsage,
        error: Option<String>,
    ) -> Result<(), AppError> {
        self.state
            .db
            .insert_request_log(RequestLog {
                id: Uuid::new_v4().to_string(),
                api_key_id: self.api_key_id.clone(),
                provider_account_id: Some(self.selection.account.account.id.clone()),
                method: "POST".to_string(),
                path: log.path.clone(),
                model: Some(self.selection.public_model_id.clone()),
                upstream_model: Some(self.selection.upstream_model_id.clone()),
                upstream_url: log.upstream_url.as_deref().map(sanitize_upstream_url),
                request_summary: log.request_summary.clone(),
                status_code,
                latency_ms: self.started.elapsed().as_millis() as u64,
                input_tokens: usage.input_tokens,
                cached_input_tokens: usage.cached_input_tokens,
                output_tokens: usage.output_tokens,
                cost_usd: 0.0,
                created_at: Utc::now(),
                error,
            })
            .await?;
        Ok(())
    }
}

impl AuthenticatedRelayAttempt {
    pub(crate) async fn select(
        self,
        wire_api: &str,
        model: &str,
    ) -> Result<RelayAttempt, AppError> {
        let selection = self
            .state
            .db
            .select_provider_account_for_wire(wire_api, Some(model))
            .await?
            .ok_or_else(|| AppError::Forbidden("no active provider account is available".into()))?;

        Ok(RelayAttempt {
            state: self.state,
            api_key_id: self.api_key_id,
            selection,
            started: self.started,
        })
    }
}

pub(crate) async fn authenticate_relay_api_key(
    state: &AppState,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Result<ApiKeyRecord, AppError> {
    let api_key = extract_api_key(headers, query)
        .ok_or_else(|| AppError::Unauthorized("missing API key".into()))?;
    if !api_key.starts_with(&state.config.api_key_prefix) {
        return Err(AppError::Unauthorized("invalid API key prefix".into()));
    }
    state
        .db
        .validate_api_key(&api_key)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid or inactive API key".into()))
}

pub(crate) fn sanitize_upstream_url(url: &str) -> String {
    match url.parse::<Uri>() {
        Ok(uri) => match (uri.scheme_str(), uri.authority()) {
            (Some(scheme), Some(authority)) => format!("{scheme}://{}{}", authority, uri.path()),
            _ => url.split('?').next().unwrap_or(url).to_string(),
        },
        Err(_) => url.split('?').next().unwrap_or(url).to_string(),
    }
}

async fn record_upstream_response_result(
    state: &AppState,
    account_id: &str,
    route_id: &str,
    provider: &str,
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Option<String>, AppError> {
    if status.is_success() {
        state
            .db
            .mark_provider_result(account_id, "healthy", None)
            .await?;
        state
            .db
            .mark_route_success(route_id, status.as_u16())
            .await?;
        return Ok(None);
    }

    let failure = classify_response_failure(provider, status, headers, body, Utc::now());
    let error = failure.error.clone();
    record_route_failure_state(state, account_id, route_id, &failure).await?;
    Ok(Some(error))
}

async fn record_route_failure_state(
    state: &AppState,
    account_id: &str,
    route_id: &str,
    failure: &RouteFailure,
) -> Result<(), AppError> {
    if let Some(provider_status) = failure.provider_status {
        state
            .db
            .mark_provider_result(account_id, provider_status, Some(&failure.error))
            .await?;
    }
    state
        .db
        .mark_route_failure(
            route_id,
            failure.route_status,
            failure.status_code,
            &failure.error,
            failure.cooldown_until,
        )
        .await?;
    Ok(())
}
