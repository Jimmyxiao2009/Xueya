pub use crate::domain::{KnowledgeRelation, RelationType};

use crate::error::AppError;
use crate::util::time::utc_now;
use uuid::Uuid;

pub fn relation(
    from_object_id: Uuid,
    to_object_id: Uuid,
    relation_type: RelationType,
    weight: f64,
) -> Result<KnowledgeRelation, AppError> {
    if !weight.is_finite() {
        return Err(AppError::InvalidInput(
            "relation weight must be finite".into(),
        ));
    }
    Ok(KnowledgeRelation {
        id: Uuid::new_v4(),
        from_object_id,
        to_object_id,
        relation_type,
        weight: weight.clamp(0.0, 1.0),
        metadata_json: "{}".into(),
        created_at: utc_now(),
    })
}
