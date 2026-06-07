#![windows_subsystem = "windows"]

mod http;
mod mcp;
mod webui;
mod state;
mod tray;

use state::PendingInputSlot;
use state::StateManager;
use state::StateChangeEvent;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tauri::Emitter;

#[tokio::main]
async fn main() {
    // Capture the foreground window HWND before Tauri creates its own window.
    // At startup (triggered by Claude Code), the foreground window is the Claude Code terminal.
    let claude_hwnd: Arc<std::sync::Mutex<isize>> = Arc::new(std::sync::Mutex::new(0));
    unsafe {
        let hwnd = GetForegroundWindow();
        *claude_hwnd.lock().unwrap() = hwnd;
        println!("Claude Code HWND bound: {}", hwnd);
    }

    let state_manager = Arc::new(Mutex::new(StateManager::new()));
    let (tx, _rx) = broadcast::channel::<StateChangeEvent>(32);
    let pending_input: PendingInputSlot = Arc::new(Mutex::new(None));

    let sm_http = state_manager.clone();
    let tx_http = tx.clone();
    let pi_http = pending_input.clone();
    let hwnd_http = claude_hwnd.clone();

    // Spawn HTTP server on :9527
    tokio::spawn(async move {
        let app = http::create_router(sm_http, tx_http, pi_http, hwnd_http);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9527").await.unwrap();
        println!("HTTP server listening on http://127.0.0.1:9527");
        axum::serve(listener, app).await.unwrap();
    });

    let sm_mcp = state_manager.clone();
    let tx_mcp = tx.clone();
    let pi_mcp = pending_input.clone();

    // Spawn MCP server on :9528
    tokio::spawn(async move {
        let app = mcp::create_mcp_router(sm_mcp, tx_mcp, pi_mcp);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9528").await.unwrap();
        println!("MCP server listening on http://127.0.0.1:9528");
        axum::serve(listener, app).await.unwrap();
    });

    // Build Tauri app
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_drag, hide_window, exit_app, open_webui, check_autostart, toggle_autostart])
        .setup(move |app| {
            // Listen to broadcast channel, forward state changes to frontend
            let handle = app.handle().clone();
            let mut rx = tx.subscribe();
            let handle2 = handle.clone();
            tokio::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    let _ = handle2.emit("state-change", event);
                }
            });

            // Send initial waving state
            let _ = handle.emit(
                "state-change",
                StateChangeEvent {
                    animation: "waving".to_string(),
                    bubble: "爱弥斯已上线~".to_string(),
                    core_signal: "idle".to_string(),
                    tool_label: None,
                    overlay: None,
                    input_type: None,
                    options: None,
                },
            );

            // Enable system tray
            if let Err(e) = tray::setup(app) {
                eprintln!("Failed to setup tray: {}", e);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Aemeath Pet");
}

extern "system" {
    fn GetForegroundWindow() -> isize;
}

#[tauri::command]
fn start_drag(window: tauri::Window) {
    let _ = window.start_dragging();
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn check_autostart() -> bool {
    let startup = std::env::var("APPDATA").unwrap_or_default()
        .replace('\\', "/") + "/Microsoft/Windows/Start Menu/Programs/Startup/Aemeath.lnk";
    std::path::Path::new(&startup).exists()
}

#[tauri::command]
fn toggle_autostart() -> bool {
    let startup_dir = std::env::var("APPDATA").unwrap_or_default()
        .replace('\\', "/") + "/Microsoft/Windows/Start Menu/Programs/Startup";
    let link_path = format!("{}/Aemeath.lnk", startup_dir);
    if std::path::Path::new(&link_path).exists() {
        std::fs::remove_file(&link_path).ok();
        return false;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    std::fs::create_dir_all(&startup_dir).ok();
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("$ws=New-Object -ComObject WScript.Shell;$s=$ws.CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Save()", &link_path, exe.to_string_lossy(), std::env::current_dir().unwrap_or_default().to_string_lossy())])
        .output();
    true
}

#[tauri::command]
fn open_webui() {
    std::thread::spawn(|| {
        webui::launch_webui();
    });
}
