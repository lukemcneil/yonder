use serde::Serialize;
use crate::cards::{Biome, Fame, RegionCard, SanctuaryCard, Wonder, WonderCount};
use crate::game::PlayerState;

/// Per-player score detail (for the scoring table visible to all).
#[derive(Debug, Clone, Serialize)]
pub struct PlayerScoreDetail {
    pub seat: usize,
    pub name: String,
    pub entries: Vec<CardScoreEntry>,
    pub total: u32,
}

/// Per-card score breakdown entry sent to the client.
#[derive(Debug, Clone, Serialize)]
pub struct CardScoreEntry {
    /// "region" or "sanctuary"
    pub kind: String,
    /// Card number (region) or tile number (sanctuary)
    pub number: u8,
    /// Points scored by this card (0 if quest failed)
    pub points: u32,
    /// Human-readable explanation, e.g. "3 Stone × 4"
    pub explanation: String,
}

/// Score all 8 region cards + sanctuaries for one player.
/// Region cards are scored right-to-left (index 7 first, index 0 last).
pub fn score_player(player: &PlayerState) -> u32 {
    score_player_detailed(player).iter().map(|e| e.points).sum()
}

#[cfg(test)]
fn score_region_card(
    card: &RegionCard,
    visible_regions: &[&RegionCard],
    sanctuaries: &[&SanctuaryCard],
) -> u32 {
    if !prerequisites_met(&card.quest, visible_regions, sanctuaries) {
        return 0;
    }
    compute_fame(&card.fame, visible_regions, sanctuaries)
}

fn score_sanctuary(
    card: &SanctuaryCard,
    visible_regions: &[&RegionCard],
    other_sanctuaries: &[&SanctuaryCard],
) -> u32 {
    compute_fame(&card.fame, visible_regions, other_sanctuaries)
}

fn prerequisites_met(
    quest: &WonderCount,
    visible_regions: &[&RegionCard],
    sanctuaries: &[&SanctuaryCard],
) -> bool {
    if quest.is_zero() {
        return true;
    }
    let ctx_wonders = count_wonders_in_context(visible_regions, sanctuaries);
    ctx_wonders.stone >= quest.stone
        && ctx_wonders.chimera >= quest.chimera
        && ctx_wonders.thistle >= quest.thistle
}

fn count_wonders_in_context(
    visible_regions: &[&RegionCard],
    sanctuaries: &[&SanctuaryCard],
) -> WonderCount {
    let mut stone: u8 = 0;
    let mut chimera: u8 = 0;
    let mut thistle: u8 = 0;
    for r in visible_regions {
        stone += r.wonders.stone;
        chimera += r.wonders.chimera;
        thistle += r.wonders.thistle;
    }
    for s in sanctuaries {
        stone += s.wonders.stone;
        chimera += s.wonders.chimera;
        thistle += s.wonders.thistle;
    }
    WonderCount { stone, chimera, thistle }
}

fn count_biome<'a>(
    biome: &Biome,
    regions: &[&'a RegionCard],
    sanctuaries: &[&'a SanctuaryCard],
) -> u32 {
    let r = regions.iter().filter(|c| &c.biome == biome).count() as u32;
    let s = sanctuaries.iter().filter(|c| &c.biome == biome).count() as u32;
    r + s
}

fn count_nights<'a>(regions: &[&'a RegionCard], sanctuaries: &[&'a SanctuaryCard]) -> u32 {
    let r = regions.iter().filter(|c| c.night).count() as u32;
    let s = sanctuaries.iter().filter(|c| c.night).count() as u32;
    r + s
}

fn count_clues<'a>(regions: &[&'a RegionCard], sanctuaries: &[&'a SanctuaryCard]) -> u32 {
    let r = regions.iter().filter(|c| c.clue).count() as u32;
    let s = sanctuaries.iter().filter(|c| c.clue).count() as u32;
    r + s
}

