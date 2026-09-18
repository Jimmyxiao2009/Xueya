use crate::error::ApiError;
use crate::state::AppState;
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
use verruckt_cloud_domain::entitlements::EntitlementSet;
use verruckt_cloud_domain::password::{hash_password, verify_password, PasswordError};
use verruckt_cloud_domain::token::{
    decode_access_token, encode_access_token, generate_refresh_token, hash_refresh_token,
    AccessTokenClaims,
};
use verruckt_cloud_protocol::auth::{
    AuthResponse, DeviceDto, DeviceInput, LoginRequest, MeResponse, RefreshRequest,
    RegisterRequest, UserDto,
};

#[derive(Debug, Clone, Copy)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    password_hash: String,
    display_name: Option<String>,
    created_at: DateTime<Utc>,
}

impl UserRow {
    fn into_dto(self) -> UserDto {
        UserDto {
            id: self.id,
            email: self.email,
            display_name: self.display_name,
            created_at: self.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    name: String,
    platform: String,
    app_version: String,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

impl DeviceRow {
    fn into_dto(self) -> DeviceDto {
        DeviceDto {
            id: self.id,
            name: self.name,
            platform: self.platform,
            app_version: self.app_version,
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
        }
    }
}

struct SessionCreated {
    session_id: Uuid,
    refresh_token: String,
}

pub fn normalize_email(email: &str) -> Result<String, ApiError> {
    let trimmed = email.trim();
    if trimmed.is_empty() || trimmed.len() > 320 || !trimmed.contains('@') {
        return Err(ApiError::bad_request(
            "invalid_email",
            "a valid email address is required",
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub async fn register(
    state: &AppState,
    request: RegisterRequest,
) -> Result<AuthResponse, ApiError> {
    let email_normalized = normalize_email(&request.email)?;
    let password_hash = hash_password(&request.password).map_err(map_password_error)?;

    let mut tx = state.pool.begin().await?;
    let user_id = Uuid::new_v4();
    let insert = sqlx::query(
        r#"
        INSERT INTO users (id, email, email_normalized, password_hash)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(request.email.trim())
    .bind(&email_normalized)
    .bind(password_hash)
    .execute(&mut *tx)
    .await;

    match insert {
        Ok(_) => {}
        Err(sqlx::Error::Database(database_error)) if database_error.is_unique_violation() => {
            return Err(ApiError::conflict(
                "email_already_registered",
                "an account with that email already exists",
            ));
        }
        Err(error) => return Err(error.into()),
    }

    sqlx::query(
        r#"
        INSERT INTO subscriptions (id, user_id, plan_id, status, current_period_start, current_period_end)
        VALUES ($1, $2, 'free', 'active', now(), now() + interval '30 days')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    let device = match request.device {
        Some(input) => {
            let max_devices = load_entitlements_tx(&mut tx, user_id).await?.max_devices();
            Some(create_device_tx(&mut tx, user_id, &input, max_devices).await?)
        }
        None => None,
    };

    let session = create_session_tx(
        &mut tx,
        user_id,
        device.as_ref().map(|device| device.id),
        None,
        None,
        state.config.refresh_token_ttl(),
    )
    .await?;

    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, display_name, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let access_token = issue_access_token(state, user_id, session.session_id)?;
    Ok(AuthResponse {
        access_token,
        refresh_token: session.refresh_token,
        token_type: "Bearer".into(),
        expires_in: state.config.access_token_ttl_secs,
        user: user.into_dto(),
        device,
    })
}

pub async fn login(state: &AppState, request: LoginRequest) -> Result<AuthResponse, ApiError> {
    let email_normalized = normalize_email(&request.email)?;
    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, display_name, created_at FROM users WHERE email_normalized = $1",
    )
    .bind(&email_normalized)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(invalid_credentials)?;

    if verify_password(&request.password, &user.password_hash).is_err() {
        return Err(invalid_credentials());
    }

    let mut tx = state.pool.begin().await?;
    let entitlements = load_entitlements_tx(&mut tx, user.id).await?;

    let device = if let Some(device_id) = request.device_id {
        Some(update_or_get_device_tx(&mut tx, user.id, device_id, request.device.as_ref()).await?)
    } else if let Some(input) = request.device.as_ref() {
        let count = count_devices_tx(&mut tx, user.id).await?;
        if count >= entitlements.max_devices() {
            return Err(ApiError::forbidden(
                "device_limit_reached",
                format!(
                    "this plan allows at most {} device(s)",
                    entitlements.max_devices()
                ),
            ));
        }
        Some(create_device_tx(&mut tx, user.id, input, entitlements.max_devices()).await?)
    } else {
        None
    };

    let session = create_session_tx(
        &mut tx,
        user.id,
        device.as_ref().map(|device| device.id),
        None,
        None,
        state.config.refresh_token_ttl(),
    )
    .await?;

    tx.commit().await?;
    let access_token = issue_access_token(state, user.id, session.session_id)?;
    Ok(AuthResponse {
        access_token,
        refresh_token: session.refresh_token,
        token_type: "Bearer".into(),
        expires_in: state.config.access_token_ttl_secs,
        user: user.into_dto(),
        device,
    })
}

pub async fn refresh(state: &AppState, request: RefreshRequest) -> Result<AuthResponse, ApiError> {
    let token_hash = hash_refresh_token(&request.refresh_token);
    let mut tx = state.pool.begin().await?;

    let session = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            Uuid,
            Option<Uuid>,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT id, user_id, family_id, device_id, expires_at, revoked_at
        FROM sessions
        WHERE refresh_token_hash = $1
        FOR UPDATE
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(invalid_refresh_token)?;

    let (session_id, user_id, family_id, device_id, expires_at, revoked_at) = session;
    if revoked_at.is_some() {
        revoke_session_family_tx(&mut tx, user_id, family_id).await?;
        tx.commit().await?;
        return Err(ApiError::unauthorized(
            "refresh_token_reuse_detected",
            "this refresh token has already been used; all sessions for the account were revoked",
        ));
    }
    if expires_at <= Utc::now() {
        revoke_session_family_tx(&mut tx, user_id, family_id).await?;
        tx.commit().await?;
        return Err(ApiError::unauthorized(
            "refresh_token_expired",
            "the refresh token has expired",
        ));
    }

    let entitlements = load_entitlements_tx(&mut tx, user_id).await?;
    let device = match device_id {
        Some(id) => {
            Some(update_or_get_device_tx(&mut tx, user_id, id, request.device.as_ref()).await?)
        }
        None => match request.device.as_ref() {
            Some(input) => {
                let count = count_devices_tx(&mut tx, user_id).await?;
                if count >= entitlements.max_devices() {
                    return Err(ApiError::forbidden(
                        "device_limit_reached",
                        format!(
                            "this plan allows at most {} device(s)",
                            entitlements.max_devices()
                        ),
                    ));
                }
                let device =
                    create_device_tx(&mut tx, user_id, input, entitlements.max_devices()).await?;
                sqlx::query("UPDATE sessions SET device_id = $1 WHERE id = $2")
                    .bind(device.id)
                    .bind(session_id)
                    .execute(&mut *tx)
                    .await?;
                Some(device)
            }
            None => None,
        },
    };

    let new_session = create_session_tx(
        &mut tx,
        user_id,
        device.as_ref().map(|device| device.id),
        Some(family_id),
        Some(session_id),
        state.config.refresh_token_ttl(),
    )
    .await?;

    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, display_name, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    let access_token = issue_access_token(state, user_id, new_session.session_id)?;
    Ok(AuthResponse {
        access_token,
        refresh_token: new_session.refresh_token,
        token_type: "Bearer".into(),
        expires_in: state.config.access_token_ttl_secs,
        user: user.into_dto(),
        device,
    })
}

pub async fn logout(
    state: &AppState,
    authenticated: AuthenticatedUser,
    refresh_token: Option<String>,
) -> Result<(), ApiError> {
    if let Some(refresh_token) = refresh_token {
        let token_hash = hash_refresh_token(&refresh_token);
        let row = sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT user_id, family_id FROM sessions WHERE refresh_token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&state.pool)
        .await?;
        if let Some((user_id, family_id)) = row {
            if user_id == authenticated.user_id {
                sqlx::query(
                    "UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND family_id = $2 AND revoked_at IS NULL",
                )
                .bind(user_id)
                .bind(family_id)
                .execute(&state.pool)
                .await?;
            }
        }
        return Ok(());
    }

    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1 AND user_id = $2")
        .bind(authenticated.session_id)
        .bind(authenticated.user_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn me(state: &AppState, user_id: Uuid) -> Result<MeResponse, ApiError> {
    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, display_name, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("user_not_found", "user not found"))?;
    Ok(MeResponse {
        user: user.into_dto(),
    })
}

pub async fn authenticate(
    state: &AppState,
    authorization: Option<&str>,
) -> Result<AuthenticatedUser, ApiError> {
    let token = authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| {
            ApiError::unauthorized("missing_access_token", "a bearer access token is required")
        })?;

    let claims = decode_access_token(
        token,
        &state.config.access_token_secret,
        &state.config.token_issuer,
    )
    .map_err(|error| match error {
        verruckt_cloud_domain::token::TokenError::ExpiredAccessToken => {
            ApiError::unauthorized("access_token_expired", "the access token has expired")
        }
        _ => ApiError::unauthorized("invalid_access_token", "the access token is invalid"),
    })?;

    let session_exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM sessions
            WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL AND expires_at > now()
        )
        "#,
    )
    .bind(claims.sid)
    .bind(claims.sub)
    .fetch_one(&state.pool)
    .await?;

    if !session_exists {
        return Err(ApiError::unauthorized(
            "session_revoked",
            "the session is no longer active",
        ));
    }

    Ok(AuthenticatedUser {
        user_id: claims.sub,
        session_id: claims.sid,
    })
}

async fn create_device_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    input: &DeviceInput,
    max_devices: i64,
) -> Result<DeviceDto, ApiError> {
    let count = count_devices_tx(tx, user_id).await?;
    if count >= max_devices {
        return Err(ApiError::forbidden(
            "device_limit_reached",
            format!("this plan allows at most {max_devices} device(s)"),
        ));
    }
    let device_id = Uuid::new_v4();
    let device = sqlx::query_as::<_, DeviceRow>(
        r#"
        INSERT INTO devices (id, user_id, name, platform, app_version)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, name, platform, app_version, created_at, last_seen_at
        "#,
    )
    .bind(device_id)
    .bind(user_id)
    .bind(&input.name)
    .bind(&input.platform)
    .bind(&input.app_version)
    .fetch_one(&mut **tx)
    .await?;
    Ok(device.into_dto())
}

async fn update_or_get_device_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    device_id: Uuid,
    input: Option<&DeviceInput>,
) -> Result<DeviceDto, ApiError> {
    let device = sqlx::query_as::<_, DeviceRow>(
        r#"
        UPDATE devices
        SET last_seen_at = now(),
            name = COALESCE($3, name),
            platform = COALESCE($4, platform),
            app_version = COALESCE($5, app_version)
        WHERE id = $1 AND user_id = $2
        RETURNING id, name, platform, app_version, created_at, last_seen_at
        "#,
    )
    .bind(device_id)
    .bind(user_id)
    .bind(input.map(|value| value.name.as_str()))
    .bind(input.map(|value| value.platform.as_str()))
    .bind(input.map(|value| value.app_version.as_str()))
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ApiError::not_found("device_not_found", "device not found"))?;
    Ok(device.into_dto())
}

async fn count_devices_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<i64, ApiError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM devices WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(count)
}

async fn create_session_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    device_id: Option<Uuid>,
    family_id: Option<Uuid>,
    previous_session_id: Option<Uuid>,
    ttl: chrono::Duration,
) -> Result<SessionCreated, ApiError> {
    let session_id = Uuid::new_v4();
    let family_id = family_id.unwrap_or(session_id);
    let refresh_token = generate_refresh_token();
    let refresh_token_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + ttl;

    sqlx::query(
        r#"
        INSERT INTO sessions (id, user_id, family_id, device_id, refresh_token_hash, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(family_id)
    .bind(device_id)
    .bind(refresh_token_hash)
    .bind(expires_at)
    .execute(&mut **tx)
    .await?;

    if let Some(previous_session_id) = previous_session_id {
        sqlx::query(
            "UPDATE sessions SET revoked_at = now(), replaced_by_session_id = $1 WHERE id = $2 AND user_id = $3",
        )
        .bind(session_id)
        .bind(previous_session_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(SessionCreated {
        session_id,
        refresh_token,
    })
}

async fn revoke_session_family_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    family_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND family_id = $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(family_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_entitlements_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<EntitlementSet, ApiError> {
    let plan_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT plan_id FROM subscriptions
        WHERE user_id = $1 AND status = 'active'
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_else(|| verruckt_cloud_domain::PLAN_FREE.into());

    let rows = sqlx::query_as::<_, (String, bool, Option<i64>)>(
        "SELECT key, enabled, limit_value FROM entitlements WHERE plan_id = $1",
    )
    .bind(&plan_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(EntitlementSet::from_rows(
        plan_id,
        rows.into_iter().map(|(key, enabled, limit_value)| {
            verruckt_cloud_domain::entitlements::PlanEntitlement {
                key,
                enabled,
                limit_value,
            }
        }),
    ))
}

fn issue_access_token(
    state: &AppState,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<String, ApiError> {
    let claims = AccessTokenClaims::new(
        user_id,
        session_id,
        state.config.access_token_ttl(),
        &state.config.token_issuer,
    );
    encode_access_token(&claims, &state.config.access_token_secret).map_err(|error| {
        ApiError::internal_message(format!("failed to sign access token: {error}"))
    })
}

fn map_password_error(error: PasswordError) -> ApiError {
    match error {
        PasswordError::InvalidLength { min, max } => ApiError::bad_request(
            "invalid_password",
            format!("password must be between {min} and {max} characters"),
        ),
        _ => ApiError::internal_message(format!("password hashing failed: {error}")),
    }
}

fn invalid_credentials() -> ApiError {
    ApiError::unauthorized("invalid_credentials", "invalid email or password")
}

fn invalid_refresh_token() -> ApiError {
    ApiError::unauthorized("invalid_refresh_token", "the refresh token is invalid")
}

impl axum::extract::FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        authenticate(state, authorization).await
    }
}
