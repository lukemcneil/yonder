use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    Red,
    Green,
    Blue,
    Yellow,
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wonder {
    Stone,
    Chimera,
    Thistle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WonderCount {
    pub stone: u8,
    pub chimera: u8,
    pub thistle: u8,
}

impl WonderCount {
    pub fn zero() -> Self {
        Self { stone: 0, chimera: 0, thistle: 0 }
    }

    pub fn is_zero(&self) -> bool {
        self.stone == 0 && self.chimera == 0 && self.thistle == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionCard {
    pub number: u8,
    pub biome: Biome,
    pub night: bool,
    pub clue: bool,
    pub wonders: WonderCount,
    pub quest: WonderCount,
    pub fame: Fame,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanctuaryCard {
    pub tile: u8,
    pub biome: Biome,
    pub night: bool,
    pub clue: bool,
    pub wonders: WonderCount,
    pub fame: Fame,
}

pub fn get_region_deck() -> Vec<RegionCard> {
    let mut deck = regions();
    deck.shuffle(&mut thread_rng());
    deck
}

pub fn get_region_deck_with_expansion() -> Vec<RegionCard> {
    let mut deck = regions();
    deck.extend(expansion_regions());
    deck.shuffle(&mut thread_rng());
    deck
}

pub fn get_sanctuary_deck() -> Vec<SanctuaryCard> {
    let mut deck = sanctuaries();
    deck.shuffle(&mut thread_rng());
    deck
}

pub fn get_sanctuary_deck_with_expansion() -> Vec<SanctuaryCard> {
    let mut deck = sanctuaries();
    deck.extend(expansion_sanctuaries());
    deck.shuffle(&mut thread_rng());
    deck
}

fn w(stone: u8, chimera: u8, thistle: u8) -> WonderCount {
    WonderCount { stone, chimera, thistle }
}

fn regions() -> Vec<RegionCard> {
    vec![
        RegionCard { number: 1, biome: Biome::Red, night: false, clue: false, wonders: w(1,1,0), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 2, biome: Biome::Blue, night: false, clue: false, wonders: w(2,0,0), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 3, biome: Biome::Green, night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::Flat(4) },
        RegionCard { number: 4, biome: Biome::Red, night: false, clue: false, wonders: w(1,0,1), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 5, biome: Biome::Green, night: false, clue: false, wonders: w(0,1,0), quest: WonderCount::zero(), fame: Fame::Flat(2) },
        RegionCard { number: 6, biome: Biome::Blue, night: false, clue: true, wonders: w(1,0,0), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 7, biome: Biome::Red, night: false, clue: false, wonders: w(0,1,1), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 8, biome: Biome::Green, night: false, clue: true, wonders: w(0,1,0), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 9, biome: Biome::Blue, night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::Flat(5) },
        RegionCard { number: 10, biome: Biome::Red, night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerNight { score_per: 3 } },
        RegionCard { number: 11, biome: Biome::Green, night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerClue { score_per: 2 } },
        RegionCard { number: 12, biome: Biome::Yellow, night: false, clue: true, wonders: w(0,0,1), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 13, biome: Biome::Blue, night: false, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 2 } },
        RegionCard { number: 14, biome: Biome::Red, night: false, clue: false, wonders: w(0,0,1), quest: WonderCount::zero(), fame: Fame::PerNight { score_per: 2 } },
        RegionCard { number: 15, biome: Biome::Green, night: false, clue: true, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 2 } },
        RegionCard { number: 16, biome: Biome::Red, night: false, clue: false, wonders: w(0,1,0), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 2 } },
        RegionCard { number: 17, biome: Biome::Blue, night: false, clue: false, wonders: w(1,0,0), quest: w(0,2,0), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 3 } },
        RegionCard { number: 18, biome: Biome::Green, night: false, clue: false, wonders: w(0,1,0), quest: WonderCount::zero(), fame: Fame::PerColourSet { score_per: 10 } },
        RegionCard { number: 19, biome: Biome::Red, night: false, clue: false, wonders: w(0,0,1), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 2 } },
        RegionCard { number: 20, biome: Biome::Green, night: true, clue: true, wonders: WonderCount::zero(), quest: w(1,0,0), fame: Fame::PerNight { score_per: 2 } },
        RegionCard { number: 21, biome: Biome::Blue, night: true, clue: false, wonders: WonderCount::zero(), quest: w(2,0,0), fame: Fame::Flat(8) },
        RegionCard { number: 22, biome: Biome::Green, night: true, clue: true, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerClue { score_per: 1 } },
        RegionCard { number: 23, biome: Biome::Red, night: true, clue: false, wonders: w(1,1,0), quest: WonderCount::zero(), fame: Fame::PerColourSet { score_per: 10 } },
        RegionCard { number: 24, biome: Biome::Blue, night: true, clue: false, wonders: w(1,0,0), quest: w(0,1,0), fame: Fame::PerNight { score_per: 2 } },
        RegionCard { number: 25, biome: Biome::Yellow, night: true, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Green, score_per: 1 } },
        RegionCard { number: 26, biome: Biome::Red, night: true, clue: false, wonders: w(0,1,0), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 3 } },
        RegionCard { number: 27, biome: Biome::Yellow, night: true, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Blue, score_per: 1 } },
        RegionCard { number: 28, biome: Biome::Red, night: true, clue: false, wonders: w(1,0,0), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 3 } },
        RegionCard { number: 29, biome: Biome::Yellow, night: true, clue: false, wonders: w(0,0,1), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 2 } },
        RegionCard { number: 30, biome: Biome::Red, night: true, clue: false, wonders: w(1,0,0), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 2 } },
        RegionCard { number: 31, biome: Biome::Yellow, night: true, clue: false, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Red, score_per: 1 } },
        RegionCard { number: 32, biome: Biome::Red, night: true, clue: false, wonders: w(1,1,0), quest: w(3,0,0), fame: Fame::Flat(7) },
        RegionCard { number: 33, biome: Biome::Yellow, night: true, clue: true, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 3 } },
        RegionCard { number: 34, biome: Biome::Green, night: true, clue: false, wonders: w(0,1,0), quest: w(2,0,0), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 3 } },
        RegionCard { number: 35, biome: Biome::Yellow, night: true, clue: false, wonders: w(0,1,0), quest: WonderCount::zero(), fame: Fame::PerColourSet { score_per: 10 } },
        RegionCard { number: 36, biome: Biome::Red, night: true, clue: false, wonders: WonderCount::zero(), quest: w(0,2,0), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 4 } },
        RegionCard { number: 37, biome: Biome::Yellow, night: true, clue: false, wonders: WonderCount::zero(), quest: w(0,0,1), fame: Fame::PerNight { score_per: 3 } },
        RegionCard { number: 38, biome: Biome::Green, night: true, clue: false, wonders: WonderCount::zero(), quest: w(0,1,1), fame: Fame::PerClue { score_per: 3 } },
        RegionCard { number: 39, biome: Biome::Red, night: true, clue: false, wonders: w(1,0,1), quest: w(0,2,0), fame: Fame::Flat(9) },
        RegionCard { number: 40, biome: Biome::Blue, night: true, clue: false, wonders: WonderCount::zero(), quest: w(1,1,1), fame: Fame::PerNight { score_per: 3 } },
        RegionCard { number: 41, biome: Biome::Green, night: false, clue: false, wonders: w(0,0,1), quest: w(2,1,0), fame: Fame::PerNight { score_per: 4 } },
        RegionCard { number: 42, biome: Biome::Yellow, night: false, clue: false, wonders: WonderCount::zero(), quest: w(1,1,0), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Green, score_per: 2 } },
        RegionCard { number: 43, biome: Biome::Blue, night: false, clue: false, wonders: w(1,0,0), quest: WonderCount::zero(), fame: Fame::PerColourSet { score_per: 10 } },
        RegionCard { number: 44, biome: Biome::Yellow, night: false, clue: false, wonders: WonderCount::zero(), quest: w(1,0,1), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Blue, score_per: 2 } },
        RegionCard { number: 45, biome: Biome::Green, night: false, clue: false, wonders: w(1,0,0), quest: w(0,3,0), fame: Fame::Flat(13) },
        RegionCard { number: 46, biome: Biome::Blue, night: false, clue: true, wonders: WonderCount::zero(), quest: w(2,1,0), fame: Fame::Flat(10) },
        RegionCard { number: 47, biome: Biome::Yellow, night: false, clue: false, wonders: WonderCount::zero(), quest: w(0,1,1), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Red, score_per: 2 } },
        RegionCard { number: 48, biome: Biome::Red, night: false, clue: false, wonders: w(0,1,0), quest: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 3 } },
        RegionCard { number: 49, biome: Biome::Blue, night: false, clue: true, wonders: WonderCount::zero(), quest: w(2,0,1), fame: Fame::Flat(12) },
        RegionCard { number: 50, biome: Biome::Yellow, night: false, clue: false, wonders: w(1,0,0), quest: w(0,0,2), fame: Fame::PerColour { biome: Biome::Green, score_per: 4 } },
        RegionCard { number: 51, biome: Biome::Blue, night: false, clue: false, wonders: w(1,0,0), quest: w(4,0,0), fame: Fame::Flat(14) },
        RegionCard { number: 52, biome: Biome::Red, night: false, clue: false, wonders: WonderCount::zero(), quest: w(3,0,0), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 4 } },
        RegionCard { number: 53, biome: Biome::Yellow, night: false, clue: false, wonders: w(0,1,0), quest: w(0,0,2), fame: Fame::PerColour { biome: Biome::Red, score_per: 4 } },
        RegionCard { number: 54, biome: Biome::Green, night: false, clue: false, wonders: w(0,1,0), quest: w(0,0,2), fame: Fame::PerClue { score_per: 4 } },
        RegionCard { number: 55, biome: Biome::Blue, night: false, clue: true, wonders: w(1,0,0), quest: w(0,1,2), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 3 } },
        RegionCard { number: 56, biome: Biome::Yellow, night: false, clue: false, wonders: w(0,0,1), quest: w(1,2,0), fame: Fame::PerColour { biome: Biome::Blue, score_per: 4 } },
        RegionCard { number: 57, biome: Biome::Red, night: false, clue: false, wonders: WonderCount::zero(), quest: w(0,0,3), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 4 } },
        RegionCard { number: 58, biome: Biome::Green, night: false, clue: true, wonders: WonderCount::zero(), quest: w(0,3,0), fame: Fame::PerClue { score_per: 3 } },
        RegionCard { number: 59, biome: Biome::Yellow, night: false, clue: true, wonders: WonderCount::zero(), quest: w(1,3,0), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Red, score_per: 3 } },
        RegionCard { number: 60, biome: Biome::Blue, night: false, clue: true, wonders: WonderCount::zero(), quest: w(2,2,0), fame: Fame::Flat(16) },
        RegionCard { number: 61, biome: Biome::Green, night: false, clue: false, wonders: w(0,0,1), quest: w(0,4,0), fame: Fame::Flat(17) },
        RegionCard { number: 62, biome: Biome::Yellow, night: false, clue: true, wonders: WonderCount::zero(), quest: w(0,0,3), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Blue, score_per: 3 } },
        RegionCard { number: 63, biome: Biome::Green, night: false, clue: true, wonders: WonderCount::zero(), quest: w(0,2,1), fame: Fame::Flat(15) },
        RegionCard { number: 64, biome: Biome::Blue, night: false, clue: true, wonders: WonderCount::zero(), quest: w(2,0,2), fame: Fame::Flat(18) },
        RegionCard { number: 65, biome: Biome::Yellow, night: false, clue: true, wonders: WonderCount::zero(), quest: w(0,0,3), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Green, score_per: 3 } },
        RegionCard { number: 66, biome: Biome::Blue, night: false, clue: false, wonders: WonderCount::zero(), quest: w(4,0,0), fame: Fame::Flat(20) },
        RegionCard { number: 67, biome: Biome::Green, night: false, clue: true, wonders: WonderCount::zero(), quest: w(0,2,2), fame: Fame::Flat(19) },
        RegionCard { number: 68, biome: Biome::Blue, night: false, clue: false, wonders: WonderCount::zero(), quest: w(5,0,0), fame: Fame::Flat(24) },
    ]
}

