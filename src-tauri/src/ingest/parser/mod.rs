use crate::error::AppError;

pub mod docx;
pub mod html;
pub mod markdown;
pub mod pdf;
pub mod text;

#[derive(Debug, Clone)]
pub struct ParsedPage {
    pub page_number: Option<i64>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub title: String,
    pub pages: Vec<ParsedPage>,
    pub metadata_json: String,
}

impl ParsedDocument {
    pub fn full_text(&self) -> String {
        self.pages
            .iter()
            .map(|page| page.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn page_count(&self) -> i64 {
        self.pages
            .iter()
            .filter(|page| page.page_number.is_some())
            .count() as i64
    }
}

pub trait DocumentParser: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, extension: &str, mime_type: Option<&str>) -> bool;
    fn parse(
        &self,
        filename: &str,
        mime_type: Option<&str>,
        bytes: &[u8],
    ) -> Result<ParsedDocument, AppError>;
}
