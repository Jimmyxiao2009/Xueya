use crate::error::AppError;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualPath {
    Root,
    Sources,
    Source(Uuid),
    SourceDocuments(Uuid),
    SourceDocument(Uuid, Uuid),
    Concepts,
    Concept(Uuid),
    Courses,
    Course(Uuid),
    CourseNodes(Uuid),
    CourseNode(Uuid, Uuid),
}

impl VirtualPath {
    pub fn parse(path: &str) -> Result<Self, AppError> {
        if path.contains("..") {
            return Err(AppError::InvalidInput(
                "virtual path traversal rejected".into(),
            ));
        }
        let parts: Vec<&str> = path
            .trim()
            .trim_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        match parts.as_slice() {
            [] => Ok(Self::Root),
            ["Sources"] => Ok(Self::Sources),
            ["Sources", id] => Ok(Self::Source(parse_id(id, "source")?)),
            ["Sources", source, "Documents"] => {
                Ok(Self::SourceDocuments(parse_id(source, "source")?))
            }
            ["Sources", source, "Documents", document] => Ok(Self::SourceDocument(
                parse_id(source, "source")?,
                parse_id(document, "document")?,
            )),
            ["Concepts"] => Ok(Self::Concepts),
            ["Concepts", id] => Ok(Self::Concept(parse_id(id, "concept")?)),
            ["Courses"] => Ok(Self::Courses),
            ["Courses", id] => Ok(Self::Course(parse_id(id, "course")?)),
            ["Courses", id, "Nodes"] => Ok(Self::CourseNodes(parse_id(id, "course")?)),
            ["Courses", course, "Nodes", node] => Ok(Self::CourseNode(
                parse_id(course, "course")?,
                parse_id(node, "node")?,
            )),
            _ => Err(AppError::NotFound(format!(
                "unsupported virtual path: {path}"
            ))),
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::Root => "/".into(),
            Self::Sources => "/Sources".into(),
            Self::Source(id) => format!("/Sources/{id}"),
            Self::SourceDocuments(id) => format!("/Sources/{id}/Documents"),
            Self::SourceDocument(source, document) => {
                format!("/Sources/{source}/Documents/{document}")
            }
            Self::Concepts => "/Concepts".into(),
            Self::Concept(id) => format!("/Concepts/{id}"),
            Self::Courses => "/Courses".into(),
            Self::Course(id) => format!("/Courses/{id}"),
            Self::CourseNodes(id) => format!("/Courses/{id}/Nodes"),
            Self::CourseNode(course, node) => format!("/Courses/{course}/Nodes/{node}"),
        }
    }
}

fn parse_id(value: &str, label: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value)
        .map_err(|_| AppError::InvalidInput(format!("invalid {label} id in virtual path: {value}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_paths() {
        let id = Uuid::new_v4();
        let path = VirtualPath::parse(&format!("/Courses/{id}/Nodes")).expect("parse");
        assert_eq!(path, VirtualPath::CourseNodes(id));
        assert!(VirtualPath::parse("../../etc").is_err());
    }
}
