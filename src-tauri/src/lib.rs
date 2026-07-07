use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, PhysicalPosition,
};
use uuid::Uuid;

const UNGROUPED_ID: &str = "group-ungrouped";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutRecord {
    pub id: String,
    pub name: String,
    pub original_shortcut_path: String,
    pub target_path: String,
    pub arguments: String,
    pub working_directory: String,
    pub icon_path: String,
    pub icon_index: i32,
    #[serde(default)]
    pub icon_data_url: String,
    pub group_id: String,
    pub is_favorite: bool,
    pub sort_order: i32,
    pub remark: String,
    pub description: String,
    pub hotkey: String,
    pub show_command: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutInfo {
    pub name: String,
    pub original_shortcut_path: String,
    pub target_path: String,
    pub arguments: String,
    pub working_directory: String,
    pub icon_path: String,
    pub icon_index: i32,
    #[serde(default)]
    pub icon_data_url: String,
    pub description: String,
    pub hotkey: String,
    pub show_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRecord {
    pub id: String,
    pub name: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub auto_delete_desktop_shortcut: bool,
    pub auto_hide_when_docked: bool,
    pub always_on_top: bool,
    pub launch_at_startup: bool,
    pub hide_after_launch: bool,
    pub show_group_bar: bool,
    pub dock_position: String,
    pub is_pinned: bool,
    pub window_width: i32,
    pub window_height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateData {
    pub shortcuts: Vec<ShortcutRecord>,
    pub groups: Vec<GroupRecord>,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub deleted: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub launched: bool,
    pub warning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockResult {
    pub docked: bool,
    pub position: String,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            save_app_state,
            parse_shortcut,
            extract_icon_data,
            delete_shortcut_file,
            launch_shortcut,
            open_target_folder,
            minimize_window,
            hide_window_to_tray,
            show_main_window,
            exit_app,
            set_window_always_on_top,
            set_launch_at_startup,
            snap_window_if_near_edge,
            hide_docked_window,
            show_docked_window
        ])
        .run(tauri::generate_context!())
        .expect("failed to run DeskShortcut");
}

fn setup_tray(app: &mut App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "隐藏到托盘", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &hide_item, &quit_item])?;

    let mut tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("DeskShortcut")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                let _ = show_window(app);
            }
            "hide" => {
                let _ = hide_window(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

#[tauri::command]
fn get_app_state(app: AppHandle) -> Result<AppStateData, String> {
    let path = state_file_path(&app)?;
    if !path.exists() {
        let state = default_state();
        write_state(&path, &state)?;
        return Ok(state);
    }

    let content = fs::read_to_string(&path).map_err(|error| format!("读取配置失败：{error}"))?;
    serde_json::from_str::<AppStateData>(&content).map_err(|error| format!("配置文件格式错误：{error}"))
}

#[tauri::command]
fn save_app_state(app: AppHandle, state: AppStateData) -> Result<(), String> {
    let path = state_file_path(&app)?;
    write_state(&path, &state)
}

#[tauri::command]
fn parse_shortcut(path: String) -> Result<ShortcutInfo, String> {
    validate_lnk_path(&path)?;
    platform::parse_shortcut(Path::new(&path))
}

#[tauri::command]
fn extract_icon_data(icon_path: String, target_path: String, icon_index: i32) -> Result<String, String> {
    platform::extract_icon_data(&icon_path, &target_path, icon_index)
}

#[tauri::command]
fn delete_shortcut_file(path: String, expected_path: String) -> Result<DeleteResult, String> {
    validate_lnk_path(&path)?;
    validate_lnk_path(&expected_path)?;

    let path_buf = PathBuf::from(&path);
    let expected_buf = PathBuf::from(&expected_path);
    if path_buf != expected_buf {
        return Err("删除路径与导入时记录的原快捷方式路径不一致".to_string());
    }

    if !path_buf.exists() {
        return Ok(DeleteResult {
            deleted: false,
            message: "原快捷方式已不存在".to_string(),
        });
    }
    if path_buf.is_dir() {
        return Err("不允许删除目录".to_string());
    }

    match fs::remove_file(&path_buf) {
        Ok(()) => Ok(DeleteResult {
            deleted: true,
            message: "已删除桌面快捷方式".to_string(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Ok(DeleteResult {
            deleted: false,
            message: "权限不足，无法删除快捷方式".to_string(),
        }),
        Err(error) => Ok(DeleteResult {
            deleted: false,
            message: format!("删除失败，请稍后手动删除：{error}"),
        }),
    }
}

#[tauri::command]
fn launch_shortcut(app: AppHandle, shortcut_id: String) -> Result<LaunchResult, String> {
    let state = get_app_state(app.clone())?;
    let shortcut = state
        .shortcuts
        .iter()
        .find(|item| item.id == shortcut_id)
        .ok_or_else(|| "快捷方式记录不存在".to_string())?;

    platform::launch_shortcut(shortcut)
}

#[tauri::command]
fn open_target_folder(app: AppHandle, shortcut_id: String) -> Result<(), String> {
    let state = get_app_state(app)?;
    let shortcut = state
        .shortcuts
        .iter()
        .find(|item| item.id == shortcut_id)
        .ok_or_else(|| "快捷方式记录不存在".to_string())?;

    platform::open_target_folder(&shortcut.target_path)
}

#[tauri::command]
fn minimize_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    window
        .minimize()
        .map_err(|error| format!("最小化窗口失败：{error}"))
}

#[tauri::command]
fn hide_window_to_tray(app: AppHandle) -> Result<(), String> {
    hide_window(&app)
}

#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_window(&app)
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn set_window_always_on_top(app: AppHandle, enabled: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    window
        .set_always_on_top(enabled)
        .map_err(|error| format!("设置置顶失败：{error}"))
}

#[tauri::command]
fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
    platform::set_launch_at_startup(enabled)
}

#[tauri::command]
fn snap_window_if_near_edge(app: AppHandle) -> Result<DockResult, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| format!("读取显示器失败：{error}"))?
        .ok_or_else(|| "未找到当前显示器".to_string())?;
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let position = window
        .outer_position()
        .map_err(|error| format!("读取窗口位置失败：{error}"))?;
    let size = window
        .outer_size()
        .map_err(|error| format!("读取窗口尺寸失败：{error}"))?;

    let threshold = 24;
    let left_distance = (position.x - monitor_pos.x).abs();
    let top_distance = (position.y - monitor_pos.y).abs();
    let right_edge = monitor_pos.x + monitor_size.width as i32;
    let right_distance = (right_edge - (position.x + size.width as i32)).abs();

    if top_distance <= threshold {
        let x = position
            .x
            .clamp(monitor_pos.x, right_edge - size.width as i32);
        window
            .set_position(PhysicalPosition::new(x, monitor_pos.y))
            .map_err(|error| format!("窗口吸附失败：{error}"))?;
        return Ok(DockResult {
            docked: true,
            position: "top".to_string(),
        });
    }

    if left_distance <= threshold {
        window
            .set_position(PhysicalPosition::new(monitor_pos.x, position.y.max(monitor_pos.y)))
            .map_err(|error| format!("窗口吸附失败：{error}"))?;
        return Ok(DockResult {
            docked: true,
            position: "left".to_string(),
        });
    }

    if right_distance <= threshold {
        window
            .set_position(PhysicalPosition::new(
                right_edge - size.width as i32,
                position.y.max(monitor_pos.y),
            ))
            .map_err(|error| format!("窗口吸附失败：{error}"))?;
        return Ok(DockResult {
            docked: true,
            position: "right".to_string(),
        });
    }

    Ok(DockResult {
        docked: false,
        position: "none".to_string(),
    })
}

#[tauri::command]
fn hide_docked_window(app: AppHandle, position: String) -> Result<(), String> {
    move_docked_window(app.clone(), &position, true)?;
    restore_configured_always_on_top(&app)
}

#[tauri::command]
fn show_docked_window(app: AppHandle, position: String) -> Result<(), String> {
    move_docked_window(app.clone(), &position, false)?;
    raise_window_for_dock_reveal(&app)
}

fn move_docked_window(app: AppHandle, position: &str, hide: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| format!("读取显示器失败：{error}"))?
        .ok_or_else(|| "未找到当前显示器".to_string())?;
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let size = window
        .outer_size()
        .map_err(|error| format!("读取窗口尺寸失败：{error}"))?;
    let current = window
        .outer_position()
        .map_err(|error| format!("读取窗口位置失败：{error}"))?;
    let trigger = 4;

    let next = match (position, hide) {
        ("top", true) => PhysicalPosition::new(current.x, monitor_pos.y - size.height as i32 + trigger),
        ("top", false) => PhysicalPosition::new(current.x, monitor_pos.y),
        ("left", true) => PhysicalPosition::new(monitor_pos.x - size.width as i32 + trigger, current.y),
        ("left", false) => PhysicalPosition::new(monitor_pos.x, current.y),
        ("right", true) => PhysicalPosition::new(
            monitor_pos.x + monitor_size.width as i32 - trigger,
            current.y,
        ),
        ("right", false) => PhysicalPosition::new(
            monitor_pos.x + monitor_size.width as i32 - size.width as i32,
            current.y,
        ),
        _ => return Ok(()),
    };

    window
        .set_position(next)
        .map_err(|error| format!("移动窗口失败：{error}"))
}

