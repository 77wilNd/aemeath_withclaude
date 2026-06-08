use crate::webui;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "显示/隐藏", true, None::<&str>)?;
    let scale_1x = MenuItem::with_id(app, "scale_1x", "缩放 1x", true, None::<&str>)?;
    let scale_1_5x = MenuItem::with_id(app, "scale_1_5x", "缩放 1.5x", true, None::<&str>)?;
    let scale_2x = MenuItem::with_id(app, "scale_2x", "缩放 2x", true, None::<&str>)?;
    let webui_item = MenuItem::with_id(app, "webui", "📊 对话管理", true, None::<&str>)?;
    let about_item = MenuItem::with_id(app, "about", "关于", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &scale_1x,
            &scale_1_5x,
            &scale_2x,
            &webui_item,
            &about_item,
            &quit_item,
        ],
    )?;

    let _tray = TrayIconBuilder::with_id("aemeath-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("aemeath") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                "webui" => {
                    std::thread::spawn(|| {
                        webui::launch_webui();
                    });
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("aemeath") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
