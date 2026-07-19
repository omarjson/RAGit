use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use tokio::sync::Notify;

use crate::rag::store::Store;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    Admin,
    Editor,
    Viewer,
}

impl Role {
    fn from_str(s: &str) -> Role {
        match s.to_lowercase().as_str() {
            "admin" => Role::Admin,
            "editor" => Role::Editor,
            _ => Role::Viewer,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Editor => "editor",
            Role::Viewer => "viewer",
        }
    }
    fn can(&self, required: &Role) -> bool {
        let rank = |r: &Role| match r {
            Role::Viewer => 1,
            Role::Editor => 2,
            Role::Admin => 3,
        };
        rank(self) >= rank(required)
    }
}

pub struct TeamState {
    pub store: Store,
    pub secret: String,
    pub shutdown: Arc<Notify>,
}

fn secret_file() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("ragit");
    std::fs::create_dir_all(&dir).ok();
    dir.push("team_secret.txt");
    dir
}

fn load_or_create_secret() -> String {
    let path = secret_file();
    if let Ok(s) = std::fs::read_to_string(&path) {
        let s = s.trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }
    let secret = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::write(&path, &secret);
    secret
}

fn hash_password(pw: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

fn verify_password(pw: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false };
    Argon2::default().verify_password(pw.as_bytes(), &parsed).is_ok()
}

fn sign_token(user_id: &str, secret: &str, expires_at: i64) -> String {
    let payload = format!("{user_id}.{expires_at}");
    let b = B64.encode(payload);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(b.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{b}.{sig}")
}

fn verify_token(token: &str, secret: &str) -> Option<String> {
    let (b, sig) = token.split_once('.')?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(b.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if expected != sig { return None; }
    let payload = B64.decode(b).ok()?;
    let payload = String::from_utf8(payload).ok()?;
    let (user_id, _exp) = payload.split_once('.')?;
    Some(user_id.to_string())
}

#[derive(Deserialize)]
struct RegisterReq { username: String, password: String, role: Option<String> }

#[derive(Deserialize)]
struct LoginReq { username: String, password: String }

#[derive(Serialize)]
struct UserOut { id: String, username: String, role: String }

fn auth_from_headers(headers: &HeaderMap, state: &TeamState) -> Option<(String, Role)> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))?;
    let _user_id = verify_token(token, &state.secret)?;
    let (uid, role) = state.store.user_for_token(token).ok().flatten()?;
    Some((uid, Role::from_str(&role)))
}

fn unauthorized() -> axum::response::Response {
    (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorized"}))).into_response()
}

async fn register(
    State(state): State<Arc<TeamState>>,
    headers: HeaderMap,
    Json(req): Json<RegisterReq>,
) -> impl IntoResponse {
    let existing = state.store.list_users().map(|u| u.len()).unwrap_or(0);
    let role = if existing == 0 {
        Role::Admin
    } else {
        let Some((_, caller)) = auth_from_headers(&headers, &state) else {
            return unauthorized();
        };
        if !caller.can(&Role::Admin) {
            return (StatusCode::FORBIDDEN, Json(json!({"error": "admin required"}))).into_response();
        }
        Role::from_str(&req.role.unwrap_or_else(|| "viewer".into()))
    };
    if req.username.trim().is_empty() || req.password.len() < 4 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid input"}))).into_response();
    }
    let pw_hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    };
    let id = uuid::Uuid::new_v4().to_string();
    match state.store.create_user(&id, &req.username, &pw_hash, role.as_str()) {
        Ok(()) => (StatusCode::CREATED, Json(json!({"id": id, "role": role.as_str()}))).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn login(
    State(state): State<Arc<TeamState>>,
    Json(req): Json<LoginReq>,
) -> impl IntoResponse {
    let user = match state.store.find_user_by_username(&req.username) {
        Ok(Some(u)) => u,
        _ => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid credentials"}))).into_response(),
    };
    let (uid, hash, role) = user;
    if !verify_password(&req.password, &hash) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid credentials"}))).into_response();
    }
    let expires = Utc::now().timestamp() + 60 * 60 * 24 * 7;
    let token = sign_token(&uid, &state.secret, expires);
    let _ = state.store.add_session(&token, &uid, expires);
    (StatusCode::OK, Json(json!({"token": token, "role": role, "user_id": uid}))).into_response()
}

async fn logout(State(state): State<Arc<TeamState>>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v).to_string())
    {
        let _ = state.store.delete_session(&token);
    }
    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

async fn me(State(state): State<Arc<TeamState>>, headers: HeaderMap) -> impl IntoResponse {
    let Some((uid, role)) = auth_from_headers(&headers, &state) else {
        return unauthorized();
    };
    (StatusCode::OK, Json(json!({"user_id": uid, "role": role.as_str()}))).into_response()
}

