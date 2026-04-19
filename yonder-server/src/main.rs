#[macro_use]
extern crate rocket;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Instant, SystemTime};

use game::{ActionError, ClientAction, GameState, PostGameHighlight};
use rocket::futures::{SinkExt, StreamExt};
use rocket::tokio::sync::broadcast::{self, Sender};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::fs::{FileServer, NamedFile};
use rocket::http::{Header, Status};
use rocket::request::Request;
use rocket::serde::json::Json;
use std::path::PathBuf;
use rocket::{futures::lock::Mutex, tokio::select, State};
use rusqlite::Connection;
use serde::Serialize;
use ws::{stream::DuplexStream, Message};

mod cards;
mod db;
mod game;
mod scoring;
mod tests;

// ─── Room registry ────────────────────────────────────────────────────────────

struct GameRoom {
    state: GameState,
    sender: Sender<()>,
    last_activity: Instant,
    rematch_code: Option<String>,
    /// Wall-clock time the game started (set when StartGame succeeds).
    started_at: Option<SystemTime>,
    /// Setup flags captured at StartGame.
    started_advanced: bool,
    started_expansion: bool,
    /// True once the completed game has been written to the DB.
    persisted: bool,
    /// True for demo rooms — skip persistence so stats stay clean.
    skip_persistence: bool,
    /// Row id assigned after persistence; forwarded to clients on game-over.
    game_record_id: Option<i64>,
    /// Per-seat highlights computed after persistence.
    post_game_highlights: Option<Vec<PostGameHighlight>>,
}

impl GameRoom {
    fn new(state: GameState, sender: Sender<()>) -> Self {
        Self {
            state,
            sender,
            last_activity: Instant::now(),
            rematch_code: None,
            started_at: None,
            started_advanced: false,
            started_expansion: false,
            persisted: false,
            skip_persistence: false,
            game_record_id: None,
            post_game_highlights: None,
        }
    }
}

/// Shared SQLite connection handle. std::sync::Mutex is fine — queries are
/// short and we never hold the lock across await points.
struct Db(Arc<StdMutex<Connection>>);

#[derive(Default)]
struct Rooms(HashMap<String, GameRoom>);

fn generate_room_code() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut rng = rand::thread_rng();
    (0..4).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect()
}

/// Shared broadcast channel that fires whenever the room list changes
/// (player joins, game starts, etc.) so lobby WebSocket clients get updates.
struct LobbySender(Sender<()>);

// ─── WebSocket endpoint ───────────────────────────────────────────────────────

/// Decorate a client snapshot with room-level info (game record id,
/// post-game highlights). Kept here because `GameState::to_client_state` has no
/// access to the surrounding `GameRoom`.
fn stamp_snapshot(snapshot: &mut game::ClientGameState, room: &GameRoom) {
    snapshot.game_record_id = room.game_record_id;
    snapshot.post_game_highlights = room.post_game_highlights.clone();
}

