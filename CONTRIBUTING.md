# Contributing & Development Guide

> **For Claude Code sessions:** You should have arrived here from `CLAUDE.md`. This file explains how to work, what to build next, and how to test everything.

---

## How to Get Started (New Session)

1. Read `TODO.md` — find the next unchecked `[ ]` task in the current milestone.
2. Read `docs/design.md` for architecture context if needed.
3. Read `RULES.md` if you need to understand game mechanics.
4. Pick up the task, implement it, test it, then commit.

**The workflow for every task:**
```
1. Mark task as in-progress (mentally — no file change needed)
2. Implement the change
3. Test it (see Testing section below)
4. Mark task [x] in TODO.md
5. Add any newly discovered tasks to the Discovered section of TODO.md
6. Update docs — if anything in RULES.md, docs/design.md, or CONTRIBUTING.md
   is wrong, incomplete, or out of date, fix it now
7. Commit everything together: git commit -m "feat: <task description>"
   Code + TODO.md + any doc updates all in one commit
```

**Before ending a session:** always run `git status` and commit any remaining changes. A new Claude instance starts from git — if it's not committed, it's lost.

---

## Project Structure

```
yonder/
├── RULES.md                  # Complete game rules reference
├── TODO.md                   # Task board — source of truth for what to do next
├── CONTRIBUTING.md           # This file
├── Faraway_analysis.xlsx     # Card data spreadsheet (source of truth for card stats)
├── rules-en.pdf              # Official English rulebook PDF
├── docs/
│   └── design.md             # Full architecture design document
├── yonder-server/           # Rust WebSocket game server
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # Rocket server, WS endpoint, room management
│       ├── game.rs           # GameState, PlayerState, phase transitions
│       ├── cards.rs          # Card definitions
│       └── scoring.rs        # Reverse-scoring engine
└── yonder-client/           # Vanilla HTML/CSS/JS frontend
    ├── index.html
    ├── game.js
    ├── style.css
    ├── region/               # Region card images: tile001.jpg ... tile068.jpg
    └── sanctuary/            # Sanctuary card images: tile001.jpg ... tile045.jpg
```

---

## Cloning (first time)

```bash
git clone <repo-url>
```

Card images are in `yonder-client/region/` and `yonder-client/sanctuary/`.

---

## Running the Project

### Server

```bash
cd yonder-server
cargo run
# Server starts on http://localhost:8000
# WebSocket: ws://localhost:8000/game/<room>?player=<name>
```

> **Note:** Port 8000 may be in use (e.g. by the `cards/` Nuxt app). To use a different port,
> set `ROCKET_PORT` when running the binary directly — `cargo run` does not inherit it:
> ```bash
> cargo build && ROCKET_PORT=8001 ./target/debug/yonder-server
> ```

### Frontend

Serve the client as static files. The simplest way:

```bash
cd yonder-client
python3 -m http.server 3001
# Open http://localhost:3001
```

Or configure Rocket to serve static files from `yonder-client/` — see `yonder-server/src/main.rs`.

---

## Testing

### Server unit tests

```bash
cd yonder-server
cargo test
```

All scoring logic must have unit tests in `scoring.rs`. When adding a new scoring type, add a test.

### UI testing with playwright-cli (REQUIRED for all frontend tasks)

Use `playwright-cli` in **headed mode** (it opens a visible browser window) to test UI changes:

```bash
# Open the app in a browser
playwright-cli open http://localhost:3001

# After making changes, take a screenshot
playwright-cli screenshot

# Navigate
playwright-cli goto http://localhost:3001

# Click a button by its ref (get ref from snapshot)
playwright-cli snapshot
playwright-cli click <ref>

# Type into an input
playwright-cli fill <ref> "some text"
```

**Standard UI test flow for frontend tasks:**

1. Start the server: `cd yonder-server && cargo run`
2. Start the frontend: `cd yonder-client && python3 -m http.server 3001`
3. Open playwright-cli: `playwright-cli open http://localhost:3001`
4. Take a snapshot: `playwright-cli snapshot`
5. Interact and verify the feature works
6. Take a screenshot for reference: `playwright-cli screenshot`

### Manual end-to-end test

For game logic, open two browser tabs to the same room and play through a full game.

---

## Architecture Reference

See `docs/design.md` for full design. Key points:

- **Server is authoritative.** All game logic lives in Rust. The frontend only displays state and sends actions.
- **Full-state broadcasts.** After every successful action, the server broadcasts the full `GameState` JSON to all players in the room. Clients replace their entire state on each message.
- **Per-player snapshots.** Each player only sees their own hand. The server sends a tailored view.
- **Rollback on error.** If an action fails validation, state is unchanged. Client receives `{"Err": "ErrorName"}`.
- **Named rooms.** Players connect to `ws://host/game/<room_name>?player=<name>`. First N connections fill seats; game starts when all seats are filled or host calls `StartGame`.

### WebSocket Message Formats

**Client → Server:**
```json
{ "action": "PlayCard", "card_index": 2 }
{ "action": "ChooseSanctuary", "sanctuary_index": 1 }
{ "action": "DraftCard", "market_index": 0 }
{ "action": "StartGame" }
```

**Server → Client (success):**
Full `GameState` JSON snapshot (personalised: includes `my_hand`, hides opponents' hands).

**Server → Client (error):**
```json
{ "Err": "NotYourTurn" }
{ "Err": "InvalidCardIndex" }
```

### Card Data

The canonical card data is in `Faraway_analysis.xlsx`. The Rust server uses `yonder-server/src/cards.rs`. Base game only (regions 1–68, sanctuaries 1–45).

### Scoring Engine

See `RULES.md` for the full scoring algorithm. The engine lives in `yonder-server/src/scoring.rs`. Key invariant: **only cards to the RIGHT of the card being scored (plus all sanctuaries) are visible** during scoring.

---

## Reference Architecture

The server/client pattern is modelled after `~/personal/shields-up-engineering`. When in doubt:

```bash
ls ~/personal/shields-up-engineering/
```

Key files to reference:
- `shields-up-engineering-server/src/main.rs` — WS endpoint, room management
- `shields-up-engineering-server/src/game.rs` — state machine pattern
- `shields-up-engineering-client/game.js` — how the client handles WS messages

---

## Commit Style

```
feat: implement PlayCard action and simultaneous reveal
fix: sanctuary clue count was off-by-one
test: add scoring tests for wonder-set type
docs: update CONTRIBUTING with new test steps
chore: mark M1 tasks complete in TODO.md
```

Always include `TODO.md` in the commit when marking tasks complete.
