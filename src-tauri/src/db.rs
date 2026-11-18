use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

use crate::ChatMessage;
use rusqlite::Connection;

// A message row to persist. Used by both the send arm (own messages)
// and the receive arm (incoming messages) of the p2p loop.
pub struct NewMessage {
    pub uid: Option<String>,
    pub sender: String,
    pub sender_pid: Option<String>,
    pub room: String,
    pub body: String,
    pub timestamp: String,
}

pub struct Db {
    conn: Mutex<Connection>,
    received_files_path: PathBuf,
}

fn config_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    let mut dir = app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(profile_suffix) = std::env::var("APP_PROFILE") {
        dir.push(profile_suffix);
    }
    dir
}

impl Db {
    pub fn open(app_handle: &tauri::AppHandle) -> Db {
        let mut db_path = config_dir(app_handle);
        let _ = fs::create_dir_all(&db_path);
        let received_files_path = db_path.join("received_files.json");
        db_path.push("chat_history.db");

        let conn = Connection::open(db_path).expect("Failed to open SQLite database connection");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uid TEXT,
                sender TEXT NOT NULL,
                room TEXT NOT NULL,
                body TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )
        .expect("Failed to initialize database tables");

        let _ = conn.execute("ALTER TABLE messages ADD COLUMN uid TEXT", []);
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN sender_pid TEXT", []);

        Db {
            conn: Mutex::new(conn),
            received_files_path,
        }
    }

    pub fn history(&self) -> Result<Vec<ChatMessage>, String> {
        let db = self.conn.lock().unwrap();
        let mut stmt = db
            .prepare("SELECT id, uid, sender, sender_pid, room, body, timestamp FROM messages ORDER BY id ASC")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ChatMessage {
                    id: Some(row.get(0)?),
                    uid: row.get(1)?,
                    sender: row.get(2)?,
                    sender_pid: row.get(3)?,
                    room: row.get(4)?,
                    body: row.get(5)?,
                    timestamp: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let history: Vec<_> = rows.flatten().collect();
        Ok(history)
    }

    // Single write path for every message row.
    pub fn insert_message(&self, msg: &NewMessage) {
        let db = self.conn.lock().unwrap();
        let result = db.execute(
            "INSERT INTO messages (uid, sender, sender_pid, room, body, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                msg.uid.as_deref(),
                msg.sender.as_str(),
                msg.sender_pid.as_deref(),
                msg.room.as_str(),
                msg.body.as_str(),
                msg.timestamp.as_str()
            ],
        );
        if let Err(e) = result {
            log::warn!("Failed to persist message: {}", e);
        }
    }

    pub fn record_received_file(&self, id: &str, name: &str, size: u64) {
        let mut map: std::collections::HashMap<String, serde_json::Value> =
            fs::read_to_string(&self.received_files_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
        map.insert(
            id.to_string(),
            serde_json::json!({ "name": name, "size": size }),
        );
        let _ = fs::write(
            &self.received_files_path,
            serde_json::to_string(&map).unwrap_or_default(),
        );
    }

    pub fn received_files(&self) -> serde_json::Value {
        fs::read_to_string(&self.received_files_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    }
}
