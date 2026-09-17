pub use tokio_util::sync::CancellationToken;

pub fn cancellation_token() -> CancellationToken {
    CancellationToken::new()
}
