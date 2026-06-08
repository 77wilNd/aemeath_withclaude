use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::state::{PendingInputSlot, PetState, SharedState, StateChangeEvent};

#[derive(Debug, Deserialize)]
pub struct StateRequest {
    pub s: String,
    #[serde(default)]
    pub tool: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CurrentResponse {
    pub animation: String,
    pub bubble: String,
    pub core_signal: String,
    pub tool_label: Option<String>,
    pub overlay: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserInputRequest {
    pub value: String,
    #[serde(rename = "type", default)]
    pub _input_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserMessageRequest {
    pub value: String,
}

pub fn create_router(
    state: SharedState,
    tx: broadcast::Sender<StateChangeEvent>,
    pending_input: PendingInputSlot,
    claude_hwnd: Arc<std::sync::Mutex<isize>>,
) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/state", post(handle_state))
        .route("/api/heartbeat", get(handle_heartbeat))
        .route("/api/current", get(handle_current))
        .route("/api/hook/thinking", post(handle_hook_thinking))
        .route("/api/hook/working", post(handle_hook_working))
        .route("/api/hook/done", post(handle_hook_done))
        .route("/api/hook/idle", post(handle_hook_idle))
        .route("/api/hook/permission", post(handle_hook_permission))
        .route("/api/user/input", post(handle_user_input))
        .route("/api/user/pending", get(handle_user_pending))
        .route("/api/user/message", post(handle_user_message))
        .route("/api/user/message/pending", get(handle_user_message_pending))
        .route("/api/session/messages", get(handle_session_messages))
        .layer(cors)
        .with_state(AppState {
            state,
            tx,
            pending_input,
            claude_hwnd,
        })
}

#[derive(Clone)]
struct AppState {
    state: SharedState,
    tx: broadcast::Sender<StateChangeEvent>,
    pending_input: PendingInputSlot,
    claude_hwnd: Arc<std::sync::Mutex<isize>>,
}

/// Helper: build a StateChangeEvent from state + tool, with derived core_signal/overlay
fn build_event(state: &PetState, tool: Option<&str>) -> StateChangeEvent {
    StateChangeEvent {
        animation: state.animation_name().to_string(),
        bubble: state.bubble_text(tool).to_string(),
        core_signal: state.core_signal().to_string(),
        tool_label: tool.map(|s| s.to_string()),
        overlay: state.overlay().map(|s| s.to_string()),
        input_type: None,
        options: None,
    }
}

async fn handle_state(
    State(app): State<AppState>,
    Json(body): Json<StateRequest>,
) -> StatusCode {
    let pet_state = PetState::from_hook(&body.s, body.tool.as_deref());
    let tool = body.tool.clone();

    {
        let mut mgr = app.state.lock().await;
        mgr.set_state(pet_state.clone(), tool.clone());
    }

    let event = build_event(&pet_state, tool.as_deref());
    let _ = app.tx.send(event);
    StatusCode::OK
}

async fn handle_heartbeat() -> StatusCode {
    StatusCode::OK
}

async fn handle_current(
    State(app): State<AppState>,
) -> Json<CurrentResponse> {
    let mgr = app.state.lock().await;
    let current = mgr.current_state();
    let tool = mgr.current_tool();
    Json(CurrentResponse {
        animation: current.animation_name().to_string(),
        bubble: current.bubble_text(tool).to_string(),
        core_signal: current.core_signal().to_string(),
        tool_label: tool.map(|s| s.to_string()),
        overlay: current.overlay().map(|s| s.to_string()),
    })
}

async fn handle_hook_thinking(
    State(app): State<AppState>,
) -> StatusCode {
    set_pet_state(&app, PetState::Chatting, None).await;
    StatusCode::OK
}

async fn handle_hook_working(
    State(app): State<AppState>,
    body: String,
) -> StatusCode {
    let tool = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("tool_name").and_then(|t| t.as_str().map(String::from)));

    // Map specific tools to specific animations
    let state = match tool.as_deref() {
        Some("WebFetch") => PetState::Fetching,
        Some("WebSearch") => PetState::Searching,
        Some("Write") | Some("Edit") => PetState::Building,
        Some("Agent") | Some("TaskCreate") | Some("TaskUpdate") => PetState::Analyzing,
        _ => PetState::Running,
    };

    set_pet_state(&app, state, tool).await;
    StatusCode::OK
}

async fn handle_hook_done(
    State(app): State<AppState>,
) -> StatusCode {
    set_pet_state(&app, PetState::Celebrating, None).await;
    StatusCode::OK
}

async fn handle_hook_idle(
    State(app): State<AppState>,
) -> StatusCode {
    set_pet_state(&app, PetState::Idle, None).await;
    StatusCode::OK
}

async fn handle_hook_permission(
    State(app): State<AppState>,
) -> StatusCode {
    {
        let mut mgr = app.state.lock().await;
        mgr.set_state(PetState::Permission, None);
    }
    let event = build_event(&PetState::Permission, None);
    let _ = app.tx.send(event);
    StatusCode::OK
}

async fn set_pet_state(app: &AppState, state: PetState, tool: Option<String>) {
    // W1: single lock acquisition to avoid TOCTOU
    let mut mgr = app.state.lock().await;
    let is_active = matches!(mgr.current_state(),
        PetState::Running | PetState::Chatting | PetState::Fetching
        | PetState::Searching | PetState::Analyzing | PetState::Building
    );
    if !matches!(state,
        PetState::Running | PetState::Chatting | PetState::Fetching
        | PetState::Searching | PetState::Analyzing | PetState::Building
    ) && is_active
        && mgr.should_keep_running(800)
    {
        return;
    }
    mgr.set_state(state.clone(), tool.clone());
    drop(mgr);
    let event = build_event(&state, tool.as_deref());
    let _ = app.tx.send(event);
}

/// POST /api/user/input — receive user response from frontend, forward to pending oneshot
async fn handle_user_input(
    State(app): State<AppState>,
    Json(body): Json<UserInputRequest>,
) -> StatusCode {
    let mut pi = app.pending_input.lock().await;
    let pending = pi.take();
    drop(pi); // C1: release pending_input lock before acquiring app.state

    if let Some(pending) = pending {
        let _ = pending.tx.send(body.value);
        // Clear input overlay — send a running event to reset
        let anim = app.state.lock().await.current_state().animation_name().to_string();
        let _ = app.tx.send(StateChangeEvent {
            animation: anim,
            bubble: "收到!".to_string(),
            core_signal: "running".to_string(),
            tool_label: None,
            overlay: None,
            input_type: None,
            options: None,
        });
    } else {
        // C4: No pending input — broadcast message as bubble instead of silent discard
        let anim = app.state.lock().await.current_state().animation_name().to_string();
        let _ = app.tx.send(StateChangeEvent {
            animation: anim,
            bubble: body.value,
            core_signal: "waiting".to_string(),
            tool_label: None,
            overlay: None,
            input_type: None,
            options: None,
        });
    }
    StatusCode::OK
}

/// GET /api/user/pending — tell frontend whether backend is waiting for user input + input type
async fn handle_user_pending(
    State(app): State<AppState>,
) -> Json<serde_json::Value> {
    let pi = app.pending_input.lock().await;
    if let Some(ref pending) = *pi {
        Json(json!({
            "waiting": true,
            "input_type": pending.input_type,
            "options": pending.options,
        }))
    } else {
        Json(json!({ "waiting": false }))
    }
}

/// POST /api/user/message — receive message from pet UI, relay to Claude Code terminal
async fn handle_user_message(
    State(app): State<AppState>,
    Json(body): Json<UserMessageRequest>,
) -> StatusCode {
    let msg = body.value.clone();
    let mut mgr = app.state.lock().await;
    mgr.push_message(msg.clone());
    drop(mgr);

    let hwnd = *app.claude_hwnd.lock().unwrap();

    // Relay the message to Claude Code terminal as keystrokes
    relay_to_terminal(&msg, hwnd);

    StatusCode::OK
}

/// Copy message to clipboard, then paste into the Claude Code terminal window.
fn relay_to_terminal(msg: &str, hwnd: isize) {
    use std::os::windows::process::CommandExt;

    // 1. Copy message to clipboard
    let escaped = msg.replace('\'', "''");
    let set_clip = format!("Set-Clipboard -Value '{}'", escaped);
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &set_clip])
        .creation_flags(0x08000000)
        .output();

    // 2. Paste into Claude Code window
    std::thread::spawn(move || {
        unsafe {
            paste_to_window(hwnd);
        }
    });
}

