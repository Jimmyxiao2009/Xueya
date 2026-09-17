use super::{DocumentParser, ParsedDocument, ParsedPage};
use crate::error::AppError;

pub struct TextParser;

impl DocumentParser for TextParser {
    fn name(&self) -> &'static str {
        "text"
    }

    fn supports(&self, extension: &str, mime_type: Option<&str>) -> bool {
        matches!(extension, "txt" | "text")
            || mime_type
                .map(|value| value.starts_with("text/plain"))
                .unwrap_or(false)
    }

    fn parse(
        &self,
        filename: &str,
        _mime_type: Option<&str>,
        bytes: &[u8],
    ) -> Result<ParsedDocument, AppError> {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let title = filename
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(filename)
            .to_owned();
        Ok(ParsedDocument {
            title,
            pages: vec![ParsedPage {
                page_number: None,
                text,
            }],
            metadata_json: "{}".into(),
        })
    }
}