fn expansion_regions() -> Vec<RegionCard> {
    vec![
        RegionCard { number: 0, biome: Biome::Colorless, night: false, clue: false, wonders: w(1,1,1), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 69, biome: Biome::Red, night: false, clue: true, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::PerWonderSet { score_per: 7 } },
        RegionCard { number: 70, biome: Biome::Colorless, night: false, clue: true, wonders: WonderCount::zero(), quest: WonderCount::zero(), fame: Fame::None },
        RegionCard { number: 71, biome: Biome::Green, night: true, clue: false, wonders: w(1,0,0), quest: WonderCount::zero(), fame: Fame::PerWonderSet { score_per: 7 } },
        RegionCard { number: 72, biome: Biome::Colorless, night: true, clue: true, wonders: WonderCount::zero(), quest: w(0,5,0), fame: Fame::Flat(26) },
        RegionCard { number: 73, biome: Biome::Yellow, night: true, clue: false, wonders: w(0,1,0), quest: w(0,0,4), fame: Fame::PerColour { biome: Biome::Colorless, score_per: 5 } },
        RegionCard { number: 74, biome: Biome::Colorless, night: true, clue: false, wonders: w(0,0,1), quest: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Colorless, score_per: 7 } },
        RegionCard { number: 75, biome: Biome::Blue, night: true, clue: true, wonders: WonderCount::zero(), quest: w(6,0,0), fame: Fame::Flat(28) },
        RegionCard { number: 76, biome: Biome::Colorless, night: true, clue: false, wonders: WonderCount::zero(), quest: w(2,2,2), fame: Fame::PerColour { biome: Biome::Colorless, score_per: 4 } },
    ]
}