async fn list_users(State(state): State<Arc<TeamState>>, headers: HeaderMap) -> impl IntoResponse {
    let Some((_, role)) = auth_from_headers(&headers, &state) else {
        return unauthorized();
    };
    if !role.can(&Role::Admin) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "admin required"}))).into_response();
    }
    match state.store.list_users() {
        Ok(users) => {
            let out: Vec<UserOut> = users.into_iter()
                .map(|(id, username, role)| UserOut { id, username, role })
                .collect();
            (StatusCode::OK, Json(json!(out))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

#[derive(Deserialize)]
struct SetRoleReq { user_id: String, role: String }

async fn set_role(
    State(state): State<Arc<TeamState>>,
    headers: HeaderMap,
    Json(req): Json<SetRoleReq>,
) -> impl IntoResponse {
    let Some((_, role)) = auth_from_headers(&headers, &state) else {
        return unauthorized();
    };
    if !role.can(&Role::Admin) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "admin required"}))).into_response();
    }
    match state.store.set_user_role(&req.user_id, &req.role) {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

#[derive(Deserialize)]
struct ChatReq { message: String, library_id: Option<String>, rerank: Option<bool> }

async fn chat(
    State(state): State<Arc<TeamState>>,
    headers: HeaderMap,
    Json(req): Json<ChatReq>,
) -> impl IntoResponse {
    let Some((uid, role)) = auth_from_headers(&headers, &state) else {
        return unauthorized();
    };
    let library_id = req.library_id.unwrap_or_else(|| "default".into());
    if !role.can(&Role::Admin) {
        let lib_role = state.store.library_role(&library_id, &uid).ok().flatten();
        if lib_role.is_none() {
            return (StatusCode::FORBIDDEN, Json(json!({"error": "no access to library"}))).into_response();
        }
    }
    let store = state.store.clone();
    let rerank = req.rerank.unwrap_or(false);
    let result = tokio::task::spawn_blocking({
        let library_id = library_id.clone();
        let message = req.message.clone();
        move || crate::rag::indexer::team_rag_answer(&store, &library_id, &message, rerank)
    }).await;
    match result {
        Ok(Ok(answer)) => (StatusCode::OK, Json(json!({"answer": answer}))).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

#[derive(Deserialize)]
struct GrantReq { library_id: String, user_id: String, role: String }

async fn grant(
    State(state): State<Arc<TeamState>>,
    headers: HeaderMap,
    Json(req): Json<GrantReq>,
) -> impl IntoResponse {
    let Some((_, role)) = auth_from_headers(&headers, &state) else {
        return unauthorized();
    };
    if !role.can(&Role::Admin) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "admin required"}))).into_response();
    }
    match state.store.grant_membership(&req.library_id, &req.user_id, &req.role) {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn list_libraries(
    State(state): State<Arc<TeamState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some((uid, role)) = auth_from_headers(&headers, &state) else {
        return unauthorized();
    };
    let libs = match state.store.list_files_libs() {
        Ok(l) => l,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };
    let filtered: Vec<String> = if role.can(&Role::Admin) {
        libs
    } else {
        libs.into_iter()
            .filter(|lib| state.store.library_role(lib, &uid).ok().flatten().is_some())
            .collect()
    };
    (StatusCode::OK, Json(json!(filtered))).into_response()
}

fn build_router(state: Arc<TeamState>) -> Router {
    Router::new()
        .route("/api/register", post(register))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/users", get(list_users))
        .route("/api/users/role", post(set_role))
        .route("/api/libraries", get(list_libraries))
        .route("/api/libraries/grant", post(grant))
        .route("/api/chat", post(chat))
        .with_state(state)
}

pub fn start_team_server(port: u16) -> Result<Arc<TeamState>, String> {
    let state = crate::APP_STATE.get().ok_or("app not initialized")?;
    let secret = load_or_create_secret();
    let shutdown = Arc::new(Notify::new());
    let team_state = Arc::new(TeamState {
        store: state.store.clone(),
        secret,
        shutdown: shutdown.clone(),
    });
    let app = build_router(team_state.clone());
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("team server bind error: {e}");
                    return;
                }
            };
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(async move { shutdown.notified().await })
                .await
            {
                eprintln!("team server error: {e}");
            }
        });
    });
    Ok(team_state)
}

#[tauri::command]
pub fn start_team_server_cmd(port: Option<u16>) -> Result<String, String> {
    let port = port.unwrap_or(11436);
    let state = crate::APP_STATE.get().ok_or("app not initialized")?;
    if state.team.lock().map_err(|e| e.to_string())?.is_some() {
        return Ok(format!("Team server already running on port {port}"));
    }
    let team_state = start_team_server(port)?;
    *state.team.lock().map_err(|e| e.to_string())? = Some(team_state);
    Ok(format!("Team server listening on http://0.0.0.0:{port}"))
}

#[tauri::command]
pub fn stop_team_server_cmd() -> Result<String, String> {
    let state = crate::APP_STATE.get().ok_or("app not initialized")?;
    match state.team.lock().map_err(|e| e.to_string())?.take() {
        Some(s) => {
            s.shutdown.notify_one();
            Ok("Team server stopped".into())
        }
        None => Ok("Team server was not running".into()),
    }
}

#[tauri::command]
pub fn team_status_cmd() -> Result<bool, String> {
    let state = crate::APP_STATE.get().ok_or("app not initialized")?;
    Ok(state.team.lock().map_err(|e| e.to_string())?.is_some())
}
