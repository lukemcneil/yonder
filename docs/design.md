# Yonder — Architecture Design

*Date: 2026-03-01*

---

## Overview

An online multiplayer implementation of the Faraway board game (Catch Up Games / Pandasaurus Games, 2023). Players can play 2–6 player games in a browser, communicating via WebSocket with a Rust game server.

**Design goals:**
- Server-authoritative game logic (no cheating, no desyncs)
- Real-time multiplayer (all players see state changes instantly)
- Simple to deploy and run
- Modelled after `~/personal/shields-up-engineering` architecture

---

## Directory Structure

```
yonder/
├── RULES.md                  # Complete game rules reference
├── TODO.md                   # Task board
├── CONTRIBUTING.md           # Developer workflow guide
├── Faraway_analysis.xlsx     # Card data source of truth
├── rules-en.pdf              # Official English rulebook
├── docs/
│   └── design.md             # This file
├── yonder-server/           # Rust WebSocket game server
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # Server entry, WS endpoint, room management
│       ├── game.rs           # GameState, PlayerState, action handling
│       ├── cards.rs          # Card definitions
│       └── scoring.rs        # Scoring engine
└── yonder-client/           # Vanilla HTML/CSS/JS frontend
    ├── index.html
    ├── game.js
    └── style.css
```

---

## Server Architecture

**Stack:** Rust, Rocket 0.5, rocket_ws, Tokio, serde_json

### WebSocket Endpoint

```
GET /game/<room_name>?player=<player_name>
```

- Creates the room if it doesn't exist
- Assigns the connecting player a seat (0–5)
- Returns the current `GameState` snapshot immediately on connect
- Broadcasts updated `GameState` to all clients in the room after every successful action

### Room Management

```rust
struct GameRoom {
    state: GameState,
    sender: broadcast::Sender<()>,  // triggers broadcast to all clients
}

struct Rooms(HashMap<String, GameRoom>);
// Wrapped in Arc<Mutex<Rooms>> and managed by Rocket
```

### Concurrency Pattern (from shields-up)

Each WebSocket connection runs in its own async task:
```rust
select! {
    msg = stream.next() => {
        // parse action, apply to GameState, broadcast if Ok
    }
    _ = state_updated_receiver.recv() => {
        // send full GameState snapshot to this client
    }
}
```

### Per-Player State Snapshots

The server sends a **personalised** snapshot to each player — they see their own hand in full, but only the hand *size* of opponents. This prevents hand snooping.

---

## Game State Machine

### Top-Level State

```rust
enum GamePhase {
    WaitingForPlayers { needed: usize },
    Playing(RoundPhase),
    Scoring(ScoringState),
    GameOver { scores: Vec<PlayerScore> },
}
```

### Round Phases

```
ChoosingCards
  → All players select 1 card from hand (simultaneous)
  → When all have played: transition to RevealingAndSanctuaries

RevealingAndSanctuaries
  → Cards revealed simultaneously
  → Each eligible player draws sanctuary cards
  → If any player has pending sanctuary choice: SanctuaryChoice { seat }
  → Otherwise: Drafting

SanctuaryChoice { seat }
  → Player at `seat` chooses 1 sanctuary to keep
  → When done: check next eligible player, or → Drafting

Drafting { order: Vec<usize>, current: usize }
  → Player at order[current] picks from market
  → Advances through order
  → When last player drafts: discard leftover, refill market, advance round
  → If round == 8: → Scoring
  → Else: → ChoosingCards
```

### Scoring State

```rust
struct ScoringState {
    // Which card index (0–7) we're currently revealing, counting down from 7
    revealing_index: usize,
    // Scores accumulated so far, per player
    scores: Vec<Vec<CardScore>>,  // scores[player][card]
}
```

Scoring runs automatically (no player action needed). Each step:
1. Reveal tableau[revealing_index] for each player
2. Compute score for that card given visible context
3. Broadcast updated state (clients animate the reveal)
4. Move to next index (revealing_index - 1)
5. After index 0, score sanctuaries, transition to GameOver

---

## Card Data

### Region Card

```rust
pub struct RegionCard {
    pub number: u8,             // 1–68 (unique)
    pub biome: Biome,           // Red/Green/Blue/Yellow
    pub night: bool,
    pub clue: bool,
    pub wonders: WonderCount,   // icons on the card
    pub quest: WonderCount,     // prerequisite wonder icons to score
    pub fame: Fame,             // scoring condition
}

pub struct WonderCount {
    pub stone: u8,
    pub chimera: u8,
    pub thistle: u8,
}

pub enum Biome {
    Red, Green, Blue, Yellow, Colorless,
}

pub enum Fame {
    None,
    Flat(u32),
    PerIcon { icon: Wonder, score_per: u32 },
    PerColour { biome: Biome, score_per: u32 },
    PerColourPair { biome1: Biome, biome2: Biome, score_per: u32 },
    PerNight { score_per: u32 },
    PerClue { score_per: u32 },
    PerWonderSet { score_per: u32 },
    PerColourSet { score_per: u32 },
}
```

