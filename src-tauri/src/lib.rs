use std::{env, fs, path::PathBuf};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PetJson {
    id: String,
    display_name: String,
    description: String,
    spritesheet_path: String,
}

#[derive(Debug, Serialize)]
struct PetAsset {
    id: String,
    display_name: String,
    description: String,
    image_data_url: String,
}

#[tauri::command]
fn load_default_pet() -> Result<PetAsset, String> {
    let pet_dir = find_first_pet_dir().ok_or_else(|| "No hatch-pet package found".to_string())?;
    let pet_json_path = pet_dir.join("pet.json");
    let pet_json = fs::read_to_string(&pet_json_path)
        .map_err(|error| format!("Failed to read {}: {error}", pet_json_path.display()))?;
    let pet: PetJson = serde_json::from_str(&pet_json)
        .map_err(|error| format!("Failed to parse {}: {error}", pet_json_path.display()))?;

    let sprite_path = pet_dir.join(&pet.spritesheet_path);
    let sprite = fs::read(&sprite_path)
        .map_err(|error| format!("Failed to read {}: {error}", sprite_path.display()))?;
    let encoded = general_purpose::STANDARD.encode(sprite);

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

fn find_first_pet_dir() -> Option<PathBuf> {
    let codex_home = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))?;
    let pets_dir = codex_home.join("pets");

    let mut dirs = fs::read_dir(pets_dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            path.is_dir().then_some(path)
        })
        .collect::<Vec<_>>();
    dirs.sort();

    dirs.into_iter()
        .find(|dir| dir.join("pet.json").is_file() && dir.join("spritesheet.webp").is_file())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_default_pet, quit_app])
        .run(tauri::generate_context!())
        .expect("error while running desktop pet");
}