fn raise_window_for_dock_reveal(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    window
        .unminimize()
        .map_err(|error| format!("恢复窗口失败：{error}"))?;
    window
        .show()
        .map_err(|error| format!("显示窗口失败：{error}"))?;
    window
        .set_always_on_top(true)
        .map_err(|error| format!("提升窗口失败：{error}"))?;
    let _ = window.set_focus();
    Ok(())
}

fn restore_configured_always_on_top(app: &AppHandle) -> Result<(), String> {
    let state = get_app_state(app.clone())?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    window
        .set_always_on_top(state.settings.always_on_top)
        .map_err(|error| format!("恢复置顶设置失败：{error}"))
}

fn show_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    window
        .unminimize()
        .map_err(|error| format!("恢复窗口失败：{error}"))?;
    window
        .show()
        .map_err(|error| format!("显示窗口失败：{error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("激活窗口失败：{error}"))
}

fn hide_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    window
        .hide()
        .map_err(|error| format!("隐藏到托盘失败：{error}"))
}

fn state_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败：{error}"))?;
    fs::create_dir_all(&dir).map_err(|error| format!("创建应用数据目录失败：{error}"))?;
    Ok(dir.join("deskshortcut.json"))
}

fn write_state(path: &Path, state: &AppStateData) -> Result<(), String> {
    let content = serde_json::to_string_pretty(state).map_err(|error| format!("序列化配置失败：{error}"))?;
    fs::write(path, content).map_err(|error| format!("保存配置失败：{error}"))
}

