// Reuses the repository's fixtures; run only tests whose name starts astra_.
include!("cloud_integration.rs");

async fn astra_account() -> (AppState, Uuid, Uuid) {
    let pool = test_pool()
        .await
        .expect("audit requires its isolated PostgreSQL");
    let state = test_state(pool.clone(), None, None);
    let email = unique_email("astra");
    admin::create_user(&state, &email, PASSWORD, Some("pro"))
        .await
        .unwrap();
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email_normalized=$1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let device = Uuid::new_v4();
    sqlx::query("INSERT INTO devices(id,user_id,name,platform,app_version) VALUES($1,$2,'audit','test','0.1')").bind(device).bind(user).execute(&pool).await.unwrap();
    (state, user, device)
}
async fn astra_push(
    state: &AppState,
    user: Uuid,
    device: Uuid,
    id: Uuid,
    base: i64,
    revision: i64,
) {
    let body = json!({"device_id":device,"changes":[{"change_id":Uuid::new_v4(),"entity_type":"audit","entity_id":id,"base_revision":base,"revision":revision,"operation":"upsert","payload":{"revision":revision},"updated_at":Utc::now()}]});
    let result =
        verruckt_cloud::services::sync::push(state, user, serde_json::from_value(body).unwrap())
            .await
            .unwrap();
    assert_eq!(result.accepted.len(), 1);
}

#[tokio::test]
async fn astra_bootstrap_preserves_updates_during_pagination() {
    let (s, u, d) = astra_account().await;
    let a = Uuid::from_u128(1);
    let b = Uuid::from_u128(2);
    astra_push(&s, u, d, a, 0, 1).await;
    astra_push(&s, u, d, b, 0, 1).await;
    let first = verruckt_cloud::services::sync::bootstrap(&s, u, None, Some(1))
        .await
        .unwrap();
    assert!(first.has_more);
    assert_eq!(first.entities[0].entity_id, a);
    assert_eq!(first.entities[0].revision, 1);
    astra_push(&s, u, d, a, 1, 2).await;
    let last = verruckt_cloud::services::sync::bootstrap(&s, u, Some(&first.next_cursor), Some(1))
        .await
        .unwrap();
    assert!(!last.has_more);
    let pull = verruckt_cloud::services::sync::pull(&s, u, Some(&last.sync_cursor), None)
        .await
        .unwrap();
    assert_eq!(pull.changes.len(), 1);
    assert_eq!(pull.changes[0].revision, 2);
}

#[tokio::test]
async fn astra_sequence_cursor_follows_commit_order() {
    let (s, u, d) = astra_account().await;
    let mut tx = s.pool.begin().await.unwrap();
    let slow:i64=sqlx::query_scalar("INSERT INTO sync_changes(user_id,device_id,change_id,entity_type,entity_id,revision,operation,payload) VALUES($1,$2,$3,'audit',$4,1,'upsert','{}') RETURNING cursor")
 .bind(u).bind(d).bind(Uuid::new_v4()).bind(Uuid::new_v4()).fetch_one(&mut *tx).await.unwrap();
    let task_state = s.clone();
    let fast = tokio::spawn(async move {
        astra_push(&task_state, u, d, Uuid::new_v4(), 0, 1).await;
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !fast.is_finished(),
        "later publication must wait for earlier commit"
    );
    let first = verruckt_cloud::services::sync::pull(&s, u, None, None)
        .await
        .unwrap();
    assert!(first.changes.is_empty());
    tx.commit().await.unwrap();
    fast.await.unwrap();
    let after = verruckt_cloud::services::sync::pull(&s, u, Some(&first.next_cursor), None)
        .await
        .unwrap();
    assert_eq!(after.changes.len(), 2);
    assert_eq!(
        verruckt_cloud::services::sync::decode_cursor(&after.next_cursor).unwrap() > slow,
        true
    );
}

#[tokio::test]
async fn astra_gateway_enforces_output_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/chat/completions",
        post(|Json(body): Json<Value>| async move { Json(body) }),
    );
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let ai = test_ai_gateway(&format!("http://{addr}"));
    let body = json!({"model":"verruckt-fast","messages":[{"role":"user","content":"hi"}],"max_tokens":1000000,"n":20});
    let meta = ai.validate_chat(&body).unwrap();
    assert_eq!(meta.max_tokens, 8192);
    let echoed: Value = ai
        .chat_completion(&meta.upstream_model, &body)
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(echoed["max_tokens"], 8192);
    assert_eq!(echoed["n"], 1);
    handle.abort();
}

