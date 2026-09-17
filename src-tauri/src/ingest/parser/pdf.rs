use super::{DocumentParser, ParsedDocument, ParsedPage};
use crate::error::AppError;

pub struct PdfParser;

impl DocumentParser for PdfParser {
    fn name(&self) -> &'static str {
        "pdf"
    }

    fn supports(&self, extension: &str, mime_type: Option<&str>) -> bool {
        extension.eq_ignore_ascii_case("pdf")
            || mime_type
                .map(|value| value.contains("application/pdf"))
                .unwrap_or(false)
    }

    fn parse(
        &self,
        filename: &str,
        _mime_type: Option<&str>,
        bytes: &[u8],
    ) -> Result<ParsedDocument, AppError> {
        let pages = pdf_extract::extract_text_from_mem_by_pages(bytes)
            .map_err(|error| AppError::Parser(format!("pdf text extraction failed: {error}")))?;
        let title = filename
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(filename)
            .to_owned();
        Ok(ParsedDocument {
            title,
            pages: pages
                .into_iter()
                .enumerate()
                .map(|(index, text)| ParsedPage {
                    page_number: Some(index as i64 + 1),
                    text,
                })
                .collect(),
            metadata_json: "{}".into(),
        })
    }
}
