use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cards::{RegionCard, SanctuaryCard};
use crate::cards::{get_region_deck, get_sanctuary_deck};

// ─── Phase ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    WaitingForPlayers { needed: usize },
    Playing(RoundPhase),
    GameOver { scores: Vec<PlayerScore> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoundPhase {
    /// All players are choosing which card to play this round.
    ChoosingCards,
    /// Cards have been played; process sanctuaries for each eligible player in
    /// order of seat, then move to Drafting.
    RevealingAndSanctuaries,
    /// All eligible players choose a sanctuary simultaneously.
    SanctuaryChoice {
        /// Each eligible seat's drawn choices (removed once they pick).
        pending: HashMap<usize, Vec<SanctuaryCard>>,
    },
    /// Players draft from market in `order`; `current` indexes into `order`.
    Drafting {
        order: Vec<usize>,
        current: usize,
    },
}

// ─── Player state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub seat: usize,
    pub name: String,
    /// Cards played into tableau, in order (index 0 = first played = rightmost during scoring).
    pub tableau: Vec<RegionCard>,
    /// Sanctuaries kept.
    pub sanctuaries: Vec<SanctuaryCard>,
    /// Current hand (3 cards between rounds; varies during round).
    pub hand: Vec<RegionCard>,
    /// Card played this round (face-down until reveal).
    pub played_this_round: Option<RegionCard>,
}

impl PlayerState {
    fn new(seat: usize, name: String) -> Self {
        Self {
            seat,
            name,
            tableau: Vec::new(),
            sanctuaries: Vec::new(),
            hand: Vec::new(),
            played_this_round: None,
        }
    }
}

// ─── Scoring ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerScore {
    pub seat: usize,
    pub name: String,
    pub total: u32,
    /// Sum of card numbers (tiebreaker: lower is better).
    pub card_number_sum: u32,
}

// ─── Full game state ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub phase: GamePhase,
    pub round: u8,
    pub players: Vec<PlayerState>,
    pub region_deck: Vec<RegionCard>,
    pub sanctuary_deck: Vec<SanctuaryCard>,
    /// Face-up market cards (N+1 where N = player count).
    pub market: Vec<RegionCard>,
    pub player_count: usize,
}

impl GameState {
    pub fn new_waiting(max_players: usize) -> Self {
        Self {
            phase: GamePhase::WaitingForPlayers { needed: max_players },
            round: 0,
            players: Vec::new(),
            region_deck: get_region_deck(),
            sanctuary_deck: get_sanctuary_deck(),
            market: Vec::new(),
            player_count: max_players,
        }
    }

    /// Add a player to the waiting room. Returns their seat index, or an error
    /// if the room is full or the game has already started.
    pub fn join(&mut self, name: &str) -> Result<usize, ActionError> {
        match &self.phase {
            GamePhase::WaitingForPlayers { .. } => {}
            _ => return Err(ActionError::GameAlreadyStarted),
        }
        if self.players.iter().any(|p| p.name == name) {
            // Re-joining is fine — return their existing seat.
            return Ok(self.players.iter().position(|p| p.name == name).unwrap());
        }
        if self.players.len() >= self.player_count {
            return Err(ActionError::RoomFull);
        }
        let seat = self.players.len();
        self.players.push(PlayerState::new(seat, name.to_string()));
        Ok(seat)
    }

    /// Start the game. Deals 3 cards to each player, reveals market.
    pub fn start_game(&mut self, seat: usize) -> Result<(), ActionError> {
        if seat != 0 {
            return Err(ActionError::NotYourTurn);
        }
        match &self.phase {
            GamePhase::WaitingForPlayers { .. } => {}
            _ => return Err(ActionError::GameAlreadyStarted),
        }
        if self.players.len() < 2 {
            return Err(ActionError::NotEnoughPlayers);
        }
        // Lock in player count to however many joined.
        self.player_count = self.players.len();
        // Deal 3 cards to each player.
        for player in &mut self.players {
            for _ in 0..3 {
                let card = self.region_deck.pop().ok_or(ActionError::DeckEmpty)?;
                player.hand.push(card);
            }
        }
        // Reveal market: player_count + 1 cards.
        for _ in 0..=self.players.len() {
            let card = self.region_deck.pop().ok_or(ActionError::DeckEmpty)?;
            self.market.push(card);
        }
        self.round = 1;
        self.phase = GamePhase::Playing(RoundPhase::ChoosingCards);
        Ok(())
    }

    /// Play a card from hand (ChoosingCards phase).
    pub fn play_card(&mut self, seat: usize, card_index: usize) -> Result<(), ActionError> {
        self.require_phase_choosing_cards()?;
        let player = self.player_mut(seat)?;
        if player.played_this_round.is_some() {
            return Err(ActionError::AlreadyPlayedThisRound);
        }
        if card_index >= player.hand.len() {
            return Err(ActionError::InvalidCardIndex);
        }
        let card = player.hand.remove(card_index);
        player.played_this_round = Some(card);
        // If all players have played, advance phase.
        if self.players.iter().all(|p| p.played_this_round.is_some()) {
            self.advance_to_reveal();
        }
        Ok(())
    }

