use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone)]
pub struct RouteFailure {
    pub provider_status: Option<&'static str>,
    pub route_status: &'static str,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub error: String,
    pub status_code: Option<u16>,
}

pub fn classify_response_failure(
    provider: &str,
    status: StatusCode,
    headers: &HeaderMap,
    _body: &[u8],
    now: DateTime<Utc>,
) -> RouteFailure {
    let error = response_error(status);
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return RouteFailure {
            provider_status: Some("blocked"),
            route_status: "degraded",
            cooldown_until: None,
            error,
            status_code: Some(status.as_u16()),
        };
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        let cooldown = if minimax_model_quota_exhausted(provider, headers) {
            Duration::hours(1)
        } else {
            Duration::seconds(60)
        };
        return RouteFailure {
            provider_status: None,
            route_status: "cooling_down",
            cooldown_until: Some(now + cooldown),
            error,
            status_code: Some(status.as_u16()),
        };
    }

    if status.is_server_error() {
        return RouteFailure {
            provider_status: None,
            route_status: "cooling_down",
            cooldown_until: Some(now + Duration::seconds(30)),
            error,
            status_code: Some(status.as_u16()),
        };
    }

    RouteFailure {
        provider_status: None,
        route_status: "degraded",
        cooldown_until: None,
        error,
        status_code: Some(status.as_u16()),
    }
}

pub fn classify_transport_failure(error: String, now: DateTime<Utc>) -> RouteFailure {
    RouteFailure {
        provider_status: None,
        route_status: "cooling_down",
        cooldown_until: Some(now + Duration::seconds(30)),
        error,
        status_code: None,
    }
}

pub fn classify_upstream_application_failure(
    code: Option<&str>,
    error: String,
    now: DateTime<Utc>,
) -> Option<RouteFailure> {
    let (status, cooldown) = match code {
        Some("server_error") => (StatusCode::BAD_GATEWAY, Duration::seconds(30)),
        Some("rate_limit_exceeded") => (StatusCode::TOO_MANY_REQUESTS, Duration::seconds(60)),
        _ => return None,
    };

    Some(RouteFailure {
        provider_status: None,
        route_status: "cooling_down",
        cooldown_until: Some(now + cooldown),
        error,
        status_code: Some(status.as_u16()),
    })
}

fn response_error(status: StatusCode) -> String {
    format!("upstream returned {}", status.as_u16())
}

fn minimax_model_quota_exhausted(provider: &str, headers: &HeaderMap) -> bool {
    if !provider.starts_with("minimax") {
        return false;
    }

    headers
        .get("x-model-quota-remaining")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split([',', ';']).any(|part| {
                let part = part.trim();
                part == "0"
                    || part
                        .rsplit_once('=')
                        .is_some_and(|(_, remaining)| remaining.trim() == "0")
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_rate_limit_cools_route_for_one_minute() {
        let now = Utc::now();
        let failure = classify_response_failure(
            "deepseek",
            StatusCode::TOO_MANY_REQUESTS,
            &HeaderMap::new(),
            b"{}",
            now,
        );

        assert_eq!(failure.provider_status, None);
        assert_eq!(failure.route_status, "cooling_down");
        assert_eq!(failure.cooldown_until, Some(now + Duration::seconds(60)));
    }

    #[test]
    fn minimax_model_quota_cools_only_route_for_one_hour() {
        let now = Utc::now();
        let mut headers = HeaderMap::new();
        headers.insert("x-model-quota-remaining", "MiniMax-M3=0".parse().unwrap());

        let failure = classify_response_failure(
            "minimax-coding-plan",
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            b"{}",
            now,
        );

        assert_eq!(failure.provider_status, None);
        assert_eq!(failure.route_status, "cooling_down");
        assert_eq!(failure.cooldown_until, Some(now + Duration::hours(1)));
    }

    #[test]
    fn authorization_failure_blocks_account() {
        let failure = classify_response_failure(
            "openai",
            StatusCode::UNAUTHORIZED,
            &HeaderMap::new(),
            b"{}",
            Utc::now(),
        );

        assert_eq!(failure.provider_status, Some("blocked"));
        assert_eq!(failure.route_status, "degraded");
        assert!(failure.cooldown_until.is_none());
    }

    #[test]
    fn application_server_errors_cool_the_route_for_thirty_seconds() {
        let now = Utc::now();
        let failure = classify_upstream_application_failure(
            Some("server_error"),
            "upstream stream failed (server_error)".to_string(),
            now,
        )
        .expect("classify server error");

        assert_eq!(failure.status_code, Some(502));
        assert_eq!(failure.route_status, "cooling_down");
        assert_eq!(failure.cooldown_until, Some(now + Duration::seconds(30)));
    }

    #[test]
    fn application_rate_limits_cool_the_route_for_sixty_seconds() {
        let now = Utc::now();
        let failure = classify_upstream_application_failure(
            Some("rate_limit_exceeded"),
            "upstream stream failed (rate_limit_exceeded)".to_string(),
            now,
        )
        .expect("classify rate limit");

        assert_eq!(failure.status_code, Some(429));
        assert_eq!(failure.route_status, "cooling_down");
        assert_eq!(failure.cooldown_until, Some(now + Duration::seconds(60)));
    }

    #[test]
    fn unknown_application_errors_do_not_change_route_health() {
        assert!(
            classify_upstream_application_failure(
                Some("provider_specific_error"),
                "upstream stream failed (provider_specific_error)".to_string(),
                Utc::now(),
            )
            .is_none()
        );
    }
}
