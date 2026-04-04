#[macro_use]
extern crate rocket;

use std::collections::HashMap;
use std::sync::Arc;

use game::{ActionError, ClientAction, GameState};
use rocket::futures::{SinkExt, StreamExt};
use rocket::tokio::sync::broadcast::{self, Sender};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::fs::FileServer;
use rocket::http::{Header, Status};
use rocket::request::Request;
use rocket::serde::json::Json;
use rocket::{futures::lock::Mutex, tokio::select, State};
use serde::Serialize;
use ws::{stream::DuplexStream, Message};

mod cards;
mod game;
mod scoring;
mod tests;

// ─── Room registry ────────────────────────────────────────────────────────────

struct GameRoom {
    state: GameState,
    sender: Sender<()>,
}

#[derive(Default)]
struct Rooms(HashMap<String, GameRoom>);

/// Shared broadcast channel that fires whenever the room list changes
/// (player joins, game starts, etc.) so lobby WebSocket clients get updates.
struct LobbySender(Sender<()>);

// ─── WebSocket endpoint ───────────────────────────────────────────────────────

/// GET /game/<room_name>?player=<player_name>
#[get("/game/<room_name>?<player>")]
async fn play_game(
    ws: ws::WebSocket,
    room_name: &str,
    player: Option<&str>,
    rooms_state: &State<Arc<Mutex<Rooms>>>,
    lobby_sender: &State<LobbySender>,
) -> ws::Channel<'static> {
    let player_name = player.unwrap_or("Anonymous").to_string();
    let room_name = room_name.to_string();
    let rooms_state = Arc::clone(rooms_state);
    let lobby_sender = lobby_sender.0.clone();

    ws.channel(move |mut stream| {
        Box::pin(async move {
            // ── Join / create room ────────────────────────────────────────
            let state_updated_sender;
            {
                let mut rooms = rooms_state.lock().await;
                let room = rooms.0.entry(room_name.clone()).or_insert_with(|| {
                    let (sender, _) = broadcast::channel(1);
                    GameRoom {
                        state: GameState::new_waiting(6), // up to 6 players; StartGame locks in count
                        sender,
                    }
                });
                let my_seat = match room.state.join(&player_name) {
                    Ok(seat) => seat,
                    Err(e) => {
                        let _ = stream
                            .send(Message::Text(serde_json::to_string(&e).unwrap()))
                            .await;
                        return Ok(());
                    }
                };
                state_updated_sender = room.sender.clone();
                // Broadcast join to existing clients, then send initial snapshot to joiner.
                let _ = state_updated_sender.send(());
                let snapshot = room.state.to_client_state(my_seat);
                let _ = stream
                    .send(Message::Text(serde_json::to_string(&snapshot).unwrap()))
                    .await;
            }
            // Notify lobby clients that room list changed.
            let _ = lobby_sender.send(());

            let mut state_updated_receiver = state_updated_sender.subscribe();

            // ── Main event loop ───────────────────────────────────────────
            // Use player_name to look up current seat (seats shift when players leave).
            loop {
                select! {
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(message)) => {
                                handle_message(
                                    message,
                                    &player_name,
                                    &room_name,
                                    rooms_state.clone(),
                                    &mut stream,
                                    &state_updated_sender,
                                    &lobby_sender,
                                ).await;
                            }
                            _ => break,
                        }
                    }
                    _ = state_updated_receiver.recv() => {
                        let rooms = rooms_state.lock().await;
                        if let Some(room) = rooms.0.get(&room_name) {
                            if let Some(seat) = room.state.seat_of(&player_name) {
                                let snapshot = room.state.to_client_state(seat);
                                let _ = stream
                                    .send(Message::Text(serde_json::to_string(&snapshot).unwrap()))
                                    .await;
                            } else {
                                break; // player was removed
                            }
                        }
                    }
                }
            }

            // ── Disconnect cleanup ───────────────────────────────────────
            {
                let mut rooms = rooms_state.lock().await;
                if let Some(room) = rooms.0.get_mut(&room_name) {
                    if matches!(room.state.phase, game::GamePhase::WaitingForPlayers { .. }) {
                        room.state.remove_player(&player_name);
                        if room.state.players.is_empty() {
                            rooms.0.remove(&room_name);
                        } else {
                            let _ = room.sender.send(());
                        }
                        let _ = lobby_sender.send(());
                    }
                }
            }
            Ok(())
        })
    })
}