    /// Choose a sanctuary to keep (SanctuaryChoice phase). All eligible players
    /// choose simultaneously; once everyone has chosen, advance to drafting.
    pub fn choose_sanctuary(&mut self, seat: usize, sanctuary_index: usize) -> Result<(), ActionError> {
        let pending = match &mut self.phase {
            GamePhase::Playing(RoundPhase::SanctuaryChoice { pending }) => pending,
            _ => return Err(ActionError::WrongPhase),
        };
        let choices = pending.remove(&seat).ok_or(ActionError::NotYourTurn)?;
        if sanctuary_index >= choices.len() {
            // Put choices back so the player can retry.
            if let GamePhase::Playing(RoundPhase::SanctuaryChoice { pending }) = &mut self.phase {
                pending.insert(seat, choices);
            }
            return Err(ActionError::InvalidCardIndex);
        }
        let kept = choices[sanctuary_index].clone();
        self.player_mut(seat)?.sanctuaries.push(kept);
        // If all players have chosen, advance to drafting.
        let all_done = match &self.phase {
            GamePhase::Playing(RoundPhase::SanctuaryChoice { pending }) => pending.is_empty(),
            _ => false,
        };
        if all_done {
            self.begin_drafting();
        }
        Ok(())
    }

    /// Draft a card from the market (Drafting phase).
    pub fn draft_card(&mut self, seat: usize, market_index: usize) -> Result<(), ActionError> {
        let (order, current) = match &self.phase {
            GamePhase::Playing(RoundPhase::Drafting { order, current }) => {
                (order.clone(), *current)
            }
            _ => return Err(ActionError::WrongPhase),
        };
        if order[current] != seat {
            return Err(ActionError::NotYourTurn);
        }
        if market_index >= self.market.len() {
            return Err(ActionError::InvalidCardIndex);
        }
        let card = self.market.remove(market_index);
        self.player_mut(seat)?.hand.push(card);
        let next = current + 1;
        if next >= order.len() {
            // Last drafter: end of round.
            self.end_round()?;
        } else {
            self.phase = GamePhase::Playing(RoundPhase::Drafting { order, current: next });
        }
        Ok(())
    }

    // ─── Internal helpers ──────────────────────────────────────────────────

    fn advance_to_reveal(&mut self) {
        // Commit played cards to tableau.
        for player in &mut self.players {
            if let Some(card) = player.played_this_round.take() {
                player.tableau.push(card);
            }
        }
        // Determine sanctuary eligibility: played number > previous number in tableau.
        // "Previous" = the card just before the one played this round (index len-2).
        let eligible_seats: Vec<usize> = self.players.iter().filter_map(|p| {
            let len = p.tableau.len();
            if len < 2 {
                // First card played — no previous to compare against.
                None
            } else {
                let played = &p.tableau[len - 1];
                let previous = &p.tableau[len - 2];
                if played.number > previous.number {
                    Some(p.seat)
                } else {
                    None
                }
            }
        }).collect();

        if eligible_seats.is_empty() {
            self.begin_drafting();
        } else {
            let mut pending = HashMap::new();
            for seat in eligible_seats {
                let choices = self.draw_sanctuary_choices(seat);
                pending.insert(seat, choices);
            }
            self.phase = GamePhase::Playing(RoundPhase::SanctuaryChoice { pending });
        }
    }

    fn draw_sanctuary_choices(&mut self, seat: usize) -> Vec<SanctuaryCard> {
        // Draw 1 + (clue icons across tableau cards AND held sanctuary cards).
        let clue_count = self.players[seat].tableau.iter().filter(|c| c.clue).count()
            + self.players[seat].sanctuaries.iter().filter(|c| c.clue).count();
        let draw_count = 1 + clue_count;
        let mut choices = Vec::new();
        for _ in 0..draw_count {
            if let Some(card) = self.sanctuary_deck.pop() {
                choices.push(card);
            }
        }
        choices
    }

    fn begin_drafting(&mut self) {
        // Round 8 exception: no drafting after the last round.
        if self.round == 8 {
            self.market.clear();
            self.finalize_scores();
            return;
        }
        // Draft order: ascending card number order of played cards.
        let mut order: Vec<(usize, u8)> = self.players.iter().map(|p| {
            let played_num = p.tableau.last().map(|c| c.number).unwrap_or(0);
            (p.seat, played_num)
        }).collect();
        order.sort_by_key(|&(_, num)| num);
        let seat_order: Vec<usize> = order.into_iter().map(|(s, _)| s).collect();
        self.phase = GamePhase::Playing(RoundPhase::Drafting {
            order: seat_order,
            current: 0,
        });
    }

    fn end_round(&mut self) -> Result<(), ActionError> {
        // Discard leftover market card (there should be exactly 1 left).
        self.market.clear();
        if self.round == 8 {
            self.finalize_scores();
            return Ok(());
        }
        // Refill market for next round.
        self.round += 1;
        for _ in 0..=self.players.len() {
            if let Some(card) = self.region_deck.pop() {
                self.market.push(card);
            }
        }
        self.phase = GamePhase::Playing(RoundPhase::ChoosingCards);
        Ok(())
    }

