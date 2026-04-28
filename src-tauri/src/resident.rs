use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, Window, WindowEvent,
};

use crate::app_state::AppState;

const MAIN_WINDOW_LABEL: &str = "main";
const MENU_SHOW: &str = "show";
const MENU_START: &str = "start_recording";
const MENU_STOP: &str = "stop_recording";
const MENU_QUIT: &str = "quit";

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "显示 MindBack", true, None::<&str>)?;
    let start = MenuItem::with_id(app, MENU_START, "开始记录", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, MENU_STOP, "停止记录", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出 MindBack", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &start, &stop, &separator, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("MindBack")
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone()).icon_as_template(true);
    }

    tray.build(app)?;
    Ok(())
}

pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let _ = window.hide();
    }
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW => show_main_window(app),
        MENU_START => app.state::<AppState>().start_recording_worker(),
        MENU_STOP => app.state::<AppState>().stop_recording_worker(),
        MENU_QUIT => app.exit(0),
        _ => {}
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
