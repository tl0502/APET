// LLM Provider 错误分类（依 issue #12 §3）。
//
// 设计：错误粒度面向 UI 用户提示（401 → "API Key 错了" / 429 → "限流稍后" / 5xx →
// "服务端炸了" / Network → "看网线" / Cancelled → 不提示），不堆栈式包装。
// 不暴露 reqwest::Error 类型给上游避免依赖泄漏。

use thiserror::Error;

use super::types::Usage;

#[derive(Debug, Clone, Error)]
pub enum LLMError {
    #[error("network error: {0}")]
    Network(String),
    #[error("auth failed (HTTP 401/403): {0}")]
    AuthFailed(String),
    #[error("rate limited (HTTP 429): {0}")]
    RateLimit(String),
    #[error("bad request (HTTP 4xx): {0}")]
    BadRequest(String),
    #[error("server error (HTTP 5xx): {0}")]
    ServerError(String),
    /// 取消时若已经收到过 usage chunk（OpenAI stream_options.include_usage 在中段就可能下发）
    /// 通过 `partial_usage` 透传给 ChatService，让取消流的 Done 事件仍能报已烧 token 数。
    #[error("cancelled by caller")]
    Cancelled { partial_usage: Option<Usage> },
    #[error("parse error: {0}")]
    ParseError(String),
}

impl From<reqwest::Error> for LLMError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() || err.is_connect() {
            return Self::Network(err.to_string());
        }
        if let Some(status) = err.status() {
            return classify_status(status, &err.to_string());
        }
        Self::Network(err.to_string())
    }
}

/// HTTP status → LLMError 分类（chat_stream 在 response 不成功时调用）。
pub(crate) fn classify_status(status: reqwest::StatusCode, body: &str) -> LLMError {
    match status.as_u16() {
        401 | 403 => LLMError::AuthFailed(format!("{}: {}", status, body)),
        429 => LLMError::RateLimit(format!("{}: {}", status, body)),
        400..=499 => LLMError::BadRequest(format!("{}: {}", status, body)),
        500..=599 => LLMError::ServerError(format!("{}: {}", status, body)),
        _ => LLMError::ServerError(format!("unexpected status {}: {}", status, body)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn classify_401_to_auth_failed() {
        match classify_status(StatusCode::UNAUTHORIZED, "Invalid API Key") {
            LLMError::AuthFailed(_) => {}
            other => panic!("expected AuthFailed, got {other:?}"),
        }
    }

    #[test]
    fn classify_403_to_auth_failed() {
        assert!(matches!(
            classify_status(StatusCode::FORBIDDEN, "Forbidden"),
            LLMError::AuthFailed(_)
        ));
    }

    #[test]
    fn classify_429_to_rate_limit() {
        assert!(matches!(
            classify_status(StatusCode::TOO_MANY_REQUESTS, "slow down"),
            LLMError::RateLimit(_)
        ));
    }

    #[test]
    fn classify_400_to_bad_request() {
        assert!(matches!(
            classify_status(StatusCode::BAD_REQUEST, "model not found"),
            LLMError::BadRequest(_)
        ));
    }

    #[test]
    fn classify_500_to_server_error() {
        assert!(matches!(
            classify_status(StatusCode::INTERNAL_SERVER_ERROR, "boom"),
            LLMError::ServerError(_)
        ));
        assert!(matches!(
            classify_status(StatusCode::BAD_GATEWAY, "upstream"),
            LLMError::ServerError(_)
        ));
    }
}
