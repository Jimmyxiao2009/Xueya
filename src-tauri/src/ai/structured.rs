use super::prompts::repair_messages;
use super::schemas::parse_json;
use super::traits::{ChatMessage, ChatProvider, ChatRequest};
use crate::error::AppError;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Generate a typed value with a single repair retry.
///
/// The sequence is intentionally finite:
/// generate -> parse -> serde validation -> business validation -> one repair.
/// A model that cannot produce valid JSON after the repair is a hard failure;
/// the app never loops forever asking it to try again.
pub async fn generate_structured<T, V>(
    chat: &dyn ChatProvider,
    model: &str,
    messages: Vec<ChatMessage>,
    validator: V,
) -> Result<T, AppError>
where
    T: DeserializeOwned + Serialize,
    V: Fn(&T) -> Result<(), AppError>,
{
    let first = chat
        .chat(ChatRequest::json(model.to_owned(), messages.clone()))
        .await?;
    match parse_json::<T>(&first.content) {
        Ok(value) => match validator(&value) {
            Ok(()) => Ok(value),
            Err(error) => {
                return repair::<T, V>(chat, model, messages, first.content, error, validator)
                    .await;
            }
        },
        Err(error) => {
            return repair::<T, V>(chat, model, messages, first.content, error, validator).await;
        }
    }
}

async fn repair<T, V>(
    chat: &dyn ChatProvider,
    model: &str,
    messages: Vec<ChatMessage>,
    invalid_content: String,
    error: AppError,
    validator: V,
) -> Result<T, AppError>
where
    T: DeserializeOwned + Serialize,
    V: Fn(&T) -> Result<(), AppError>,
{
    let mut repaired = messages;
    repaired.push(ChatMessage::assistant(invalid_content));
    repaired.extend(repair_messages(&error));
    let response = chat
        .chat(ChatRequest::json(model.to_owned(), repaired))
        .await?;
    let value = parse_json::<T>(&response.content)?;
    validator(&value)?;
    Ok(value)
}