async fn handle_message(
    message: Message,
    player_name: &str,
    room_name: &str,
    rooms_state: Arc<Mutex<Rooms>>,
    stream: &mut DuplexStream,
    sender: &Sender<()>,
    lobby_sender: &Sender<()>,
) {
    if let Message::Text(text) = message {
        println!("[{}] {}: {}", room_name, player_name, text);
        match serde_json::from_str::<ClientAction>(&text) {
            Ok(action) => {
                let is_start = matches!(action, ClientAction::StartGame { .. });
                let result = {
                    let mut rooms = rooms_state.lock().await;
                    let room = rooms.0.get_mut(room_name).ok_or(ActionError::InvalidSeat);
                    match room {
                        Ok(room) => {
                            match room.state.seat_of(player_name) {
                                Some(seat) => {
                                    let r = match &action {
                                        ClientAction::StartGame { advanced, expansion } =>
                                            room.state.start_game(seat, *advanced, *expansion),
                                        ClientAction::KeepCards { indices } =>
                                            room.state.keep_cards(seat, indices),
                                        ClientAction::PlayCard { card_index } =>
                                            room.state.play_card(seat, *card_index),
                                        ClientAction::ChooseSanctuary { sanctuary_index } =>
                                            room.state.choose_sanctuary(seat, *sanctuary_index),
                                        ClientAction::DraftCard { market_index } =>
                                            room.state.draft_card(seat, *market_index),
                                    };
                                    r
                                }
                                None => Err(ActionError::InvalidSeat),
                            }
                        }
                        Err(e) => Err(e),
                    }
                };
                match result {
                    Ok(()) => {
                        // Broadcast to all other clients in the room.
                        let _ = sender.send(());
                        // Send snapshot back to acting client too.
                        let rooms = rooms_state.lock().await;
                        if let Some(room) = rooms.0.get(room_name) {
                            if let Some(seat) = room.state.seat_of(player_name) {
                                let snapshot = room.state.to_client_state(seat);
                                let _ = stream
                                    .send(Message::Text(serde_json::to_string(&snapshot).unwrap()))
                                    .await;
                            }
                        }
                        // Notify lobby when game starts (room leaves WaitingForPlayers).
                        if is_start {
                            let _ = lobby_sender.send(());
                        }
                    }
                    Err(e) => {
                        let err_json = format!("{{\"Err\":\"{:?}\"}}", e);
                        let _ = stream.send(Message::Text(err_json)).await;
                    }
                }
            }
            Err(_) => {
                let _ = stream
                    .send(Message::Text("{\"Err\":\"MalformedAction\"}".to_string()))
                    .await;
            }
        }
    } else {
        let _ = stream
            .send(Message::Text("{\"Err\":\"SentNonTextMessage\"}".to_string()))
            .await;
    }
}

// ─── Demo endpoint ────────────────────────────────────────────────────────────

/// GET /demo/<room_name> — create a room with a pre-completed game for testing scoring UI.
#[get("/demo/<room_name>")]
async fn demo_game(
    room_name: &str,
    rooms_state: &State<Arc<Mutex<Rooms>>>,
) -> String {
    let mut rooms = rooms_state.lock().await;
    let room_name = room_name.to_string();
    let (sender, _) = broadcast::channel(1);
    rooms.0.insert(room_name.clone(), GameRoom {
        state: GameState::new_demo(),
        sender,
    });
    format!("Demo game created in room '{}'. Connect as Alice or Bob.", room_name)
}

// ─── Room listing ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RoomInfo {
    code: String,
    players: Vec<String>,
    player_count: usize,
}