    fn finalize_scores(&mut self) {
        use crate::scoring::score_player;
        let scores: Vec<PlayerScore> = self.players.iter().map(|p| {
            let total = score_player(p);
            let card_number_sum = p.tableau.iter().map(|c| c.number as u32).sum::<u32>()
                + p.sanctuaries.iter().map(|c| c.tile as u32).sum::<u32>();
            PlayerScore {
                seat: p.seat,
                name: p.name.clone(),
                total,
                card_number_sum,
            }
        }).collect();
        self.phase = GamePhase::GameOver { scores };
    }

    fn require_phase_choosing_cards(&self) -> Result<(), ActionError> {
        match &self.phase {
            GamePhase::Playing(RoundPhase::ChoosingCards) => Ok(()),
            _ => Err(ActionError::WrongPhase),
        }
    }

    fn player_mut(&mut self, seat: usize) -> Result<&mut PlayerState, ActionError> {
        self.players.get_mut(seat).ok_or(ActionError::InvalidSeat)
    }
}

// ─── Client-facing snapshot ───────────────────────────────────────────────────

/// The JSON payload sent to a specific player.
#[derive(Debug, Serialize)]
pub struct ClientGameState {
    pub phase: ClientPhase,
    pub round: u8,
    pub my_seat: usize,
    pub my_hand: Vec<RegionCard>,
    pub players: Vec<ClientPlayerState>,
    pub market: Vec<RegionCard>,
    pub deck_size: usize,
    pub sanctuary_deck_size: usize,
    pub draft_order: Vec<usize>,
    pub current_drafter: Option<usize>,
    /// Present only when it's this player's turn to choose a sanctuary.
    pub sanctuary_choices: Option<Vec<SanctuaryCard>>,
    pub scores: Option<Vec<PlayerScore>>,
    pub player_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ClientPlayerState {
    pub seat: usize,
    pub name: String,
    pub hand_size: usize,
    pub tableau: Vec<RegionCard>,
    pub sanctuaries: Vec<SanctuaryCard>,
    pub played_this_round: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientPhase {
    WaitingForPlayers,
    ChoosingCards,
    SanctuaryChoice,
    Drafting,
    GameOver,
}

impl GameState {
    pub fn to_client_state(&self, my_seat: usize) -> ClientGameState {
        let (phase, draft_order, current_drafter, sanctuary_choices) = match &self.phase {
            GamePhase::WaitingForPlayers { .. } => {
                (ClientPhase::WaitingForPlayers, vec![], None, None)
            }
            GamePhase::Playing(RoundPhase::ChoosingCards) => {
                (ClientPhase::ChoosingCards, vec![], None, None)
            }
            GamePhase::Playing(RoundPhase::RevealingAndSanctuaries) => {
                (ClientPhase::ChoosingCards, vec![], None, None)
            }
            GamePhase::Playing(RoundPhase::SanctuaryChoice { pending }) => {
                let my_choices = pending.get(&my_seat).cloned();
                (ClientPhase::SanctuaryChoice, vec![], None, my_choices)
            }
            GamePhase::Playing(RoundPhase::Drafting { order, current }) => {
                let drafter = order.get(*current).copied();
                (ClientPhase::Drafting, order.clone(), drafter, None)
            }
            GamePhase::GameOver { .. } => (ClientPhase::GameOver, vec![], None, None),
        };

        let scores = match &self.phase {
            GamePhase::GameOver { scores } => Some(scores.clone()),
            _ => None,
        };

        let players: Vec<ClientPlayerState> = self.players.iter().map(|p| {
            ClientPlayerState {
                seat: p.seat,
                name: p.name.clone(),
                hand_size: p.hand.len(),
                tableau: p.tableau.clone(),
                sanctuaries: p.sanctuaries.clone(),
                played_this_round: p.played_this_round.is_some(),
            }
        }).collect();

        let my_hand = self.players.get(my_seat)
            .map(|p| p.hand.clone())
            .unwrap_or_default();

        ClientGameState {
            phase,
            round: self.round,
            my_seat,
            my_hand,
            players,
            market: self.market.clone(),
            deck_size: self.region_deck.len(),
            sanctuary_deck_size: self.sanctuary_deck.len(),
            draft_order,
            current_drafter,
            sanctuary_choices,
            scores,
            player_count: self.player_count,
        }
    }
}

// ─── Actions ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
pub enum ClientAction {
    StartGame,
    PlayCard { card_index: usize },
    ChooseSanctuary { sanctuary_index: usize },
    DraftCard { market_index: usize },
}

#[derive(Debug, Serialize)]
pub enum ActionError {
    GameAlreadyStarted,
    NotEnoughPlayers,
    RoomFull,
    NotYourTurn,
    WrongPhase,
    AlreadyPlayedThisRound,
    InvalidCardIndex,
    InvalidSeat,
    DeckEmpty,
}