// ---- Windows API FFI ----

static FOUND_HWND: std::sync::Mutex<isize> = std::sync::Mutex::new(0);

// Search terms for finding the Claude Code terminal window
const SEARCH_TERMS: &[&str] = &["claude"];

unsafe extern "system" fn enum_callback(hwnd: isize, _lparam: isize) -> i32 {
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
    if len > 0 {
        let title = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
        for term in SEARCH_TERMS {
            if title.contains(term) {
                *FOUND_HWND.lock().unwrap() = hwnd;
                return 0; // stop
            }
        }
    }
    1 // continue
}

unsafe fn paste_to_window(bound_hwnd: isize) {
    // 1. Search for the current topmost "claude" window
    {
        *FOUND_HWND.lock().unwrap() = 0;
    }
    EnumWindows(Some(enum_callback), 0);
    let mut hwnd = *FOUND_HWND.lock().unwrap();

    // 2. Fall back to bound HWND if search found nothing (e.g. terminal minimized)
    if hwnd == 0 && bound_hwnd != 0 && IsWindow(bound_hwnd) != 0 {
        hwnd = bound_hwnd;
    }

    if hwnd == 0 {
        return;
    }

    let prev = GetForegroundWindow();

    ShowWindow(hwnd, 9); // SW_RESTORE
    SetForegroundWindow(hwnd);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Ctrl+V
    keybd_event(0x11, 0, 0, 0);
    keybd_event(0x56, 0, 0, 0);
    keybd_event(0x56, 0, 2, 0);
    keybd_event(0x11, 0, 2, 0);

    std::thread::sleep(std::time::Duration::from_millis(80));

    // Enter
    keybd_event(0x0D, 0, 0, 0);
    keybd_event(0x0D, 0, 2, 0);

    // Restore previous focus
    std::thread::sleep(std::time::Duration::from_millis(150));
    if prev != 0 {
        SetForegroundWindow(prev);
    }
}

