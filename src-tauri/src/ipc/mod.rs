pub mod app;
pub mod courses;
pub mod demo;
pub mod dto;
pub mod jobs;
pub mod knowledge;
pub mod learning;
pub mod providers;
pub mod quiz;
pub mod search;
pub mod settings;
pub mod sources;
pub mod tutor;
pub mod vector_diagnostics;

// The same command/service code runs behind Tauri IPC and the browser server.
#[cfg(feature = "desktop")]
pub use tauri::{AppHandle, State};
#[cfg(not(feature = "desktop"))]
pub struct State<'a, T: ?Sized>(&'a T);
#[cfg(not(feature = "desktop"))]
impl<'a, T: ?Sized> State<'a, T> {
    pub fn new(value: &'a T) -> Self {
        Self(value)
    }
    pub fn inner(&self) -> &'a T {
        self.0
    }
}
#[cfg(not(feature = "desktop"))]
impl<T: ?Sized> std::ops::Deref for State<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0
    }
}
#[cfg(not(feature = "desktop"))]
#[derive(Clone)]
pub struct AppHandle;
#[cfg(not(feature = "desktop"))]
impl AppHandle {
    pub fn emit<T: serde::Serialize>(
        &self,
        _event: &str,
        _payload: T,
    ) -> Result<(), std::convert::Infallible> {
        Ok(())
    }
}
