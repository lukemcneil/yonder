# Faraway — TODO Board

> **How to use:** Pick the next `[ ]` task from the current milestone. Implement it. Test it. Mark it `[x]`. Commit with the TODO update. See CONTRIBUTING.md for the full workflow.

---

## Current Status

**Active milestone:** M1 — Server foundation
**Next task:** Set up Rust project

---

## M1 — Server Foundation

- [ ] Create `faraway-server/` Rust project with Rocket 0.5 + rocket_ws
- [ ] Add card data structs to `faraway-server/src/cards.rs` (base game: regions 1–68, sanctuaries 1–45, ported from `cards/cards.ts`)
- [ ] Add `GameState` and `PlayerState` structs in `faraway-server/src/game.rs`
- [ ] Implement WebSocket endpoint `/game/<room>?player=<name>` in `main.rs`
- [ ] Implement multi-room management (`HashMap<String, GameRoom>` with Tokio broadcast channels)
- [ ] Implement `GameState` → JSON serialisation (serde) with per-player hidden hand logic
- [ ] Verify: two browser tabs can connect to the same room and both receive the initial state broadcast

## M2 — Core Game Loop

- [ ] Implement `StartGame` action: validate player count, transition from WaitingForPlayers → Playing
- [ ] Implement setup: shuffle deck, deal 3 cards to each player, reveal market (N+1 cards)
- [ ] Implement `PlayCard` action: place card face-down, wait for all players, then reveal simultaneously
- [ ] Implement sanctuary eligibility check (played number > previous number)
- [ ] Implement sanctuary draw: `1 + visible_clues` cards drawn
- [ ] Implement `ChooseSanctuary` action: keep 1, discard rest to bottom of sanctuary deck
- [ ] Implement `DraftCard` action: in ascending-number order, pick 1 from market
- [ ] Implement round end: discard leftover market card, refill market, advance round counter
- [ ] Implement round 8 special case: no draft after round 8, transition to scoring
- [ ] Verify: full 8-round game can complete end-to-end via raw WebSocket messages (use wscat or similar)

## M3 — Scoring Engine

- [ ] Implement scoring in `faraway-server/src/scoring.rs`
- [ ] Implement right-to-left reveal loop
- [ ] Implement prerequisite validation (count wonder icons in visible cards + sanctuaries)
- [ ] Implement flat fame scoring
- [ ] Implement per-icon scoring (stone, chimera, thistle)
- [ ] Implement per-colour scoring (red, green, blue, yellow)
- [ ] Implement per-colour-pair scoring (e.g. yellow+green)
- [ ] Implement per-night scoring
- [ ] Implement per-clue scoring
- [ ] Implement wonder-set scoring (floor of min of 3 wonder counts)
- [ ] Implement colour-set scoring (floor of min of 4 colour counts)
- [ ] Implement sanctuary own-fame scoring (runs after all 8 region cards)
- [ ] Implement tiebreaker (lowest sum of card numbers)
- [ ] Write unit tests for scoring engine covering all 8 scoring types + prerequisites
- [ ] Verify: manually calculate a known game's score and confirm server matches

## M4 — Frontend Lobby & Game Board

- [ ] Create `faraway-client/` directory with `index.html`, `game.js`, `style.css`
- [ ] Symlink or copy card images from `cards/public/` into `faraway-client/`
- [ ] Implement lobby screen (name, room, player count selector, connect button)
- [ ] Implement WebSocket connection and full-state JSON parsing
- [ ] Render opponent tableaux (card images in order, hand size count)
- [ ] Render shared market (cards, deck count)
- [ ] Render your own tableau and sanctuaries
- [ ] Render your hand (3 cards, clickable during ChoosingCards phase)
- [ ] Implement status bar (current phase, whose turn, instructions)
- [ ] Implement `PlayCard` UI: click hand card → sends action
- [ ] Implement sanctuary chooser modal: shows drawn cards, click to keep 1
- [ ] Implement draft picker: highlight your turn, click market card to draft
- [ ] Verify with playwright-cli: full lobby → game flow in headed mode

## M5 — Scoring UI

- [ ] Implement scoring screen: cards reveal right-to-left one at a time
- [ ] Show score gained per card with brief explanation (e.g. "+12 fame: 3 Uddu Stone × 4")
- [ ] Show running total per player as cards reveal
- [ ] Show final leaderboard with all scores and tiebreaker display
- [ ] Verify with playwright-cli: scoring animation plays correctly

## M6 — Polish

- [ ] Reconnect on refresh (store room + player name in URL hash)
- [ ] Spectator mode (join after game started → read-only view, no actions)
- [ ] Card tooltip on hover (show card data: wonders, quest, scoring condition)
- [ ] Advanced setup variant (deal 5, keep 3 — add toggle on lobby screen)
- [ ] Game cleanup: remove rooms after game ends + 1 hour idle timeout

---

## Discovered / Backlog

_(Add tasks here as they are discovered during implementation)_

- [ ] Decide: serve frontend as static files from Rocket, or run separately?
- [ ] Decide: how to handle a player disconnecting mid-game?

---

## Completed

- [x] 2026-03-01 — Created CLAUDE.md, TODO.md, CONTRIBUTING.md, RULES.md, docs/design.md
- [x] 2026-03-01 — Downloaded official English rulebook (rules-en.pdf)
- [x] 2026-03-01 — Moved Faraway_analysis.xlsx into project root
- [x] 2026-03-01 — Initialised git repository
