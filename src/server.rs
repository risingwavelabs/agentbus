use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Extension, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::signal;
use tower_http::cors::CorsLayer;

use crate::config::ServerConfig;
use crate::daemon;
use crate::db::Database;

// --- Tenant ---

#[derive(Clone)]
pub struct Tenant(pub String);

// --- App State ---

pub struct AppState {
    pub db: Database,
}

pub type SharedState = Arc<AppState>;

// --- Request/Response types ---

#[derive(Deserialize)]
struct RegisterAgentRequest {
    id: String,
    #[serde(default)]
    aliases: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SendInboxRequest {
    thread_id: String,
    from: String,
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct InboxQuery {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    timeout: Option<f64>,
}

#[derive(Deserialize)]
struct RegisterWorkerRequest {
    name: String,
    instructions: String,
    #[serde(default = "default_node_id")]
    node_id: String,
}

fn default_node_id() -> String {
    "local".to_string()
}

#[derive(Deserialize)]
struct RegisterNodeRequest {
    id: String,
}

#[derive(Deserialize)]
struct TeamInviteRequest {
    #[serde(default)]
    description: Option<String>,
}

// --- Auth Middleware ---

async fn auth_middleware(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: Next,
) -> impl IntoResponse {
    if !state.db.has_api_keys() {
        // No API keys configured — open access, default tenant
        request
            .extensions_mut()
            .insert(Tenant("default".to_string()));
        return next.run(request).await;
    }

    let key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if key.is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "missing X-API-Key header"})),
        )
            .into_response();
    }

    match state.db.validate_api_key(key) {
        Ok(Some(tenant)) => {
            request.extensions_mut().insert(Tenant(tenant));
            next.run(request).await
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid API key"})),
        )
            .into_response(),
    }
}

// --- Handlers: Health ---

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "version": "0.1.0"
    }))
}

// --- Handlers: Agents ---

async fn list_agents_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
) -> impl IntoResponse {
    match state.db.list_agents(&tenant) {
        Ok(agents) => (StatusCode::OK, Json(serde_json::json!({"agents": agents}))).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn register_agent_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Json(req): Json<RegisterAgentRequest>,
) -> impl IntoResponse {
    match state
        .db
        .register_agent(&tenant, &req.id, req.aliases.as_deref())
    {
        Ok(agent) => (
            StatusCode::CREATED,
            Json(serde_json::to_value(agent).unwrap()),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn delete_agent_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_agent(&tenant, &agent_id) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted", "agent_id": agent_id})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

// --- Handlers: Inbox ---

async fn send_inbox_message_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(agent_id): Path<String>,
    Json(req): Json<SendInboxRequest>,
) -> impl IntoResponse {
    let resolved_id = match state.db.resolve_agent(&tenant, &agent_id) {
        Ok(Some(id)) => id,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "agent not found"),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let valid_types = ["request", "question", "answer", "done", "failed", "message"];
    if !valid_types.contains(&req.msg_type.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "type must be one of: request, question, answer, done, failed, message",
        );
    }

    match state.db.send_inbox_message(
        &tenant,
        &req.thread_id,
        &req.from,
        &resolved_id,
        &req.msg_type,
        req.content.as_ref(),
    ) {
        Ok(msg) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "message_id": msg.id,
                "created_at": msg.created_at,
            })),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_inbox_messages_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(agent_id): Path<String>,
    Query(params): Query<InboxQuery>,
) -> impl IntoResponse {
    let resolved_id = match state.db.resolve_agent(&tenant, &agent_id) {
        Ok(Some(id)) => id,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "agent not found"),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let _ = state.db.update_last_seen(&tenant, &resolved_id);

    let timeout = params.timeout.unwrap_or(0.0).clamp(0.0, 30.0);
    let start = std::time::Instant::now();

    loop {
        match state.db.get_inbox_messages(
            &tenant,
            &resolved_id,
            params.status.as_deref(),
            params.thread_id.as_deref(),
        ) {
            Ok(messages) if !messages.is_empty() || timeout <= 0.0 => {
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({"messages": messages})),
                )
                    .into_response();
            }
            Ok(_) => {}
            Err(e) => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
            }
        }

        if start.elapsed().as_secs_f64() >= timeout {
            let empty: Vec<crate::db::InboxMessage> = vec![];
            return (
                StatusCode::OK,
                Json(serde_json::json!({"messages": empty})),
            )
                .into_response();
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

async fn ack_inbox_message_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(message_id): Path<String>,
) -> impl IntoResponse {
    match state.db.ack_inbox_message(&tenant, &message_id) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "acked", "message_id": message_id})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

