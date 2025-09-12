use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use pretty_assertions::assert_eq;

#[test]
fn inline_api_key_is_used() {
    let provider = ModelProviderInfo {
        name: "custom".into(),
        base_url: Some("https://example.com".into()),
        api_key: Some("secret".into()),
        env_key: None,
        env_key_instructions: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        http_proxy: None,
        default_model: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    };
    assert_eq!(provider.api_key().unwrap().as_deref(), Some("secret"));
}