extern "system" {
    fn EnumWindows(cb: Option<unsafe extern "system" fn(isize, isize) -> i32>, lp: isize) -> i32;
    fn GetWindowTextW(hwnd: isize, text: *mut u16, max: i32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn GetForegroundWindow() -> isize;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
}

/// GET /api/user/message/pending — return and clear pending user messages
async fn handle_user_message_pending(
    State(app): State<AppState>,
) -> Json<serde_json::Value> {
    let mut mgr = app.state.lock().await;
    let msgs = mgr.drain_messages();
    Json(json!({
        "messages": msgs,
        "count": msgs.len(),
    }))
}


/// GET /api/session/messages — return recent messages from the active session
async fn handle_session_messages() -> Json<serde_json::Value> {
    let claude_dir = std::path::PathBuf::from(
        std::env::var("USERPROFILE").unwrap_or_default()
    ).join(".claude").join("projects");
    
    let mut messages = Vec::new();
    let mut newest_time = std::time::SystemTime::UNIX_EPOCH;
    let mut newest_file = None;
    
    // Find the most recent jsonl across all projects
    if let Ok(projects) = std::fs::read_dir(&claude_dir) {
        for project in projects.flatten() {
            let proj = project.path();
            if !proj.is_dir() { continue; }
            if let Ok(sessions) = std::fs::read_dir(&proj) {
                for session in sessions.flatten() {
                    let path = session.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
                    if let Ok(meta) = path.metadata() {
                        if let Ok(mod_time) = meta.modified() {
                            if mod_time > newest_time {
                                newest_time = mod_time;
                                newest_file = Some(path.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Read last 30 messages from newest session
    if let Some(path) = newest_file {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let mut msgs: Vec<serde_json::Value> = content
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .collect();
            let start = if msgs.len() > 30 { msgs.len() - 30 } else { 0 };
            for m in &msgs[start..] {
                let role = match m.get("type").and_then(|t| t.as_str()) {
                    Some("user") => "user",
                    Some("assistant") => "assistant",
                    _ => "tool",
                };
                let text = if role == "user" || role == "assistant" {
                    m.get("message").and_then(|msg| msg.get("content")).and_then(|c| c.as_str()).unwrap_or("")
                } else { "" };
                if text.len() > 1 {
                    messages.push(serde_json::json!({"role": role, "content": text}));
                }
            }
        }
    }
    
    Json(serde_json::json!({"messages": messages}))
}
