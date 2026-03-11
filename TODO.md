# Faraway — TODO Board

> **How to use:** Pick the next `[ ]` task from the current milestone. Implement it. Test it. Mark it `[x]`. Commit with the TODO update. See CONTRIBUTING.md for the full workflow.

---

## Current Status

**Active milestone:** M6 — Polish
**Next task:** Reconnect on refresh (store room + player name in URL hash)

---

## M1 — Server Foundation

- [x] Create `faraway-server/` Rust project with Rocket 0.5 + rocket_ws
- [x] Add card data structs to `faraway-server/src/cards.rs` (base game: regions 1–68, sanctuaries 1–45, ported from `cards/cards.ts`)
- [x] Add `GameState` and `PlayerState` structs in `faraway-server/src/game.rs`
- [x] Implement WebSocket endpoint `/game/<room>?player=<name>` in `main.rs`
- [x] Implement multi-room management (`HashMap<String, GameRoom>` with Tokio broadcast channels)
- [x] Implement `GameState` → JSON serialisation (serde) with per-player hidden hand logic
- [x] Verify: two browser tabs can connect to the same room and both receive the initial state broadcast

## M2 — Core Game Loop

- [x] Implement `StartGame` action: validate player count, transition from WaitingForPlayers → Playing
- [x] Implement setup: shuffle deck, deal 3 cards to each player, reveal market (N+1 cards)
- [x] Implement `PlayCard` action: place card face-down, wait for all players, then reveal simultaneously
- [x] Implement sanctuary eligibility check (played number > previous number)
- [x] Implement sanctuary draw: `1 + visible_clues` cards drawn (clues from tableau AND sanctuaries)
- [x] Implement `ChooseSanctuary` action: keep 1, discard rest to bottom of sanctuary deck
- [x] Implement `DraftCard` action: in ascending-number order, pick 1 from market
- [x] Implement round end: discard leftover market card, refill market, advance round counter
- [x] Implement round 8 special case: no draft after round 8, transition to scoring
- [x] Verify: full 8-round game can complete end-to-end via raw WebSocket messages (use wscat or similar)

## M3 — Scoring Engine

- [x] Implement scoring in `faraway-server/src/scoring.rs`
- [x] Implement right-to-left reveal loop
- [x] Implement prerequisite validation (count wonder icons in visible cards + sanctuaries)
- [x] Implement flat fame scoring
- [x] Implement per-icon scoring (stone, chimera, thistle)
- [x] Implement per-colour scoring (red, green, blue, yellow)
- [x] Implement per-colour-pair scoring (e.g. yellow+green)
- [x] Implement per-night scoring
- [x] Implement per-clue scoring
- [x] Implement wonder-set scoring (floor of min of 3 wonder counts)
- [x] Implement colour-set scoring (floor of min of 4 colour counts)
- [x] Implement sanctuary own-fame scoring (runs after all 8 region cards)
- [x] Implement tiebreaker (lowest sum of card numbers)
- [x] Write unit tests for scoring engine covering all 8 scoring types + prerequisites
- [x] Verify: manually calculate a known game's score and confirm server matches

## M4 — Frontend Lobby & Game Board

- [x] Create `faraway-client/` directory with `index.html`, `game.js`, `style.css`
- [x] Symlink or copy card images from `cards/public/` into `faraway-client/`
- [x] Implement lobby screen (name, room, player count selector, connect button)
- [x] Implement WebSocket connection and full-state JSON parsing
- [x] Render opponent tableaux (card images in order, hand size count)
- [x] Render shared market (cards, deck count)
- [x] Render your own tableau and sanctuaries
- [x] Render your hand (3 cards, clickable during ChoosingCards phase)
- [x] Implement status bar (current phase, whose turn, instructions)
- [x] Implement `PlayCard` UI: click hand card → sends action
- [x] Implement sanctuary chooser modal: shows drawn cards, click to keep 1
- [x] Implement draft picker: highlight your turn, click market card to draft
- [x] Verify with playwright-cli: full lobby → game flow in headed mode

## M5 — Scoring UI

- [x] Implement scoring screen: cards reveal right-to-left one at a time
- [x] Show score gained per card with brief explanation (e.g. "+12 fame: 3 Uddu Stone × 4")
- [x] Show running total per player as cards reveal
- [x] Show final leaderboard with all scores and tiebreaker display
- [x] Verify with playwright-cli: scoring animation plays correctly

## M6 — Polish

- [x] Reconnect on refresh (store room + player name in URL hash)
- [~] Spectator mode (join after game started → read-only view, no actions) — won't do
- [~] Card tooltip on hover (show card data: wonders, quest, scoring condition) — won't do
- [x] Advanced setup variant (deal 5, keep 3 — add toggle on lobby screen)
- [~] Game cleanup: remove rooms after game ends + 1 hour idle timeout — do later

---

## Discovered / Backlog

_(Add tasks here as they are discovered during implementation)_

- [ ] Decide: serve frontend as static files from Rocket, or run separately?
- [ ] Decide: how to handle a player disconnecting mid-game?
- [x] Mobile support: make the game playable on mobile devices (responsive layout, touch interactions)
- [x] Remove white corners on card images (round corners or mask to match card art)
- [x] Put sanctuary cards on a separate row below region cards in the tableau
- [x] Rework scoring animation: flip all 8 region cards face-down in place, then reveal right-to-left with score animations and running totals below each card (inline, no popup overlay).
- [x] Add scoring table on the side with 9 rows (one per region card + one for all sanctuaries) and a column per player (needs server to send all_score_details for all players).
- [x] Audit all 45 sanctuary cards against images (like the region card audit that found 6 errors).
- [x] When you play a card, show it immediately in your tableau (face-down) instead of waiting for all players to reveal.
- [ ] Draft phase: show each player's highest region number so you can tell draft order at a glance.
- [ ] Live stats sidebar: show the current player's resource counts (stone, chimera, thistle), color counts (red, green, blue, yellow), clue count, and night/day count. Visible during gameplay on the right side.
- [ ] Add option to play with expansion cards
- [x] Live card scores during play: each region card in the tableau shows its current score (based on regions to the right + sanctuaries). Hover/tooltip gives details (e.g. "quest not met yet" or "3 stone × 4 = 12").

---

## Completed

- [x] 2026-03-01 — Created CLAUDE.md, TODO.md, CONTRIBUTING.md, RULES.md, docs/design.md
- [x] 2026-03-01 — Downloaded official English rulebook (rules-en.pdf)
- [x] 2026-03-01 — Moved Faraway_analysis.xlsx into project root
- [x] 2026-03-01 — Initialised git repository
- [x] 2026-03-02 — M1 complete: faraway-server Rust project, cards.rs (68 regions + 45 sanctuaries), game.rs (GameState/PlayerState/phases), scoring.rs (full engine + 11 unit tests), main.rs (WS endpoint, multi-room, per-player JSON snapshots)
- [x] 2026-03-02 — M2 complete: full game loop (StartGame, PlayCard, ChooseSanctuary, DraftCard), round 8 no-draft fix, 42 unit tests, e2e_test.py verifies full 8-round game via WebSocket
- [x] 2026-03-02 — M3 complete: scoring engine (all 8 fame types + prerequisites + sanctuary scoring), 43 unit tests, known-game integration test confirms hand-calculated score of 23
- [x] 2026-03-02 — M4 complete: faraway-client/ (index.html, style.css, game.js), card image symlinks, full game UI — lobby, board, hand, market, opponent panels, sanctuary modal, draft picker, game-over overlay — verified full 8-round game with playwright-cli headed mode
