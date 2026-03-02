use crate::cards::{Biome, Fame, RegionCard, SanctuaryCard, Wonder, WonderCount};
use crate::game::PlayerState;

/// Score all 8 region cards + sanctuaries for one player.
/// Region cards are scored right-to-left (index 7 first, index 0 last).
/// "Visible context" when scoring card at index i = cards at i+1..7 + all sanctuaries.
pub fn score_player(player: &PlayerState) -> u32 {
    let tableau = &player.tableau;
    let sanctuaries = &player.sanctuaries;
    let mut total: u32 = 0;

    // Score each region card right-to-left.
    let len = tableau.len();
    for i in (0..len).rev() {
        let visible_regions: Vec<&RegionCard> = tableau[i + 1..].iter().collect();
        let visible_sanctuaries: Vec<&SanctuaryCard> = sanctuaries.iter().collect();
        total += score_region_card(&tableau[i], &visible_regions, &visible_sanctuaries);
    }

    // Score sanctuaries using full tableau + other sanctuaries.
    for (j, sanc) in sanctuaries.iter().enumerate() {
        let full_regions: Vec<&RegionCard> = tableau.iter().collect();
        let other_sanctuaries: Vec<&SanctuaryCard> = sanctuaries.iter()
            .enumerate()
            .filter(|&(k, _)| k != j)
            .map(|(_, s)| s)
            .collect();
        total += score_sanctuary(sanc, &full_regions, &other_sanctuaries);
    }

    total
}

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
    /// Hand-calculated total = 23:
    /// (Sanctuaries are always in visible context when scoring region cards.)
    ///   i=7 PerIcon(Stone)×2:     visible=[]+sancts;         stone=0           → 0
    ///   i=6 PerColourPair(Y+G)×1: visible=[30]+sancts;       Y=0,G=1(tile1)    → 1
    ///   i=5 PerIcon(Chimera)×2:   visible=[25,30]+sancts;    chimera=0         → 0
    ///   i=4 PerNight×2:           visible=[16,25,30]+sancts; nights=2(25,30)   → 4
    ///   i=3 PerIcon(Stone)×2:     visible=[14,16,25,30]+s;   stone=1(30)       → 2
    ///   i=2 PerClue×3:            visible=[13..30]+sancts;   clues=0           → 0
    ///   i=1 Flat(5):              no quest                                     → 5
    ///   i=0 Flat(4):              no quest                                     → 4
    ///   tile24 Flat(5):           full tableau + [tile1]                       → 5
    ///   tile1  PerColour(Green)×1:full tableau + [tile24];   G=2(cards 3,11)   → 2
    ///   Total = 0+1+0+4+2+0+5+4+5+2 = 23
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
        assert_eq!(super::score_player(&player), 23);
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
