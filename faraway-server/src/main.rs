#[macro_use]
extern crate rocket;

use std::collections::HashMap;
use std::sync::Arc;

use game::{ActionError, ClientAction, GameState};
use rocket::futures::{SinkExt, StreamExt};
use rocket::tokio::sync::broadcast::{self, Sender};
use rocket::{futures::lock::Mutex, tokio::select, State};
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

// ─── WebSocket endpoint ───────────────────────────────────────────────────────

/// GET /game/<room_name>?player=<player_name>
#[get("/game/<room_name>?<player>")]
async fn play_game(
    ws: ws::WebSocket,
    room_name: &str,
    player: Option<&str>,
    rooms_state: &State<Arc<Mutex<Rooms>>>,
) -> ws::Channel<'static> {
    let player_name = player.unwrap_or("Anonymous").to_string();
    let room_name = room_name.to_string();
    let rooms_state = Arc::clone(rooms_state);

    ws.channel(move |mut stream| {
        Box::pin(async move {
            // ── Join / create room ────────────────────────────────────────
            let my_seat;
            let state_updated_sender;
            {
                let mut rooms = rooms_state.lock().await;
                let room = rooms.0.entry(room_name.clone()).or_insert_with(|| {
                    let (sender, _) = broadcast::channel(1);
                    GameRoom {
                        state: GameState::new_waiting(2), // default 2-player; StartGame validates
                        sender,
                    }
                });
                match room.state.join(&player_name) {
                    Ok(seat) => {
                        my_seat = seat;
                    }
                    Err(e) => {
                        let _ = stream
                            .send(Message::Text(serde_json::to_string(&e).unwrap()))
                            .await;
                        return Ok(());
                    }
                }
                state_updated_sender = room.sender.clone();
                // Broadcast join to existing clients, then send initial snapshot to joiner.
                let _ = state_updated_sender.send(());
                let snapshot = room.state.to_client_state(my_seat);
                let _ = stream
                    .send(Message::Text(serde_json::to_string(&snapshot).unwrap()))
                    .await;
            }

            let mut state_updated_receiver = state_updated_sender.subscribe();

            // ── Main event loop ───────────────────────────────────────────
            loop {
                select! {
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(message)) => {
                                handle_message(
                                    message,
                                    my_seat,
                                    &room_name,
                                    rooms_state.clone(),
                                    &mut stream,
                                    &state_updated_sender,
                                ).await;
                            }
                            _ => break,
                        }
                    }
                    _ = state_updated_receiver.recv() => {
                        let rooms = rooms_state.lock().await;
                        if let Some(room) = rooms.0.get(&room_name) {
                            let snapshot = room.state.to_client_state(my_seat);
                            let _ = stream
                                .send(Message::Text(serde_json::to_string(&snapshot).unwrap()))
                                .await;
                        }
                    }
                }
            }
            Ok(())
        })
    })
}

async fn handle_message(
    message: Message,
    seat: usize,
    room_name: &str,
    rooms_state: Arc<Mutex<Rooms>>,
    stream: &mut DuplexStream,
    sender: &Sender<()>,
) {
    if let Message::Text(text) = message {
        println!("[{}] seat {}: {}", room_name, seat, text);
        match serde_json::from_str::<ClientAction>(&text) {
            Ok(action) => {
                let result = apply_action(action, seat, room_name, &rooms_state).await;
                match result {
                    Ok(()) => {
                        // Broadcast to all other clients in the room.
                        let _ = sender.send(());
                        // Send snapshot back to acting client too.
                        let rooms = rooms_state.lock().await;
                        if let Some(room) = rooms.0.get(room_name) {
                            let snapshot = room.state.to_client_state(seat);
                            let _ = stream
                                .send(Message::Text(serde_json::to_string(&snapshot).unwrap()))
                                .await;
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

async fn apply_action(
    action: ClientAction,
    seat: usize,
    room_name: &str,
    rooms_state: &Arc<Mutex<Rooms>>,
) -> Result<(), ActionError> {
    let mut rooms = rooms_state.lock().await;
    let room = rooms.0.get_mut(room_name).ok_or(ActionError::InvalidSeat)?;
    match action {
        ClientAction::StartGame => room.state.start_game(seat),
        ClientAction::PlayCard { card_index } => room.state.play_card(seat, card_index),
        ClientAction::ChooseSanctuary { sanctuary_index } => {
            room.state.choose_sanctuary(seat, sanctuary_index)
        }
        ClientAction::DraftCard { market_index } => room.state.draft_card(seat, market_index),
    }
}

// ─── Health check ─────────────────────────────────────────────────────────────

#[get("/")]
fn index() -> &'static str {
    "faraway-server ok"
}

// ─── Launch ───────────────────────────────────────────────────────────────────

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, play_game])
        .configure(rocket::Config {
            address: "0.0.0.0".parse().unwrap(),
            port: std::env::var("ROCKET_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8000),
            ..Default::default()
        })
        .manage(Arc::new(Mutex::new(Rooms::default())))
}