fn count_icon<'a>(
    icon: &Wonder,
    regions: &[&'a RegionCard],
    sanctuaries: &[&'a SanctuaryCard],
) -> u32 {
    let wonders = count_wonders_in_context(regions, sanctuaries);
    match icon {
        Wonder::Stone => wonders.stone as u32,
        Wonder::Chimera => wonders.chimera as u32,
        Wonder::Thistle => wonders.thistle as u32,
    }
}

fn compute_fame(
    fame: &Fame,
    visible_regions: &[&RegionCard],
    sanctuaries: &[&SanctuaryCard],
) -> u32 {
    match fame {
        Fame::None => 0,
        Fame::Flat(v) => *v,
        Fame::PerIcon { icon, score_per } => {
            count_icon(icon, visible_regions, sanctuaries) * score_per
        }
        Fame::PerColour { biome, score_per } => {
            count_biome(biome, visible_regions, sanctuaries) * score_per
        }
        Fame::PerColourPair { biome1, biome2, score_per } => {
            let n = count_biome(biome1, visible_regions, sanctuaries)
                + count_biome(biome2, visible_regions, sanctuaries);
            n * score_per
        }
        Fame::PerNight { score_per } => {
            count_nights(visible_regions, sanctuaries) * score_per
        }
        Fame::PerClue { score_per } => {
            count_clues(visible_regions, sanctuaries) * score_per
        }
        Fame::PerWonderSet { score_per } => {
            let w = count_wonders_in_context(visible_regions, sanctuaries);
            let sets = w.stone.min(w.chimera).min(w.thistle) as u32;
            sets * score_per
        }
        Fame::PerColourSet { score_per } => {
            let red = count_biome(&Biome::Red, visible_regions, sanctuaries);
            let green = count_biome(&Biome::Green, visible_regions, sanctuaries);
            let blue = count_biome(&Biome::Blue, visible_regions, sanctuaries);
            let yellow = count_biome(&Biome::Yellow, visible_regions, sanctuaries);
            let sets = red.min(green).min(blue).min(yellow);
            sets * score_per
        }
    }
}

fn biome_name(biome: &Biome) -> &'static str {
    match biome {
        Biome::Red => "Forest",
        Biome::Green => "River",
        Biome::Blue => "Desert",
        Biome::Yellow => "City",
        Biome::Colorless => "Colorless",
    }
}

fn fame_explanation(
    fame: &Fame,
    visible_regions: &[&RegionCard],
    sanctuaries: &[&SanctuaryCard],
) -> String {
    match fame {
        Fame::None => "No scoring condition".to_string(),
        Fame::Flat(v) => format!("+{} fame", v),
        Fame::PerIcon { icon, score_per } => {
            let count = count_icon(icon, visible_regions, sanctuaries);
            let name = match icon {
                Wonder::Stone => "Stone",
                Wonder::Chimera => "Chimera",
                Wonder::Thistle => "Thistle",
            };
            format!("{} {} × {}", count, name, score_per)
        }
        Fame::PerColour { biome, score_per } => {
            let count = count_biome(biome, visible_regions, sanctuaries);
            format!("{} {} × {}", count, biome_name(biome), score_per)
        }
        Fame::PerColourPair { biome1, biome2, score_per } => {
            let n = count_biome(biome1, visible_regions, sanctuaries)
                + count_biome(biome2, visible_regions, sanctuaries);
            format!("{} {}/{} × {}", n, biome_name(biome1), biome_name(biome2), score_per)
        }
        Fame::PerNight { score_per } => {
            let count = count_nights(visible_regions, sanctuaries);
            format!("{} Night × {}", count, score_per)
        }
        Fame::PerClue { score_per } => {
            let count = count_clues(visible_regions, sanctuaries);
            format!("{} Clue × {}", count, score_per)
        }
        Fame::PerWonderSet { score_per } => {
            let w = count_wonders_in_context(visible_regions, sanctuaries);
            let sets = w.stone.min(w.chimera).min(w.thistle) as u32;
            format!("{} Wonder sets × {}", sets, score_per)
        }
        Fame::PerColourSet { score_per } => {
            let red = count_biome(&Biome::Red, visible_regions, sanctuaries);
            let green = count_biome(&Biome::Green, visible_regions, sanctuaries);
            let blue = count_biome(&Biome::Blue, visible_regions, sanctuaries);
            let yellow = count_biome(&Biome::Yellow, visible_regions, sanctuaries);
            let sets = red.min(green).min(blue).min(yellow);
            format!("{} Colour sets × {}", sets, score_per)
        }
    }
}

