use super::{DocumentParser, ParsedDocument, ParsedPage};
use crate::error::AppError;
use regex::Regex;
use std::io::{Cursor, Read};

pub struct DocxParser;

impl DocxParser {
    fn decode_entities(input: &str) -> String {
        input
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
    }
}

impl DocumentParser for DocxParser {
    fn name(&self) -> &'static str {
        "docx"
    }

    fn supports(&self, extension: &str, mime_type: Option<&str>) -> bool {
        extension.eq_ignore_ascii_case("docx")
            || mime_type
                .map(|value| value.contains("wordprocessingml"))
                .unwrap_or(false)
    }

    fn parse(
        &self,
        filename: &str,
        _mime_type: Option<&str>,
        bytes: &[u8],
    ) -> Result<ParsedDocument, AppError> {
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|error| AppError::Parser(format!("invalid docx archive: {error}")))?;
        let mut document_xml = String::new();
        let file = archive
            .by_name("word/document.xml")
            .map_err(|error| AppError::Parser(format!("docx document.xml missing: {error}")))?;
        const MAX_XML: u64 = 16 * 1024 * 1024;
        if file.size() > MAX_XML {
            return Err(AppError::Parser("docx XML exceeds size limit".into()));
        }
        file.take(MAX_XML + 1)
            .read_to_string(&mut document_xml)
            .map_err(|error| AppError::Parser(format!("docx document.xml unreadable: {error}")))?;
        if document_xml.len() as u64 > MAX_XML {
            return Err(AppError::Parser("docx XML exceeds size limit".into()));
        }

        let paragraph = Regex::new(r"(?is)<w:p(?:\s[^>]*)?>(.*?)</w:p>")
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let text = Regex::new(r"(?is)<w:t(?:\s[^>]*)?>(.*?)</w:t>")
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let tags =
            Regex::new(r"(?s)<[^>]+>").map_err(|error| AppError::Internal(error.to_string()))?;

        let mut output = String::new();
        for captures in paragraph.captures_iter(&document_xml) {
            let Some(body) = captures.get(1) else {
                continue;
            };
            let text_value = text.replace_all(body.as_str(), "$1");
            let clean = tags.replace_all(&text_value, " ");
            let clean = Self::decode_entities(&clean);
            let clean = clean.trim();
            if !clean.is_empty() {
                output.push_str(clean);
                output.push('\n');
            }
        }

        let title = filename
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(filename)
            .to_owned();
        Ok(ParsedDocument {
            title,
            pages: vec![ParsedPage {
                page_number: None,
                text: output,
            }],
            metadata_json: "{}".into(),
        })
    }
}