fn expansion_sanctuaries() -> Vec<SanctuaryCard> {
    vec![
        SanctuaryCard { tile: 46, biome: Biome::Colorless, night: false, clue: false, wonders: w(2,0,0), fame: Fame::None },
        SanctuaryCard { tile: 47, biome: Biome::Colorless, night: false, clue: true, wonders: WonderCount::zero(), fame: Fame::PerWonderSet { score_per: 3 } },
        SanctuaryCard { tile: 48, biome: Biome::Colorless, night: false, clue: false, wonders: w(0,1,0), fame: Fame::PerColour { biome: Biome::Colorless, score_per: 1 } },
        SanctuaryCard { tile: 49, biome: Biome::Colorless, night: false, clue: false, wonders: w(0,0,1), fame: Fame::PerColour { biome: Biome::Colorless, score_per: 1 } },
        SanctuaryCard { tile: 50, biome: Biome::Green, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerWonderSet { score_per: 3 } },
        SanctuaryCard { tile: 51, biome: Biome::Blue, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Colorless, score_per: 2 } },
        SanctuaryCard { tile: 52, biome: Biome::Yellow, night: true, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Colorless, score_per: 1 } },
        SanctuaryCard { tile: 53, biome: Biome::Red, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerWonderSet { score_per: 3 } },
    ]
}

