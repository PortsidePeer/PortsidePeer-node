//! libp2p networking: swarm construction, the p2p event loop, and the
//! wire types both sides of a connection speak. Everything Tauri-facing
//! lives in commands.rs; shared app state lives in the crate root.

use futures::StreamExt;
use libp2p::swarm::dial_opts::DialOpts;
use libp2p::{
    core::transport::Transport,
    identify, kad, mdns, relay, request_response,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId,
};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;

use crate::db;
use crate::{AppState, ChatMessage, UserProfile};

const CHUNK_SIZE: usize = 256 * 1024;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RelayEndpoint {
    pub peer_id: String,
    pub address: String,
}

#[derive(Clone, serde::Serialize)]
pub struct RelayStatus {
    pub connected: bool,
    pub listening_via_relay: bool,
    pub relay_peer_id: Option<String>,
}

#[derive(NetworkBehaviour)]
pub struct ChatBehavior {
    pub mdns: mdns::tokio::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub relay: relay::client::Behaviour,
    pub dcutr: libp2p::dcutr::Behaviour,
    pub identify: identify::Behaviour,
    pub request_response: libp2p::request_response::cbor::Behaviour<WireMessage, WireResponse>,
    pub file_transfer: libp2p::request_response::cbor::Behaviour<WireFileChunk, FileChunkAck>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WireMessage {
    pub uid: Option<String>,
    pub timestamp: Option<String>,
    pub room: Option<String>,
    pub sender_pid: Option<String>,
    pub sender: String,
    pub body: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WireResponse {
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WireFileChunk {
    pub transfer_id: String,
    pub name: String,
    pub size: u64,
    pub seq: u32,
    pub total: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChunkAck {
    pub status: String,
}

#[derive(Clone, serde::Serialize)]
struct DiscoveredPeerPayload {
    peer_id: String,
}

#[derive(Debug, Clone)]
pub enum OutboundCommand {
    Chat {
        uid: String,
        room: String,
        body: String,
    },
    FileTransfer {
        id: String,
        name: String,
        size: u64,
        data: Vec<u8>,
    },
    Prune(String),
    Dial(String),
    Graft,
}

pub fn get_saved_relay_endpoint(app_handle: &tauri::AppHandle) -> Option<RelayEndpoint> {
    let mut config_dir = app_handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(profile_suffix) = std::env::var("APP_PROFILE") {
        config_dir.push(profile_suffix);
    }
    let mut path = config_dir;
    path.push("relay_endpoint.json");

    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).ok()
    } else {
        None
    }
}

pub fn build_circuit_addr(
    endpoint: &RelayEndpoint,
    destination: Option<PeerId>,
) -> Option<Multiaddr> {
    let relay_pid = PeerId::from_str(&endpoint.peer_id).ok()?;
    let mut base = Multiaddr::from_str(&endpoint.address).ok()?;

    while matches!(
        base.iter().last(),
        Some(libp2p::multiaddr::Protocol::P2p(_)) | Some(libp2p::multiaddr::Protocol::P2pCircuit)
    ) {
        base.pop();
    }

    let mut addr = base
        .with(libp2p::multiaddr::Protocol::P2p(relay_pid))
        .with(libp2p::multiaddr::Protocol::P2pCircuit);

    if let Some(dst) = destination {
        addr = addr.with(libp2p::multiaddr::Protocol::P2p(dst));
    }

    Some(addr)
}

pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' => '_',
            _ => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned.chars().take(120).collect()
    }
}

