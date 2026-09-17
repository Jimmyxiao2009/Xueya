use super::manager::JobHandle;
use crate::error::AppError;

pub use super::manager::JobHandle as WorkerHandle;

pub fn ensure_not_cancelled(handle: &JobHandle) -> Result<(), AppError> {
    if handle.is_cancelled() {
        Err(AppError::JobCancelled)
    } else {
        Ok(())
    }
}