async fn get_thread_messages_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(thread_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_thread_messages(&tenant, &thread_id) {
        Ok(messages) => (
            StatusCode::OK,
            Json(serde_json::json!({"messages": messages})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// --- Handlers: Workers ---

async fn register_worker_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Json(req): Json<RegisterWorkerRequest>,
) -> impl IntoResponse {
    match state
        .db
        .register_worker(&tenant, &req.name, &req.instructions, &req.node_id)
    {
        Ok(worker) => (
            StatusCode::CREATED,
            Json(serde_json::to_value(worker).unwrap()),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_workers_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
) -> impl IntoResponse {
    match state.db.list_workers(&tenant) {
        Ok(workers) => (
            StatusCode::OK,
            Json(serde_json::json!({"workers": workers})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_worker_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.db.get_worker(&tenant, &name) {
        Ok(Some(worker)) => {
            (StatusCode::OK, Json(serde_json::to_value(worker).unwrap())).into_response()
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "worker not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn remove_worker_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.db.remove_worker(&tenant, &name) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "removed", "worker": name})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

// --- Handlers: Nodes ---

async fn register_node_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Json(req): Json<RegisterNodeRequest>,
) -> impl IntoResponse {
    match state.db.register_node(&tenant, &req.id) {
        Ok(node) => (
            StatusCode::CREATED,
            Json(serde_json::to_value(node).unwrap()),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_nodes_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
) -> impl IntoResponse {
    match state.db.list_nodes(&tenant) {
        Ok(nodes) => (StatusCode::OK, Json(serde_json::json!({"nodes": nodes}))).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn remove_node_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(node_id): Path<String>,
) -> impl IntoResponse {
    match state.db.remove_node(&tenant, &node_id) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "removed", "node": node_id})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn heartbeat_node_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(node_id): Path<String>,
) -> impl IntoResponse {
    match state.db.heartbeat_node(&tenant, &node_id) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok"})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// --- Handlers: Team ---

async fn team_invite_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Json(req): Json<TeamInviteRequest>,
) -> impl IntoResponse {
    let desc = req.description.unwrap_or_default();
    match state.db.create_api_key_full(&tenant, &desc) {
        Ok((full_key, info)) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "key": full_key,
                "key_prefix": info.key_prefix,
                "description": info.description,
            })),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn team_list_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
) -> impl IntoResponse {
    match state.db.list_api_keys(&tenant) {
        Ok(keys) => (StatusCode::OK, Json(serde_json::json!({"keys": keys}))).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn team_revoke_handler(
    State(state): State<SharedState>,
    Extension(Tenant(tenant)): Extension<Tenant>,
    Path(key_prefix): Path<String>,
) -> impl IntoResponse {
    match state.db.revoke_api_key(&tenant, &key_prefix) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "revoked"})),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

// --- Helpers ---

fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({"error": message}))).into_response()
}

// --- Server ---

pub async fn run(config: ServerConfig) {
    let db = Database::new(&config.db_path).expect("failed to initialize database");

    // Auto-register "local" node
    let _ = db.register_node("default", "local");

    let state = Arc::new(AppState { db });

    // Spawn daemon for "local" node
    let daemon_state = state.clone();
    tokio::spawn(async move {
        daemon::run_local(daemon_state).await;
    });

    // Public routes
    let public = Router::new().route("/health", get(health_handler));

    // Protected routes
    let protected = Router::new()
        // Agents
        .route(
            "/agents",
            get(list_agents_handler).post(register_agent_handler),
        )
        .route("/agents/{agent_id}", delete(delete_agent_handler))
        // Inbox
        .route(
            "/agents/{agent_id}/inbox",
            get(get_inbox_messages_handler).post(send_inbox_message_handler),
        )
        .route(
            "/inbox/messages/{message_id}/ack",
            post(ack_inbox_message_handler),
        )
        .route(
            "/threads/{thread_id}/messages",
            get(get_thread_messages_handler),
        )
        // Workers
        .route(
            "/workers",
            get(list_workers_handler).post(register_worker_handler),
        )
        .route(
            "/workers/{name}",
            get(get_worker_handler).delete(remove_worker_handler),
        )
        // Nodes
        .route("/nodes", get(list_nodes_handler).post(register_node_handler))
        .route("/nodes/{node_id}", delete(remove_node_handler))
        .route("/nodes/{node_id}/heartbeat", post(heartbeat_node_handler))
        // Team
        .route("/team", get(team_list_handler))
        .route("/team/invite", post(team_invite_handler))
        .route("/team/{key_prefix}", delete(team_revoke_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let app = Router::new()
        .merge(public)
        .merge(protected)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = config.address();
    tracing::info!(address = %addr, "Boxhouse server starting");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    tracing::info!("Boxhouse server stopped");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
