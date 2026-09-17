use super::parser::{
    docx::DocxParser, html::HtmlParser, markdown::MarkdownParser, pdf::PdfParser, text::TextParser,
    DocumentParser,
};
use crate::error::AppError;
use std::path::Path;

pub fn detect_parser(
    filename: &str,
    mime_type: Option<&str>,
) -> Result<Box<dyn DocumentParser>, AppError> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let parsers: Vec<Box<dyn DocumentParser>> = vec![
        Box::new(MarkdownParser),
        Box::new(TextParser),
        Box::new(PdfParser),
        Box::new(HtmlParser),
        Box::new(DocxParser),
    ];
    parsers
        .into_iter()
        .find(|parser| parser.supports(&extension, mime_type))
        .ok_or_else(|| {
            AppError::UnsupportedFormat(format!(
                "no parser for extension '.{extension}' (mime: {})",
                mime_type.unwrap_or("unknown")
            ))
        })
}
