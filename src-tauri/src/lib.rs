use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition};

const DEFAULT_PET_JSON: &str = include_str!("../resources/pets/melina/pet.json");
const DEFAULT_SPRITESHEET: &[u8] = include_bytes!("../resources/pets/melina/spritesheet.webp");

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

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_default_pet,
            quit_app,
            pet_window_position,
            move_pet_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running desktop pet");
}
