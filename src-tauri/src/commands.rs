//! Tauri commands: the invoke_handler surface plus the JSON-file storage
//! helpers they share (identity, rooms, klipy). Network plumbing lives in
//! net.rs; shared state types live in the crate root.

use libp2p::{Multiaddr, PeerId};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use tauri::{Emitter, Manager};

use crate::db;
use crate::net::{sanitize_filename, OutboundCommand, RelayEndpoint, RelayStatus};
use crate::{AppState, ChatMessage, KlipyConfig, RoomInfo, UserProfile};

// Refuse transfers above 512 MiB before allocating the buffer.
const MAX_FILE_SIZE: u64 = 512 * 1024 * 1024;

pub fn get_or_create_identity(
    app_handle: &tauri::AppHandle,
) -> (libp2p::identity::Keypair, UserProfile, Vec<String>) {
    let mut config_dir = app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(profile_suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(profile_suffix);
    }
    let _ = fs::create_dir_all(&config_dir);

    let mut key_path = config_dir.clone();
    key_path.push("peer_identity.bin");
    let mut profile_path = config_dir.clone();
    profile_path.push("user_profile.json");
    let mut allow_list_path = config_dir.clone();
    allow_list_path.push("allow_list.json");

    let keypair = if key_path.exists() {
        let bytes = fs::read(&key_path).unwrap();
        libp2p::identity::Keypair::from_protobuf_encoding(&bytes).unwrap_or_else(|_| {
            let new_key = libp2p::identity::Keypair::generate_ed25519();
            let _ = fs::write(&key_path, new_key.to_protobuf_encoding().unwrap());
            new_key
        })
    } else {
        let new_key = libp2p::identity::Keypair::generate_ed25519();
        let _ = fs::write(&key_path, new_key.to_protobuf_encoding().unwrap());
        new_key
    };

    let peer_id = PeerId::from(keypair.public()).to_string();

    let profile = if profile_path.exists() {
        let data = fs::read_to_string(&profile_path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_else(|_| UserProfile {
            peer_id: peer_id.clone(),
            nickname: format!("Peer-{}", &peer_id[..6]),
        })
    } else {
        let new_profile = UserProfile {
            peer_id: peer_id.clone(),
            nickname: format!("Peer-{}", &peer_id[..6]),
        };
        let _ = fs::write(&profile_path, serde_json::to_string(&new_profile).unwrap());
        new_profile
    };

    let allow_list = if allow_list_path.exists() {
        let data = fs::read_to_string(&allow_list_path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_else(|_| vec![])
    } else {
        let default_list: Vec<String> = vec![];
        let _ = fs::write(
            &allow_list_path,
            serde_json::to_string(&default_list).unwrap(),
        );
        default_list
    };

    (keypair, profile, allow_list)
}

pub fn get_saved_rooms(app_handle: &tauri::AppHandle) -> Vec<RoomInfo> {
    let mut config_dir = app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(profile_suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(profile_suffix);
    }
    let mut path = config_dir;
    path.push("rooms.json");

    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| {
            vec![RoomInfo {
                id: "sbb-lounge".to_string(),
                name: "sbb-lounge".to_string(),
            }]
        })
}

fn valid_room_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

fn persist_rooms(app_handle: &tauri::AppHandle, list: &[RoomInfo]) -> Result<(), String> {
    let mut config_dir = app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(suffix);
    }
    config_dir.push("rooms.json");
    fs::write(&config_dir, serde_json::to_string(list).unwrap()).map_err(|e| e.to_string())
}