fn build_room_list(rooms: &Rooms) -> Vec<RoomInfo> {
    rooms.0.iter().filter_map(|(code, room)| {
        if matches!(room.state.phase, game::GamePhase::WaitingForPlayers { .. }) {
            Some(RoomInfo {
                code: code.clone(),
                players: room.state.players.iter().map(|p| p.name.clone()).collect(),
                player_count: room.state.players.len(),
            })
        } else {
            None
        }
    }).collect()
}

#[get("/api/rooms")]
async fn list_rooms(rooms_state: &State<Arc<Mutex<Rooms>>>) -> Json<Vec<RoomInfo>> {
    Json(build_room_list(&*rooms_state.lock().await))
}

/// WebSocket endpoint for live lobby updates. Pushes room list whenever it changes.
#[get("/lobby")]
async fn lobby_ws(
    ws: ws::WebSocket,
    rooms_state: &State<Arc<Mutex<Rooms>>>,
    lobby_sender: &State<LobbySender>,
) -> ws::Channel<'static> {
    let rooms_state = Arc::clone(rooms_state);
    let mut receiver = lobby_sender.0.subscribe();

    ws.channel(move |mut stream| {
        Box::pin(async move {
            // Send current room list immediately.
            let list = build_room_list(&*rooms_state.lock().await);
            let _ = stream.send(Message::Text(serde_json::to_string(&list).unwrap())).await;

            // Push updates whenever lobby_sender fires.
            loop {
                select! {
                    _ = receiver.recv() => {
                        let list = build_room_list(&*rooms_state.lock().await);
                        let _ = stream.send(Message::Text(serde_json::to_string(&list).unwrap())).await;
                    }
                    msg = stream.next() => {
                        // Client closed or sent something (we ignore messages).
                        if msg.is_none() { break; }
                    }
                }
            }
            Ok(())
        })
    })
}

// ─── Health check ─────────────────────────────────────────────────────────────

#[get("/health")]
fn health() -> &'static str {
    "yonder-server ok"
}

// ─── 404 catcher ─────────────────────────────────────────────────────────────

#[catch(404)]
fn not_found(req: &Request) -> (Status, String) {
    (Status::NotFound, format!("Not found: {}", req.uri()))
}

// ─── Response fairing (no-cache + CORS) ─────────────────────────────────────

struct ResponseFairing;

#[rocket::async_trait]
impl Fairing for ResponseFairing {
    fn info(&self) -> Info {
        Info { name: "Response Headers", kind: Kind::Response }
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut rocket::Response<'r>) {
        let path = req.uri().path().as_str();
        if path.ends_with(".html") || path.ends_with(".js") || path.ends_with(".css") || path == "/" {
            res.set_header(Header::new("Cache-Control", "no-cache, no-store, must-revalidate"));
        }
        // CORS for API endpoints (needed when client is served from a different origin)
        if path.starts_with("/api/") {
            res.set_header(Header::new("Access-Control-Allow-Origin", "*"));
            res.set_header(Header::new("Access-Control-Allow-Methods", "GET, OPTIONS"));
            res.set_header(Header::new("Access-Control-Allow-Headers", "Content-Type"));
        }
    }
}

// ─── Launch ───────────────────────────────────────────────────────────────────

#[launch]
fn rocket() -> _ {
    let client_dir = std::env::var("YONDER_CLIENT_DIR")
        .unwrap_or_else(|_| "../yonder-client".to_string());
    println!("Serving static files from: {}", client_dir);

    rocket::build()
        .attach(ResponseFairing)
        .mount("/", routes![health, play_game, demo_game, list_rooms, lobby_ws])
        .mount("/", FileServer::from(&client_dir))
        .register("/", catchers![not_found])
        .configure(rocket::Config {
            address: "0.0.0.0".parse().unwrap(),
            port: std::env::var("ROCKET_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8085),
            ..Default::default()
        })
        .manage(Arc::new(Mutex::new(Rooms::default())))
        .manage(LobbySender(broadcast::channel(16).0))
}