/// GET /game/<room_name>?player=<player_name>
#[get("/game/<room_name>?<player>")]
async fn play_game(
    ws: ws::WebSocket,
    room_name: &str,
    player: Option<&str>,
    rooms_state: &State<Arc<Mutex<Rooms>>>,
    lobby_sender: &State<LobbySender>,
    db: &State<Db>,
) -> ws::Channel<'static> {
    let player_name = player.unwrap_or("Anonymous").to_string();
    let room_name = room_name.to_string();
    let rooms_state = Arc::clone(rooms_state);
    let lobby_sender = lobby_sender.0.clone();
    let db_handle = Arc::clone(&db.0);

    ws.channel(move |mut stream| {
        Box::pin(async move {
            // ── Join / create room ────────────────────────────────────────
            let state_updated_sender;
            {
                let mut rooms = rooms_state.lock().await;
                let room = rooms.0.entry(room_name.clone()).or_insert_with(|| {
                    let (sender, _) = broadcast::channel(1);
                    GameRoom::new(GameState::new_waiting(6), sender)
                });
                room.last_activity = Instant::now();
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
                let mut snapshot = room.state.to_client_state(my_seat);
                stamp_snapshot(&mut snapshot, room);
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
                                    Arc::clone(&db_handle),
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
                                let mut snapshot = room.state.to_client_state(seat);
                                stamp_snapshot(&mut snapshot, room);
                                let _ = stream
                                    .send(Message::Text(serde_json::to_string(&snapshot).unwrap()))
                                    .await;
                            } else {
                                break; // player was removed
                            }
                        } else {
                            // Room was deleted (stale cleanup). Notify client and disconnect.
                            let _ = stream
                                .send(Message::Text("{\"Err\":\"RoomExpired\"}".to_string()))
                                .await;
                            break;
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
    db: Arc<StdMutex<Connection>>,
    stream: &mut DuplexStream,
    sender: &Sender<()>,
    lobby_sender: &Sender<()>,
) {
    if let Message::Text(text) = message {
        if text == "ping" { return; }
        println!("[{}] {}: {}", room_name, player_name, text);
        match serde_json::from_str::<ClientAction>(&text) {
            Ok(action) => {
                // Handle Rematch separately — it's a room-level action, not a game state action.
                if matches!(action, ClientAction::Rematch) {
                    let code = {
                        let mut rooms = rooms_state.lock().await;
                        let existing_code = rooms.0.get(room_name).and_then(|r| {
                            if matches!(r.state.phase, game::GamePhase::GameOver { .. }) {
                                r.rematch_code.clone()
                            } else {
                                None
                            }
                        });
                        // Check phase validity
                        match rooms.0.get(room_name) {
                            Some(r) if !matches!(r.state.phase, game::GamePhase::GameOver { .. }) => {
                                let _ = stream.send(Message::Text("{\"Err\":\"WrongPhase\"}".to_string())).await;
                                return;
                            }
                            None => return,
                            _ => {}
                        }
                        if let Some(code) = existing_code {
                            code
                        } else {
                            let code = generate_room_code();
                            let (tx, _) = broadcast::channel(1);
                            rooms.0.insert(code.clone(), GameRoom::new(GameState::new_waiting(6), tx));
                            rooms.0.get_mut(room_name).unwrap().rematch_code = Some(code.clone());
                            code
                        }
                    };
                    let _ = lobby_sender.send(());
                    let msg = format!("{{\"rematch_code\":\"{}\"}}", code);
                    let _ = stream.send(Message::Text(msg)).await;
                    return;
                }

                let is_start = matches!(action, ClientAction::StartGame { .. });
                // Apply the action, then note if the game just ended so we can
                // persist after releasing the rooms lock (the DB lock is sync).
                struct PersistInfo {
                    room_code: String,
                    started_at: SystemTime,
                    state_snapshot: GameState,
                    advanced: bool,
                    expansion: bool,
                }
                let mut to_persist: Option<PersistInfo> = None;
                let result = {
                    let mut rooms = rooms_state.lock().await;
                    let room = rooms.0.get_mut(room_name).ok_or(ActionError::InvalidSeat);
                    match room {
                        Ok(room) => {
                            match room.state.seat_of(player_name) {
                                Some(seat) => {
                                    room.last_activity = Instant::now();
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
                                        ClientAction::Rematch => unreachable!(),
                                    };
                                    if r.is_ok() {
                                        // Stamp start-of-game bookkeeping.
                                        if let ClientAction::StartGame { advanced, expansion } = &action {
                                            if room.started_at.is_none() {
                                                room.started_at = Some(SystemTime::now());
                                                room.started_advanced = *advanced;
                                                room.started_expansion = *expansion;
                                            }
                                        }
                                        // If the game just ended and hasn't been saved, queue a save.
                                        if matches!(room.state.phase, game::GamePhase::GameOver { .. })
                                            && !room.persisted
                                            && !room.skip_persistence
                                        {
                                            to_persist = Some(PersistInfo {
                                                room_code: room_name.to_string(),
                                                started_at: room.started_at.unwrap_or_else(SystemTime::now),
                                                state_snapshot: room.state.clone(),
                                                advanced: room.started_advanced,
                                                expansion: room.started_expansion,
                                            });
                                        }
                                    }
                                    r
                                }
                                None => Err(ActionError::InvalidSeat),
                            }
                        }
                        Err(e) => Err(e),
                    }
                };

                // Persist outside the rooms lock, then write results back.
                if let Some(info) = to_persist {
                    let db_result = {
                        let mut conn = db.lock().expect("db mutex poisoned");
                        db::save_game(
                            &mut conn,
                            &info.room_code,
                            info.started_at,
                            SystemTime::now(),
                            &info.state_snapshot,
                            info.advanced,
                            info.expansion,
                        )
                        .and_then(|id| {
                            let highlights = compute_highlights(&conn, id, &info.state_snapshot)?;
                            Ok((id, highlights))
                        })
                    };
                    match db_result {
                        Ok((id, highlights)) => {
                            let mut rooms = rooms_state.lock().await;
                            if let Some(room) = rooms.0.get_mut(room_name) {
                                room.persisted = true;
                                room.game_record_id = Some(id);
                                room.post_game_highlights = Some(highlights);
                            }
                        }
                        Err(e) => {
                            eprintln!("[{}] failed to persist game: {}", room_name, e);
                        }
                    }
                }

                match result {
                    Ok(()) => {
                        // Broadcast to all other clients in the room.
                        let _ = sender.send(());
                        // Send snapshot back to acting client too.
                        let rooms = rooms_state.lock().await;
                        if let Some(room) = rooms.0.get(room_name) {
                            if let Some(seat) = room.state.seat_of(player_name) {
                                let mut snapshot = room.state.to_client_state(seat);
                                stamp_snapshot(&mut snapshot, room);
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

// ─── Post-game highlight computation ─────────────────────────────────────────

fn compute_highlights(
    conn: &Connection,
    game_id: i64,
    state: &GameState,
) -> rusqlite::Result<Vec<PostGameHighlight>> {
    use crate::scoring::score_player_detailed;
    // Global average is the same for every seat — compute once.
    let prev_global_avg = db::previous_global_avg(conn, game_id)?;
    let mut out = Vec::new();
    for p in &state.players {
        let total: u32 = score_player_detailed(p).iter().map(|e| e.points).sum();
        let card_sum: u32 = p.tableau.iter().map(|c| c.number as u32).sum();
        let previous = db::previous_personal_best(conn, &p.name, game_id)?;
        let personal_best = previous.map_or(true, |prev| total > prev);
        let rank = db::rank_of_score(conn, total, card_sum).ok();
        let prev_player_avg = db::previous_player_avg(conn, &p.name, game_id)?;
        out.push(PostGameHighlight {
            seat: p.seat,
            name: p.name.clone(),
            score: total,
            all_time_rank: rank,
            personal_best,
            previous_best: previous,
            previous_player_avg: prev_player_avg,
            previous_global_avg: prev_global_avg,
        });
    }
    Ok(out)
}

// ─── Stats endpoints ─────────────────────────────────────────────────────────

#[get("/api/stats/player/<name>")]
async fn stats_player(name: &str, db: &State<Db>) -> Json<db::PlayerStats> {
    let conn = db.0.lock().expect("db mutex poisoned");
    let stats = db::player_stats(&conn, name).unwrap_or_else(|e| {
        eprintln!("stats_player error: {}", e);
        db::PlayerStats {
            name: name.to_string(),
            games_played: 0,
            wins: 0,
            win_rate: 0.0,
            high_score: 0,
            high_score_game_id: None,
            avg_score: 0.0,
            placements: vec![0; 6],
            recent: vec![],
            first_game_at: None,
            last_game_at: None,
            recent_avg: None,
            total_play_time_secs: 0,
            longest_win_streak: 0,
            scoring_rate: None,
            best_card_score: None,
            avg_by_player_count: vec![],
            top_cards: vec![],
            biome_preference: vec![],
            biome_preference_regions: vec![],
            biome_preference_sanctuaries: vec![],
            head_to_head: vec![],
            score_history: vec![],
            avg_sanctuaries_per_game: 0.0,
            sanctuary_scoring_rate: None,
            best_sanctuary_score: None,
            top_sanctuaries: vec![],
            avg_by_sanctuary_count: vec![],
        }
    });
    Json(stats)
}

#[get("/api/stats/leaderboard?<limit>&<player>")]
async fn stats_leaderboard(
    limit: Option<u32>,
    player: Option<String>,
    db: &State<Db>,
) -> Json<Vec<db::LeaderboardEntry>> {
    let limit = limit.unwrap_or(10).min(100);
    let player = player
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let conn = db.0.lock().expect("db mutex poisoned");
    Json(db::leaderboard(&conn, limit, player).unwrap_or_default())
}

#[get("/api/stats/games?<limit>")]
async fn stats_games(limit: Option<u32>, db: &State<Db>) -> Json<Vec<db::GameSummary>> {
    let limit = limit.unwrap_or(20).min(100);
    let conn = db.0.lock().expect("db mutex poisoned");
    Json(db::recent_games(&conn, limit).unwrap_or_default())
}

#[get("/api/stats/games/<id>")]
async fn stats_game_detail(id: i64, db: &State<Db>) -> Option<Json<db::GameDetail>> {
    let conn = db.0.lock().expect("db mutex poisoned");
    db::game_detail(&conn, id).ok().flatten().map(Json)
}

// ─── SPA routes (stats pages served by index.html) ───────────────────────────
//
// The client-side router reads `location.pathname` to pick which view to
// render. Any path under `/stats/...` returns `index.html` so the user can
// deep-link, refresh, or share a stats URL.

fn client_file_path(name: &str) -> PathBuf {
    let dir = std::env::var("YONDER_CLIENT_DIR").unwrap_or_else(|_| "../yonder-client".to_string());
    PathBuf::from(dir).join(name)
}

#[get("/stats")]
async fn stats_root() -> Option<NamedFile> {
    NamedFile::open(client_file_path("index.html")).await.ok()
}

#[get("/stats/<_path..>", rank = 20)]
async fn stats_spa(_path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(client_file_path("index.html")).await.ok()
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
    let mut room = GameRoom::new(GameState::new_demo(), sender);
    room.skip_persistence = true;
    rooms.0.insert(room_name.clone(), room);
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
        // SPA routes (/, /stats/*) serve index.html — never cache them.
        let is_html_route = path == "/" || path.starts_with("/stats");
        if is_html_route
            || path.ends_with(".html")
            || path.ends_with(".js")
            || path.ends_with(".css")
        {
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

// ─── Stale room cleanup ──────────────────────────────────────────────────────

const STALE_TIMEOUT_DEFAULT_SECS: u64 = 2 * 60 * 60;  // 2 hours (waiting/game over)
const STALE_TIMEOUT_PLAYING_SECS: u64 = 48 * 60 * 60; // 48 hours (in-progress games)
const CLEANUP_INTERVAL_SECS: u64 = 5 * 60;             // check every 5 minutes

struct CleanupFairing;

#[rocket::async_trait]
impl Fairing for CleanupFairing {
    fn info(&self) -> Info {
        Info { name: "Stale Room Cleanup", kind: Kind::Liftoff }
    }

    async fn on_liftoff(&self, rocket: &rocket::Rocket<rocket::Orbit>) {
        let rooms = Arc::clone(rocket.state::<Arc<Mutex<Rooms>>>().unwrap());
        let lobby_sender = rocket.state::<LobbySender>().unwrap().0.clone();

        rocket::tokio::spawn(async move {
            loop {
                rocket::tokio::time::sleep(
                    std::time::Duration::from_secs(CLEANUP_INTERVAL_SECS)
                ).await;

                let mut rooms = rooms.lock().await;
                let before = rooms.0.len();
                rooms.0.retain(|name, room| {
                    let timeout = match &room.state.phase {
                        game::GamePhase::Playing(_) | game::GamePhase::AdvancedSetup { .. }
                            => STALE_TIMEOUT_PLAYING_SECS,
                        _ => STALE_TIMEOUT_DEFAULT_SECS,
                    };
                    let stale = room.last_activity.elapsed().as_secs() >= timeout;
                    if stale {
                        println!("Removing stale room '{}'", name);
                        // Notify connected clients so they detect the missing room.
                        let _ = room.sender.send(());
                    }
                    !stale
                });
                let removed = before - rooms.0.len();
                if removed > 0 {
                    println!("Cleaned up {} stale room(s)", removed);
                    let _ = lobby_sender.send(());
                }
            }
        });
    }
}

// ─── Launch ───────────────────────────────────────────────────────────────────

#[launch]
fn rocket() -> _ {
    let client_dir = std::env::var("YONDER_CLIENT_DIR")
        .unwrap_or_else(|_| "../yonder-client".to_string());
    println!("Serving static files from: {}", client_dir);

    let db_path = std::env::var("YONDER_DB_PATH").unwrap_or_else(|_| "yonder.db".to_string());
    println!("Using DB file: {}", db_path);
    let db_conn = db::open(&db_path).expect("failed to open/init DB");

    rocket::build()
        .attach(ResponseFairing)
        .attach(CleanupFairing)
        .mount("/", routes![
            health, play_game, demo_game, list_rooms, lobby_ws,
            stats_player, stats_leaderboard, stats_games, stats_game_detail,
            stats_root, stats_spa,
        ])
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
        .manage(Db(Arc::new(StdMutex::new(db_conn))))
}
