// 探测 OpenAI 兼容 provider 的 /models 端点；ProviderDrawer 探测按钮路径。
//
// 单次 GET {base_url}/models，Authorization: Bearer {api_key}（空 key 也带，无 key
// 端点会返 401 → AuthFailed；用户拿到提示后切手填）。Ollama 旧版 /v1/models 返 404
// → BadRequest 给前端 toast；不在此层为 ollama 开 special case（plan 决策）。
//
// 不复用 OpenAIProvider —— 那货建客户端要 model_default 等参数；这里只想一个无状态
// GET，自己造 reqwest::Client + 20s timeout。/models 是非流式短请求，秒级返不到就是
// 端点真有问题，没必要走 chat 那套 connect_timeout + read_timeout + 大兜底总 timeout。

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::{classify_status, LLMError};

const PROBE_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

/// 调 {base_url}/models 拿模型 id 列表。
///
/// - base_url：与 OpenAIProvider 一致 trim 末尾斜杠；调用方传啥就用啥（不在此推断 /v1）
/// - api_key：可空；空 key 端点 401 → AuthFailed
/// - HTTP 非 2xx → classify_status 复用
/// - 2xx 但 JSON 解析失败 / data 缺失 → ParseError
pub async fn probe_models(base_url: &str, api_key: &str) -> Result<Vec<String>, LLMError> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(PROBE_TIMEOUT_SECS))
        .build()
        .map_err(|e| LLMError::Network(format!("build reqwest client: {e}")))?;

    let mut request = client.get(&url);
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }
    let response = request.send().await?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| LLMError::Network(format!("read response body: {e}")))?;

    if !status.is_success() {
        return Err(classify_status(status, &body));
    }

    let parsed: ModelsResponse = serde_json::from_str(&body)
        .map_err(|e| LLMError::ParseError(format!("/models response: {e}")))?;

    let ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_openai_response() {
        let body = r#"{
            "object": "list",
            "data": [
                {"id": "gpt-4o-mini", "object": "model", "owned_by": "openai"},
                {"id": "gpt-4o", "object": "model", "owned_by": "openai"}
            ]
        }"#;
        let parsed: ModelsResponse = serde_json::from_str(body).unwrap();
        let ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["gpt-4o-mini", "gpt-4o"]);
    }

    #[test]
    fn parses_empty_data() {
        let body = r#"{"object":"list","data":[]}"#;
        let parsed: ModelsResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn missing_data_field_defaults_empty() {
        // 部分实现可能漏返 data；default 让我们拿到空 vec 而不是 ParseError
        let body = r#"{"object":"list"}"#;
        let parsed: ModelsResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn malformed_json_is_parse_error() {
        // probe_models 的解析失败路径：不是 ModelsResponse 形状
        let body = "not json at all";
        let r: Result<ModelsResponse, _> = serde_json::from_str(body);
        assert!(r.is_err());
    }
}