fn sanctuaries() -> Vec<SanctuaryCard> {
    vec![
        SanctuaryCard { tile: 1, biome: Biome::Green, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Green, score_per: 1 } },
        SanctuaryCard { tile: 2, biome: Biome::Red, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Red, score_per: 1 } },
        SanctuaryCard { tile: 3, biome: Biome::Blue, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Blue, score_per: 1 } },
        SanctuaryCard { tile: 4, biome: Biome::Yellow, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColour { biome: Biome::Yellow, score_per: 1 } },
        SanctuaryCard { tile: 5, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Blue, biome2: Biome::Yellow, score_per: 1 } },
        SanctuaryCard { tile: 6, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Green, biome2: Biome::Red, score_per: 1 } },
        SanctuaryCard { tile: 7, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Red, biome2: Biome::Yellow, score_per: 1 } },
        SanctuaryCard { tile: 8, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Yellow, biome2: Biome::Green, score_per: 1 } },
        SanctuaryCard { tile: 9, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Green, biome2: Biome::Blue, score_per: 1 } },
        SanctuaryCard { tile: 10, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColourPair { biome1: Biome::Red, biome2: Biome::Blue, score_per: 1 } },
        SanctuaryCard { tile: 11, biome: Biome::Blue, night: false, clue: false, wonders: w(0,1,0), fame: Fame::None },
        SanctuaryCard { tile: 12, biome: Biome::Blue, night: false, clue: false, wonders: w(1,0,0), fame: Fame::None },
        SanctuaryCard { tile: 13, biome: Biome::Blue, night: false, clue: false, wonders: w(0,0,1), fame: Fame::None },
        SanctuaryCard { tile: 14, biome: Biome::Green, night: false, clue: false, wonders: w(0,1,0), fame: Fame::None },
        SanctuaryCard { tile: 15, biome: Biome::Green, night: false, clue: false, wonders: w(1,0,0), fame: Fame::None },
        SanctuaryCard { tile: 16, biome: Biome::Green, night: false, clue: true, wonders: WonderCount::zero(), fame: Fame::None },
        SanctuaryCard { tile: 17, biome: Biome::Red, night: false, clue: false, wonders: w(0,1,0), fame: Fame::None },
        SanctuaryCard { tile: 18, biome: Biome::Red, night: false, clue: false, wonders: w(0,0,1), fame: Fame::None },
        SanctuaryCard { tile: 19, biome: Biome::Red, night: false, clue: false, wonders: w(1,0,0), fame: Fame::None },
        SanctuaryCard { tile: 20, biome: Biome::Yellow, night: false, clue: false, wonders: w(1,0,0), fame: Fame::None },
        SanctuaryCard { tile: 21, biome: Biome::Yellow, night: false, clue: true, wonders: WonderCount::zero(), fame: Fame::None },
        SanctuaryCard { tile: 22, biome: Biome::Yellow, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerColourSet { score_per: 4 } },
        SanctuaryCard { tile: 23, biome: Biome::Yellow, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerClue { score_per: 1 } },
        SanctuaryCard { tile: 24, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::Flat(5) },
        SanctuaryCard { tile: 25, biome: Biome::Blue, night: false, clue: true, wonders: WonderCount::zero(), fame: Fame::None },
        SanctuaryCard { tile: 26, biome: Biome::Colorless, night: false, clue: true, wonders: w(1,0,0), fame: Fame::None },
        SanctuaryCard { tile: 27, biome: Biome::Colorless, night: false, clue: true, wonders: w(0,1,0), fame: Fame::None },
        SanctuaryCard { tile: 28, biome: Biome::Colorless, night: false, clue: true, wonders: w(0,0,1), fame: Fame::None },
        SanctuaryCard { tile: 29, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 2 } },
        SanctuaryCard { tile: 30, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 2 } },
        SanctuaryCard { tile: 31, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 2 } },
        SanctuaryCard { tile: 32, biome: Biome::Colorless, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerClue { score_per: 2 } },
        SanctuaryCard { tile: 33, biome: Biome::Colorless, night: false, clue: true, wonders: WonderCount::zero(), fame: Fame::PerColourSet { score_per: 4 } },
        SanctuaryCard { tile: 34, biome: Biome::Green, night: false, clue: false, wonders: WonderCount::zero(), fame: Fame::PerNight { score_per: 1 } },
        SanctuaryCard { tile: 35, biome: Biome::Colorless, night: false, clue: false, wonders: w(1,0,0), fame: Fame::PerNight { score_per: 1 } },
        SanctuaryCard { tile: 36, biome: Biome::Colorless, night: false, clue: false, wonders: w(0,1,0), fame: Fame::PerClue { score_per: 1 } },
        SanctuaryCard { tile: 37, biome: Biome::Colorless, night: false, clue: false, wonders: w(1,0,0), fame: Fame::PerClue { score_per: 1 } },
        SanctuaryCard { tile: 38, biome: Biome::Colorless, night: false, clue: true, wonders: WonderCount::zero(), fame: Fame::PerClue { score_per: 1 } },
        SanctuaryCard { tile: 39, biome: Biome::Colorless, night: false, clue: false, wonders: w(1,0,0), fame: Fame::PerIcon { icon: Wonder::Stone, score_per: 1 } },
        SanctuaryCard { tile: 40, biome: Biome::Colorless, night: false, clue: false, wonders: w(0,1,0), fame: Fame::PerIcon { icon: Wonder::Chimera, score_per: 1 } },
        SanctuaryCard { tile: 41, biome: Biome::Colorless, night: false, clue: false, wonders: w(0,0,1), fame: Fame::PerIcon { icon: Wonder::Thistle, score_per: 1 } },
        SanctuaryCard { tile: 42, biome: Biome::Red, night: true, clue: false, wonders: WonderCount::zero(), fame: Fame::None },
        SanctuaryCard { tile: 43, biome: Biome::Colorless, night: true, clue: false, wonders: w(1,0,0), fame: Fame::None },
        SanctuaryCard { tile: 44, biome: Biome::Colorless, night: true, clue: false, wonders: w(0,1,0), fame: Fame::None },
        SanctuaryCard { tile: 45, biome: Biome::Colorless, night: true, clue: false, wonders: w(0,0,1), fame: Fame::None },
    ]
}
