use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition};

const DEFAULT_PET_JSON: &str = include_str!("../resources/pets/photoboy/pet.json");
const DEFAULT_SPRITESHEET: &[u8] = include_bytes!("../resources/pets/photoboy/spritesheet.webp");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PetJson {
    id: String,
    display_name: String,
    description: String,
}

#[derive(Debug, Serialize)]
struct PetAsset {
    id: String,
    display_name: String,
    description: String,
    image_data_url: String,
}

#[derive(Debug, Serialize)]
struct WindowPosition {
    x: i32,
    y: i32,
}

#[tauri::command]
fn load_default_pet() -> Result<PetAsset, String> {
    let pet: PetJson = serde_json::from_str(DEFAULT_PET_JSON)
        .map_err(|error| format!("Failed to parse bundled pet metadata: {error}"))?;
    let encoded = general_purpose::STANDARD.encode(DEFAULT_SPRITESHEET);

    Ok(PetAsset {
        id: pet.id,
        display_name: pet.display_name,
        description: pet.description,
        image_data_url: format!("data:image/webp;base64,{encoded}"),
    })
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn pet_window_position(app: tauri::AppHandle) -> Result<WindowPosition, String> {
    let window = app
        .get_webview_window("pet")
        .ok_or_else(|| "Pet window not found".to_string())?;
    let position = window
        .outer_position()
        .map_err(|error| format!("Failed to read pet window position: {error}"))?;

    Ok(WindowPosition {
        x: position.x,
        y: position.y,
    })
}

#[tauri::command]
fn move_pet_window(app: tauri::AppHandle, x: i32, y: i32) -> Result<(), String> {
    let window = app
        .get_webview_window("pet")
        .ok_or_else(|| "Pet window not found".to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| format!("Failed to move pet window: {error}"))
}

#[cfg(target_os = "macos")]
mod keyboard_activity {
    use std::{
        ffi::c_void,
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            OnceLock,
        },
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use macos_accessibility_client::accessibility::{
        application_is_trusted, application_is_trusted_with_prompt,
    };
    use tauri::Emitter;

    static STARTED: AtomicBool = AtomicBool::new(false);
    static TAP_CONTEXT: OnceLock<&'static KeyboardTapContext> = OnceLock::new();

    const PERMISSION_RETRY_INTERVAL: Duration = Duration::from_secs(2);
    const EVENT_EMIT_INTERVAL_MS: u64 = 50;
    const K_CG_SESSION_EVENT_TAP: u32 = 1;
    const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
    const K_CG_EVENT_KEY_DOWN: u32 = 10;
    const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
    const K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
    const KEY_DOWN_EVENT_MASK: u64 = 1_u64 << K_CG_EVENT_KEY_DOWN;

    struct KeyboardTapContext {
        app: tauri::AppHandle,
        last_emit_ms: AtomicU64,
    }

    type CGEventTapCallBack =
        unsafe extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> *mut c_void;
        fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFRunLoopCommonModes: *const c_void;

        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: *mut c_void,
            order: isize,
        ) -> *mut c_void;
        fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
        fn CFRunLoopGetCurrent() -> *mut c_void;
        fn CFRunLoopRun();
    }

    pub fn start(app: tauri::AppHandle) {
        if STARTED.swap(true, Ordering::Relaxed) {
            return;
        }

        thread::spawn(move || {
            request_permissions_if_needed();
            while !keyboard_permissions_available() {
                thread::sleep(PERMISSION_RETRY_INTERVAL);
            }

            let _ = start_event_tap(app);
        });
    }

    fn request_permissions_if_needed() {
        if !application_is_trusted() {
            let _ = application_is_trusted_with_prompt();
        }
    }

    fn keyboard_permissions_available() -> bool {
        application_is_trusted()
    }

    fn start_event_tap(app: tauri::AppHandle) -> bool {
        let context = Box::leak(Box::new(KeyboardTapContext {
            app,
            last_emit_ms: AtomicU64::new(0),
        }));
        let _ = TAP_CONTEXT.set(context);

        let tap = unsafe {
            CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
                KEY_DOWN_EVENT_MASK,
                keyboard_event_callback,
                context as *const KeyboardTapContext as *mut c_void,
            )
        };

        if tap.is_null() {
            return false;
        }

        let source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
        if source.is_null() {
            return false;
        }

        unsafe {
            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
        }

        unsafe { CFRunLoopRun() };
        true
    }

    unsafe extern "C" fn keyboard_event_callback(
        _proxy: *mut c_void,
        event_type: u32,
        event: *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void {
        if event_type == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT
            || event_type == K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT
        {
            return event;
        }

        if event_type == K_CG_EVENT_KEY_DOWN && !user_info.is_null() {
            let context = &*(user_info as *const KeyboardTapContext);
            emit_keyboard_activity(context);
        }

        event
    }

    fn emit_keyboard_activity(context: &KeyboardTapContext) {
        let now_ms = current_time_ms();
        let previous = context.last_emit_ms.load(Ordering::Relaxed);
        if now_ms.saturating_sub(previous) < EVENT_EMIT_INTERVAL_MS {
            return;
        }

        context.last_emit_ms.store(now_ms, Ordering::Relaxed);
        let _ = context.app.emit_to("pet", "keyboard-activity", ());
    }

    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[cfg(not(target_os = "macos"))]
mod keyboard_activity {
    pub fn start(_app: tauri::AppHandle) {}
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            keyboard_activity::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_default_pet,
            quit_app,
            pet_window_position,
            move_pet_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running desktop pet");
}
