use crate::domain::ProviderProtocol;
use serde::{Deserialize, Serialize};

/// What a configured endpoint can actually do.
///
/// This is deliberately produced by *probing* with minimal inference calls, not
/// by trusting `GET /models`. Many third-party OpenAI-compatible gateways list
/// models they cannot chat with, or expose embeddings on a different base path.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub chat_completions: bool,
    pub responses: bool,
    pub streaming: bool,
    pub structured_output: bool,
    pub embeddings: bool,
    pub embedding_dimension: Option<usize>,
    pub vision_optional: bool,
    pub protocol: Option<ProviderProtocol>,
    #[serde(default)]
    pub probe_errors: Vec<String>,
}

impl ProviderCapabilities {
    pub fn mock() -> Self {
        Self {
            chat_completions: true,
            responses: false,
            streaming: true,
            structured_output: true,
            embeddings: true,
            embedding_dimension: Some(crate::ai::mock::EMBEDDING_DIMENSIONS),
            vision_optional: false,
            protocol: Some(ProviderProtocol::ChatCompletions),
            probe_errors: Vec::new(),
        }
    }

    pub fn supports_chat(&self) -> bool {
        self.chat_completions || self.responses
    }

    /// Pick the protocol `auto` should use. Responses is preferred when both
    /// work because it has native structured output, but Chat Completions
    /// remains a first-class path for third-party gateways.
    pub fn preferred_protocol(&self) -> ProviderProtocol {
        if self.responses {
            ProviderProtocol::Responses
        } else {
            ProviderProtocol::ChatCompletions
        }
    }

    pub fn status_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("Chat             {}", mark(self.supports_chat())));
        lines.push(format!("Streaming        {}", mark(self.streaming)));
        lines.push(format!("Structured       {}", mark(self.structured_output)));
        match (self.embeddings, self.embedding_dimension) {
            (true, Some(dimension)) => {
                lines.push(format!("Embeddings       ✓ · {dimension} dimensions"));
            }
            (true, None) => lines.push("Embeddings       ✓ · dimension unknown".to_owned()),
            (false, _) => lines.push("Embeddings       ✗".to_owned()),
        }
        let protocol = self
            .protocol
            .map(|protocol| protocol.as_str())
            .unwrap_or("unknown");
        lines.push(format!("Protocol         {protocol}"));
        lines
    }
}

fn mark(value: bool) -> &'static str {
    if value {
        "✓"
    } else {
        "✗"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_responses_only_when_probed_available() {
        let chat_only = ProviderCapabilities {
            chat_completions: true,
            ..ProviderCapabilities::default()
        };
        assert_eq!(
            chat_only.preferred_protocol(),
            ProviderProtocol::ChatCompletions
        );

        let both = ProviderCapabilities {
            chat_completions: true,
            responses: true,
            ..ProviderCapabilities::default()
        };
        assert_eq!(both.preferred_protocol(), ProviderProtocol::Responses);
    }

    #[test]
    fn status_lines_show_dimensions_without_hard_coding_them() {
        let capabilities = ProviderCapabilities {
            embeddings: true,
            embedding_dimension: Some(768),
            ..ProviderCapabilities::default()
        };
        let lines = capabilities.status_lines();
        assert!(lines.iter().any(|line| line.contains("768 dimensions")));
    }
}
