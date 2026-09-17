use super::{DocumentParser, ParsedDocument, ParsedPage};
use crate::error::AppError;

pub struct MarkdownParser;

impl MarkdownParser {
    fn first_heading(text: &str) -> Option<String> {
        text.lines()
            .find(|line| line.trim_start().starts_with('#'))
            .map(|line| line.trim_start_matches('#').trim().to_owned())
            .filter(|value| !value.is_empty())
    }
}

impl DocumentParser for MarkdownParser {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn supports(&self, extension: &str, mime_type: Option<&str>) -> bool {
        matches!(extension, "md" | "markdown" | "mdown")
            || mime_type
                .map(|value| value.contains("markdown"))
                .unwrap_or(false)
    }

    fn parse(
        &self,
        filename: &str,
        _mime_type: Option<&str>,
        bytes: &[u8],
    ) -> Result<ParsedDocument, AppError> {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let title = Self::first_heading(&text).unwrap_or_else(|| {
            filename
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(filename)
                .to_owned()
        });
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