pub fn get_saved_klipy_config(app_handle: &tauri::AppHandle) -> Option<KlipyConfig> {
    let mut config_dir = app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(profile_suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(profile_suffix);
    }
    let mut path = config_dir;
    path.push("klipy_config.json");

    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

#[tauri::command]
pub(crate) fn get_profile(state: tauri::State<'_, AppState>) -> UserProfile {
    state.profile.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn get_chat_history(
    state: tauri::State<'_, db::Db>,
) -> Result<Vec<ChatMessage>, String> {
    state.history()
}

#[tauri::command]
pub(crate) fn update_nickname(
    new_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<UserProfile, String> {
    if new_name.trim().is_empty() {
        return Err("Nickname cannot be empty".to_string());
    }
    let mut profile = state.profile.lock().unwrap();
    profile.nickname = new_name.trim().to_string();
    let mut config_dir = state
        .app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(suffix);
    }
    config_dir.push("user_profile.json");
    fs::write(&config_dir, serde_json::to_string(&*profile).unwrap()).map_err(|e| e.to_string())?;
    Ok(profile.clone())
}

#[tauri::command]
pub(crate) fn get_allow_list(state: tauri::State<'_, AppState>) -> Vec<String> {
    state.allow_list.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn add_to_allow_list(
    target_friend_peer_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    // Validate that the input string is a legitimate libp2p PeerID format
    let _verified_pid = PeerId::from_str(&target_friend_peer_id)
        .map_err(|e| format!("Invalid PeerId format: {:?}", e))?;

    let mut list = state.allow_list.lock().unwrap();

    if !list.contains(&target_friend_peer_id) {
        list.push(target_friend_peer_id.clone());

        let mut config_dir = state
            .app_handle
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        if let Ok(suffix) = std::env::var("APP_PROFILE") {
            config_dir.push(suffix);
        }
        config_dir.push("allow_list.json");
        fs::write(&config_dir, serde_json::to_string(&*list).unwrap())
            .map_err(|e| e.to_string())?;
    }
    Ok(list.clone())
}

#[tauri::command]
pub(crate) fn remove_from_allow_list(
    target_peer_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    if PeerId::from_str(&target_peer_id).is_err() {
        return Err("Invalid PeerID format".to_string());
    }

    let mut list = state.allow_list.lock().unwrap();

    if list.contains(&target_peer_id) {
        list.retain(|id| id != &target_peer_id);

        let mut config_dir = state
            .app_handle
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        if let Ok(suffix) = std::env::var("APP_PROFILE") {
            config_dir.push(suffix);
        }
        config_dir.push("allow_list.json");
        fs::write(&config_dir, serde_json::to_string(&*list).unwrap())
            .map_err(|e| e.to_string())?;

        let _ = state
            .tx
            .send(OutboundCommand::Prune(target_peer_id.clone()));
    }

    Ok(list.clone())
}

#[tauri::command]
pub(crate) fn send_chat_message(
    message: String,
    uid: Option<String>,
    room: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let uid = uid.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let room = room.unwrap_or_else(|| "sbb-lounge".to_string());
    state
        .tx
        .send(OutboundCommand::Chat {
            uid,
            room,
            body: message,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn save_relay_endpoint(
    peer_id: String,
    address: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if PeerId::from_str(&peer_id).is_err() {
        return Err("Invalid Relay PeerID format".to_string());
    }
    if address.parse::<Multiaddr>().is_err() {
        return Err("Invalid Multiaddress format".to_string());
    }

    let endpoint = RelayEndpoint {
        peer_id: peer_id.clone(),
        address,
    };

    let mut config_dir = state
        .app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(suffix);
    }
    config_dir.push("relay_endpoint.json");

    fs::write(&config_dir, serde_json::to_string(&endpoint).unwrap()).map_err(|e| e.to_string())?;

    *state.relay_endpoint.lock().unwrap() = Some(endpoint);

    let _ = state.app_handle.emit("relay-endpoint-saved", true);

    Ok(())
}

#[tauri::command]
pub(crate) fn get_relay_endpoint(state: tauri::State<'_, AppState>) -> Option<RelayEndpoint> {
    state.relay_endpoint.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn get_relay_status(state: tauri::State<'_, AppState>) -> RelayStatus {
    state.relay_status.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn clear_relay_endpoint(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut config_dir = state
        .app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(suffix);
    }
    let path = config_dir.join("relay_endpoint.json");

    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    *state.relay_endpoint.lock().unwrap() = None;

    let _ = state.app_handle.emit("relay-endpoint-cleared", true);

    Ok(())
}

#[tauri::command]
pub(crate) fn dial_peer(address: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    log::debug!("dial_peer called with: {}", address);

    let addr: Multiaddr = address.parse().map_err(|e| {
        log::debug!("Parse error: {}", e);
        format!("Invalid address: {}", e)
    })?;

    for proto in addr.iter() {
        if let libp2p::multiaddr::Protocol::P2p(pid) = proto {
            let pid_str = pid.to_string();
            log::debug!("Found peer ID in address: {}", pid_str);

            let mut list = state.allow_list.lock().unwrap();
            if !list.contains(&pid_str) {
                list.push(pid_str.clone());
                log::debug!("Added {} to allow list", pid_str);

                let mut config_dir = state
                    .app_handle
                    .path()
                    .app_config_dir()
                    .unwrap_or_else(|_| PathBuf::from("."));
                if let Ok(suffix) = std::env::var("APP_PROFILE") {
                    config_dir.push(suffix);
                }
                config_dir.push("allow_list.json");
                let _ = fs::write(&config_dir, serde_json::to_string(&*list).unwrap());
            }
            break;
        }
    }

    match state.tx.send(OutboundCommand::Dial(address)) {
        Ok(_) => {
            log::debug!("📞 Dial message sent to network thread");
            Ok(())
        }
        Err(e) => {
            log::debug!("Failed to send dial message: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub(crate) fn send_file(
    path: String,
    uid: String,
    room: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let room = room.unwrap_or_else(|| "sbb-lounge".to_string());
    let file_path = std::path::Path::new(&path);
    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    if !file_path.is_file() {
        return Err("Not a readable file".to_string());
    }
    let size = fs::metadata(file_path)
        .map_err(|e| format!("Cannot read file: {}", e))?
        .len();
    if size > MAX_FILE_SIZE {
        return Err(format!(
            "File is too large ({} bytes; limit is {} bytes)",
            size, MAX_FILE_SIZE
        ));
    }
    let data = fs::read(file_path).map_err(|e| format!("Cannot read file: {}", e))?;
    if data.is_empty() {
        return Err("File is empty".to_string());
    }
    let size = data.len() as u64;

    let info = serde_json::json!({ "id": uid, "name": name, "size": size });

    state
        .tx
        .send(OutboundCommand::Chat {
            uid: uid.clone(),
            room,
            body: format!("P2P_MEDIA_FILE:{}", info),
        })
        .map_err(|e| e.to_string())?;
    state
        .tx
        .send(OutboundCommand::FileTransfer {
            id: uid,
            name,
            size,
            data,
        })
        .map_err(|e| e.to_string())?;

    Ok(info)
}

#[tauri::command]
pub(crate) fn get_received_files(app_handle: tauri::AppHandle) -> serde_json::Value {
    let mut dir = app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        dir.push(suffix);
    }
    fs::read_to_string(dir.join("received_files.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

#[tauri::command]
pub(crate) fn open_shared_file(
    transfer_id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut dir = state
        .app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        dir.push(suffix);
    }
    dir.push("files");
    let path = dir.join(format!("{}_{}", transfer_id, sanitize_filename(&name)));
    if !path.exists() {
        return Err("File not downloaded yet".to_string());
    }
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn open_local_file(
    path: String,
    _state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("File no longer exists".to_string());
    }
    tauri_plugin_opener::open_path(p, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn get_klipy_key(state: tauri::State<'_, AppState>) -> Option<String> {
    state
        .klipy_config
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| c.api_key.clone())
}

#[tauri::command]
pub(crate) fn save_klipy_key(
    api_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let key = api_key.trim().to_string();
    // Klipy key validation
    if key.len() < 16 || key.len() > 128 || key.chars().any(|c| c.is_whitespace()) {
        return Err("That doesn't look like a valid API key".to_string());
    }

    let config = KlipyConfig { api_key: key };

    let mut config_dir = state
        .app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(suffix);
    }
    config_dir.push("klipy_config.json");

    fs::write(&config_dir, serde_json::to_string(&config).unwrap()).map_err(|e| e.to_string())?;
    *state.klipy_config.lock().unwrap() = Some(config);

    Ok(())
}

#[tauri::command]
pub(crate) fn clear_klipy_key(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut config_dir = state
        .app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(suffix);
    }
    config_dir.push("klipy_config.json");

    if config_dir.exists() {
        fs::remove_file(&config_dir).map_err(|e| e.to_string())?;
    }
    *state.klipy_config.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub(crate) fn get_rooms(state: tauri::State<'_, AppState>) -> Vec<RoomInfo> {
    state.rooms.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn create_room(
    id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<RoomInfo, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return Err("Room name must be 1–64 characters".to_string());
    }
    if !valid_room_id(&id) {
        return Err("Invalid room id".to_string());
    }
    let mut list = state.rooms.lock().unwrap();
    if list.iter().any(|c| c.id == id) {
        return Err("Room already exists".to_string());
    }
    let info = RoomInfo { id, name };
    list.push(info.clone());
    persist_rooms(&state.app_handle, &list)?;
    Ok(info)
}

#[tauri::command]
pub(crate) fn rename_room(
    id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return Err("Room name must be 1–64 characters".to_string());
    }
    let mut list = state.rooms.lock().unwrap();
    let room = list
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| "Room not found".to_string())?;
    room.name = name;
    persist_rooms(&state.app_handle, &list)
}

#[tauri::command]
pub(crate) fn save_temp_media(
    data: Vec<u8>,
    ext: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let ext = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" => ext,
        _ => "png".to_string(),
    };

    let mut dir = state
        .app_handle
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?;
    dir.push("media");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = dir.join(format!(
        "{}_{}.{}",
        chrono::Local::now().format("%Y%m%d_%H%M%S"),
        uuid::Uuid::new_v4().simple(),
        ext
    ));
    fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub(crate) fn get_local_image_path(
    transfer_id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mut dir = state
        .app_handle
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    if let Ok(suffix) = std::env::var("APP_PROFILE") {
        dir.push(suffix);
    }
    dir.push("files");
    let path = dir.join(format!("{}_{}", transfer_id, sanitize_filename(&name)));
    if !path.exists() {
        return Err("not downloaded yet".to_string());
    }
    Ok(path.to_string_lossy().to_string())
}
