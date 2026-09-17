pub use crate::domain::Chunk;

pub fn citation_label(chunk: &Chunk, source_title: Option<&str>) -> String {
    let source = source_title.unwrap_or("local document");
    match chunk.page_number {
        Some(page) => format!("{source}, p.{page}"),
        None => source.to_owned(),
    }
}