### Sanctuary Card

```rust
pub struct SanctuaryCard {
    pub tile: u8,           // 1–45 (image filename index)
    pub biome: Biome,       // counts as this biome for scoring
    pub night: bool,
    pub clue: bool,
    pub wonders: WonderCount,
    pub fame: Fame,         // own scoring condition (may be None)
}
```

---

## Scoring Engine

File: `yonder-server/src/scoring.rs`

```rust
fn score_card(
    card: &RegionCard,
    visible_regions: &[&RegionCard],  // cards to the RIGHT (already revealed)
    sanctuaries: &[&SanctuaryCard],
) -> u32
```

Algorithm:
1. Build `visible_context` = visible_regions + sanctuaries
2. Check prerequisites: sum wonder icons in context, compare to `card.quest`
3. If prerequisites not met → return 0
4. Otherwise compute score based on `card.fame`:
   - Flat: return value directly
   - PerIcon: count icon in context (including card itself? NO — only visible context to the right + sanctuaries, NOT the card being scored)
   - PerColour: count cards of that biome in context
   - PerColourPair: count cards of either biome in context
   - PerNight: count night cards in context
   - PerClue: count clue icons in context
   - PerWonderSet: floor(min(stone, chimera, thistle)) in context
   - PerColourSet: floor(min(red, green, blue, yellow)) in context

