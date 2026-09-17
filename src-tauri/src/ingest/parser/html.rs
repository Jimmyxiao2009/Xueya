use super::{DocumentParser, ParsedDocument, ParsedPage};
use crate::error::AppError;
use regex::Regex;

pub struct HtmlParser;

impl HtmlParser {
    fn strip_markup(input: &str) -> Result<String, AppError> {
        let script = Regex::new(r"(?is)<script.*?</script>")
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let style = Regex::new(r"(?is)<style.*?</style>")
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let breaks = Regex::new(r"(?i)<(br|/p|/div|/li|/h[1-6])\s*/?>")
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let tags =
            Regex::new(r"(?s)<[^>]+>").map_err(|error| AppError::Internal(error.to_string()))?;

        let value = script.replace_all(input, " ");
        let value = style.replace_all(&value, " ");
        let value = breaks.replace_all(&value, "\n");
        let value = tags.replace_all(&value, " ");
        Ok(Self::decode_entities(&value))
    }

    fn decode_entities(input: &str) -> String {
        input
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
    }

    fn title(input: &str) -> Option<String> {
        let tag = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()?;
        let captures = tag.captures(input)?;
        Some(
            Self::decode_entities(captures.get(1)?.as_str())
                .trim()
                .to_owned(),
        )
        .filter(|value| !value.is_empty())
    }
}

impl DocumentParser for HtmlParser {
    fn name(&self) -> &'static str {
        "html"
    }

    fn supports(&self, extension: &str, mime_type: Option<&str>) -> bool {
        matches!(extension, "html" | "htm")
            || mime_type
                .map(|value| value.contains("text/html"))
                .unwrap_or(false)
    }

    fn parse(
        &self,
        filename: &str,
        _mime_type: Option<&str>,
        bytes: &[u8],
    ) -> Result<ParsedDocument, AppError> {
        let html = String::from_utf8_lossy(bytes).into_owned();
        let title = Self::title(&html).unwrap_or_else(|| {
            filename
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(filename)
                .to_owned()
        });
        let text = Self::strip_markup(&html)?;
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
