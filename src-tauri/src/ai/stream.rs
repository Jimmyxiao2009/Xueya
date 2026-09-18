use super::traits::{ChatDelta, ChatStream};
use crate::error::AppError;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Provider-neutral streaming event consumed by the Tauri channel layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AiStreamEvent {
    Started { model: String },
    Delta { text: String },
    CitationMetadata { citations: Vec<String> },
    Completed { finish_reason: Option<String> },
    Failed { code: String, message: String },
}

pub type AiStream = Pin<Box<dyn Stream<Item = Result<AiStreamEvent, AppError>> + Send>>;

/// Adapt the legacy `ChatDelta` stream into the event stream the UI consumes.
pub fn from_chat_stream(stream: ChatStream) -> AiStream {
    Box::pin(stream.map(|item| match item {
        Ok(delta) => Ok(AiStreamEvent::Delta {
            text: delta.content,
        }),
        Err(error) => Err(error),
    }))
}

/// Build a stream that emits `Started`, delegates to a provider stream, then
/// emits `Completed` or `Failed`. The final event is always terminal.
pub fn with_lifecycle(stream: ChatStream, model: impl Into<String>) -> AiStream {
    let model = model.into();
    let mapped = stream.map(|item| match item {
        Ok(delta) => Ok(AiStreamEvent::Delta {
            text: delta.content,
        }),
        Err(error) => Err(error),
    });
    let started = true;
    let failed = false;
    let model = model.clone();
    Box::pin(futures_util::stream::unfold(
        (Box::pin(mapped), started, failed, model),
        |(mut stream, mut started, mut failed, model)| async move {
            if started {
                started = false;
                return Some((
                    Ok(AiStreamEvent::Started {
                        model: model.clone(),
                    }),
                    (stream, started, failed, model),
                ));
            }
            match stream.next().await {
                Some(Ok(event)) => Some((Ok(event), (stream, started, failed, model))),
                Some(Err(error)) => {
                    if !failed {
                        failed = true;
                        let event = AiStreamEvent::Failed {
                            code: error.code().to_owned(),
                            message: error.public_message().to_owned(),
                        };
                        Some((Ok(event), (stream, started, failed, model)))
                    } else {
                        None
                    }
                }
                None => {
                    if !failed {
                        failed = true;
                        Some((
                            Ok(AiStreamEvent::Completed {
                                finish_reason: Some("stop".to_owned()),
                            }),
                            (stream, started, failed, model),
                        ))
                    } else {
                        None
                    }
                }
            }
        },
    ))
}

pub fn delta_event(text: impl Into<String>) -> AiStreamEvent {
    AiStreamEvent::Delta { text: text.into() }
}

pub fn completed_event() -> AiStreamEvent {
    AiStreamEvent::Completed {
        finish_reason: Some("stop".to_owned()),
    }
}

pub fn from_chat_delta(delta: ChatDelta) -> AiStreamEvent {
    AiStreamEvent::Delta {
        text: delta.content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lifecycle_stream_emits_terminal_event() {
        let delta = Ok(ChatDelta {
            content: "hello".to_owned(),
            finish_reason: None,
        });
        let stream: ChatStream = Box::pin(futures_util::stream::iter(vec![delta]));
        let mut events = with_lifecycle(stream, "test-model");
        let mut seen = Vec::new();
        while let Some(event) = events.next().await {
            seen.push(event.expect("event"));
        }
        assert!(matches!(seen.first(), Some(AiStreamEvent::Started { .. })));
        assert!(seen
            .iter()
            .any(|event| matches!(event, AiStreamEvent::Delta { text } if text == "hello")));
        assert!(matches!(seen.last(), Some(AiStreamEvent::Completed { .. })));
    }
}
