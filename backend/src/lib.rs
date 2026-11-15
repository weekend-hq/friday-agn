use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

const MAX_MESSAGE_LENGTH: usize = 10_000;
const MAX_SESSION_ID_LENGTH: usize = 128;

#[derive(Clone, Default)]
pub struct AppState {
    tasks: Arc<RwLock<HashMap<Uuid, Task>>>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum TaskStatus {
    Received,
    Stopped,
}

#[derive(Clone, Serialize)]
struct Task {
    task_id: Uuid,
    message_id: Uuid,
    session_id: String,
    message: String,
    status: TaskStatus,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateMessageRequest {
    session_id: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<ApiError>,
}

#[derive(Serialize)]
struct ApiError {
    code: &'static str,
    message: String,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn failure(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError {
                code,
                message: message.into(),
            }),
        }
    }
}

pub fn app() -> Router {
    app_with_state(AppState::default())
}

pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/system/health", get(health))
        .route("/api/v1/agent/status", get(agent_status))
        .route("/api/v1/agent/capabilities", get(capabilities))
        .route("/api/v1/agent/message", post(receive_message))
        .route("/api/v1/agent/tasks/{task_id}", get(get_task))
        .route("/api/v1/agent/stop", post(stop_task))
        .fallback(not_found)
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({
            "status": "ok",
            "service": "friday-agn-backend"
        }))),
    )
}

async fn agent_status() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({
            "status": "idle",
            "active_execution_id": null,
            "llm_status": "not_configured",
            "browser_status": "not_configured"
        }))),
    )
}

async fn capabilities() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({
            "capabilities": [
                "agent.message",
                "agent.status",
                "agent.stop",
                "system.health"
            ]
        }))),
    )
}

async fn receive_message(
    State(state): State<AppState>,
    Json(request): Json<CreateMessageRequest>,
) -> impl IntoResponse {
    let message = request.message.trim();
    if message.is_empty() || message.len() > MAX_MESSAGE_LENGTH {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::<()>::failure(
                "INVALID_MESSAGE",
                format!("Message must be between 1 and {MAX_MESSAGE_LENGTH} bytes."),
            )),
        )
            .into_response();
    }

    let session_id = match request.session_id {
        Some(session_id) if session_id.trim().is_empty() => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponse::<()>::failure(
                    "INVALID_SESSION_ID",
                    "Session ID must not be empty.",
                )),
            )
                .into_response();
        }
        Some(session_id) if session_id.len() > MAX_SESSION_ID_LENGTH => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponse::<()>::failure(
                    "INVALID_SESSION_ID",
                    format!("Session ID must be at most {MAX_SESSION_ID_LENGTH} bytes."),
                )),
            )
                .into_response();
        }
        Some(session_id) => session_id,
        None => Uuid::new_v4().to_string(),
    };

    let task = Task {
        task_id: Uuid::new_v4(),
        message_id: Uuid::new_v4(),
        session_id,
        message: message.to_owned(),
        status: TaskStatus::Received,
        created_at: Utc::now(),
    };
    state.tasks.write().await.insert(task.task_id, task.clone());

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse::success(serde_json::json!({
            "task_id": task.task_id,
            "message_id": task.message_id,
            "session_id": task.session_id,
            "status": task.status,
            "message": "Task received. Planning and browser execution are not connected yet."
        }))),
    )
        .into_response()
}

async fn get_task(State(state): State<AppState>, Path(task_id): Path<String>) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&task_id) {
        Ok(task_id) => task_id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::failure(
                    "INVALID_TASK_ID",
                    "Task ID must be a UUID.",
                )),
            )
                .into_response();
        }
    };

    match state.tasks.read().await.get(&task_id).cloned() {
        Some(task) => (StatusCode::OK, Json(ApiResponse::success(task))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::failure(
                "TASK_NOT_FOUND",
                "No task exists with that ID.",
            )),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct StopTaskRequest {
    task_id: String,
}

async fn stop_task(
    State(state): State<AppState>,
    Json(request): Json<StopTaskRequest>,
) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&request.task_id) {
        Ok(task_id) => task_id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::failure(
                    "INVALID_TASK_ID",
                    "Task ID must be a UUID.",
                )),
            )
                .into_response();
        }
    };

    let mut tasks = state.tasks.write().await;
    match tasks.get_mut(&task_id) {
        Some(task) if matches!(task.status, TaskStatus::Received) => {
            task.status = TaskStatus::Stopped;
            (
                StatusCode::OK,
                Json(ApiResponse::success(serde_json::json!({
                    "task_id": task_id,
                    "status": task.status
                }))),
            )
                .into_response()
        }
        Some(_) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::<()>::failure(
                "TASK_NOT_STOPPABLE",
                "Only a task that has not started can be stopped by this API.",
            )),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::failure(
                "TASK_NOT_FOUND",
                "No task exists with that ID.",
            )),
        )
            .into_response(),
    }
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<()>::failure(
            "NOT_FOUND",
            "The requested API endpoint does not exist.",
        )),
    )
}
