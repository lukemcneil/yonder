//! SQLite-backed game result storage.
//!
//! Stores one row in `games` per completed game, plus one row per seat in
//! `game_players`. Card data is stored as JSON arrays of card numbers — the
//! canonical card definitions live in [`crate::cards`] and are rehydrated by
//! number when rendering.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::Serialize;

use crate::game::GameState;
use crate::scoring::score_player_detailed;

const SCHEMA_VERSION: i32 = 1;

/// Open (or create) the DB at `path` and run migrations to the latest schema.
pub fn open<P: AsRef<Path>>(path: P) -> SqlResult<Connection> {
    let conn = Connection::open(path)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Open an in-memory DB (for tests).
#[cfg(test)]
pub fn open_in_memory() -> SqlResult<Connection> {
    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let current: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if current < 1 {
        conn.execute_batch(
            r#"
            CREATE TABLE games (
              id            INTEGER PRIMARY KEY,
              room_code     TEXT    NOT NULL,
              started_at    INTEGER NOT NULL,
              finished_at   INTEGER NOT NULL,
              player_count  INTEGER NOT NULL,
              advanced      INTEGER NOT NULL,
              expansion     INTEGER NOT NULL
            );

            CREATE TABLE game_players (
              id                     INTEGER PRIMARY KEY,
              game_id                INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
              seat                   INTEGER NOT NULL,
              name                   TEXT    NOT NULL,
              name_lower             TEXT    NOT NULL,
              final_score            INTEGER NOT NULL,
              card_number_sum        INTEGER NOT NULL,
              placement              INTEGER NOT NULL,
              region_cards_json      TEXT    NOT NULL,
              sanctuary_cards_json   TEXT    NOT NULL,
              score_breakdown_json   TEXT    NOT NULL
            );

            CREATE INDEX idx_gp_name_lower  ON game_players(name_lower);
            CREATE INDEX idx_gp_final_score ON game_players(final_score DESC, card_number_sum ASC);
            CREATE INDEX idx_games_finished ON games(finished_at DESC);
            "#,
        )?;
    }

    conn.execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION))?;
    Ok(())
}

fn system_time_to_unix(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Persist a completed game. Must only be called when `state.phase` is
/// `GameOver`. Returns the new `games.id`.
pub fn save_game(
    conn: &mut Connection,
    room_code: &str,
    started_at: SystemTime,
    finished_at: SystemTime,
    state: &GameState,
    advanced: bool,
    expansion: bool,
) -> SqlResult<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO games (room_code, started_at, finished_at, player_count, advanced, expansion)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            room_code,
            system_time_to_unix(started_at),
            system_time_to_unix(finished_at),
            state.players.len() as i64,
            advanced as i64,
            expansion as i64,
        ],
    )?;
    let game_id = tx.last_insert_rowid();

    // Rank players by (final_score DESC, card_number_sum ASC) to assign placement.
    // Tied players share the same placement (standard competition ranking).
    let mut ranked: Vec<(usize, u32, u32)> = state
        .players
        .iter()
        .map(|p| {
            let entries = score_player_detailed(p);
            let total: u32 = entries.iter().map(|e| e.points).sum();
            let card_sum: u32 = p.tableau.iter().map(|c| c.number as u32).sum();
            (p.seat, total, card_sum)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));

    let mut placement_of = vec![0u32; state.players.len()];
    let mut current_rank: u32 = 0;
    let mut processed: u32 = 0;
    let mut prev: Option<(u32, u32)> = None;
    for (seat, score, csum) in &ranked {
        processed += 1;
        if prev.map_or(true, |p| p != (*score, *csum)) {
            current_rank = processed;
        }
        placement_of[*seat] = current_rank;
        prev = Some((*score, *csum));
    }

    for p in &state.players {
        let entries = score_player_detailed(p);
        let total: u32 = entries.iter().map(|e| e.points).sum();
        let card_sum: u32 = p.tableau.iter().map(|c| c.number as u32).sum();
        let region_numbers: Vec<u8> = p.tableau.iter().map(|c| c.number).collect();
        let sanctuary_numbers: Vec<u8> = p.sanctuaries.iter().map(|c| c.tile).collect();

        tx.execute(
            "INSERT INTO game_players (
                game_id, seat, name, name_lower, final_score, card_number_sum,
                placement, region_cards_json, sanctuary_cards_json, score_breakdown_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                game_id,
                p.seat as i64,
                p.name,
                p.name.to_lowercase(),
                total as i64,
                card_sum as i64,
                placement_of[p.seat] as i64,
                serde_json::to_string(&region_numbers).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&sanctuary_numbers).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string()),
            ],
        )?;
    }

    tx.commit()?;
    Ok(game_id)
}

