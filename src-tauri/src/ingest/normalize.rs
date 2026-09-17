use super::parser::ParsedDocument;
use crate::util::text::{collapse_blank_lines, normalize_newlines};

pub fn normalize_document(document: ParsedDocument) -> ParsedDocument {
    ParsedDocument {
        title: document.title.trim().to_owned(),
        pages: document
            .pages
            .into_iter()
            .map(|page| super::parser::ParsedPage {
                page_number: page.page_number,
                text: collapse_blank_lines(&normalize_newlines(&page.text)),
            })
            .collect(),
        metadata_json: document.metadata_json,
    }
}