#[tokio::test]
async fn astra_device_removal_revokes_tokens() {
    let (s, _, _) = astra_account().await;
    let email = unique_email("astra-device");
    admin::create_user(&s, &email, PASSWORD, Some("pro"))
        .await
        .unwrap();
    let (base, server) = spawn_app(s.clone()).await;
    let client = reqwest::Client::new();
    let auth = login(&client, &base, &email, "removed audit device").await;
    let access = auth["access_token"].as_str().unwrap();
    let device = auth["device"]["id"].as_str().unwrap();
    let removed = client
        .delete(format!("{base}/api/v1/me/devices/{device}"))
        .bearer_auth(access)
        .send()
        .await
        .unwrap();
    assert!(removed.status().is_success());
    let still_valid = client
        .get(format!("{base}/api/v1/me"))
        .bearer_auth(access)
        .send()
        .await
        .unwrap();
    assert_eq!(still_valid.status(), 401);
    let refreshed = client
        .post(format!("{base}/api/v1/auth/refresh"))
        .json(&json!({"refresh_token":auth["refresh_token"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(refreshed.status(), 401);
    server.abort();
}

#[tokio::test]
async fn astra_storage_verifies_upload_before_dedup() {
    use sha2::{Digest, Sha256};
    let (mut s, u, _) = astra_account().await;
    let storage = test_storage().expect("isolated MinIO required");
    storage.ensure_bucket().await.unwrap();
    s.storage = Some(storage.clone());
    let bytes = b"verified backup";
    let sha = hex::encode(Sha256::digest(bytes));
    let request = || {
        serde_json::from_value(json!({"sha256":sha,"size":bytes.len(),"mime":"text/plain"}))
            .unwrap()
    };
    let first = verruckt_cloud::services::files::presign_upload(&s, u, request())
        .await
        .unwrap();
    assert!(!first.deduplicated);
    let second = verruckt_cloud::services::files::presign_upload(&s, u, request())
        .await
        .unwrap();
    assert!(!second.deduplicated);
    assert_eq!(first.file_id, second.file_id);
    assert!(
        verruckt_cloud::services::files::complete_upload(&s, u, first.file_id)
            .await
            .is_err()
    );
    assert!(
        verruckt_cloud::services::files::presign_download(&s, u, first.file_id)
            .await
            .is_err()
    );
    let client = reqwest::Client::new();
    // Changing either the signed size or content digest is rejected by storage.
    let mut wrong = client.put(&first.upload_url);
    for (k, v) in &first.headers {
        wrong = wrong.header(k, v);
    }
    let invalid = wrong
        .header("content-length", "32")
        .body(vec![b'x'; 32])
        .send()
        .await
        .unwrap();
    assert!(!invalid.status().is_success());
    let mut wrong_hash = client.put(&first.upload_url);
    for (k, v) in &first.headers {
        wrong_hash = wrong_hash.header(k, v);
    }
    let invalid = wrong_hash
        .body(vec![b'x'; bytes.len()])
        .send()
        .await
        .unwrap();
    assert!(!invalid.status().is_success());
    let mut put = client.put(&first.upload_url);
    for (k, v) in &first.headers {
        put = put.header(k, v);
    }
    let uploaded = put.body(bytes.to_vec()).send().await.unwrap();
    let status = uploaded.status();
    assert!(
        status.is_success(),
        "{} {}",
        status,
        uploaded.text().await.unwrap()
    );
    verruckt_cloud::services::files::complete_upload(&s, u, first.file_id)
        .await
        .unwrap();
    let third = verruckt_cloud::services::files::presign_upload(&s, u, request())
        .await
        .unwrap();
    assert!(third.deduplicated);
    let get = verruckt_cloud::services::files::presign_download(&s, u, first.file_id)
        .await
        .unwrap();
    assert_eq!(
        client
            .get(get.download_url)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
            .as_ref(),
        bytes
    );
    let charged: i64 =
        sqlx::query_scalar("SELECT bytes_used FROM user_storage_usage WHERE user_id=$1")
            .bind(u)
            .fetch_one(&s.pool)
            .await
            .unwrap();
    assert_eq!(charged, bytes.len() as i64);
}

#[tokio::test]
async fn astra_disconnect_after_output_settles_credits() {
    use futures_util::StreamExt;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let upstream=Router::new().route("/chat/completions",post(||async {
  let stream=async_stream::stream! {
   yield Ok::<_,std::convert::Infallible>(bytes::Bytes::from_static(b"data: {\"choices\":[{\"delta\":{\"content\":\"paid output received\"}}]}\n\n"));
   tokio::time::sleep(Duration::from_millis(200)).await;
   yield Ok::<_,std::convert::Infallible>(bytes::Bytes::from_static(b"data: [DONE]\n\n"));
  };
  Response::builder().header("content-type","text/event-stream").body(Body::from_stream(stream)).unwrap()
 }));
    let upstream_handle = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let (s, _, _) = astra_account().await;
    let pool = s.pool.clone();
    let s = test_state(
        pool.clone(),
        Some(test_ai_gateway(&format!("http://{addr}"))),
        None,
    );
    let email = unique_email("astra-stream");
    admin::create_user(&s, &email, PASSWORD, Some("pro"))
        .await
        .unwrap();
    let (base, server) = spawn_app(s).await;
    let client = reqwest::Client::new();
    let auth = login(&client, &base, &email, "stream audit").await;
    let user: Uuid = serde_json::from_value(auth["user"]["id"].clone()).unwrap();
    let response=client.post(format!("{base}/api/v1/ai/chat/stream")).bearer_auth(auth["access_token"].as_str().unwrap()).json(&json!({"model":"verruckt-fast","messages":[{"role":"user","content":"hello"}],"max_tokens":1})).send().await.unwrap();
    assert!(response.status().is_success());
    let mut body = response.bytes_stream();
    let chunk = body.next().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&chunk).contains("paid output received"));
    drop(body);
    let mut released = false;
    for _ in 0..100 {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM ai_reservations WHERE user_id=$1")
                .bind(user)
                .fetch_optional(&pool)
                .await
                .unwrap();
        if status.as_deref() == Some("completed") {
            released = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(released, "disconnect must finish accounting");
    let used: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(used),0)::bigint FROM usage_counters WHERE user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(used > 0);
    server.abort();
    upstream_handle.abort();
}