// ─── Query results ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PlayerStats {
    pub name: String,
    pub games_played: u32,
    pub wins: u32,
    pub win_rate: f64,                            // 0..100
    pub high_score: u32,
    pub high_score_game_id: Option<i64>,
    pub avg_score: f64,
    pub placements: Vec<u32>,
    pub recent: Vec<RecentEntry>,

    // Derived extras
    pub first_game_at: Option<i64>,
    pub last_game_at: Option<i64>,
    pub recent_avg: Option<f64>,                  // last 5 games avg
    pub total_play_time_secs: i64,                // sum of finished_at - started_at across saved games
    pub longest_win_streak: u32,
    pub scoring_rate: Option<f64>,                // % of region cards that earned >0 points
    pub best_card_score: Option<BestCard>,        // single best +N play, with source game
    pub avg_by_player_count: Vec<AvgByPlayerCount>,
    pub top_cards: Vec<TopCard>,                  // 3 most-played region cards
    pub biome_preference: Vec<BiomePref>,         // combined regions + sanctuaries
    pub biome_preference_regions: Vec<BiomePref>, // regions only
    pub biome_preference_sanctuaries: Vec<BiomePref>, // sanctuaries only
    pub head_to_head: Vec<HeadToHead>,            // top 6 opponents by games played
    pub score_history: Vec<ScorePoint>,           // chronological scores for sparkline

    // Sanctuary-focused stats
    pub avg_sanctuaries_per_game: f64,
    pub sanctuary_scoring_rate: Option<f64>,      // % of kept sanctuaries that earned >0 points
    pub best_sanctuary_score: Option<BestCard>,   // best single sanctuary play
    pub top_sanctuaries: Vec<TopCard>,            // 3 most-played sanctuary tiles
    pub avg_by_sanctuary_count: Vec<AvgBySanctuaryCount>,
}

#[derive(Debug, Serialize)]
pub struct AvgBySanctuaryCount {
    pub sanctuary_count: u32,
    pub games: u32,
    pub avg_score: f64,
}

#[derive(Debug, Serialize)]
pub struct BestCard {
    pub kind: String,        // "region" | "sanctuary"
    pub number: u8,
    pub points: u32,
    pub explanation: String,
    pub game_id: i64,
    pub finished_at: i64,
}

#[derive(Debug, Serialize)]
pub struct AvgByPlayerCount {
    pub player_count: u32,
    pub games: u32,
    pub avg_score: f64,
}

#[derive(Debug, Serialize)]
pub struct TopCard {
    pub number: u8,
    pub times_played: u32,
}

#[derive(Debug, Serialize)]
pub struct BiomePref {
    pub biome: String,         // "Red" | "Green" | "Blue" | "Yellow" | "Colorless"
    pub count: u32,
    pub percent: f64,          // 0..100
}

#[derive(Debug, Serialize)]
pub struct HeadToHead {
    pub name: String,          // opponent's display name (most recent casing)
    pub games: u32,
    pub wins: u32,             // times this player placed strictly better than the opponent
    pub losses: u32,           // times opponent placed strictly better
    pub ties: u32,
}

#[derive(Debug, Serialize)]
pub struct ScorePoint {
    pub game_id: i64,
    pub finished_at: i64,
    pub score: u32,
    pub placement: u32,
}

