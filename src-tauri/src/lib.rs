//! Crate root: shared state types, AppState, and the Tauri entry point.
//! db.rs   — SQLite history + received-file registry
//! net.rs  — libp2p swarm, event loop, wire types
//! commands.rs — the invoke_handler surface

mod commands;
mod db;
mod net;

use std::sync::{Arc, Mutex};
use tauri::Manager;
use tokio::sync::mpsc;

use crate::net::{OutboundCommand, RelayEndpoint, RelayStatus};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct KlipyConfig {
    pub api_key: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct UserProfile {
    pub peer_id: String,
    pub nickname: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub id: Option<i32>,
    pub uid: Option<String>,
    pub sender: String,
    pub sender_pid: Option<String>,
    pub room: String,
    pub body: String,
    pub timestamp: String,
}

pub struct AppState {
    pub tx: mpsc::UnboundedSender<OutboundCommand>,
    pub profile: Arc<Mutex<UserProfile>>,
    pub allow_list: Arc<Mutex<Vec<String>>>,
    pub db: db::Db,
    pub app_handle: tauri::AppHandle,
    pub relay_endpoint: Arc<Mutex<Option<RelayEndpoint>>>,
    pub relay_status: Arc<Mutex<RelayStatus>>,
    pub klipy_config: Arc<Mutex<Option<KlipyConfig>>>,
    pub rooms: Arc<Mutex<Vec<RoomInfo>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // One Db for State<'_, db::Db> consumers (get_chat_history), one
            // writer connection inside AppState for the net event loop.
            app.manage(db::Db::open(&app_handle));

            let (keypair, profile, allow_list_data) = commands::get_or_create_identity(&app_handle);
            let shared_allow_list = Arc::new(Mutex::new(allow_list_data));

            let db_store = db::Db::open(&app_handle);

            let shared_profile = Arc::new(Mutex::new(profile));

            let relay_endpoint = net::get_saved_relay_endpoint(&app_handle);
            let shared_relay_endpoint = Arc::new(Mutex::new(relay_endpoint.clone()));

            let initial_relay_status = RelayStatus {
                connected: false,
                listening_via_relay: false,
                relay_peer_id: relay_endpoint.as_ref().map(|ep| ep.peer_id.clone()),
            };
            let shared_relay_status = Arc::new(Mutex::new(initial_relay_status));

            let network_tx = net::start_p2p_backend(
                app_handle.clone(),
                keypair,
                shared_allow_list.clone(),
                shared_profile.clone(),
                relay_endpoint,
                shared_relay_status.clone(),
            );

            let klipy_config = commands::get_saved_klipy_config(&app_handle);
            let shared_klipy = Arc::new(Mutex::new(klipy_config));

            let rooms = commands::get_saved_rooms(&app_handle);
            let shared_rooms = Arc::new(Mutex::new(rooms));

            app.manage(AppState {
                tx: network_tx,
                profile: shared_profile,
                allow_list: shared_allow_list,
                db: db_store,
                app_handle,
                relay_endpoint: shared_relay_endpoint,
                relay_status: shared_relay_status,
                klipy_config: shared_klipy,
                rooms: shared_rooms,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_chat_message,
            commands::get_profile,
            commands::update_nickname,
            commands::get_allow_list,
            commands::add_to_allow_list,
            commands::remove_from_allow_list,
            commands::get_chat_history,
            commands::save_relay_endpoint,
            commands::get_relay_endpoint,
            commands::get_relay_status,
            commands::clear_relay_endpoint,
            commands::dial_peer,
            commands::send_file,
            commands::get_received_files,
            commands::open_shared_file,
            commands::open_local_file,
            commands::save_klipy_key,
            commands::get_klipy_key,
            commands::clear_klipy_key,
            commands::get_rooms,
            commands::create_room,
            commands::rename_room,
            commands::save_temp_media,
            commands::get_local_image_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
