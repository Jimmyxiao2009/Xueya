use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOperation {
    Upsert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevisionDecision {
    Accept,
    Conflict,
}

/// Determines whether a client revision may overwrite the current server
/// revision. This intentionally implements last-writer-wins only when the
/// client's base revision matches the server revision.
pub fn decide_revision(current_revision: Option<i64>, base_revision: i64) -> RevisionDecision {
    match current_revision {
        None => {
            if base_revision == 0 {
                RevisionDecision::Accept
            } else {
                RevisionDecision::Conflict
            }
        }
        Some(current) => {
            if current == base_revision {
                RevisionDecision::Accept
            } else {
                RevisionDecision::Conflict
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_new_entity_with_zero_base() {
        assert_eq!(decide_revision(None, 0), RevisionDecision::Accept);
    }

    #[test]
    fn rejects_new_entity_with_nonzero_base() {
        assert_eq!(decide_revision(None, 3), RevisionDecision::Conflict);
    }

    #[test]
    fn accepts_matching_revision() {
        assert_eq!(decide_revision(Some(19), 19), RevisionDecision::Accept);
    }

    #[test]
    fn detects_stale_client_revision() {
        assert_eq!(decide_revision(Some(19), 17), RevisionDecision::Conflict);
    }

    #[test]
    fn detects_future_client_revision() {
        assert_eq!(decide_revision(Some(19), 20), RevisionDecision::Conflict);
    }
}