#[derive(Debug, Serialize)]
pub struct RecentEntry {
    pub game_id: i64,
    pub finished_at: i64,
    pub score: u32,
    pub placement: u32,
    pub player_count: u32,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub name: String,
    pub score: u32,
    pub card_number_sum: u32,
    pub game_id: i64,
    pub finished_at: i64,
    pub player_count: u32,
    pub region_cards: Vec<u8>,
    pub sanctuary_cards: Vec<u8>,
    /// Per-card score breakdown (same shape as `CardScoreEntry`): an array of
    /// `{kind, number, points, explanation}`. Returned as raw JSON so the
    /// client can render the full scoring tableau with badges.
    pub score_breakdown: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct GameSummary {
    pub game_id: i64,
    pub finished_at: i64,
    pub player_count: u32,
    pub winner_name: String,
    pub winner_score: u32,
}

#[derive(Debug, Serialize)]
pub struct GameDetail {
    pub game_id: i64,
    pub room_code: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub player_count: u32,
    pub advanced: bool,
    pub expansion: bool,
    pub players: Vec<GameDetailPlayer>,
}

#[derive(Debug, Serialize)]
pub struct GameDetailPlayer {
    pub seat: u32,
    pub name: String,
    pub final_score: u32,
    pub card_number_sum: u32,
    pub placement: u32,
    pub region_cards: Vec<u8>,
    pub sanctuary_cards: Vec<u8>,
    pub score_breakdown: serde_json::Value,
}

// ─── Query functions ─────────────────────────────────────────────────────────

pub fn player_stats(conn: &Connection, name: &str) -> SqlResult<PlayerStats> {
    use std::collections::HashMap;
    let name_lower = name.to_lowercase();

    // Aggregate stats.
    let row: Option<(Option<String>, i64, Option<i64>, Option<f64>)> = conn
        .query_row(
            "SELECT
                MAX(name),
                COUNT(*),
                MAX(final_score),
                AVG(final_score)
             FROM game_players
             WHERE name_lower = ?1",
            params![name_lower],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    let (display_name, games_played, high_score, avg_score) = match row {
        Some((n, c, h, a)) => (n, c as u32, h.unwrap_or(0) as u32, a.unwrap_or(0.0)),
        None => (None, 0, 0, 0.0),
    };

    let wins: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM game_players WHERE name_lower = ?1 AND placement = 1",
            params![name_lower],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as u32;

    let win_rate = if games_played > 0 {
        100.0 * wins as f64 / games_played as f64
    } else {
        0.0
    };

    // Placements distribution (index 0 = 1st place).
    let mut placements: Vec<u32> = vec![0; 6];
    if games_played > 0 {
        let mut stmt = conn.prepare(
            "SELECT placement, COUNT(*) FROM game_players WHERE name_lower = ?1 GROUP BY placement",
        )?;
        let rows = stmt.query_map(params![name_lower], |r| {
            Ok((r.get::<_, i64>(0)? as usize, r.get::<_, i64>(1)? as u32))
        })?;
        for row in rows {
            let (pl, count) = row?;
            if pl >= 1 && pl <= placements.len() {
                placements[pl - 1] = count;
            }
        }
    }

    // Which game gave the high score? (Pick the earliest to break ties.)
    let high_score_game_id: Option<i64> = if games_played > 0 && high_score > 0 {
        conn.query_row(
            "SELECT game_id FROM game_players
             WHERE name_lower = ?1 AND final_score = ?2
             ORDER BY game_id ASC
             LIMIT 1",
            params![name_lower, high_score as i64],
            |r| r.get(0),
        )
        .optional()?
    } else {
        None
    };

    // Pull every saved game for this player in chronological order, along with
    // the metadata we need to compute derived metrics in one pass.
    struct GameRow {
        game_id: i64,
        finished_at: i64,
        started_at: i64,
        player_count: u32,
        final_score: u32,
        placement: u32,
        region_cards_json: String,
        sanctuary_cards_json: String,
        score_breakdown_json: String,
    }
    let mut stmt = conn.prepare(
        "SELECT gp.game_id, g.finished_at, g.started_at, g.player_count,
                gp.final_score, gp.placement,
                gp.region_cards_json, gp.sanctuary_cards_json, gp.score_breakdown_json
         FROM game_players gp
         JOIN games g ON g.id = gp.game_id
         WHERE gp.name_lower = ?1
         ORDER BY g.finished_at ASC",
    )?;
    let all_games: Vec<GameRow> = stmt
        .query_map(params![name_lower], |r| {
            Ok(GameRow {
                game_id: r.get(0)?,
                finished_at: r.get(1)?,
                started_at: r.get(2)?,
                player_count: r.get::<_, i64>(3)? as u32,
                final_score: r.get::<_, i64>(4)? as u32,
                placement: r.get::<_, i64>(5)? as u32,
                region_cards_json: r.get(6)?,
                sanctuary_cards_json: r.get(7)?,
                score_breakdown_json: r.get(8)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    // ── Derived: first/last game, play time, win streak, sparkline ──────
    let first_game_at = all_games.first().map(|g| g.finished_at);
    let last_game_at = all_games.last().map(|g| g.finished_at);
    let total_play_time_secs: i64 = all_games
        .iter()
        .map(|g| (g.finished_at - g.started_at).max(0))
        .sum();
    let mut longest_win_streak: u32 = 0;
    let mut current_streak: u32 = 0;
    for g in &all_games {
        if g.placement == 1 {
            current_streak += 1;
            if current_streak > longest_win_streak {
                longest_win_streak = current_streak;
            }
        } else {
            current_streak = 0;
        }
    }
    let score_history: Vec<ScorePoint> = all_games
        .iter()
        .map(|g| ScorePoint {
            game_id: g.game_id,
            finished_at: g.finished_at,
            score: g.final_score,
            placement: g.placement,
        })
        .collect();

    // Recent avg = last 5 games (chronologically most recent).
    let recent_avg: Option<f64> = if all_games.is_empty() {
        None
    } else {
        let n = all_games.len().min(5);
        let slice = &all_games[all_games.len() - n..];
        let sum: u32 = slice.iter().map(|g| g.final_score).sum();
        Some(sum as f64 / n as f64)
    };

    // ── Derived: avg by player count ───────────────────────────────────
    let mut by_pc: HashMap<u32, (u32, u64)> = HashMap::new();
    for g in &all_games {
        let e = by_pc.entry(g.player_count).or_insert((0, 0));
        e.0 += 1;
        e.1 += g.final_score as u64;
    }
    let mut avg_by_player_count: Vec<AvgByPlayerCount> = by_pc
        .into_iter()
        .map(|(pc, (games, sum))| AvgByPlayerCount {
            player_count: pc,
            games,
            avg_score: sum as f64 / games as f64,
        })
        .collect();
    avg_by_player_count.sort_by_key(|e| e.player_count);

    // ── Derived: top cards, biome preference, best single-card, scoring rate ──
    // Build a lookup table for region biomes once; sanctuary biomes come from cards too.
    let region_biome: HashMap<u8, crate::cards::Biome> = crate::cards::all_regions()
        .into_iter()
        .map(|c| (c.number, c.biome))
        .collect();
    let sanctuary_biome: HashMap<u8, crate::cards::Biome> = {
        // Quick rehydration for sanctuaries: reuse the deck builder, drain, collect.
        use crate::cards::{get_sanctuary_deck, get_sanctuary_deck_with_expansion};
        // get_sanctuary_deck_with_expansion is full set; shuffle order doesn't matter here.
        let mut deck = get_sanctuary_deck_with_expansion();
        if deck.is_empty() { deck = get_sanctuary_deck(); }
        deck.into_iter().map(|c| (c.tile, c.biome)).collect()
    };

    let mut region_card_counts: HashMap<u8, u32> = HashMap::new();
    let mut sanctuary_card_counts: HashMap<u8, u32> = HashMap::new();
    let mut region_biome_counts: HashMap<String, u32> = HashMap::new();
    let mut region_biome_total: u32 = 0;
    let mut sanctuary_biome_counts: HashMap<String, u32> = HashMap::new();
    let mut sanctuary_biome_total: u32 = 0;
    let mut best_card: Option<BestCard> = None;
    let mut best_sanctuary: Option<BestCard> = None;
    let mut region_entry_count: u32 = 0;
    let mut region_scored_count: u32 = 0;
    let mut sanctuary_entry_count: u32 = 0;
    let mut sanctuary_scored_count: u32 = 0;
    let mut total_sanctuaries: u32 = 0;
    // (sanctuary_count, final_score) pairs for the avg-by-sanctuary-count breakdown.
    let mut sanctuary_count_series: Vec<(u32, u32)> = Vec::with_capacity(all_games.len());

    for g in &all_games {
        // Region cards played → counts for top cards + biome prefs.
        let region_nums: Vec<u8> =
            serde_json::from_str(&g.region_cards_json).unwrap_or_default();
        for n in &region_nums {
            *region_card_counts.entry(*n).or_insert(0) += 1;
            if let Some(b) = region_biome.get(n) {
                *region_biome_counts.entry(biome_label(b)).or_insert(0) += 1;
                region_biome_total += 1;
            }
        }
        // Sanctuary tiles.
        let sanct_nums: Vec<u8> =
            serde_json::from_str(&g.sanctuary_cards_json).unwrap_or_default();
        for n in &sanct_nums {
            *sanctuary_card_counts.entry(*n).or_insert(0) += 1;
            if let Some(b) = sanctuary_biome.get(n) {
                *sanctuary_biome_counts.entry(biome_label(b)).or_insert(0) += 1;
                sanctuary_biome_total += 1;
            }
        }
        total_sanctuaries += sanct_nums.len() as u32;
        sanctuary_count_series.push((sanct_nums.len() as u32, g.final_score));

        // Best single-card play (all kinds) + per-kind bests + scoring rates.
        let breakdown: serde_json::Value =
            serde_json::from_str(&g.score_breakdown_json).unwrap_or(serde_json::Value::Null);
        if let Some(arr) = breakdown.as_array() {
            for entry in arr {
                let kind = entry.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let number = entry.get("number").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                let points = entry.get("points").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let explanation = entry
                    .get("explanation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                match kind {
                    "region" => {
                        region_entry_count += 1;
                        if points > 0 { region_scored_count += 1; }
                    }
                    "sanctuary" => {
                        sanctuary_entry_count += 1;
                        if points > 0 { sanctuary_scored_count += 1; }
                        if best_sanctuary.as_ref().map_or(true, |b| points > b.points) && points > 0 {
                            best_sanctuary = Some(BestCard {
                                kind: kind.to_string(),
                                number,
                                points,
                                explanation: explanation.clone(),
                                game_id: g.game_id,
                                finished_at: g.finished_at,
                            });
                        }
                    }
                    _ => {}
                }
                if best_card.as_ref().map_or(true, |b| points > b.points) && points > 0 {
                    best_card = Some(BestCard {
                        kind: kind.to_string(),
                        number,
                        points,
                        explanation,
                        game_id: g.game_id,
                        finished_at: g.finished_at,
                    });
                }
            }
        }
    }

    let scoring_rate = if region_entry_count > 0 {
        Some(100.0 * region_scored_count as f64 / region_entry_count as f64)
    } else {
        None
    };
    let sanctuary_scoring_rate = if sanctuary_entry_count > 0 {
        Some(100.0 * sanctuary_scored_count as f64 / sanctuary_entry_count as f64)
    } else {
        None
    };
    let avg_sanctuaries_per_game = if all_games.is_empty() {
        0.0
    } else {
        total_sanctuaries as f64 / all_games.len() as f64
    };

    let mut top_cards: Vec<TopCard> = region_card_counts
        .into_iter()
        .map(|(number, times_played)| TopCard { number, times_played })
        .collect();
    top_cards.sort_by(|a, b| b.times_played.cmp(&a.times_played).then(a.number.cmp(&b.number)));
    top_cards.truncate(3);

    let mut top_sanctuaries: Vec<TopCard> = sanctuary_card_counts
        .into_iter()
        .map(|(number, times_played)| TopCard { number, times_played })
        .collect();
    top_sanctuaries.sort_by(|a, b| b.times_played.cmp(&a.times_played).then(a.number.cmp(&b.number)));
    top_sanctuaries.truncate(3);

    // Combined biome preference (regions + sanctuaries) — preserved for back-compat.
    let mut combined_counts: HashMap<String, u32> = HashMap::new();
    for (k, v) in &region_biome_counts { *combined_counts.entry(k.clone()).or_insert(0) += v; }
    for (k, v) in &sanctuary_biome_counts { *combined_counts.entry(k.clone()).or_insert(0) += v; }
    let combined_total = region_biome_total + sanctuary_biome_total;
    let biome_preference = make_biome_prefs(combined_counts, combined_total);
    let biome_preference_regions = make_biome_prefs(region_biome_counts, region_biome_total);
    let biome_preference_sanctuaries = make_biome_prefs(sanctuary_biome_counts, sanctuary_biome_total);

    // Avg score by number of sanctuaries kept this game.
    let mut by_sc: HashMap<u32, (u32, u64)> = HashMap::new();
    for (count, score) in &sanctuary_count_series {
        let e = by_sc.entry(*count).or_insert((0, 0));
        e.0 += 1;
        e.1 += *score as u64;
    }
    let mut avg_by_sanctuary_count: Vec<AvgBySanctuaryCount> = by_sc
        .into_iter()
        .map(|(sanctuary_count, (games, sum))| AvgBySanctuaryCount {
            sanctuary_count,
            games,
            avg_score: sum as f64 / games as f64,
        })
        .collect();
    avg_by_sanctuary_count.sort_by_key(|e| e.sanctuary_count);

    // ── Derived: head-to-head ──────────────────────────────────────────
    let mut h2h_stmt = conn.prepare(
        "SELECT op.name_lower,
                MAX(op.name) AS name,
                COUNT(*) AS games,
                SUM(CASE WHEN me.placement < op.placement THEN 1 ELSE 0 END) AS wins,
                SUM(CASE WHEN me.placement > op.placement THEN 1 ELSE 0 END) AS losses,
                SUM(CASE WHEN me.placement = op.placement THEN 1 ELSE 0 END) AS ties
         FROM game_players me
         JOIN game_players op ON op.game_id = me.game_id AND op.id != me.id
         WHERE me.name_lower = ?1
         GROUP BY op.name_lower
         ORDER BY games DESC, name ASC
         LIMIT 6",
    )?;
    let head_to_head: Vec<HeadToHead> = h2h_stmt
        .query_map(params![name_lower], |r| {
            Ok(HeadToHead {
                name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                games: r.get::<_, i64>(2)? as u32,
                wins: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u32,
                losses: r.get::<_, Option<i64>>(4)?.unwrap_or(0) as u32,
                ties: r.get::<_, Option<i64>>(5)?.unwrap_or(0) as u32,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    // Recent entries (last 10) for the "Recent Games" list on the page.
    let mut recent_stmt = conn.prepare(
        "SELECT gp.game_id, g.finished_at, gp.final_score, gp.placement, g.player_count
         FROM game_players gp
         JOIN games g ON g.id = gp.game_id
         WHERE gp.name_lower = ?1
         ORDER BY g.finished_at DESC
         LIMIT 10",
    )?;
    let recent = recent_stmt
        .query_map(params![name_lower], |r| {
            Ok(RecentEntry {
                game_id: r.get(0)?,
                finished_at: r.get(1)?,
                score: r.get::<_, i64>(2)? as u32,
                placement: r.get::<_, i64>(3)? as u32,
                player_count: r.get::<_, i64>(4)? as u32,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(PlayerStats {
        name: display_name.unwrap_or_else(|| name.to_string()),
        games_played,
        wins,
        win_rate,
        high_score,
        high_score_game_id,
        avg_score,
        placements,
        recent,
        first_game_at,
        last_game_at,
        recent_avg,
        total_play_time_secs,
        longest_win_streak,
        scoring_rate,
        best_card_score: best_card,
        avg_by_player_count,
        top_cards,
        biome_preference,
        biome_preference_regions,
        biome_preference_sanctuaries,
        head_to_head,
        score_history,
        avg_sanctuaries_per_game,
        sanctuary_scoring_rate,
        best_sanctuary_score: best_sanctuary,
        top_sanctuaries,
        avg_by_sanctuary_count,
    })
}

fn biome_label(b: &crate::cards::Biome) -> String {
    use crate::cards::Biome::*;
    match b {
        Red => "Red".to_string(),
        Green => "Green".to_string(),
        Blue => "Blue".to_string(),
        Yellow => "Yellow".to_string(),
        Colorless => "Colorless".to_string(),
    }
}

fn make_biome_prefs(counts: std::collections::HashMap<String, u32>, total: u32) -> Vec<BiomePref> {
    let mut out: Vec<BiomePref> = counts
        .into_iter()
        .map(|(biome, count)| BiomePref {
            biome,
            count,
            percent: if total > 0 { 100.0 * count as f64 / total as f64 } else { 0.0 },
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count));
    out
}

fn map_leaderboard_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LeaderboardEntry> {
    let region_json: String = r.get(6)?;
    let sanc_json: String = r.get(7)?;
    let breakdown_json: String = r.get(8)?;
    let region_cards: Vec<u8> = serde_json::from_str(&region_json).unwrap_or_default();
    let sanctuary_cards: Vec<u8> = serde_json::from_str(&sanc_json).unwrap_or_default();
    let score_breakdown: serde_json::Value =
        serde_json::from_str(&breakdown_json).unwrap_or(serde_json::Value::Null);
    Ok(LeaderboardEntry {
        rank: 0,
        name: r.get(0)?,
        score: r.get::<_, i64>(1)? as u32,
        card_number_sum: r.get::<_, i64>(2)? as u32,
        game_id: r.get(3)?,
        finished_at: r.get(4)?,
        player_count: r.get::<_, i64>(5)? as u32,
        region_cards,
        sanctuary_cards,
        score_breakdown,
    })
}

/// All-time leaderboard, or that player's best games ranked by score when `player` is set.
pub fn leaderboard(conn: &Connection, limit: u32, player: Option<&str>) -> SqlResult<Vec<LeaderboardEntry>> {
    let rows: Vec<LeaderboardEntry> = if let Some(p) = player.filter(|s| !s.is_empty()) {
        let name_lower = p.to_lowercase();
        let mut stmt = conn.prepare(
            "SELECT gp.name, gp.final_score, gp.card_number_sum, gp.game_id, g.finished_at,
                    g.player_count, gp.region_cards_json, gp.sanctuary_cards_json,
                    gp.score_breakdown_json
             FROM game_players gp
             JOIN games g ON g.id = gp.game_id
             WHERE gp.name_lower = ?1
             ORDER BY gp.final_score DESC, gp.card_number_sum ASC, gp.game_id ASC
             LIMIT ?2",
        )?;
        let v = stmt
            .query_map(params![name_lower, limit as i64], map_leaderboard_row)?
            .collect::<SqlResult<Vec<_>>>()?;
        v
    } else {
        let mut stmt = conn.prepare(
            "SELECT gp.name, gp.final_score, gp.card_number_sum, gp.game_id, g.finished_at,
                    g.player_count, gp.region_cards_json, gp.sanctuary_cards_json,
                    gp.score_breakdown_json
             FROM game_players gp
             JOIN games g ON g.id = gp.game_id
             ORDER BY gp.final_score DESC, gp.card_number_sum ASC, gp.game_id ASC
             LIMIT ?1",
        )?;
        let v = stmt
            .query_map(params![limit as i64], map_leaderboard_row)?
            .collect::<SqlResult<Vec<_>>>()?;
        v
    };

    // Assign dense-rank style placement (ties share rank).
    let mut entries = rows;
    let mut current_rank: u32 = 0;
    let mut processed: u32 = 0;
    let mut prev: Option<(u32, u32)> = None;
    for e in entries.iter_mut() {
        processed += 1;
        if prev.map_or(true, |p| p != (e.score, e.card_number_sum)) {
            current_rank = processed;
        }
        e.rank = current_rank;
        prev = Some((e.score, e.card_number_sum));
    }
    Ok(entries)
}

pub fn recent_games(conn: &Connection, limit: u32) -> SqlResult<Vec<GameSummary>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.finished_at, g.player_count,
                (SELECT name FROM game_players WHERE game_id = g.id
                 ORDER BY final_score DESC, card_number_sum ASC LIMIT 1),
                (SELECT final_score FROM game_players WHERE game_id = g.id
                 ORDER BY final_score DESC, card_number_sum ASC LIMIT 1)
         FROM games g
         ORDER BY g.finished_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(GameSummary {
                game_id: r.get(0)?,
                finished_at: r.get(1)?,
                player_count: r.get::<_, i64>(2)? as u32,
                winner_name: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                winner_score: r.get::<_, Option<i64>>(4)?.unwrap_or(0) as u32,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
}

pub fn game_detail(conn: &Connection, game_id: i64) -> SqlResult<Option<GameDetail>> {
    let game: Option<(String, i64, i64, i64, i64, i64)> = conn
        .query_row(
            "SELECT room_code, started_at, finished_at, player_count, advanced, expansion
             FROM games WHERE id = ?1",
            params![game_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;

    let (room_code, started_at, finished_at, player_count, advanced, expansion) = match game {
        Some(g) => g,
        None => return Ok(None),
    };

    let mut stmt = conn.prepare(
        "SELECT seat, name, final_score, card_number_sum, placement,
                region_cards_json, sanctuary_cards_json, score_breakdown_json
         FROM game_players
         WHERE game_id = ?1
         ORDER BY seat ASC",
    )?;
    let players = stmt
        .query_map(params![game_id], |r| {
            let region_json: String = r.get(5)?;
            let sanc_json: String = r.get(6)?;
            let breakdown_json: String = r.get(7)?;
            Ok(GameDetailPlayer {
                seat: r.get::<_, i64>(0)? as u32,
                name: r.get(1)?,
                final_score: r.get::<_, i64>(2)? as u32,
                card_number_sum: r.get::<_, i64>(3)? as u32,
                placement: r.get::<_, i64>(4)? as u32,
                region_cards: serde_json::from_str(&region_json).unwrap_or_default(),
                sanctuary_cards: serde_json::from_str(&sanc_json).unwrap_or_default(),
                score_breakdown: serde_json::from_str(&breakdown_json)
                    .unwrap_or(serde_json::Value::Null),
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(Some(GameDetail {
        game_id,
        room_code,
        started_at,
        finished_at,
        player_count: player_count as u32,
        advanced: advanced != 0,
        expansion: expansion != 0,
        players,
    }))
}

// ─── Post-game highlight helpers ─────────────────────────────────────────────

/// Highest score this player (by case-insensitive name) has achieved BEFORE
/// the given `exclude_game_id`. Used to detect personal bests set by the
/// just-saved game.
pub fn previous_personal_best(
    conn: &Connection,
    name: &str,
    exclude_game_id: i64,
) -> SqlResult<Option<u32>> {
    let score: Option<i64> = conn
        .query_row(
            "SELECT MAX(final_score) FROM game_players
             WHERE name_lower = ?1 AND game_id != ?2",
            params![name.to_lowercase(), exclude_game_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(score.map(|s| s as u32))
}

/// Rank of `(score, card_sum)` across all-time entries — how many entries
/// strictly beat it, plus 1.
pub fn rank_of_score(conn: &Connection, score: u32, card_sum: u32) -> SqlResult<u32> {
    let better: i64 = conn.query_row(
        "SELECT COUNT(*) FROM game_players
         WHERE final_score > ?1
            OR (final_score = ?1 AND card_number_sum < ?2)",
        params![score as i64, card_sum as i64],
        |r| r.get(0),
    )?;
    Ok(better as u32 + 1)
}

/// Average final_score for a player BEFORE the given game — used so a newly
/// saved game can be compared against the player's historical baseline.
/// Returns `None` if this was the player's first game.
pub fn previous_player_avg(
    conn: &Connection,
    name: &str,
    exclude_game_id: i64,
) -> SqlResult<Option<f64>> {
    let row: Option<Option<f64>> = conn
        .query_row(
            "SELECT AVG(final_score) FROM game_players
             WHERE name_lower = ?1 AND game_id != ?2",
            params![name.to_lowercase(), exclude_game_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(row.flatten())
}

/// Average final_score across ALL players BEFORE the given game — used so
/// a newly saved game can be compared against the community baseline.
/// Returns `None` if no other games exist.
pub fn previous_global_avg(conn: &Connection, exclude_game_id: i64) -> SqlResult<Option<f64>> {
    let row: Option<Option<f64>> = conn
        .query_row(
            "SELECT AVG(final_score) FROM game_players WHERE game_id != ?1",
            params![exclude_game_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(row.flatten())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Biome, Fame, RegionCard, SanctuaryCard, WonderCount};
    use crate::game::{GamePhase, GameState, PlayerScore, PlayerState};
    use std::time::Duration;

    fn region(number: u8) -> RegionCard {
        RegionCard {
            number,
            biome: Biome::Blue,
            night: false,
            clue: false,
            wonders: WonderCount::zero(),
            quest: WonderCount::zero(),
            fame: Fame::Flat(number as u32),
        }
    }

    fn sanctuary(tile: u8) -> SanctuaryCard {
        SanctuaryCard {
            tile,
            biome: Biome::Colorless,
            night: false,
            clue: false,
            wonders: WonderCount::zero(),
            fame: Fame::None,
        }
    }

    fn player(seat: usize, name: &str, tableau: Vec<u8>, sanctuaries: Vec<u8>) -> PlayerState {
        PlayerState {
            seat,
            name: name.to_string(),
            tableau: tableau.into_iter().map(region).collect(),
            sanctuaries: sanctuaries.into_iter().map(sanctuary).collect(),
            hand: vec![],
            played_this_round: None,
        }
    }

    fn make_game(players: Vec<PlayerState>) -> GameState {
        let scores: Vec<PlayerScore> = players
            .iter()
            .map(|p| PlayerScore {
                seat: p.seat,
                name: p.name.clone(),
                total: super::score_player_detailed(p).iter().map(|e| e.points).sum(),
                card_number_sum: p.tableau.iter().map(|c| c.number as u32).sum(),
            })
            .collect();
        GameState {
            phase: GamePhase::GameOver { scores },
            round: 8,
            player_count: players.len(),
            players,
            region_deck: vec![],
            sanctuary_deck: vec![],
            market: vec![],
        }
    }

    #[test]
    fn save_and_read_game() {
        let mut conn = open_in_memory().unwrap();
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let t1 = t0 + Duration::from_secs(1200);
        let game = make_game(vec![
            player(0, "Alice", vec![1, 2, 3, 4, 5, 6, 7, 8], vec![10, 11]),
            player(1, "Bob",   vec![2, 3, 4, 5, 6, 7, 8, 9], vec![12]),
        ]);
        let id = save_game(&mut conn, "ABCD", t0, t1, &game, false, true).unwrap();
        assert!(id > 0);

        let detail = game_detail(&conn, id).unwrap().unwrap();
        assert_eq!(detail.room_code, "ABCD");
        assert_eq!(detail.player_count, 2);
        assert_eq!(detail.expansion, true);
        assert_eq!(detail.players.len(), 2);
        assert_eq!(detail.players[0].region_cards, vec![1,2,3,4,5,6,7,8]);
        assert_eq!(detail.players[0].sanctuary_cards, vec![10,11]);
    }

    #[test]
    fn case_insensitive_player_lookup() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000);
        let g1 = make_game(vec![
            player(0, "Luke",  vec![10,11,12,13,14,15,16,17], vec![]),
            player(1, "Alice", vec![1,2,3,4,5,6,7,8], vec![]),
        ]);
        let g2 = make_game(vec![
            player(0, "LUKE",  vec![20,21,22,23,24,25,26,27], vec![]),
            player(1, "bob",   vec![2,3,4,5,6,7,8,9], vec![]),
        ]);
        save_game(&mut conn, "R1", t, t, &g1, false, false).unwrap();
        save_game(&mut conn, "R2", t + Duration::from_secs(10), t + Duration::from_secs(10), &g2, false, false).unwrap();

        let stats = player_stats(&conn, "luke").unwrap();
        assert_eq!(stats.games_played, 2, "case-insensitive should find both");
        assert!(stats.high_score > 0);

        let stats_upper = player_stats(&conn, "LUKE").unwrap();
        assert_eq!(stats_upper.games_played, 2);
    }

    #[test]
    fn placement_and_wins() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(3_000_000);
        // Alice should win (higher card numbers → higher Flat fame).
        let g = make_game(vec![
            player(0, "Alice", vec![60,61,62,63,64,65,66,67], vec![]),
            player(1, "Bob",   vec![1,2,3,4,5,6,7,8], vec![]),
        ]);
        save_game(&mut conn, "W1", t, t, &g, false, false).unwrap();

        let alice = player_stats(&conn, "alice").unwrap();
        assert_eq!(alice.wins, 1);
        assert_eq!(alice.placements[0], 1);
        assert_eq!(alice.placements[1], 0);

        let bob = player_stats(&conn, "bob").unwrap();
        assert_eq!(bob.wins, 0);
        assert_eq!(bob.placements[0], 0);
        assert_eq!(bob.placements[1], 1);
    }

    #[test]
    fn leaderboard_ordering() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(4_000_000);
        save_game(&mut conn, "G1", t, t, &make_game(vec![
            player(0, "P1", vec![1,2,3,4,5,6,7,8], vec![]),
            player(1, "P2", vec![10,11,12,13,14,15,16,17], vec![]),
        ]), false, false).unwrap();
        save_game(&mut conn, "G2", t+Duration::from_secs(1), t+Duration::from_secs(1), &make_game(vec![
            player(0, "P3", vec![60,61,62,63,64,65,66,67], vec![]),
            player(1, "P4", vec![2,3,4,5,6,7,8,9], vec![]),
        ]), false, false).unwrap();

        let lb = leaderboard(&conn, 10, None).unwrap();
        assert!(lb.len() >= 4);
        // Should be sorted by final_score DESC.
        for w in lb.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
        // Rank of first should be 1.
        assert_eq!(lb[0].rank, 1);
    }

    #[test]
    fn leaderboard_filter_by_player() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(4_000_000);
        save_game(&mut conn, "G1", t, t, &make_game(vec![
            player(0, "P1", vec![1,2,3,4,5,6,7,8], vec![]),
            player(1, "P2", vec![10,11,12,13,14,15,16,17], vec![]),
        ]), false, false).unwrap();
        save_game(&mut conn, "G2", t+Duration::from_secs(1), t+Duration::from_secs(1), &make_game(vec![
            player(0, "P3", vec![60,61,62,63,64,65,66,67], vec![]),
            player(1, "P4", vec![2,3,4,5,6,7,8,9], vec![]),
        ]), false, false).unwrap();

        let p1_only = leaderboard(&conn, 10, Some("p1")).unwrap();
        assert_eq!(p1_only.len(), 1);
        assert_eq!(p1_only[0].name, "P1");

        let global = leaderboard(&conn, 10, None).unwrap();
        assert!(global.len() >= 2);
    }

    #[test]
    fn personal_best_and_rank() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(5_000_000);
        let id1 = save_game(&mut conn, "G1", t, t, &make_game(vec![
            player(0, "Luke", vec![1,2,3,4,5,6,7,8], vec![]),
        ]), false, false).unwrap();
        // Before the second save, Luke's PB excluding id2 should exist.
        let pb_excluding_nothing = previous_personal_best(&conn, "luke", 9999).unwrap();
        assert!(pb_excluding_nothing.is_some());

        let id2 = save_game(&mut conn, "G2", t+Duration::from_secs(1), t+Duration::from_secs(1), &make_game(vec![
            player(0, "luke", vec![60,61,62,63,64,65,66,67], vec![]),
        ]), false, false).unwrap();
        let pb_before_id2 = previous_personal_best(&conn, "luke", id2).unwrap().unwrap();
        let detail_id1 = game_detail(&conn, id1).unwrap().unwrap();
        assert_eq!(pb_before_id2, detail_id1.players[0].final_score);

        // Rank of the first-place score should be 1.
        let detail_id2 = game_detail(&conn, id2).unwrap().unwrap();
        let r = rank_of_score(&conn, detail_id2.players[0].final_score, detail_id2.players[0].card_number_sum).unwrap();
        assert_eq!(r, 1);
    }

    #[test]
    fn recent_games_returns_winner() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(6_000_000);
        save_game(&mut conn, "R1", t, t, &make_game(vec![
            player(0, "Alice", vec![60,61,62,63,64,65,66,67], vec![]),
            player(1, "Bob",   vec![1,2,3,4,5,6,7,8], vec![]),
        ]), false, false).unwrap();
        let summaries = recent_games(&conn, 10).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].winner_name, "Alice");
    }

    #[test]
    fn previous_player_avg_case_insensitive_and_excludes_game() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(7_000_000);
        // First game — no prior history for anyone.
        let g1 = save_game(&mut conn, "G1", t, t, &make_game(vec![
            player(0, "Luke",  vec![10,11,12,13,14,15,16,17], vec![]),
            player(1, "Alice", vec![1,2,3,4,5,6,7,8],         vec![]),
        ]), false, false).unwrap();
        // Luke has no prior games → average excluding g1 is None.
        assert!(previous_player_avg(&conn, "luke", g1).unwrap().is_none());

        // Play a second game — Luke has one prior result now.
        let g2 = save_game(&mut conn, "G2", t + std::time::Duration::from_secs(10), t + std::time::Duration::from_secs(10), &make_game(vec![
            player(0, "LUKE",  vec![20,21,22,23,24,25,26,27], vec![]),
            player(1, "Bob",   vec![2,3,4,5,6,7,8,9],         vec![]),
        ]), false, false).unwrap();

        // Before g2, Luke's only prior score is from g1 — avg equals that score.
        let g1_luke_score = game_detail(&conn, g1).unwrap().unwrap().players[0].final_score as f64;
        let avg = previous_player_avg(&conn, "luke", g2).unwrap().unwrap();
        assert!((avg - g1_luke_score).abs() < 1e-9);

        // Uppercase lookup hits the same rows.
        let avg_upper = previous_player_avg(&conn, "LUKE", g2).unwrap().unwrap();
        assert!((avg_upper - avg).abs() < 1e-9);
    }

    #[test]
    fn previous_global_avg_excludes_current_game() {
        let mut conn = open_in_memory().unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(8_000_000);
        // Nothing saved yet — no prior global avg.
        assert!(previous_global_avg(&conn, 0).unwrap().is_none());

        // Save two games; after the second, global avg before that game
        // should equal the (mean of both players from g1).
        let g1 = save_game(&mut conn, "G1", t, t, &make_game(vec![
            player(0, "A", vec![1,2,3,4,5,6,7,8],            vec![]),
            player(1, "B", vec![10,11,12,13,14,15,16,17],    vec![]),
        ]), false, false).unwrap();
        let g2 = save_game(&mut conn, "G2", t + std::time::Duration::from_secs(10), t + std::time::Duration::from_secs(10), &make_game(vec![
            player(0, "C", vec![60,61,62,63,64,65,66,67], vec![]),
            player(1, "D", vec![2,3,4,5,6,7,8,9],         vec![]),
        ]), false, false).unwrap();

        let g1_detail = game_detail(&conn, g1).unwrap().unwrap();
        let g1_mean = (g1_detail.players[0].final_score as f64
            + g1_detail.players[1].final_score as f64) / 2.0;
        let avg_before_g2 = previous_global_avg(&conn, g2).unwrap().unwrap();
        assert!((avg_before_g2 - g1_mean).abs() < 1e-9);
    }

}
