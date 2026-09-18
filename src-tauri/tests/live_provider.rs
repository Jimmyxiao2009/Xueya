//! Live BYOK smoke test.
//!
//! This is intentionally `#[ignore]`d: it is the only test that talks to a real
//! provider, and it is opt-in through environment variables. It never prints
//! the API key, and it skips cleanly when the variables are absent so CI stays
//! offline and deterministic.
//!
//! Run with:
//! `cargo test --test live_provider -- --ignored --nocapture`

use verruckt_lib::ai::openai_compatible::OpenAiCompatibleProvider;
use verruckt_lib::ai::traits::{ChatMessage, ChatProvider, ChatRequest, EmbeddingProvider};
use verruckt_lib::domain::ProviderProtocol;

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[tokio::test]
#[ignore = "requires VERRUCKT_TEST_BASE_URL/API_KEY/CHAT_MODEL/EMBEDDING_MODEL"]
async fn live_provider_round_trip() {
    let Some(base_url) = env("VERRUCKT_TEST_BASE_URL") else {
        eprintln!("live provider smoke skipped: VERRUCKT_TEST_BASE_URL is not set");
        return;
    };
    let Some(api_key) = env("VERRUCKT_TEST_API_KEY") else {
        eprintln!("live provider smoke skipped: VERRUCKT_TEST_API_KEY is not set");
        return;
    };
    let Some(chat_model) = env("VERRUCKT_TEST_CHAT_MODEL") else {
        eprintln!("live provider smoke skipped: VERRUCKT_TEST_CHAT_MODEL is not set");
        return;
    };
    let Some(embedding_model) = env("VERRUCKT_TEST_EMBEDDING_MODEL") else {
        eprintln!("live provider smoke skipped: VERRUCKT_TEST_EMBEDDING_MODEL is not set");
        return;
    };

    let provider = OpenAiCompatibleProvider::with_protocol(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .build()
            .expect("live http client"),
        base_url,
        api_key,
        chat_model,
        embedding_model,
        ProviderProtocol::Auto,
    );

    let capabilities = provider.probe_capabilities().await;
    assert!(
        capabilities.supports_chat(),
        "live provider has no usable chat endpoint: {capabilities:?}"
    );

    let chat = provider
        .chat(ChatRequest::new(
            "",
            vec![ChatMessage::user("Reply with exactly one word: pong")],
        ))
        .await
        .expect("live chat");
    assert!(!chat.content.trim().is_empty(), "live chat returned empty");

    let embeddings = provider
        .embed(vec!["orbit live provider smoke".to_owned()])
        .await
        .expect("live embeddings");
    assert_eq!(embeddings.len(), 1);
    assert!(
        !embeddings[0].is_empty(),
        "live embedding returned an empty vector"
    );

    eprintln!("live provider smoke ok");
}