/// Returns per-card breakdown for the scoring screen.
/// Order: region cards right-to-left (index 7 first), then sanctuaries.
pub fn score_player_detailed(player: &PlayerState) -> Vec<CardScoreEntry> {
    let tableau = &player.tableau;
    let sanctuaries = &player.sanctuaries;
    let mut entries: Vec<CardScoreEntry> = Vec::new();

    let len = tableau.len();
    for i in (0..len).rev() {
        let visible_regions: Vec<&RegionCard> = tableau[i..].iter().collect();
        let visible_sanctuaries: Vec<&SanctuaryCard> = sanctuaries.iter().collect();
        let card = &tableau[i];
        let quest_met = prerequisites_met(&card.quest, &visible_regions, &visible_sanctuaries);
        let points = if quest_met {
            compute_fame(&card.fame, &visible_regions, &visible_sanctuaries)
        } else {
            0
        };
        let explanation = if !quest_met {
            "Quest not met".to_string()
        } else {
            fame_explanation(&card.fame, &visible_regions, &visible_sanctuaries)
        };
        entries.push(CardScoreEntry {
            kind: "region".to_string(),
            number: card.number,
            points,
            explanation,
        });
    }

    for (j, sanc) in sanctuaries.iter().enumerate() {
        let full_regions: Vec<&RegionCard> = tableau.iter().collect();
        let other_sanctuaries: Vec<&SanctuaryCard> = sanctuaries
            .iter()
            .enumerate()
            .filter(|&(k, _)| k != j)
            .map(|(_, s)| s)
            .collect();
        let points = score_sanctuary(sanc, &full_regions, &other_sanctuaries);
        let explanation = fame_explanation(&sanc.fame, &full_regions, &other_sanctuaries);
        entries.push(CardScoreEntry {
            kind: "sanctuary".to_string(),
            number: sanc.tile,
            points,
            explanation,
        });
    }

    entries
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Biome, Fame, RegionCard, Wonder, WonderCount};

    fn region(number: u8, biome: Biome, night: bool, clue: bool, wonders: WonderCount, quest: WonderCount, fame: Fame) -> RegionCard {
        RegionCard { number, biome, night, clue, wonders, quest, fame }
    }

    fn no_wonders() -> WonderCount {
        WonderCount::zero()
    }

    fn w(stone: u8, chimera: u8, thistle: u8) -> WonderCount {
        WonderCount { stone, chimera, thistle }
    }

    #[test]
    fn flat_fame_no_quest() {
        let card = region(9, Biome::Blue, false, false, no_wonders(), no_wonders(), Fame::Flat(5));
        let score = score_region_card(&card, &[], &[]);
        assert_eq!(score, 5);
    }

    #[test]
    fn flat_fame_quest_met() {
        let visible = region(2, Biome::Blue, false, false, w(2,0,0), no_wonders(), Fame::None);
        let card = region(21, Biome::Blue, true, false, no_wonders(), w(2,0,0), Fame::Flat(8));
        let score = score_region_card(&card, &[&visible], &[]);
        assert_eq!(score, 8);
    }

    #[test]
    fn flat_fame_quest_not_met() {
        let card = region(21, Biome::Blue, true, false, no_wonders(), w(2,0,0), Fame::Flat(8));
        let score = score_region_card(&card, &[], &[]);
        assert_eq!(score, 0);
    }

    #[test]
    fn per_icon_stone() {
        let v1 = region(1, Biome::Red, false, false, w(1,1,0), no_wonders(), Fame::None);
        let v2 = region(2, Biome::Blue, false, false, w(2,0,0), no_wonders(), Fame::None);
        let card = region(13, Biome::Blue, false, false, no_wonders(), no_wonders(), Fame::PerIcon { icon: Wonder::Stone, score_per: 2 });
        // 1 + 2 = 3 stone icons visible → 3 * 2 = 6
        let score = score_region_card(&card, &[&v1, &v2], &[]);
        assert_eq!(score, 6);
    }

    #[test]
    fn per_night() {
        let night1 = region(20, Biome::Green, true, true, no_wonders(), no_wonders(), Fame::None);
        let night2 = region(21, Biome::Blue, true, false, no_wonders(), no_wonders(), Fame::None);
        let card = region(10, Biome::Red, false, false, no_wonders(), no_wonders(), Fame::PerNight { score_per: 3 });
        let score = score_region_card(&card, &[&night1, &night2], &[]);
        assert_eq!(score, 6);
    }

    #[test]
    fn per_clue() {
        let c1 = region(6, Biome::Blue, false, true, no_wonders(), no_wonders(), Fame::None);
        let c2 = region(8, Biome::Green, false, true, no_wonders(), no_wonders(), Fame::None);
        let card = region(11, Biome::Green, false, false, no_wonders(), no_wonders(), Fame::PerClue { score_per: 3 });
        let score = score_region_card(&card, &[&c1, &c2], &[]);
        assert_eq!(score, 6);
    }

    #[test]
    fn per_colour() {
        let r1 = region(1, Biome::Red, false, false, no_wonders(), no_wonders(), Fame::None);
        let r2 = region(4, Biome::Red, false, false, no_wonders(), no_wonders(), Fame::None);
        let card = region(53, Biome::Yellow, false, false, w(0,1,0), no_wonders(), Fame::PerColour { biome: Biome::Red, score_per: 4 });
        let score = score_region_card(&card, &[&r1, &r2], &[]);
        assert_eq!(score, 8);
    }

    #[test]
    fn per_colour_pair() {
        let y1 = region(25, Biome::Yellow, true, false, no_wonders(), no_wonders(), Fame::None);
        let g1 = region(3, Biome::Green, false, false, no_wonders(), no_wonders(), Fame::None);
        let g2 = region(5, Biome::Green, false, false, no_wonders(), no_wonders(), Fame::None);
        let card = region(42, Biome::Yellow, false, false, no_wonders(), no_wonders(), Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Green, score_per: 2 });
        // 1 yellow + 2 green = 3 → 3 * 2 = 6
        let score = score_region_card(&card, &[&y1, &g1, &g2], &[]);
        assert_eq!(score, 6);
    }

    #[test]
    fn per_wonder_set() {
        let v1 = region(1, Biome::Red, false, false, w(1,1,0), no_wonders(), Fame::None);
        let v2 = region(7, Biome::Red, false, false, w(0,1,1), no_wonders(), Fame::None);
        // stone=1, chimera=2, thistle=1 → min=1 → 1*10=10
        let card = region(18, Biome::Green, false, false, w(0,1,0), no_wonders(), Fame::PerWonderSet { score_per: 10 });
        let score = score_region_card(&card, &[&v1, &v2], &[]);
        assert_eq!(score, 10);
    }

    #[test]
    fn per_colour_set() {
        let r = region(1, Biome::Red, false, false, no_wonders(), no_wonders(), Fame::None);
        let g = region(3, Biome::Green, false, false, no_wonders(), no_wonders(), Fame::None);
        let b = region(2, Biome::Blue, false, false, no_wonders(), no_wonders(), Fame::None);
        let y = region(12, Biome::Yellow, false, true, no_wonders(), no_wonders(), Fame::None);
        // min(1,1,1,1) = 1 → 1*10=10
        let card = region(23, Biome::Red, true, false, w(1,1,0), no_wonders(), Fame::PerColourSet { score_per: 10 });
        let score = score_region_card(&card, &[&r, &g, &b, &y], &[]);
        assert_eq!(score, 10);
    }

    /// Integration test: manually-calculated score for a known 8-card game.
    ///
    /// Tableau (played order, index 0 = first played = rightmost during scoring):
    ///   [0] #3  Green  Flat(4)               no quest
    ///   [1] #9  Blue   Flat(5)               no quest
    ///   [2] #11 Green  PerClue×3             no quest
    ///   [3] #13 Blue   PerIcon(Stone)×2      no quest
    ///   [4] #14 Red    PerNight×2            no quest
    ///   [5] #16 Red    PerIcon(Chimera)×2    no quest  wonders=(0,1,0)
    ///   [6] #25 Yellow PerColourPair(Y+G)×1  no quest  night=true
    ///   [7] #30 Red    PerIcon(Stone)×2      no quest  night=true  wonders=(1,0,0)
    ///
    /// Sanctuaries: tile24 Flat(5), tile1 PerColour(Green)×1
    ///
    /// Hand-calculated total = 28:
    /// (Each card counts itself + cards to its right + all sanctuaries.)
    ///   i=7 PerIcon(Stone)×2:     visible=[30]+sancts;          stone=1(30)       → 2
    ///   i=6 PerColourPair(Y+G)×1: visible=[25,30]+sancts;      Y=1(25),G=1(t1)  → 2
    ///   i=5 PerIcon(Chimera)×2:   visible=[16,25,30]+sancts;   chimera=1(16)     → 2
    ///   i=4 PerNight×2:           visible=[14,16,25,30]+sancts; nights=2(25,30)  → 4
    ///   i=3 PerIcon(Stone)×2:     visible=[13,14,16,25,30]+s;  stone=1(30)       → 2
    ///   i=2 PerClue×3:            visible=[11..30]+sancts;     clues=0            → 0
    ///   i=1 Flat(5):              no quest                                        → 5
    ///   i=0 Flat(4):              no quest                                        → 4
    ///   tile24 Flat(5):           full tableau + [tile1]                          → 5
    ///   tile1  PerColour(Green)×1:full tableau + [tile24];     G=2(cards 3,11)   → 2
    ///   Total = 2+2+2+4+2+0+5+4+5+2 = 28
    #[test]
    fn known_game_score_matches_hand_calculation() {
        use crate::cards::Wonder;
        use crate::game::PlayerState;

        let tableau = vec![
            RegionCard { number: 3,  biome: Biome::Green,  night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::Flat(4) },
            RegionCard { number: 9,  biome: Biome::Blue,   night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::Flat(5) },
            RegionCard { number: 11, biome: Biome::Green,  night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerClue { score_per: 3 } },
            RegionCard { number: 13, biome: Biome::Blue,   night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 2 } },
            RegionCard { number: 14, biome: Biome::Red,    night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerNight { score_per: 2 } },
            RegionCard { number: 16, biome: Biome::Red,    night: false, clue: false, wonders: w(0,1,0),           quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 2 } },
            RegionCard { number: 25, biome: Biome::Yellow, night: true,  clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Green, score_per: 1 } },
            RegionCard { number: 30, biome: Biome::Red,    night: true,  clue: false, wonders: w(1,0,0),           quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 2 } },
        ];
        let sanctuaries = vec![
            SanctuaryCard { tile: 24, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::Flat(5) },
            SanctuaryCard { tile: 1,  biome: Biome::Green,     night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Green, score_per: 1 } },
        ];
        let player = PlayerState { seat: 0, name: "Test".into(), tableau, sanctuaries, hand: vec![], played_this_round: None };
        assert_eq!(super::score_player(&player), 28);
    }

    #[test]
    fn detailed_breakdown_matches_total() {
        use crate::cards::Wonder;
        use crate::game::PlayerState;

        let tableau = vec![
            RegionCard { number: 3,  biome: Biome::Green,  night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::Flat(4) },
            RegionCard { number: 9,  biome: Biome::Blue,   night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::Flat(5) },
            RegionCard { number: 11, biome: Biome::Green,  night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerClue { score_per: 3 } },
            RegionCard { number: 13, biome: Biome::Blue,   night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 2 } },
            RegionCard { number: 14, biome: Biome::Red,    night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerNight { score_per: 2 } },
            RegionCard { number: 16, biome: Biome::Red,    night: false, clue: false, wonders: w(0,1,0),           quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 2 } },
            RegionCard { number: 25, biome: Biome::Yellow, night: true,  clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Green, score_per: 1 } },
            RegionCard { number: 30, biome: Biome::Red,    night: true,  clue: false, wonders: w(1,0,0),           quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 2 } },
        ];
        let sanctuaries = vec![
            SanctuaryCard { tile: 24, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::Flat(5) },
            SanctuaryCard { tile: 1,  biome: Biome::Green,     night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Green, score_per: 1 } },
        ];
        let player = PlayerState { seat: 0, name: "Test".into(), tableau, sanctuaries, hand: vec![], played_this_round: None };

        let detail = super::score_player_detailed(&player);
        // Should have 8 region + 2 sanctuary = 10 entries
        assert_eq!(detail.len(), 10);
        // Sum of detail points should match total
        let detail_total: u32 = detail.iter().map(|e| e.points).sum();
        assert_eq!(detail_total, 28);
        // First entry should be rightmost card (#30)
        assert_eq!(detail[0].number, 30);
        assert_eq!(detail[0].kind, "region");
        assert_eq!(detail[0].points, 2);
        // Last two should be sanctuaries
        assert_eq!(detail[8].kind, "sanctuary");
        assert_eq!(detail[9].kind, "sanctuary");
    }

    /// Test Case: "Night Patrol" — a night-heavy strategy with thistle icons.
    ///
    /// Tableau (played order, left-to-right; scored RIGHT-to-LEFT):
    ///
    ///   ┌──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
    ///   │ #10      │ #14      │ #22      │ #26      │ #29      │ #12      │ #7       │ #1       │
    ///   │ Red      │ Red      │ Green    │ Red      │ Yellow   │ Yellow   │ Red      │ Red      │
    ///   │          │          │ 🌙 📎   │ 🌙       │ 🌙       │ 📎      │          │          │
    ///   │          │          │          │ chimera:1│ thistle:1│ thistle:1│ chim:1   │ stone:1  │
    ///   │          │          │          │          │          │          │ thist:1  │ chim:1   │
    ///   │ PerNight │ PerNight │ PerClue  │ PerIcon  │ PerIcon  │ (none)   │ (none)   │ (none)   │
    ///   │ ×3       │ ×2       │ ×1       │ Thistle  │ Thistle  │          │          │          │
    ///   │          │          │          │ ×3       │ ×2       │          │          │          │
    ///   └──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘
    ///     scored     scored     scored     scored     scored     scored     scored     scored
    ///     last(8th)  7th        6th        5th        4th        3rd        2nd        1st
    ///
    /// Sanctuaries: tile34 (Green, PerNight×1), tile42 (Red, night=🌙)
    ///
    /// Scoring (right-to-left, card counts itself + cards to right + sanctuaries):
    ///   #1  (1st): Fame::None                                                       → 0 pts
    ///   #7  (2nd): Fame::None                                                       → 0 pts
    ///   #12 (3rd): Fame::None                                                       → 0 pts
    ///   #29 (4th): PerIcon(Thistle)×2, thistle: #29(1)+#12(1)+#7(1) = 3            → 6 pts
    ///   #26 (5th): PerIcon(Thistle)×3, thistle: #29(1)+#12(1)+#7(1) = 3 (#26=0)   → 9 pts
    ///   #22 (6th): PerClue×1, clues: #22(1)+#12(1) = 2                             → 2 pts
    ///   #14 (7th): PerNight×2, nights: #22+#26+#29+tile42 = 4                      → 8 pts
    ///   #10 (8th): PerNight×3, nights: #22+#26+#29+tile42 = 4                      → 12 pts
    ///   tile34:    PerNight×1, all nights: #22+#26+#29+tile42 = 4                   → 4 pts
    ///   tile42:    Fame::None                                                       → 0 pts
    ///                                                                         Total: 41 pts
    #[test]
    fn visual_scoring_night_patrol() {
        use crate::cards::SanctuaryCard;
        use crate::game::PlayerState;

        let tableau = vec![
            // [0] #10 Red — PerNight×3
            RegionCard { number: 10, biome: Biome::Red, night: false, clue: false,
                wonders: WonderCount::zero(), quest: WonderCount::zero(),
                fame: Fame::PerNight { score_per: 3 } },
            // [1] #14 Red — PerNight×2
            RegionCard { number: 14, biome: Biome::Red, night: false, clue: false,
                wonders: WonderCount::zero(), quest: WonderCount::zero(),
                fame: Fame::PerNight { score_per: 2 } },
            // [2] #22 Green — night, clue, PerClue×1
            RegionCard { number: 22, biome: Biome::Green, night: true, clue: true,
                wonders: WonderCount::zero(), quest: WonderCount::zero(),
                fame: Fame::PerClue { score_per: 1 } },
            // [3] #26 Red — night, chimera:1, PerIcon(Thistle)×3
            RegionCard { number: 26, biome: Biome::Red, night: true, clue: false,
                wonders: w(0,1,0), quest: WonderCount::zero(),
                fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 3 } },
            // [4] #29 Yellow — night, thistle:1, PerIcon(Thistle)×2
            RegionCard { number: 29, biome: Biome::Yellow, night: true, clue: false,
                wonders: w(0,0,1), quest: WonderCount::zero(),
                fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 2 } },
            // [5] #12 Yellow — clue, thistle:1, no fame
            RegionCard { number: 12, biome: Biome::Yellow, night: false, clue: true,
                wonders: w(0,0,1), quest: WonderCount::zero(),
                fame: Fame::None },
            // [6] #7 Red — chimera:1 thistle:1, no fame
            RegionCard { number: 7, biome: Biome::Red, night: false, clue: false,
                wonders: w(0,1,1), quest: WonderCount::zero(),
                fame: Fame::None },
            // [7] #1 Red — stone:1 chimera:1, no fame
            RegionCard { number: 1, biome: Biome::Red, night: false, clue: false,
                wonders: w(1,1,0), quest: WonderCount::zero(),
                fame: Fame::None },
        ];
        let sanctuaries = vec![
            // tile34 Green — PerNight×1
            SanctuaryCard { tile: 34, biome: Biome::Green, night: false, clue: false,
                wonders: WonderCount::zero(),
                fame: Fame::PerNight { score_per: 1 } },
            // tile42 Red — night
            SanctuaryCard { tile: 42, biome: Biome::Red, night: true, clue: false,
                wonders: WonderCount::zero(),
                fame: Fame::None },
        ];

        let player = PlayerState {
            seat: 0, name: "NightPatrol".into(),
            tableau, sanctuaries, hand: vec![], played_this_round: None,
        };

        // Per-card breakdown:  0 + 0 + 0 + 6 + 9 + 2 + 8 + 12 + (sanct: 4 + 0) = 41
        assert_eq!(score_player(&player), 41);
    }

    /// Test Case: "Quest Master" — quest-heavy strategy with stone/chimera providers.
    ///
    /// Tableau (played order, left-to-right; scored RIGHT-to-LEFT):
    ///
    ///   ┌──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
    ///   │ #46      │ #21      │ #20      │ #38      │ #4       │ #8       │ #19      │ #2       │
    ///   │ Blue     │ Blue     │ Green    │ Green    │ Red      │ Green    │ Red      │ Blue     │
    ///   │ 📎      │ 🌙      │ 🌙 📎   │ 🌙       │          │ 📎      │ thistle:1│ stone:2  │
    ///   │          │          │          │          │ stone:1  │ chim:1   │          │          │
    ///   │          │          │          │          │ chim:1   │          │          │          │
    ///   │ quest:   │ quest:   │ quest:   │ quest:   │          │          │ PerIcon  │          │
    ///   │ 🪨🪨🐉 │ 🪨🪨    │ 🪨      │ 🐉🌿    │ (none)   │ (none)   │ Thistle  │ (none)   │
    ///   │ Flat(10) │ Flat(8)  │ PerNight │ PerClue  │          │          │ ×2       │          │
    ///   │          │          │ ×2       │ ×3       │          │          │          │          │
    ///   └──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘
    ///     scored     scored     scored     scored     scored     scored     scored     scored
    ///     last(8th)  7th        6th        5th        4th        3rd        2nd        1st
    ///
    /// (🪨 = stone, 🐉 = chimera, 🌿 = thistle in quest requirements)
    ///
    /// Sanctuaries: tile28 (Colorless, 📎, thistle:1), tile32 (Colorless, PerClue×2)
    ///
    /// Scoring (right-to-left, card counts itself + cards to right + sanctuaries):
    ///   #2  (1st): Fame::None                                                            → 0 pts
    ///   #19 (2nd): PerIcon(Thistle)×2, thistle: #19(1)+tile28(1) = 2                    → 4 pts
    ///   #8  (3rd): Fame::None                                                            → 0 pts
    ///   #4  (4th): Fame::None                                                            → 0 pts
    ///   #38 (5th): PerClue×3, quest=(🐉🌿), chimera=2✓ thistle=2✓ → MET
    ///              clues: #8(1)+tile28(1) = 2                                            → 6 pts
    ///   #20 (6th): PerNight×2, quest=(🪨), stone=3✓ → MET
    ///              nights: #20(1)+#38(1) = 2                                             → 4 pts
    ///   #21 (7th): Flat(8), quest=(🪨🪨), stone=3✓ → MET                               → 8 pts
    ///   #46 (8th): Flat(10), quest=(🪨🪨🐉), stone=3✓ chimera=2✓ → MET                → 10 pts
    ///   tile28:    Fame::None                                                            → 0 pts
    ///   tile32:    PerClue×2, all clues: #46(1)+#20(1)+#8(1)+tile28(1) = 4              → 8 pts
    ///                                                                              Total: 40 pts
    #[test]
    fn visual_scoring_quest_master() {
        use crate::cards::SanctuaryCard;
        use crate::game::PlayerState;

        let tableau = vec![
            // [0] #46 Blue — clue, quest=(2,1,0), Flat(10)
            RegionCard { number: 46, biome: Biome::Blue, night: false, clue: true,
                wonders: WonderCount::zero(), quest: w(2,1,0),
                fame: Fame::Flat(10) },
            // [1] #21 Blue — night, quest=(2,0,0), Flat(8)
            RegionCard { number: 21, biome: Biome::Blue, night: true, clue: false,
                wonders: WonderCount::zero(), quest: w(2,0,0),
                fame: Fame::Flat(8) },
            // [2] #20 Green — night, clue, quest=(1,0,0), PerNight×2
            RegionCard { number: 20, biome: Biome::Green, night: true, clue: true,
                wonders: WonderCount::zero(), quest: w(1,0,0),
                fame: Fame::PerNight { score_per: 2 } },
            // [3] #38 Green — night, quest=(0,1,1), PerClue×3
            RegionCard { number: 38, biome: Biome::Green, night: true, clue: false,
                wonders: WonderCount::zero(), quest: w(0,1,1),
                fame: Fame::PerClue { score_per: 3 } },
            // [4] #4 Red — stone:1 chimera:1
            RegionCard { number: 4, biome: Biome::Red, night: false, clue: false,
                wonders: w(1,1,0), quest: WonderCount::zero(),
                fame: Fame::None },
            // [5] #8 Green — clue, chimera:1
            RegionCard { number: 8, biome: Biome::Green, night: false, clue: true,
                wonders: w(0,1,0), quest: WonderCount::zero(),
                fame: Fame::None },
            // [6] #19 Red — thistle:1, PerIcon(Thistle)×2
            RegionCard { number: 19, biome: Biome::Red, night: false, clue: false,
                wonders: w(0,0,1), quest: WonderCount::zero(),
                fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 2 } },
            // [7] #2 Blue — stone:2
            RegionCard { number: 2, biome: Biome::Blue, night: false, clue: false,
                wonders: w(2,0,0), quest: WonderCount::zero(),
                fame: Fame::None },
        ];
        let sanctuaries = vec![
            // tile28 Colorless — clue, thistle:1
            SanctuaryCard { tile: 28, biome: Biome::Colorless, night: false, clue: true,
                wonders: w(0,0,1),
                fame: Fame::None },
            // tile32 Colorless — PerClue×2
            SanctuaryCard { tile: 32, biome: Biome::Colorless, night: false, clue: false,
                wonders: WonderCount::zero(),
                fame: Fame::PerClue { score_per: 2 } },
        ];

        let player = PlayerState {
            seat: 0, name: "QuestMaster".into(),
            tableau, sanctuaries, hand: vec![], played_this_round: None,
        };

        // Per-card breakdown:  0 + 4 + 0 + 0 + 6 + 4 + 8 + 10 + (sanct: 0 + 8) = 40
        assert_eq!(score_player(&player), 40);
    }

    /// Test Case: "Failed Expedition" — big quest cards that don't find enough icons.
    ///
    /// Tableau (played order, left-to-right; scored RIGHT-to-LEFT):
    ///
    ///   ┌──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
    ///   │ #68      │ #51      │ #21      │ #9       │ #6       │ #12      │ #3       │ #5       │
    ///   │ Blue     │ Blue     │ Blue     │ Blue     │ Blue     │ Yellow   │ Green    │ Green    │
    ///   │          │ stone:1  │ 🌙      │          │ 📎      │ 📎      │          │ chim:1   │
    ///   │          │          │          │          │ stone:1  │ thistle:1│          │          │
    ///   │ quest:   │ quest:   │ quest:   │          │          │          │          │          │
    ///   │ 🪨🪨🪨  │ 🪨🪨🪨  │ 🪨🪨    │          │          │          │          │          │
    ///   │ 🪨🪨    │ 🪨      │          │          │          │          │          │          │
    ///   │ Flat(24) │ Flat(14) │ Flat(8)  │ Flat(5)  │ (none)   │ (none)   │ Flat(4)  │ (none)   │
    ///   └──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘
    ///     scored     scored     scored     scored     scored     scored     scored     scored
    ///     last(8th)  7th        6th        5th        4th        3rd        2nd        1st
    ///
    /// Sanctuaries: tile12 (Blue, stone:1), tile3 (Blue, PerColour(Blue)×1)
    ///
    /// Scoring (right-to-left, card counts itself + cards to right + sanctuaries):
    ///   #5  (1st): Fame::None                                                            → 0 pts
    ///   #3  (2nd): Flat(4), no quest                                                     → 4 pts
    ///   #12 (3rd): Fame::None                                                            → 0 pts
    ///   #6  (4th): Fame::None                                                            → 0 pts
    ///   #9  (5th): Flat(5), no quest                                                     → 5 pts
    ///   #21 (6th): Flat(8), quest=(🪨🪨), stone: #6(1)+tile12(1) = 2 MET               → 8 pts
    ///   #51 (7th): Flat(14), quest=(🪨🪨🪨🪨), stone: #51(1)+#6(1)+tile12(1) = 3 FAIL → 0 pts
    ///   #68 (8th): Flat(24), quest=(🪨🪨🪨🪨🪨), stone: #51(1)+#6(1)+tile12(1) = 3 FAIL → 0 pts
    ///   tile12:    Fame::None                                                            → 0 pts
    ///   tile3:     PerColour(Blue)×1, blue: #68+#51+#21+#9+#6+tile12 = 6               → 6 pts
    ///                                                                              Total: 23 pts
    #[test]
    fn visual_scoring_failed_expedition() {
        use crate::cards::SanctuaryCard;
        use crate::game::PlayerState;

        let tableau = vec![
            // [0] #68 Blue — quest=(5,0,0), Flat(24) — needs 5 stone
            RegionCard { number: 68, biome: Biome::Blue, night: false, clue: false,
                wonders: WonderCount::zero(), quest: w(5,0,0),
                fame: Fame::Flat(24) },
            // [1] #51 Blue — stone:1, quest=(4,0,0), Flat(14) — needs 4 stone
            RegionCard { number: 51, biome: Biome::Blue, night: false, clue: false,
                wonders: w(1,0,0), quest: w(4,0,0),
                fame: Fame::Flat(14) },
            // [2] #21 Blue — night, quest=(2,0,0), Flat(8)
            RegionCard { number: 21, biome: Biome::Blue, night: true, clue: false,
                wonders: WonderCount::zero(), quest: w(2,0,0),
                fame: Fame::Flat(8) },
            // [3] #9 Blue — Flat(5)
            RegionCard { number: 9, biome: Biome::Blue, night: false, clue: false,
                wonders: WonderCount::zero(), quest: WonderCount::zero(),
                fame: Fame::Flat(5) },
            // [4] #6 Blue — clue, stone:1
            RegionCard { number: 6, biome: Biome::Blue, night: false, clue: true,
                wonders: w(1,0,0), quest: WonderCount::zero(),
                fame: Fame::None },
            // [5] #12 Yellow — clue, thistle:1
            RegionCard { number: 12, biome: Biome::Yellow, night: false, clue: true,
                wonders: w(0,0,1), quest: WonderCount::zero(),
                fame: Fame::None },
            // [6] #3 Green — Flat(4)
            RegionCard { number: 3, biome: Biome::Green, night: false, clue: false,
                wonders: WonderCount::zero(), quest: WonderCount::zero(),
                fame: Fame::Flat(4) },
            // [7] #5 Green — chimera:1
            RegionCard { number: 5, biome: Biome::Green, night: false, clue: false,
                wonders: w(0,1,0), quest: WonderCount::zero(),
                fame: Fame::None },
        ];
        let sanctuaries = vec![
            // tile12 Blue — stone:1
            SanctuaryCard { tile: 12, biome: Biome::Blue, night: false, clue: false,
                wonders: w(1,0,0),
                fame: Fame::None },
            // tile3 Blue — PerColour(Blue)×1
            SanctuaryCard { tile: 3, biome: Biome::Blue, night: false, clue: false,
                wonders: WonderCount::zero(),
                fame: Fame::PerColour { biome: Biome::Blue, score_per: 1 } },
        ];

        let player = PlayerState {
            seat: 0, name: "FailedExpedition".into(),
            tableau, sanctuaries, hand: vec![], played_this_round: None,
        };

        // Per-card breakdown:  0 + 4 + 0 + 0 + 5 + 8 + 0 + 0 + (sanct: 0 + 6) = 23
        assert_eq!(score_player(&player), 23);
    }

    #[test]
    fn sanctuary_clue_in_context() {
        use crate::cards::SanctuaryCard;
        let sanc = SanctuaryCard {
            tile: 32,
            biome: Biome::Colorless,
            night: false,
            clue: false,
            wonders: no_wonders(),
            fame: Fame::PerClue { score_per: 2 },
        };
        let c1 = region(6, Biome::Blue, false, true, no_wonders(), no_wonders(), Fame::None);
        let score = score_sanctuary(&sanc, &[&c1], &[]);
        assert_eq!(score, 2);
    }
}
