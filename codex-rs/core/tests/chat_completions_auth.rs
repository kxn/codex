use std::sync::Arc;

use codex_core::AuthManager;
use codex_core::ContentItem;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::mcp_protocol::AuthMode;
use core_test_support::load_default_config_for_test;
use futures::StreamExt;
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header_regex;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn network_disabled() -> bool {
    std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_includes_authorization_header_with_openai_api_key() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;

    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            "data: {\"choices\":[{\"delta\":{}}]}\n\ndata: [DONE]\n\n",
            "text/event-stream",
        );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header_regex("Authorization", r"Bearer sk-test-key"))
        .respond_with(template)
        .expect(1)
        .mount(&server)
        .await;

    let prev = std::env::var("OPENAI_API_KEY").ok();
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "sk-test-key");
    }

    let provider = ModelProviderInfo {
        name: "mock".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        api_key: None,
        env_key: None,
        env_key_instructions: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: true,
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let originator = config.responses_originator_header.clone();
    let config = Arc::new(config);

    let auth_manager = Arc::new(AuthManager::new(
        codex_home.path().to_path_buf(),
        AuthMode::ApiKey,
        originator,
    ));

    let client = ModelClient::new(
        Arc::clone(&config),
        Some(auth_manager),
        provider,
        effort,
        summary,
        Uuid::new_v4(),
    );

    let mut prompt = Prompt::default();
    prompt.input = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "hello".to_string(),
        }],
    }];

    let mut stream = client.stream(&prompt).await.unwrap();
    while let Some(event) = stream.next().await {
        if let Err(e) = event {
            panic!("stream event error: {e}");
        }
    }

    let request = &server.received_requests().await.unwrap()[0];
    let auth = request.headers.get("authorization").unwrap();
    assert_eq!(auth.to_str().unwrap(), "Bearer sk-test-key");

    if let Some(prev) = prev {
        unsafe {
            std::env::set_var("OPENAI_API_KEY", prev);
        }
    } else {
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }
    }
}
