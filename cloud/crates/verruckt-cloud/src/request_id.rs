use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

tokio::task_local! {
    static REQUEST_ID: String;
}

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub fn current_request_id() -> String {
    REQUEST_ID
        .try_with(|value| value.clone())
        .unwrap_or_else(|_| "unknown".into())
}

pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| is_valid_request_id(value))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));

    let header_value =
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("unknown"));

    REQUEST_ID
        .scope(request_id, async move {
            let mut response = next.run(request).await;
            response.headers_mut().insert("x-request-id", header_value);
            response
        })
        .await
}

fn is_valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
}