**Important:** When scoring card at index `i`, "visible context" = cards at indices `i+1..7` + all sanctuaries. The card itself is NOT in the context (its own icons don't count toward its own prerequisites or scoring).

After all 8 region cards, score each Sanctuary's `fame` using the full tableau (all 8 cards + all other sanctuaries).

---

## WebSocket Protocol

### Client → Server Actions

```typescript
// Start the game (host only)
{ action: "StartGame" }

// Play a card from hand (during ChoosingCards phase)
{ action: "PlayCard", card_index: number }

// Choose a sanctuary to keep
{ action: "ChooseSanctuary", sanctuary_index: number }

// Draft a card from the market
{ action: "DraftCard", market_index: number }
```

### Server → Client (on success)

Full `ClientGameState` JSON snapshot (personalised per player):

```typescript
{
  phase: "ChoosingCards" | "SanctuaryChoice" | "Drafting" | "Scoring" | "GameOver",
  round: number,           // 1–8
  my_seat: number,
  my_hand: RegionCard[],   // only shown to this player
  players: {
    seat: number,
    name: string,
    hand_size: number,     // opponents: count only
    tableau: (RegionCard | null)[],  // null = face-down (during scoring)
    sanctuaries: SanctuaryCard[],
    played_this_round: RegionCard | null,  // revealed card this round
    score: number | null,  // null until scoring complete
  }[],
  market: RegionCard[],
  deck_size: number,
  sanctuary_deck_size: number,
  draft_order: number[],         // seats in draft order
  current_drafter: number | null,
  sanctuary_choices: SanctuaryCard[] | null,  // if I need to choose
  scoring_state: ScoringState | null,
}
```

### Server → Client (on error)

```json
{ "Err": "NotYourTurn" }
{ "Err": "InvalidCardIndex" }
{ "Err": "NotInChoosingPhase" }
{ "Err": "GameAlreadyStarted" }
```

---

## Frontend Architecture

**Vanilla HTML/CSS/JS.** No framework, no build step.

### Screens

1. **Lobby** (`#lobby` div)
   - Inputs: player name, room name, player count
   - Button: Connect
   - Status: "Waiting for players... (2/4)"

2. **Game Board** (`#game-board` div, hidden until game starts)
   - Opponent rows: tableau + sanctuaries + name/hand-size
   - Market row: face-up cards + deck count
   - My row: tableau + sanctuaries
   - My hand: clickable cards
   - Status bar: current phase + instructions

3. **Sanctuary Modal** (`#sanctuary-modal` div)
   - Shows drawn sanctuary cards
   - Click one to keep

4. **Scoring Screen** (`#scoring-screen` div)
   - Animated right-to-left reveals
   - Running scores per player
   - Final leaderboard

### State Management

```javascript
let ws = null;          // WebSocket connection
let gameState = null;   // Latest full state from server
let mySeat = null;      // My seat index

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.Err) {
    showError(msg.Err);
    return;
  }
  gameState = msg;
  render(gameState);
};
```

### Card Images

- Region cards: `/region/tile001.jpg` … `/region/tile068.jpg`
- Sanctuary cards: `/sanctuary/tile001.jpg` … `/sanctuary/tile045.jpg`

These are served from `yonder-client/`.

---

## Milestones

| Milestone | Description |
|---|---|
| M1 | Server foundation: Rocket + rocket_ws, room management, card structs, JSON broadcast |
| M2 | Core game loop: full 8-round game playable via raw WebSocket |
| M3 | Scoring engine: all 8 scoring types, unit tested |
| M4 | Frontend: lobby + game board, fully playable in browser |
| M5 | Scoring UI: animated reveal, leaderboard |
| M6 | Polish: reconnect, spectators, tooltips, advanced setup |

**Done when:** Two browsers can play a complete 2-player Faraway game from lobby through scoring with correct scores.

---

## Result Persistence

Completed games are written to a local SQLite file (default `yonder.db`, or
`YONDER_DB_PATH`). All DB code lives in `yonder-server/src/db.rs` and uses
`rusqlite` (bundled build, no system SQLite required). A single
`Arc<std::sync::Mutex<Connection>>` is managed by Rocket state; we never hold
the DB lock across an `.await`.

### Schema

```
games(id, room_code, started_at, finished_at, player_count, advanced, expansion)
game_players(id, game_id, seat, name, name_lower, final_score,
             card_number_sum, placement,
             region_cards_json, sanctuary_cards_json, score_breakdown_json)
```

- Player identity is a case-insensitive name match via `name_lower`.
- Card choices are stored as JSON arrays of card numbers (region number or
  sanctuary tile id). The card definitions in `cards.rs` are the source of
  truth — we rehydrate by number when displaying.
- `score_breakdown_json` is the output of `score_player_detailed`, so stats
  views never recompute.
- Migrations are managed via `PRAGMA user_version`; current schema version is 1.

### Persistence hook

`GameRoom` carries `started_at`, `started_advanced`, `started_expansion`,
`persisted`, `skip_persistence`, `game_record_id`, and
`post_game_highlights`. After any successful action whose resulting
`GamePhase` is `GameOver` (and `persisted` is still false), `main.rs` calls
`db::save_game` and then `compute_highlights`, stashing the results back on
the room. The next `to_client_state` call is decorated via `stamp_snapshot`
with the `game_record_id` and per-seat `post_game_highlights`.

Demo rooms created by `GET /demo/<room>` set `skip_persistence = true` so
they never pollute stats.

### HTTP API

- `GET /api/stats/player/<name>` — `PlayerStats { games_played, wins,
  high_score, high_score_game_id, avg_score, placements[6], recent[10] }`.
- `GET /api/stats/leaderboard?limit=10` — top scores with player name,
  `game_id`, `finished_at`, region/sanctuary card numbers; ties assigned
  the same rank.
- `GET /api/stats/games?limit=20` — recent games with winner name + score.
- `GET /api/stats/games/<id>` — full `GameDetail` with every seat's
  tableau, sanctuaries, and per-card score breakdown.

### Frontend stats

`index.html` adds a `Stats` button on the lobby that reveals the stats
SPA. Each tab has a real URL so it can be bookmarked / shared / refreshed:

| URL                          | Page                                  |
|------------------------------|---------------------------------------|
| `/stats`, `/stats/me`        | My Stats (case-insensitive name lookup)
| `/stats/player/<name>`       | My Stats pre-filled for a given player
| `/stats/high-scores`         | All-time high scores list             |
| `/stats/leaderboard`         | (alias of `/stats/high-scores`)       |
| `/stats/recent`              | Recent games list                     |
| `/stats/games/<id>`          | Read-only detail view of a saved game |

The client-side router lives in `game.js` (`parseRoute` / `navigate` /
`applyRoute`, driven by `history.pushState` + `popstate`). Unknown paths
under `/stats/...` are served `index.html` by the server so the router
can take over on refresh / deep link. `<base href="/">` is set in
`index.html` so relative asset paths (`region/tile001.jpg`, etc.) still
resolve from root on deep SPA URLs.

Clicking a high-score row or recent-games row pushes `/stats/games/<id>`.
The game detail view renders each player's full tableau using the SAME
`makeScoringRegionCard` / `makeScoringSanctuaryCard` factories the
game-over screen uses, so the two look identical. The High Scores
list includes the full per-card score breakdown for each entry (fetched
in the single leaderboard query) and renders it inline using the same
factories.

### Post-game highlights

On game-over, each player's score row in the in-game leaderboard is
decorated from `state.post_game_highlights`:

- `personal best! (prev N)` / `first game!` — when this game beat their
  previous case-insensitive personal best (or is their first game).
- `#N all-time` — when `all_time_rank <= 10`.
- `+X.X vs your avg (Y.Y)` — delta from the player's pre-game
  historical average (shown when they have at least one prior game).
- `+X.X vs all avg (Y.Y)` — delta from the pre-game global average
  across all saved games (shown when at least one prior game exists).

`db::previous_player_avg` and `db::previous_global_avg` power the
average comparisons and exclude the just-saved game from their `AVG()`
so the baseline is always the "before" state.

---

## Reference

- `~/personal/shields-up-engineering/` — reference server/client architecture
- `Faraway_analysis.xlsx` — authoritative card stats spreadsheet
- `rules-en.pdf` — official English rulebook PDF
- `RULES.md` — rules summary in markdown
