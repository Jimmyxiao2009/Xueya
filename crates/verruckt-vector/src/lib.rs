//! Verruckt Vector Engine — embedded vector search for the Verruckt desktop backend.
//!
//! This is a standalone, self-contained research crate. It must stay usable
//! without touching any mainline production code.

#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod id;

pub use config::{Algorithm, IndexConfig, Metric};
pub use error::VectorError;
pub use id::VectorId;