fn validate_lnk_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    let is_lnk = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("lnk"))
        .unwrap_or(false);
    if is_lnk {
        Ok(())
    } else {
        Err("当前仅支持 Windows 快捷方式（.lnk）".to_string())
    }
}

fn default_state() -> AppStateData {
    AppStateData {
        shortcuts: Vec::new(),
        groups: vec![
            group_record("group-common", "常用", 10),
            group_record(UNGROUPED_ID, "未分组", 20),
        ],
        settings: Settings {
            auto_delete_desktop_shortcut: true,
            auto_hide_when_docked: true,
            always_on_top: false,
            launch_at_startup: false,
            hide_after_launch: false,
            show_group_bar: true,
            dock_position: "none".to_string(),
            is_pinned: false,
            window_width: 420,
            window_height: 640,
        },
    }
}

fn group_record(id: &str, name: &str, sort_order: i32) -> GroupRecord {
    let now = now();
    GroupRecord {
        id: id.to_string(),
        name: name.to_string(),
        sort_order,
        created_at: now.clone(),
        updated_at: now,
    }
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

#[allow(dead_code)]
fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

#[cfg(windows)]
mod platform {
    use super::{LaunchResult, ShortcutInfo, ShortcutRecord};
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
    use std::{
        ffi::{c_void, OsStr},
        mem,
        os::windows::{ffi::OsStrExt, process::CommandExt},
        path::Path,
        process::Command,
        ptr,
    };
    use windows::{
        core::{Interface, PCWSTR},
        Win32::{
            Foundation::{HANDLE, MAX_PATH},
            Graphics::Gdi::{
                CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetObjectW,
                SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HBRUSH, HDC,
                HGDIOBJ,
            },
            Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
            },
            UI::{
                Shell::{
                    ExtractIconExW, IShellLinkW, SHGetFileInfoW, ShellExecuteW, ShellLink,
                    SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_SMALLICON, SLGP_RAWPATH,
                },
                WindowsAndMessaging::{
                    DestroyIcon, DrawIconEx, GetIconInfo, DI_NORMAL, HICON, SW_SHOWNORMAL,
                },
            },
        },
    };

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    pub fn parse_shortcut(path: &Path) -> Result<ShortcutInfo, String> {
        if !path.exists() {
            return Err("原快捷方式已不存在".to_string());
        }

        let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
        let result = parse_shortcut_inner(path);
        if com_initialized {
            unsafe { CoUninitialize() };
        }
        result
    }

    fn parse_shortcut_inner(path: &Path) -> Result<ShortcutInfo, String> {
        let shell_link: IShellLinkW = unsafe {
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("初始化快捷方式解析失败：{error}"))?
        };
        let persist_file: IPersistFile = shell_link
            .cast()
            .map_err(|error| format!("读取快捷方式接口失败：{error}"))?;
        let wide_path = wide(path.as_os_str());
        unsafe {
            persist_file
                .Load(PCWSTR(wide_path.as_ptr()), STGM_READ)
                .map_err(|error| format!("加载快捷方式失败：{error}"))?;
        }

        let mut target = [0u16; 32768];
        let mut arguments = [0u16; 4096];
        let mut working_directory = [0u16; MAX_PATH as usize];
        let mut icon_path = [0u16; MAX_PATH as usize];
        let mut description = [0u16; 1024];
        let mut icon_index = 0;
        let mut show_command = SW_SHOWNORMAL.0;
        let mut hotkey = 0;

        unsafe {
            shell_link
                .GetPath(&mut target, ptr::null_mut(), SLGP_RAWPATH.0 as u32)
                .map_err(|error| format!("读取目标路径失败：{error}"))?;
            shell_link
                .GetArguments(&mut arguments)
                .map_err(|error| format!("读取启动参数失败：{error}"))?;
            shell_link
                .GetWorkingDirectory(&mut working_directory)
                .map_err(|error| format!("读取工作目录失败：{error}"))?;
            shell_link
                .GetIconLocation(&mut icon_path, &mut icon_index)
                .map_err(|error| format!("读取图标失败：{error}"))?;
            let _ = shell_link.GetDescription(&mut description);
            if let Ok(value) = shell_link.GetShowCmd() {
                show_command = value.0;
            }
            if let Ok(value) = shell_link.GetHotkey() {
                hotkey = value;
            }
        }

        let icon = from_wide(&icon_path);
        let target_path = from_wide(&target);
        let resolved_icon_path = if icon.is_empty() {
            target_path.clone()
        } else {
            icon
        };
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("未命名快捷方式")
            .to_string();
        let icon_data_url =
            extract_icon_data(&resolved_icon_path, &target_path, icon_index).unwrap_or_default();

        Ok(ShortcutInfo {
            name,
            original_shortcut_path: path.to_string_lossy().to_string(),
            target_path,
            arguments: from_wide(&arguments),
            working_directory: from_wide(&working_directory),
            icon_path: resolved_icon_path,
            icon_index,
            icon_data_url,
            description: from_wide(&description),
            hotkey: hotkey_to_string(hotkey),
            show_command: show_command_to_string(show_command),
        })
    }

    pub fn extract_icon_data(
        icon_path: &str,
        target_path: &str,
        icon_index: i32,
    ) -> Result<String, String> {
        for (source, index) in icon_candidates(icon_path, target_path, icon_index) {
            if let Ok(icon) =
                extract_resource_icon(&source, index).or_else(|_| shell_file_icon(&source, true))
            {
                if let Ok(data_url) = hicon_to_png_data_url(icon.0) {
                    return Ok(data_url);
                }
            }
            if let Ok(icon) = shell_file_icon(&source, false) {
                if let Ok(data_url) = hicon_to_png_data_url(icon.0) {
                    return Ok(data_url);
                }
            }
        }

        Err("提取图标失败，可能是图标资源不存在或不可读取".to_string())
    }

    struct OwnedIcon(HICON);

    impl Drop for OwnedIcon {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = DestroyIcon(self.0);
                }
            }
        }
    }

    fn icon_candidates(icon_path: &str, target_path: &str, icon_index: i32) -> Vec<(String, i32)> {
        let mut candidates = Vec::new();
        push_icon_candidate(&mut candidates, icon_path, icon_index);
        push_icon_candidate(&mut candidates, target_path, 0);
        candidates
    }

    fn push_icon_candidate(candidates: &mut Vec<(String, i32)>, value: &str, fallback_index: i32) {
        if let Some((path, index)) = normalize_icon_source(value, fallback_index) {
            if !candidates.iter().any(|(existing, existing_index)| {
                existing.eq_ignore_ascii_case(&path) && *existing_index == index
            }) {
                candidates.push((path, index));
            }
        }
    }

    fn normalize_icon_source(value: &str, fallback_index: i32) -> Option<(String, i32)> {
        let trimmed = value.trim().trim_matches('"');
        if trimmed.is_empty() {
            return None;
        }

        let expanded = expand_windows_env_vars(trimmed);
        let path = Path::new(&expanded);
        if path.is_file() {
            return Some((expanded, fallback_index));
        }

        if let Some((path_part, parsed_index)) = split_icon_location(&expanded) {
            if Path::new(&path_part).is_file() {
                return Some((path_part, parsed_index));
            }
        }

        None
    }

    fn split_icon_location(value: &str) -> Option<(String, i32)> {
        let (path, index) = value.rsplit_once(',')?;
        let parsed_index = index.trim().parse::<i32>().ok()?;
        let clean_path = path.trim().trim_matches('"').to_string();
        if clean_path.is_empty() {
            None
        } else {
            Some((clean_path, parsed_index))
        }
    }

    fn expand_windows_env_vars(value: &str) -> String {
        let mut output = String::with_capacity(value.len());
        let mut rest = value;

        while let Some(start) = rest.find('%') {
            output.push_str(&rest[..start]);
            let after_start = &rest[start + 1..];
            if let Some(end) = after_start.find('%') {
                let name = &after_start[..end];
                if name.is_empty() {
                    output.push_str("%%");
                } else if let Ok(replacement) = std::env::var(name) {
                    output.push_str(&replacement);
                } else {
                    output.push('%');
                    output.push_str(name);
                    output.push('%');
                }
                rest = &after_start[end + 1..];
            } else {
                output.push('%');
                rest = after_start;
            }
        }

        output.push_str(rest);
        output
    }

    fn extract_resource_icon(source: &str, index: i32) -> Result<OwnedIcon, String> {
        let source_wide = wide_null(source);
        let mut large = [HICON::default(); 1];
        let mut small = [HICON::default(); 1];

        let extracted = unsafe {
            ExtractIconExW(
                PCWSTR(source_wide.as_ptr()),
                index,
                Some(large.as_mut_ptr()),
                Some(small.as_mut_ptr()),
                1,
            )
        };

        if extracted == 0 {
            return Err("图标资源不存在".to_string());
        }

        let icon = if !large[0].is_invalid() {
            large[0]
        } else {
            small[0]
        };
        let unused = if !large[0].is_invalid() {
            small[0]
        } else {
            large[0]
        };
        if !unused.is_invalid() {
            unsafe {
                let _ = DestroyIcon(unused);
            }
        }

        if icon.is_invalid() {
            Err("图标资源为空".to_string())
        } else {
            Ok(OwnedIcon(icon))
        }
    }

    fn shell_file_icon(source: &str, large: bool) -> Result<OwnedIcon, String> {
        let source_wide = wide_null(source);
        let mut file_info = SHFILEINFOW::default();
        let size = mem::size_of::<SHFILEINFOW>() as u32;
        let flags = if large {
            SHGFI_ICON | SHGFI_LARGEICON
        } else {
            SHGFI_ICON | SHGFI_SMALLICON
        };

        let result = unsafe {
            SHGetFileInfoW(
                PCWSTR(source_wide.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut file_info),
                size,
                flags,
            )
        };

        if result == 0 || file_info.hIcon.is_invalid() {
            Err("Shell 未返回文件图标".to_string())
        } else {
            Ok(OwnedIcon(file_info.hIcon))
        }
    }

    fn hicon_to_png_data_url(icon: HICON) -> Result<String, String> {
        let (width, height) = icon_size(icon).unwrap_or((32, 32));
        let mut rgba = draw_icon_to_rgba(icon, width, height)?;

        let mut any_alpha = false;
        let mut any_visible_rgb = false;
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            any_alpha |= pixel[3] != 0;
            any_visible_rgb |= pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0;
        }

        if !any_alpha && any_visible_rgb {
            for pixel in rgba.chunks_exact_mut(4) {
                pixel[3] = if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
                    255
                } else {
                    0
                };
            }
        }

        let mut png = Vec::new();
        let encoder = PngEncoder::new(&mut png);
        encoder
            .write_image(&rgba, width as u32, height as u32, ColorType::Rgba8.into())
            .map_err(|error| format!("编码图标 PNG 失败：{error}"))?;

        Ok(format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(png)
        ))
    }

    fn icon_size(icon: HICON) -> Option<(i32, i32)> {
        let mut icon_info = unsafe { mem::zeroed() };
        if unsafe { GetIconInfo(icon, &mut icon_info) }.is_err() {
            return None;
        }

        let bitmap = if !icon_info.hbmColor.is_invalid() {
            icon_info.hbmColor
        } else {
            icon_info.hbmMask
        };

        let mut info: BITMAP = unsafe { mem::zeroed() };
        let result = if !bitmap.is_invalid() {
            unsafe {
                GetObjectW(
                    HGDIOBJ(bitmap.0),
                    mem::size_of::<BITMAP>() as i32,
                    Some(&mut info as *mut _ as *mut c_void),
                )
            }
        } else {
            0
        };

        let used_color_bitmap = !icon_info.hbmColor.is_invalid();

        if !icon_info.hbmColor.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(icon_info.hbmColor.0));
            }
        }
        if !icon_info.hbmMask.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(icon_info.hbmMask.0));
            }
        }

        if result == 0 || info.bmWidth <= 0 || info.bmHeight <= 0 {
            return None;
        }

        let height = if used_color_bitmap {
            info.bmHeight
        } else {
            info.bmHeight / 2
        };

        Some((info.bmWidth.clamp(16, 256), height.clamp(16, 256)))
    }

    fn draw_icon_to_rgba(icon: HICON, width: i32, height: i32) -> Result<Vec<u8>, String> {
        let hdc = unsafe { CreateCompatibleDC(HDC::default()) };
        if hdc.is_invalid() {
            return Err("创建图标绘制上下文失败".to_string());
        }

        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                ..Default::default()
            },
            ..Default::default()
        };

        let bitmap = match unsafe {
            CreateDIBSection(
                hdc,
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits,
                HANDLE::default(),
                0,
            )
        } {
            Ok(bitmap) if !bits.is_null() => bitmap,
            Ok(_) => {
                unsafe {
                    let _ = DeleteDC(hdc);
                }
                return Err("创建图标位图失败".to_string());
            }
            Err(error) => {
                unsafe {
                    let _ = DeleteDC(hdc);
                }
                return Err(format!("创建图标位图失败：{error}"));
            }
        };

        let old_object = unsafe { SelectObject(hdc, HGDIOBJ(bitmap.0)) };
        let drawn = unsafe {
            DrawIconEx(
                hdc,
                0,
                0,
                icon,
                width,
                height,
                0,
                HBRUSH::default(),
                DI_NORMAL,
            )
        }
        .is_ok();
        let byte_len = width as usize * height as usize * 4;
        let rgba = if drawn {
            unsafe { std::slice::from_raw_parts(bits as *const u8, byte_len) }.to_vec()
        } else {
            Vec::new()
        };

        unsafe {
            if !old_object.is_invalid() {
                let _ = SelectObject(hdc, old_object);
            }
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(hdc);
        }

        if drawn {
            Ok(rgba)
        } else {
            Err("绘制图标失败".to_string())
        }
    }

    pub fn launch_shortcut(shortcut: &ShortcutRecord) -> Result<LaunchResult, String> {
        let target = Path::new(&shortcut.target_path);
        if !target.exists() {
            return Err("目标程序不存在，可能已被移动或卸载".to_string());
        }

        let mut warning = String::new();
        let working_directory = if shortcut.working_directory.trim().is_empty() {
            None
        } else if Path::new(&shortcut.working_directory).exists() {
            Some(shortcut.working_directory.as_str())
        } else {
            warning = "工作目录不存在，已尝试不带工作目录启动".to_string();
            None
        };

        let operation = wide_null("open");
        let target_wide = wide_null(&shortcut.target_path);
        let args_wide = wide_null(&shortcut.arguments);
        let dir_wide = working_directory.map(wide_null);
        let dir_ptr = dir_wide
            .as_ref()
            .map(|value| PCWSTR(value.as_ptr()))
            .unwrap_or(PCWSTR::null());

        let instance = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(operation.as_ptr()),
                PCWSTR(target_wide.as_ptr()),
                PCWSTR(args_wide.as_ptr()),
                dir_ptr,
                SW_SHOWNORMAL,
            )
        };
        if instance.0 as isize <= 32 {
            return Err("启动失败，可能需要管理员权限或启动参数有误".to_string());
        }

        Ok(LaunchResult {
            launched: true,
            warning,
        })
    }

    pub fn open_target_folder(target_path: &str) -> Result<(), String> {
        let argument = format!("/select,\"{target_path}\"");
        Command::new("explorer.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .arg(argument)
            .spawn()
            .map_err(|error| format!("打开目标所在位置失败：{error}"))?;
        Ok(())
    }

    pub fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|error| format!("读取当前程序路径失败：{error}"))?;
        let run_key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let status = if enabled {
            Command::new("reg.exe")
                .creation_flags(CREATE_NO_WINDOW)
                .args(["add", run_key, "/v", "DeskShortcut", "/t", "REG_SZ", "/d"])
                .arg(exe)
                .arg("/f")
                .status()
                .map_err(|error| format!("设置开机自启动失败：{error}"))?
        } else {
            Command::new("reg.exe")
                .creation_flags(CREATE_NO_WINDOW)
                .args(["delete", run_key, "/v", "DeskShortcut", "/f"])
                .status()
                .map_err(|error| format!("关闭开机自启动失败：{error}"))?
        };

        if status.success() {
            Ok(())
        } else if !enabled {
            Ok(())
        } else {
            Err("设置开机自启动失败，可能没有注册表写入权限".to_string())
        }
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    fn wide_null(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn from_wide(value: &[u16]) -> String {
        let len = value.iter().position(|item| *item == 0).unwrap_or(value.len());
        String::from_utf16_lossy(&value[..len])
    }

    fn hotkey_to_string(hotkey: u16) -> String {
        if hotkey == 0 {
            String::new()
        } else {
            format!("{hotkey}")
        }
    }

    fn show_command_to_string(show_command: i32) -> String {
        match show_command {
            3 => "maximized".to_string(),
            7 => "minimized".to_string(),
            _ => "normal".to_string(),
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{LaunchResult, ShortcutInfo, ShortcutRecord};
    use std::path::Path;

    pub fn parse_shortcut(_path: &Path) -> Result<ShortcutInfo, String> {
        Err("DeskShortcut 只能在 Windows 上解析 .lnk 快捷方式".to_string())
    }

    pub fn extract_icon_data(_icon_path: &str, _target_path: &str, _icon_index: i32) -> Result<String, String> {
        Err("DeskShortcut 只能在 Windows 上提取快捷方式图标".to_string())
    }

    pub fn launch_shortcut(_shortcut: &ShortcutRecord) -> Result<LaunchResult, String> {
        Err("DeskShortcut 只能在 Windows 上启动 Windows 快捷方式".to_string())
    }

    pub fn open_target_folder(_target_path: &str) -> Result<(), String> {
        Err("DeskShortcut 的打开目标所在位置功能仅支持 Windows".to_string())
    }

    pub fn set_launch_at_startup(_enabled: bool) -> Result<(), String> {
        Err("开机自启动仅支持 Windows".to_string())
    }
}
