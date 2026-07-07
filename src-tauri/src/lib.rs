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
    move_docked_window(app, &position, true)
}

#[tauri::command]
fn show_docked_window(app: AppHandle, position: String) -> Result<(), String> {
    move_docked_window(app, &position, false)
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
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path, ptr};
    use windows::{
        core::{Interface, PCWSTR},
        Win32::{
            Foundation::MAX_PATH,
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
                COINIT_APARTMENTTHREADED, STGM_READ,
            },
            UI::{
                Shell::{IShellLinkW, ShellExecuteW, ShellLink, SLGP_RAWPATH},
                WindowsAndMessaging::SW_SHOWNORMAL,
            },
        },
    };

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

        let target_path = from_wide(&target);
        let icon = from_wide(&icon_path);
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("未命名快捷方式")
            .to_string();

        Ok(ShortcutInfo {
            name,
            original_shortcut_path: path.to_string_lossy().to_string(),
            target_path,
            arguments: from_wide(&arguments),
            working_directory: from_wide(&working_directory),
            icon_path: if icon.is_empty() {
                from_wide(&target)
            } else {
                icon
            },
            icon_index,
            description: from_wide(&description),
            hotkey: hotkey_to_string(hotkey),
            show_command: show_command_to_string(show_command),
        })
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
        std::process::Command::new("explorer.exe")
            .arg(argument)
            .spawn()
            .map_err(|error| format!("打开目标所在位置失败：{error}"))?;
        Ok(())
    }

    pub fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|error| format!("读取当前程序路径失败：{error}"))?;
        let run_key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let status = if enabled {
            std::process::Command::new("reg.exe")
                .args(["add", run_key, "/v", "DeskShortcut", "/t", "REG_SZ", "/d"])
                .arg(exe)
                .arg("/f")
                .status()
                .map_err(|error| format!("设置开机自启动失败：{error}"))?
        } else {
            std::process::Command::new("reg.exe")
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
