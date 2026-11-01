//! HTTP transport for the real learning core; no demo seed or browser mock.
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;
use verruckt_lib::{
    error::AppError,
    ipc,
    secrets::encrypted_store::EncryptedSecretStore,
    state::{AppPaths, AppState},
};
#[path = "../bin_web/dispatch.rs"]
mod web_dispatch;

#[derive(Clone)]
struct Server {
    root: PathBuf,
    accounts: Arc<Mutex<HashMap<String, Arc<AppState>>>>,
    origin: String,
    secure: bool,
    attempts: Arc<Mutex<Vec<std::time::Instant>>>,
}
#[derive(Deserialize)]
struct Invoke {
    command: String,
    #[serde(default)]
    args: Value,
}
#[derive(Serialize, Deserialize)]
struct Session {
    owner: String,
    expires: i64,
}
fn configured(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.trim().is_empty())
}
fn hash(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}
fn value<T: Serialize>(v: T) -> Result<Value, AppError> {
    serde_json::to_value(v).map_err(|e| AppError::Internal(e.to_string()))
}
fn arg<T: DeserializeOwned>(v: &Value, key: &str) -> Result<T, AppError> {
    serde_json::from_value(v.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|_| AppError::InvalidInput(format!("参数不正确: {key}")))
}
fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"code":code,"message":message,"retryable":status.is_server_error()})),
    )
        .into_response()
}
fn signed_out() -> Value {
    json!({"state":"signedOut","loggedIn":false,"email":null,"userId":null,"plan":null,"entitlements":null,"device":null,"syncEnabled":false,"baseUrl":"","lastSuccessfulRefreshAt":null})
}
fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|v| v.trim().strip_prefix("orbit_session="))
        .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_owned)
}
impl Server {
    async fn account(&self, owner: &str) -> Result<Arc<AppState>, AppError> {
        let mut accounts = self.accounts.lock().await;
        if let Some(s) = accounts.get(owner) {
            return Ok(s.clone());
        }
        if accounts.len() >= 64 {
            return Err(AppError::InvalidInput(
                "演示服务会话已满，请联系管理员".into(),
            ));
        }
        let paths = AppPaths::new(self.root.join("accounts").join(owner));
        paths.ensure_dirs().await?;
        let secrets = Arc::new(EncryptedSecretStore::new(&paths.app_data_dir)?);
        let state = AppState::new_with_secrets(paths, secrets).await?;
        if let (Some(base), Some(key), Some(model)) = (
            configured("ORBIT_AI_BASE_URL"),
            configured("ORBIT_AI_API_KEY"),
            configured("ORBIT_AI_MODEL"),
        ) {
            state
                .providers
                .upsert(verruckt_lib::ai::registry::ProviderUpsert {
                    id: Some(Uuid::from_u128(0x01910000000000000000000000000001)),
                    display_name: "演示服务器 AI".into(),
                    base_url: base,
                    chat_model: model,
                    embedding_model: std::env::var("ORBIT_EMBEDDING_MODEL")
                        .unwrap_or_else(|_| "text-embedding-3-small".into()),
                    api_key: Some(key),
                    enabled: true,
                    protocol: verruckt_lib::domain::ProviderProtocol::parse(
                        &std::env::var("ORBIT_AI_PROTOCOL")
                            .unwrap_or_else(|_| "chat_completions".into()),
                    ),
                })
                .await?;
            verruckt_lib::services::settings_service::update_settings(
                &state,
                ipc::dto::SettingsUpdateInput {
                    search_provider: None,
                    brave_api_key: std::env::var("ORBIT_BRAVE_API_KEY").ok(),
                    tavily_api_key: std::env::var("ORBIT_TAVILY_API_KEY").ok(),
                    serper_api_key: std::env::var("ORBIT_SERPER_API_KEY").ok(),
                    searxng_url: std::env::var("ORBIT_SEARXNG_URL").ok(),
                    ai_mode: Some("byok".into()),
                },
            )
            .await?;
        }
        verruckt_lib::services::settings_service::initialize(&state).await?;
        verruckt_lib::services::node_agent::resume(&state).await?;
        let state = Arc::new(state);
        accounts.insert(owner.into(), state.clone());
        Ok(state)
    }
    async fn session(&self, headers: &HeaderMap) -> Option<Session> {
        let token = cookie_token(headers)?;
        let bytes = tokio::fs::read(self.root.join("sessions").join(hash(&token)))
            .await
            .ok()?;
        let session: Session = serde_json::from_slice(&bytes).ok()?;
        (session.expires > chrono::Utc::now().timestamp()
            && session.owner.len() == 64
            && session.owner.bytes().all(|c| c.is_ascii_hexdigit()))
        .then_some(session)
    }
}
async fn invoke(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(request): Json<Invoke>,
) -> Response {
    // JSON + custom header blocks cross-site form requests; exact Origin prevents
    // credentialed browser calls from another site (including login CSRF).
    if headers.get("x-orbit-client").and_then(|v| v.to_str().ok()) != Some("web")
        || headers
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| s != server.origin)
    {
        return failure(StatusCode::FORBIDDEN, "ORIGIN_REJECTED", "请求来源不受信任");
    }
    if matches!(request.command.as_str(), "cloud_login" | "cloud_register") {
        let mut attempts = server.attempts.lock().await;
        attempts.retain(|t| t.elapsed().as_secs() < 60);
        if attempts.len() >= 30 {
            return failure(
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "尝试过于频繁，请稍后再试",
            );
        }
        attempts.push(std::time::Instant::now());
    }
    let session = server.session(&headers).await;
    if request.command == "cloud_logout" {
        if let Some(token) = cookie_token(&headers) {
            let _ = tokio::fs::remove_file(server.root.join("sessions").join(hash(&token))).await;
        }
        let mut response = Json(signed_out()).into_response();
        response.headers_mut().insert(
            "set-cookie",
            HeaderValue::from_static(
                "orbit_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
            ),
        );
        return response;
    }
    if request.command == "cloud_account_state" && session.is_none() {
        return Json(signed_out()).into_response();
    }
    let result: Result<Response, AppError> = async {
        if matches!(request.command.as_str(), "cloud_login" | "cloud_register") {
            let input: verruckt_lib::cloud::commands::CloudCredentialsInput =
                arg(&request.args, "input")?;
            let email = input.email.trim().to_lowercase();
            if email.len() > 254 || !email.contains('@') || input.password.len() > 1024 {
                return Err(AppError::InvalidInput("邮箱或密码格式不正确".into()));
            }
            let owner = hash(&email);
            let state = server.account(&owner).await?;
            let account = if request.command == "cloud_register" {
                state.cloud.register(email, input.password).await?
            } else {
                state.cloud.login(email, input.password).await?
            };
            let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
            let record = Session {
                owner,
                expires: chrono::Utc::now().timestamp() + 7 * 86400,
            };
            tokio::fs::write(
                server.root.join("sessions").join(hash(&token)),
                serde_json::to_vec(&record).unwrap(),
            )
            .await?;
            if let Some(old) = cookie_token(&headers) {
                let _ = tokio::fs::remove_file(server.root.join("sessions").join(hash(&old))).await;
            }
            let mut response = Json(account).into_response();
            response.headers_mut().insert(
                "set-cookie",
                HeaderValue::from_str(&format!(
                    "orbit_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=604800{}",
                    if server.secure { "; Secure" } else { "" }
                ))
                .unwrap(),
            );
            return Ok(response);
        }
        let Some(session) = session else {
            return Ok(failure(
                StatusCode::UNAUTHORIZED,
                "CLOUD_AUTH_ERROR",
                "请先登录",
            ));
        };
        let state = server.account(&session.owner).await?;
        if request.command == "course_create" || request.command == "course_regenerate" {
            if std::env::var("ORBIT_AI_API_KEY")
                .unwrap_or_default()
                .is_empty()
            {
                return Ok(failure(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "PROVIDER_ERROR",
                    "服务器尚未配置真实 AI 服务，请联系管理员。",
                ));
            }
        }
        let mut output = web_dispatch::dispatch(&state, request).await?;
        if let Some(obj) = output.as_object_mut() {
            obj.remove("dataDir");
        }
        Ok(Json(output).into_response())
    }
    .await;
    let mut response = match result {
        Ok(r) => r,
        Err(e) => (StatusCode::BAD_REQUEST, Json(e)).into_response(),
    };
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}
async fn admin_proxy(
    State(_server): State<Server>,
    uri: axum::extract::OriginalUri,
    method: axum::http::Method,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let client = reqwest::Client::new();
    let mut request = client.request(
        method,
        format!(
            "{}{}",
            verruckt_lib::cloud::default_cloud_base_url(),
            uri.path()
        ),
    );
    if let Some(auth) = headers.get("authorization") {
        request = request.header("authorization", auth);
    }
    match request
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(result) => {
            let status = result.status();
            match result.bytes().await {
                Ok(bytes) => (
                    status,
                    [
                        ("content-type", "application/json"),
                        ("cache-control", "no-store"),
                    ],
                    bytes,
                )
                    .into_response(),
                Err(_) => failure(StatusCode::BAD_GATEWAY, "CLOUD_UNAVAILABLE", "云端响应中断"),
            }
        }
        Err(_) => failure(
            StatusCode::BAD_GATEWAY,
            "CLOUD_UNAVAILABLE",
            "账号服务未连接",
        ),
    }
}
async fn health() -> Json<Value> {
    Json(
        json!({"status":"ok","dataMode":"real","aiConfigured":configured("ORBIT_AI_API_KEY").is_some() && configured("ORBIT_AI_BASE_URL").is_some() && configured("ORBIT_AI_MODEL").is_some()}),
    )
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("warn").init();
    let root =
        PathBuf::from(std::env::var("ORBIT_DATA_DIR").unwrap_or_else(|_| ".orbit-server".into()));
    tokio::fs::create_dir_all(root.join("sessions")).await?;
    let origin =
        std::env::var("ORBIT_PUBLIC_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:1420".into());
    let server = Server {
        root,
        accounts: Default::default(),
        attempts: Default::default(),
        secure: origin.starts_with("https://"),
        origin,
    };
    let static_dir = std::env::var("ORBIT_STATIC_DIR").unwrap_or_else(|_| "dist".into());
    let app = Router::new()
        .route("/api/invoke", post(invoke))
        .route("/healthz", get(health))
        .route("/api/admin/overview", get(admin_proxy))
        .route("/api/admin/users/plan", post(admin_proxy))
        .layer(DefaultBodyLimit::max(70 * 1024 * 1024))
        .fallback_service(tower_http::services::ServeDir::new(static_dir))
        .with_state(server);
    let bind = std::env::var("ORBIT_BIND").unwrap_or_else(|_| "127.0.0.1:1430".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("Orbit real-data server listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn server(root: PathBuf) -> Server {
        Server {
            root,
            accounts: Default::default(),
            origin: "http://127.0.0.1:1420".into(),
            secure: false,
            attempts: Default::default(),
        }
    }
    fn headers() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-orbit-client", HeaderValue::from_static("web"));
        h.insert("origin", HeaderValue::from_static("http://127.0.0.1:1420"));
        h
    }
    #[tokio::test]
    async fn rejects_cross_origin_and_unauthenticated_data_access() {
        let dir = tempfile::tempdir().unwrap();
        let s = server(dir.path().into());
        let mut h = headers();
        h.insert("origin", HeaderValue::from_static("https://other.example"));
        let r = invoke(
            State(s.clone()),
            h,
            Json(Invoke {
                command: "cloud_login".into(),
                args: json!({}),
            }),
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let r = invoke(
            State(s),
            headers(),
            Json(Invoke {
                command: "course_list".into(),
                args: json!({}),
            }),
        )
        .await;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }
    #[tokio::test]
    async fn persisted_sessions_restore_owner_and_expired_sessions_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let s = server(dir.path().into());
        tokio::fs::create_dir(s.root.join("sessions"))
            .await
            .unwrap();
        let token = "a".repeat(64);
        let owner = hash("learner@example.com");
        let mut h = headers();
        h.insert(
            "cookie",
            HeaderValue::from_str(&format!("orbit_session={token}")).unwrap(),
        );
        let path = s.root.join("sessions").join(hash(&token));
        let mut record = Session {
            owner: owner.clone(),
            expires: chrono::Utc::now().timestamp() + 60,
        };
        tokio::fs::write(&path, serde_json::to_vec(&record).unwrap())
            .await
            .unwrap();
        assert_eq!(s.session(&h).await.unwrap().owner, owner);
        record.expires = 0;
        tokio::fs::write(&path, serde_json::to_vec(&record).unwrap())
            .await
            .unwrap();
        assert!(s.session(&h).await.is_none());
    }
    #[test]
    fn cookie_never_becomes_a_path() {
        let mut h = headers();
        h.insert(
            "cookie",
            HeaderValue::from_static("orbit_session=../../accounts"),
        );
        assert!(cookie_token(&h).is_none());
    }
}
