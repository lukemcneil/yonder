# Faraway — Claude Code Instructions

## Start Here

This is an online multiplayer implementation of the Faraway board game.

**Before doing anything else, read:**
1. `TODO.md` — find the next unchecked `[ ]` task and work on that
2. `CONTRIBUTING.md` — workflow, testing instructions, commit style
3. `docs/design.md` — full architecture design (read if you need context)
4. `RULES.md` — game rules reference (read if you need to understand game mechanics)

## Project Structure

```
faraway/
├── CLAUDE.md             ← you are here
├── TODO.md               ← task board (always start here)
├── CONTRIBUTING.md       ← how to work, test, commit
├── RULES.md              ← complete game rules
├── docs/design.md        ← architecture design
├── rules-en.pdf          ← official rulebook PDF
├── Faraway_analysis.xlsx ← canonical card data spreadsheet
├── cards/                ← existing Nuxt card browser (reference only, don't modify)
├── faraway-server/       ← Rust WebSocket server (to be built)
└── faraway-client/       ← Vanilla HTML/CSS/JS frontend (to be built)
```

## Workflow (every task)

1. Pick next `[ ]` task from `TODO.md`
2. Implement it
3. Test it (see CONTRIBUTING.md — use `playwright-cli` for UI, `cargo test` for server)
4. Mark `[x]` in `TODO.md`
5. Commit — include `TODO.md` in the same commit

## Key Facts

- Server: Rust + Rocket 0.5 + rocket_ws, modelled after `~/personal/shields-up-engineering`
- Frontend: Vanilla HTML/CSS/JS (no framework)
- Base game only (regions 1–68, sanctuaries 1–45)
- Card images live in `cards/public/region/` and `cards/public/sanctuary/`