pub fn start_p2p_backend(
    app_handle: tauri::AppHandle,
    keypair: libp2p::identity::Keypair,
    shared_allow_list: Arc<Mutex<Vec<String>>>,
    shared_profile: Arc<Mutex<UserProfile>>,
    relay_endpoint: Option<RelayEndpoint>,
    shared_relay_status: Arc<Mutex<RelayStatus>>,
) -> mpsc::UnboundedSender<OutboundCommand> {
    let (tx, mut rx) = mpsc::unbounded_channel::<OutboundCommand>();
    let app_handle_clone = app_handle.clone();
    let profile_clone = shared_profile.clone();
    let relay_status_clone = shared_relay_status.clone();
    let return_tx = tx.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let local_peer_id = PeerId::from(keypair.public());

            let (relay_transport, relay_behaviour) = relay::client::new(local_peer_id);

            let tcp_config = libp2p::tcp::Config::new().nodelay(true);
            let tcp_transport = libp2p::tcp::tokio::Transport::new(tcp_config);
            let dns_tcp = libp2p::dns::tokio::Transport::system(tcp_transport).unwrap();

            let mut explicit_yamux_config = libp2p::yamux::Config::default();
            explicit_yamux_config.set_max_num_streams(8192);

            let base_transport = dns_tcp
                .upgrade(libp2p::core::upgrade::Version::V1)
                .authenticate(libp2p::noise::Config::new(&keypair).unwrap())
                .multiplex(explicit_yamux_config.clone())
                .map(|(peer_id, muxer), _| (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

            let upgraded_relay_transport = relay_transport
                .upgrade(libp2p::core::upgrade::Version::V1)
                .authenticate(libp2p::noise::Config::new(&keypair.clone()).unwrap())
                .multiplex(explicit_yamux_config)
                .map(|(peer_id, muxer), _| (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

            let transport = upgraded_relay_transport
                .or_transport(base_transport)
                .map(|either, _| match either {
                    futures::future::Either::Left(inner) => inner,
                    futures::future::Either::Right(inner) => inner,
                })
                .boxed();

            if relay_endpoint.is_some() {
                log::info!("Relay endpoint configured.");
            } else {
                log::info!("No relay endpoint. Direct TCP only.");
            }

            let store = kad::store::MemoryStore::new(local_peer_id);

            let mdns_config = mdns::Config {
                enable_ipv6: false,
                ..Default::default()
            };

            let identify_config = identify::Config::new(
                "/portsidepeer/1.0.0".to_string(),
                keypair.public(),
            );

            let req_res_config = request_response::Config::default()
                .with_request_timeout(std::time::Duration::from_secs(30));

            let req_res_behaviour = request_response::cbor::Behaviour::new(
                [(
                    libp2p::StreamProtocol::new("/portsidepeer-chat/1.0.0"),
                    request_response::ProtocolSupport::Full,
                )],
                req_res_config,
            );

            let file_behaviour = request_response::cbor::Behaviour::new(
                [(libp2p::StreamProtocol::new("/portsidepeer-file/1.0.0"), request_response::ProtocolSupport::Full)],
                request_response::Config::default()
                    .with_request_timeout(std::time::Duration::from_secs(60)),
            );

            // Dedicated StreamProtocol for the Kademlia ledger
            let kad_protocol = libp2p::StreamProtocol::new("/portsidepeer-kad/1.0.0");
            let kad_config = kad::Config::new(kad_protocol);
            let mut kademlia = kad::Behaviour::with_config(local_peer_id, store, kad_config);
            kademlia.set_mode(Some(kad::Mode::Server));

            let behaviour = ChatBehavior {
                mdns: mdns::tokio::Behaviour::new(mdns_config, local_peer_id).unwrap(),
                kademlia,
                relay: relay_behaviour,
                identify: identify::Behaviour::new(identify_config),
                dcutr: libp2p::dcutr::Behaviour::new(local_peer_id),
                request_response: req_res_behaviour,
                file_transfer: file_behaviour,
            };

            // Swarm build
            let swarm_config = libp2p::swarm::Config::with_tokio_executor()
                .with_idle_connection_timeout(std::time::Duration::from_secs(300));

            let mut swarm = libp2p::Swarm::new(transport, behaviour, local_peer_id, swarm_config);

            // LAN listener
            if let Ok(local_addr) = libp2p::Multiaddr::from_str("/ip4/0.0.0.0/tcp/0") {
                if let Err(e) = swarm.listen_on(local_addr) {
                    log::debug!("Failed to initialize local interface listener: {:?}", e);
                } else {
                    log::debug!("Local network interface listener initialized.");
                }
            }

            if let Some(endpoint) = get_saved_relay_endpoint(&app_handle) {
                if let Ok(relay_peer_id) = libp2p::PeerId::from_str(&endpoint.peer_id) {
                    if let Ok(base_relay_addr) = libp2p::Multiaddr::from_str(&endpoint.address) {
                        swarm.behaviour_mut().kademlia.add_address(&relay_peer_id, base_relay_addr.clone());
                        if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                            log::warn!("⚠️ Kademlia discovery bootstrap failed to launch: {:?}", e);
                        } else {
                            log::debug!("🚀 Discovery initiated over direct transport links.");
                        }
                        let dial_opts = DialOpts::peer_id(relay_peer_id)
                            .addresses(vec![base_relay_addr])
                            .build();
                        match swarm.dial(dial_opts) {
                            Ok(_) => log::debug!("Dialing relay {} at startup...", relay_peer_id),
                            Err(e) => log::debug!("Could not start dialing relay: {:?}", e),
                        }
                    }
                }
            }

            let is_friend_instance = std::env::var("APP_PROFILE").is_ok();

            if is_friend_instance {
                let bridge_tx = tx.clone();

                tokio::spawn(async move {
                    log::debug!("Friend node active. Polling for main node relay address...");

                    let paths = ["src-tauri/.local_peer_bridge.txt", ".local_peer_bridge.txt"];
                    let mut addr_res = None;

                    for _attempt in 1..=15 {
                        for path in &paths {
                            if let Ok(s) = fs::read_to_string(path) {
                                if !s.trim().is_empty() {
                                    addr_res = Some(s);
                                    break;
                                }
                            }
                        }
                        if addr_res.is_some() { break; }
                        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    }

                    if let Some(addr_string) = addr_res {
                        let clean_addr = addr_string.trim();
                        log::debug!("Found main peer address! Attempting to route: {}", clean_addr);

                        // Send the instruction to our INTERNAL_DIAL receiver handler in the main loop
                        let _ = bridge_tx.send(OutboundCommand::Dial(clean_addr.to_string()));
                    } else {
                        log::debug!("Local peer bridge file not found or empty after timeout.");
                    }
                });
            }

            // Initialize discovery engine
            let mut discovery_ticker = tokio::time::interval(std::time::Duration::from_secs(30));
            discovery_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Check relay connection
            let mut relay_reconnect_ticker = tokio::time::interval(std::time::Duration::from_secs(15));
            relay_reconnect_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Dial cooldowns
            let mut dial_cooldowns: std::collections::HashMap<PeerId, std::time::Instant> =
                std::collections::HashMap::new();

            const FILE_WINDOW: usize = 4;
            let mut pending_chunks: std::collections::VecDeque<(PeerId, WireFileChunk)> = Default::default();
            let mut file_inflight: std::collections::HashMap<PeerId, usize> = Default::default();

            let mut file_recv: std::collections::HashMap<
                String,
                (String, u64, u32, std::collections::HashSet<u32>, u64),
            > = Default::default();

            let mut file_send_ticker = tokio::time::interval(std::time::Duration::from_millis(50));
            file_send_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let files_dir = {
                let mut d = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("."));
                if let Ok(suffix) = std::env::var("APP_PROFILE") { d.push(suffix); }
                d.push("files");
                let _ = fs::create_dir_all(&d);
                d
            };

            const DIAL_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(10);

            let mut relay_listener_mounted = false;

            loop {
                tokio::select! {
                    // At runtime automagic discovery engine
                    _ = discovery_ticker.tick() => {
                        let _ = swarm.behaviour_mut().kademlia.start_providing(
                            libp2p::kad::RecordKey::new(&local_peer_id.to_bytes())
                        );

                        let current_allow_list = shared_allow_list.lock().unwrap().clone();
                        for friend_pid_str in &current_allow_list {
                            if let Ok(target_pid) = PeerId::from_str(friend_pid_str) {
                                swarm.behaviour_mut().kademlia.get_closest_peers(target_pid);
                            }
                        }
                    }

                    // Reservations
                    _ = relay_reconnect_ticker.tick() => {
                        if let Some(endpoint) = get_saved_relay_endpoint(&app_handle_clone) {
                            if let Ok(relay_pid) = PeerId::from_str(&endpoint.peer_id) {
                                if !swarm.is_connected(&relay_pid) {
                                    if let Ok(base_addr) = Multiaddr::from_str(&endpoint.address) {
                                        log::debug!("Relay link is down — redialing {}...", relay_pid);
                                        if let Err(e) = swarm.dial(DialOpts::peer_id(relay_pid).addresses(vec![base_addr]).build()) {
                                            log::debug!("Relay redial failed to start: {:?}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // File transfer
                    _ = file_send_ticker.tick() => {
                        while !pending_chunks.is_empty() {
                            let peer = pending_chunks.front().unwrap().0;
                            if *file_inflight.get(&peer).unwrap_or(&0) >= FILE_WINDOW { break; }
                            let (peer, chunk) = pending_chunks.pop_front().unwrap();
                            *file_inflight.entry(peer).or_insert(0) += 1;
                            swarm.behaviour_mut().file_transfer.send_request(&peer, chunk);
                        }
                    }

                    // Tauri internal frontend instruction channel handler
                    opt_msg = rx.recv() => {
                        if let Some(cmd) = opt_msg {
                            match cmd {
                                OutboundCommand::Graft => {
                                    let _ = swarm.behaviour_mut().kademlia.bootstrap();
                                }
                                OutboundCommand::Prune(peer_id_str) => {
                                    if let Ok(peer_id) = PeerId::from_str(&peer_id_str) {
                                        let _ = swarm.disconnect_peer_id(peer_id);
                                    }
                                }
                                OutboundCommand::Dial(dial_path) => {
                                    log::debug!("Received dial request: {}", dial_path);
                                    match Multiaddr::from_str(&dial_path) {
                                        Ok(addr) => {
                                            let peer_ids: Vec<_> = addr.iter().filter_map(|p| {
                                                if let libp2p::multiaddr::Protocol::P2p(pid) = p { Some(pid) } else { None }
                                            }).collect();

                                            let target_peer_id = peer_ids.last().cloned();
                                            let opts = match target_peer_id {
                                                Some(pid) => DialOpts::peer_id(pid).addresses(vec![addr]).build(),
                                                None => DialOpts::unknown_peer_id().address(addr).build(),
                                            };

                                            match swarm.dial(opts) {
                                                Ok(_) => log::info!("Dial initiated successfully"),
                                                Err(e) => log::info!("Dial failed: {}", e),
                                            }
                                        }
                                        Err(e) => log::info!("❌ Failed to parse address: {}", e),
                                    }
                                }
                                OutboundCommand::FileTransfer { id, name, size, data } => {
                                    if let Some(app_state) = app_handle_clone.try_state::<AppState>() {
                                        let allow = app_state.allow_list.lock().unwrap().clone();
                                        let total = data.len().div_ceil(CHUNK_SIZE) as u32;
                                        for friend in &allow {
                                            if let Ok(peer) = PeerId::from_str(friend) {
                                                for (seq, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
                                                    pending_chunks.push_back((peer, WireFileChunk {
                                                        transfer_id: id.clone(),
                                                        name: name.clone(),
                                                        size,
                                                        seq: seq as u32,
                                                        total,
                                                        data: chunk.to_vec(),
                                                    }));
                                                }
                                            }
                                        }
                                        log::info!("Queued file '{}' ({} bytes, {} chunks/peer, {} peers)", name, size, total, allow.len());
                                    }
                                }
                                OutboundCommand::Chat { uid, room, body } => {
                                    if let Some(app_state) = app_handle_clone.try_state::<AppState>() {
                                        let current_allow_list = app_state.allow_list.lock().unwrap().clone();
                                        let nickname = profile_clone.lock().unwrap().nickname.clone();
                                        let peer_id = profile_clone.lock().unwrap().peer_id.clone();
                                        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                                        app_state.db.insert_message(&db::NewMessage {
                                            uid: Some(uid.clone()),
                                            sender: nickname.clone(),
                                            sender_pid: Some(peer_id.clone()),
                                            room: room.clone(),
                                            body: body.clone(),
                                            timestamp: now.clone(),
                                        });
                                        for friend_pid_str in &current_allow_list {
                                            if let Ok(friend_peer_id) = PeerId::from_str(friend_pid_str) {
                                                let is_connected = swarm.is_connected(&friend_peer_id);

                                                if !is_connected {
                                                    let cooling_down = dial_cooldowns
                                                        .get(&friend_peer_id)
                                                        .map(|t| t.elapsed() < DIAL_COOLDOWN)
                                                        .unwrap_or(false);

                                                    if cooling_down {
                                                        log::debug!("Dial to {} still cooling down; the request below stays queued.", friend_peer_id);
                                                    } else {
                                                        log::debug!("Transport line to {} is COLD. Resolving relay circuit tunnel parameters...", friend_peer_id);
                                                        if let Some(relay_ep) = get_saved_relay_endpoint(&app_handle_clone) {
                                                            if let Some(target_addr) = build_circuit_addr(&relay_ep, Some(friend_peer_id)) {
                                                                log::debug!("Injecting forced inline dial shortcut for peer: {}", friend_peer_id);
                                                                swarm.behaviour_mut().kademlia.add_address(&friend_peer_id, target_addr.clone());
                                                                dial_cooldowns.insert(friend_peer_id, std::time::Instant::now());
                                                                let dial_options = DialOpts::peer_id(friend_peer_id)
                                                                    .addresses(vec![target_addr])
                                                                    .build();
                                                                if let Err(e) = swarm.dial(dial_options) {
                                                                    log::debug!("Failed to start circuit dial to {}: {:?}", friend_peer_id, e);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                let peer_id = profile_clone.lock().unwrap().peer_id.clone();

                                                swarm.behaviour_mut().request_response.send_request(
                                                    &friend_peer_id,
                                                    WireMessage {
                                                        uid: Some(uid.clone()),
                                                        timestamp: Some(now.clone()),
                                                        room: Some(room.clone()),
                                                        sender_pid: Some(peer_id.clone()),
                                                        sender: nickname.clone(),
                                                        body: body.clone(),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Libp2p swarm network event interface
                    net_event = swarm.select_next_some() => {
                        match net_event {
                            SwarmEvent::NewListenAddr { address, .. } => {
                                let addr_str = address.to_string();
                                if addr_str.contains("p2p-circuit") {
                                    log::debug!("Relay circuit active: {}/p2p/{}", address, local_peer_id);
                                    let _ = app_handle_clone.emit("relay-listening", format!("{}/p2p/{}", address, local_peer_id));
                                    let mut status = relay_status_clone.lock().unwrap();
                                    status.listening_via_relay = true;
                                } else if !addr_str.contains("127.0.0.1") && !addr_str.contains("0.0.0.0") {
                                    let full_addr = format!("{}/p2p/{}", address, local_peer_id);
                                    log::debug!("LAN address: {}", full_addr);
                                    if !is_friend_instance {
                                        let _ = fs::write("src-tauri/.local_peer_bridge.txt", &full_addr);
                                        let _ = fs::write(".local_peer_bridge.txt", &full_addr);
                                    }
                                }
                            }

                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("Connected to peer: {}", peer_id);

                                dial_cooldowns.remove(&peer_id);

                                let _ = app_handle_clone.emit(
                                    "peer-discovered",
                                    DiscoveredPeerPayload { peer_id: peer_id.to_string() }
                                );

                                // Relay server authentication
                                if let Some(endpoint_config) = get_saved_relay_endpoint(&app_handle_clone) {
                                    if let Ok(relay_pid) = PeerId::from_str(&endpoint_config.peer_id) {
                                        if peer_id == relay_pid {
                                            log::info!("Secure connection established to designated Relay Server!");
                                            swarm.behaviour_mut().identify.push(std::iter::once(peer_id));
                                            {
                                                let mut status = relay_status_clone.lock().unwrap();
                                                status.connected = true;
                                                status.relay_peer_id = Some(relay_pid.to_string());
                                                let _ = app_handle_clone.emit("relay-status-changed", status.clone());
                                            }
                                            if !relay_listener_mounted {
                                                if let Ok(base_relay_addr) = Multiaddr::from_str(&endpoint_config.address) {
                                                    swarm.behaviour_mut().kademlia.add_address(&relay_pid, base_relay_addr);

                                                    if let Some(relay_listen_addr) = build_circuit_addr(&endpoint_config, None) {
                                                        log::debug!("Mounting client proxy listener channel: {}", relay_listen_addr);

                                                        if let Err(e) = swarm.listen_on(relay_listen_addr) {
                                                            log::debug!("Failed to mount relay proxy listener: {:?}", e);
                                                        } else {
                                                            relay_listener_mounted = true;
                                                            log::debug!("Remote proxy circuit routing pipeline successfully mounted!");
                                                            let _ = swarm.behaviour_mut().kademlia.bootstrap();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                                dial_cooldowns.remove(&peer_id);

                                let mut status = relay_status_clone.lock().unwrap();
                                if status.relay_peer_id.as_ref() == Some(&peer_id.to_string()) {
                                    status.connected = false;
                                    status.listening_via_relay = false;
                                    relay_listener_mounted = false;
                                    let _ = app_handle_clone.emit("relay-status-changed", status.clone());
                                }
                            }
                            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                                if let Some(pid) = peer_id {
                                    dial_cooldowns.remove(&pid);
                                }
                                match &error {
                                    libp2p::swarm::DialError::Transport(errors) => {
                                        log::debug!("Dial failed to {:?}: {:?}", peer_id, errors);
                                    }
                                    other => {
                                        log::warn!("Outgoing connection error to {:?}: {:?}", peer_id, other);
                                    }
                                }
                            }
                            SwarmEvent::IncomingConnectionError { .. } => {
                            }
                            SwarmEvent::ListenerClosed { addresses, reason, .. } => {
                                if addresses.iter().any(|a| a.to_string().contains("p2p-circuit")) {
                                    log::debug!("Relay circuit listener closed: {:?}", reason);
                                    relay_listener_mounted = false;
                                    let mut status = relay_status_clone.lock().unwrap();
                                    status.listening_via_relay = false;
                                    let _ = app_handle_clone.emit("relay-status-changed", status.clone());
                                }
                            }
                            SwarmEvent::Behaviour(chat_event) => match chat_event {
                                ChatBehaviorEvent::Mdns(mdns::Event::Discovered(list)) => {
                                    for (peer_id, addr) in list {
                                        swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                                        let _ = app_handle_clone.emit("peer-discovered", DiscoveredPeerPayload { peer_id: peer_id.to_string() });
                                    }
                                }
                                ChatBehaviorEvent::Mdns(mdns::Event::Expired(_)) => {}
                                ChatBehaviorEvent::RequestResponse(libp2p::request_response::Event::Message { message, .. }) => {
                                    match message {
                                        libp2p::request_response::Message::Request { request, channel, .. } => {
                                            log::debug!("Inbound packet decoded cleanly from sender: {}", request.sender);
                                            let timestamp = request.timestamp.clone().unwrap_or_else(|| {
                                                chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
                                            });
                                            let chat_room = request.room.clone().unwrap_or_else(|| "sbb-lounge".to_string());
                                            if let Some(app_state) = app_handle_clone.try_state::<AppState>() {
                                                app_state.db.insert_message(&db::NewMessage {
                                                    uid: request.uid.clone(),
                                                    sender: request.sender.clone(),
                                                    sender_pid: request.sender_pid.clone(),
                                                    room: chat_room.clone(),
                                                    body: request.body.clone(),
                                                    timestamp: timestamp.clone(),
                                                });
                                            }

                                            let _ = app_handle_clone.emit("chat-msg", ChatMessage {
                                                id: None,
                                                uid: request.uid,
                                                sender: request.sender,
                                                sender_pid: request.sender_pid.clone(),
                                                room: chat_room,
                                                body: request.body,
                                                timestamp,
                                            });

                                            let _ = swarm.behaviour_mut().request_response.send_response(
                                                channel,
                                                WireResponse { status: "DELIVERED".to_string() }
                                            );
                                        }

                                        libp2p::request_response::Message::Response { response, .. } => {
                                            log::debug!("Synchronous Delivery Verified via Relay Proxy Tunnel! Status: {}", response.status);
                                            let _ = app_handle_clone.emit("message-delivery-confirmed", true);
                                        }
                                    }
                                }
                                ChatBehaviorEvent::RequestResponse(_other_events) => {}
                                ChatBehaviorEvent::FileTransfer(libp2p::request_response::Event::Message { peer, message, .. }) => {
                                    match message {
                                        libp2p::request_response::Message::Request { request, channel, .. } => {
                                            let mut finalize: Option<(String, u64)> = None;
                                            {
                                                let (fname, fsize, ftotal, seqs, received) = file_recv
                                                    .entry(request.transfer_id.clone())
                                                    .or_insert_with(|| (request.name.clone(), request.size, request.total, Default::default(), 0));

                                                if seqs.insert(request.seq) {
                                                    let offset = (request.seq as usize) * CHUNK_SIZE;
                                                    let part = files_dir.join(format!("{}.part", request.transfer_id));
                                                    let mut ok = false;
                                                    if let Ok(mut f) = fs::OpenOptions::new().truncate(true).write(true).open(&part) {
                                                        use std::io::{Seek, SeekFrom, Write};
                                                        if f.seek(SeekFrom::Start(offset as u64)).is_ok() && f.write_all(&request.data).is_ok() {
                                                            ok = true;
                                                        }
                                                    }
                                                    if ok {
                                                        *received += request.data.len() as u64;
                                                        let _ = app_handle_clone.emit("file-progress", serde_json::json!({
                                                            "id": request.transfer_id, "name": fname,
                                                            "received": *received, "total": *fsize,
                                                        }));
                                                    } else {
                                                        seqs.remove(&request.seq);
                                                    }
                                                    if *ftotal > 0 && seqs.len() as u32 == *ftotal {
                                                        finalize = Some((fname.clone(), *fsize));
                                                    }
                                                }
                                            }

                                            if let Some((fname, fsize)) = finalize {
                                                let part = files_dir.join(format!("{}.part", request.transfer_id));
                                                let final_path = files_dir.join(format!("{}_{}", request.transfer_id, sanitize_filename(&fname)));
                                                if fs::rename(&part, &final_path).is_ok() {
                                                    if let Some(app_state) = app_handle_clone.try_state::<AppState>() {
                                                        app_state.db.record_received_file(&request.transfer_id, &fname, fsize);
                                                    }
                                                    log::debug!("File complete: {}", final_path.display());
                                                    let _ = app_handle_clone.emit("file-received", serde_json::json!({
                                                        "id": request.transfer_id, "name": fname, "size": fsize,
                                                    }));
                                                }
                                                file_recv.remove(&request.transfer_id);
                                            }

                                            let _ = swarm.behaviour_mut().file_transfer.send_response(
                                                channel,
                                                FileChunkAck { status: "ok".to_string() },
                                            );
                                        }
                                        libp2p::request_response::Message::Response { .. } => {
                                            if let Some(n) = file_inflight.get_mut(&peer) { *n = n.saturating_sub(1); }
                                        }
                                    }
                                }
                                ChatBehaviorEvent::FileTransfer(libp2p::request_response::Event::OutboundFailure { peer, .. }) => {
                                    if let Some(n) = file_inflight.get_mut(&peer) { *n = n.saturating_sub(1); }
                                    log::debug!("File chunk send failed to {}", peer);
                                }
                                ChatBehaviorEvent::FileTransfer(_) => {}

                                ChatBehaviorEvent::Kademlia(libp2p::kad::Event::OutboundQueryProgressed {
                                    result: libp2p::kad::QueryResult::GetClosestPeers(Ok(ok)), ..
                                }) => {
                                    if let Some(app_state) = app_handle_clone.try_state::<AppState>() {
                                        let current_allow_list = app_state.allow_list.lock().unwrap().clone();

                                        for peer in ok.peers {
                                            if current_allow_list.contains(&peer.peer_id.to_string()) {
                                                log::debug!("Found friend's routing record on relay directory: {}", peer.peer_id);

                                                if swarm.is_connected(&peer.peer_id) {
                                                    continue;
                                                }
                                                let cooling_down = dial_cooldowns
                                                    .get(&peer.peer_id)
                                                    .map(|t| t.elapsed() < DIAL_COOLDOWN)
                                                    .unwrap_or(false);
                                                if cooling_down {
                                                    continue;
                                                }

                                                if let Some(relay_ep) = get_saved_relay_endpoint(&app_handle_clone) {
                                                    if let Some(target_addr) = build_circuit_addr(&relay_ep, Some(peer.peer_id)) {
                                                        log::debug!("Executing dynamic background dial to circuit: {}", target_addr);
                                                        swarm.behaviour_mut().kademlia.add_address(&peer.peer_id, target_addr.clone());
                                                        dial_cooldowns.insert(peer.peer_id, std::time::Instant::now());
                                                        let _ = swarm.dial(DialOpts::peer_id(peer.peer_id).addresses(vec![target_addr]).build());
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        log::debug!("Failed to access AppState for Kademlia routing auto-discovery verification.");
                                    }
                                }
                                ChatBehaviorEvent::Kademlia(_other_kad_events) => {}

                                ChatBehaviorEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                                    log::debug!("📋 Identified and completed handshake with remote peer: {}", peer_id);
                                    for addr in info.listen_addrs {
                                        swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                                    }
                                }
                                ChatBehaviorEvent::Identify(_other_identify_events) => {}
                                ChatBehaviorEvent::Relay(relay_event) => {
                                    match relay_event {
                                        relay::client::Event::ReservationReqAccepted { relay_peer_id, renewal, .. } => {
                                            log::debug!("Reservation {} on relay {}", if renewal { "renewed" } else { "accepted" }, relay_peer_id);
                                            relay_listener_mounted = true;
                                        }
                                        other => log::debug!("Relay client event: {:?}", other),
                                    }
                                }
                                ChatBehaviorEvent::Dcutr(dcutr_event) => {
                                    log::debug!("DCUtR event: {:?}", dcutr_event);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    });
    return_tx
}
