use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cards::{RegionCard, SanctuaryCard};
use crate::cards::{get_region_deck, get_sanctuary_deck};
use crate::scoring::CardScoreEntry;

// ─── Phase ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    WaitingForPlayers { needed: usize },
    /// Advanced variant: each player has 5 cards and must keep exactly 3.
    AdvancedSetup { pending: HashMap<usize, Vec<RegionCard>> },
    Playing(RoundPhase),
    GameOver { scores: Vec<PlayerScore> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoundPhase {
    /// All players are choosing which card to play this round.
    ChoosingCards,
    /// Players draft from market in `order`; `current` indexes into `order`.
    /// Sanctuary choices happen during this phase — players can choose early
    /// or must choose on their draft turn before the next player goes.
    Drafting {
        order: Vec<usize>,
        current: usize,
        /// Seats still waiting for sanctuary cards to be dealt (in draft order).
        /// Cards are dealt eagerly when the deck has enough; otherwise they wait
        /// until earlier players discard unchosen cards back to the deck.
        sanctuary_waiting: Vec<usize>,
        /// Sanctuary cards dealt to players, awaiting their choice.
        /// Multiple players can have pending choices simultaneously.
        pending_sanctuaries: HashMap<usize, Vec<SanctuaryCard>>,
        /// Whether the current drafter has picked their market card.
        /// On round 8 (no market), starts as true.
        current_has_drafted: bool,
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

    /// Add a player to the room, or reconnect an existing player.
    /// Returns their seat index.
    pub fn join(&mut self, name: &str) -> Result<usize, ActionError> {
        // Reconnect: if a player with this name already exists, return their seat
        // regardless of game phase.
        if let Some(pos) = self.players.iter().position(|p| p.name == name) {
            return Ok(pos);
        }
        // New player: only allowed during WaitingForPlayers.
        match &self.phase {
            GamePhase::WaitingForPlayers { .. } => {}
            _ => return Err(ActionError::GameAlreadyStarted),
        }
        if self.players.len() >= self.player_count {
            return Err(ActionError::RoomFull);
        }
        let seat = self.players.len();
        self.players.push(PlayerState::new(seat, name.to_string()));
        Ok(seat)
    }

    /// Find a player's current seat by name.
    pub fn seat_of(&self, name: &str) -> Option<usize> {
        self.players.iter().position(|p| p.name == name)
    }

    /// Remove a player during WaitingForPlayers. Re-indexes remaining seats.
    pub fn remove_player(&mut self, name: &str) {
        if let Some(pos) = self.players.iter().position(|p| p.name == name) {
            self.players.remove(pos);
            // Re-index seat fields.
            for (i, p) in self.players.iter_mut().enumerate() {
                p.seat = i;
            }
        }
    }

    /// Start the game. Deals 3 cards to each player (or 5 in advanced mode), reveals market.
    pub fn start_game(&mut self, seat: usize, advanced: bool, expansion: bool) -> Result<(), ActionError> {
        if seat != 0 {
            return Err(ActionError::NotYourTurn);
        }
        match &self.phase {
            GamePhase::WaitingForPlayers { .. } => {}
            _ => return Err(ActionError::GameAlreadyStarted),
        }
        if self.players.is_empty() {
            return Err(ActionError::NotEnoughPlayers);
        }
        // Lock in player count to however many joined.
        self.player_count = self.players.len();

        // Add expansion cards if enabled.
        if expansion {
            use crate::cards::{get_region_deck_with_expansion, get_sanctuary_deck_with_expansion};
            self.region_deck = get_region_deck_with_expansion();
            self.sanctuary_deck = get_sanctuary_deck_with_expansion();
        }

        if advanced {
            // Deal 5 cards to each player; they must keep exactly 3.
            let mut pending = HashMap::new();
            for player in &mut self.players {
                let mut dealt = Vec::new();
                for _ in 0..5 {
                    let card = self.region_deck.pop().ok_or(ActionError::DeckEmpty)?;
                    dealt.push(card);
                }
                pending.insert(player.seat, dealt);
            }
            self.phase = GamePhase::AdvancedSetup { pending };
        } else {
            self.begin_normal_start()?;
        }
        Ok(())
    }

    /// Deal 3 cards per player, reveal market, start round 1.
    fn begin_normal_start(&mut self) -> Result<(), ActionError> {
        for player in &mut self.players {
            for _ in 0..3 {
                let card = self.region_deck.pop().ok_or(ActionError::DeckEmpty)?;
                player.hand.push(card);
            }
        }
        self.begin_round_1()
    }

    /// Reveal market and start round 1 (hands already filled).
    fn begin_round_1(&mut self) -> Result<(), ActionError> {
        // Reveal market: player_count + 1 cards.
        for _ in 0..=self.player_count {
            let card = self.region_deck.pop().ok_or(ActionError::DeckEmpty)?;
            self.market.push(card);
        }
        self.round = 1;
        self.phase = GamePhase::Playing(RoundPhase::ChoosingCards);
        Ok(())
    }

    /// Advanced setup: player keeps exactly 3 cards from their dealt 5 by index.
    pub fn keep_cards(&mut self, seat: usize, indices: &[usize; 3]) -> Result<(), ActionError> {
        let pending = match &mut self.phase {
            GamePhase::AdvancedSetup { pending } => pending,
            _ => return Err(ActionError::WrongPhase),
        };
        let dealt = pending.remove(&seat).ok_or(ActionError::NotYourTurn)?;

        // Validate indices are in-bounds and distinct.
        let mut sorted = *indices;
        sorted.sort();
        if sorted[2] >= dealt.len() || sorted[0] == sorted[1] || sorted[1] == sorted[2] {
            // Put choices back so player can retry.
            if let GamePhase::AdvancedSetup { pending } = &mut self.phase {
                pending.insert(seat, dealt);
            }
            return Err(ActionError::InvalidCardIndex);
        }

        // Separate kept vs discarded.
        let kept: Vec<RegionCard> = indices.iter().map(|&i| dealt[i].clone()).collect();
        let discarded: Vec<RegionCard> = dealt.into_iter().enumerate()
            .filter(|(i, _)| !indices.contains(i))
            .map(|(_, c)| c)
            .collect();

        // Give the player their 3 kept cards.
        self.player_mut(seat)?.hand = kept;

        // Shuffle discards back into the region deck.
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let mut deck_with_discards = discarded;
        deck_with_discards.extend(self.region_deck.drain(..));
        deck_with_discards.shuffle(&mut rng);
        self.region_deck = deck_with_discards;

        // If all players have chosen, reveal market and start round 1.
        let all_done = match &self.phase {
            GamePhase::AdvancedSetup { pending } => pending.is_empty(),
            _ => false,
        };
        if all_done {
            self.begin_round_1()?;
        }
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

    /// Choose a sanctuary to keep. Any player with pending sanctuaries can choose
    /// at any time during the Drafting phase. After choosing, unchosen cards return
    /// to the bottom of the deck, then try to deal to waiting players.
    pub fn choose_sanctuary(&mut self, seat: usize, sanctuary_index: usize) -> Result<(), ActionError> {
        // Validate we're in Drafting and this player has pending sanctuaries.
        let choices = match &mut self.phase {
            GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) => {
                pending_sanctuaries.remove(&seat).ok_or(ActionError::NotYourTurn)?
            }
            _ => return Err(ActionError::WrongPhase),
        };
        if sanctuary_index >= choices.len() {
            // Put choices back so the player can retry.
            if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) = &mut self.phase {
                pending_sanctuaries.insert(seat, choices);
            }
            return Err(ActionError::InvalidCardIndex);
        }
        let kept = choices[sanctuary_index].clone();
        self.player_mut(seat)?.sanctuaries.push(kept);
        // Put unchosen sanctuary cards back at the bottom of the deck.
        for (i, card) in choices.into_iter().enumerate() {
            if i != sanctuary_index {
                self.sanctuary_deck.insert(0, card);
            }
        }
        // Cards returned to deck — try to deal to waiting players.
        self.deal_available_sanctuaries();
        // If the current drafter just finished choosing, advance.
        self.try_advance_drafter();
        Ok(())
    }

    /// Draft a card from the market (Drafting phase).
    pub fn draft_card(&mut self, seat: usize, market_index: usize) -> Result<(), ActionError> {
        let (order, current, current_has_drafted) = match &self.phase {
            GamePhase::Playing(RoundPhase::Drafting { order, current, current_has_drafted, .. }) => {
                (order.clone(), *current, *current_has_drafted)
            }
            _ => return Err(ActionError::WrongPhase),
        };
        if order[current] != seat {
            return Err(ActionError::NotYourTurn);
        }
        if current_has_drafted {
            return Err(ActionError::WrongPhase);
        }
        if market_index >= self.market.len() {
            return Err(ActionError::InvalidCardIndex);
        }
        let card = self.market.remove(market_index);
        self.player_mut(seat)?.hand.push(card);

        // Check if this player has a pending sanctuary choice.
        let has_pending = match &self.phase {
            GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) => {
                pending_sanctuaries.contains_key(&seat)
            }
            _ => false,
        };

        if has_pending {
            // Must choose sanctuary before next player can draft.
            if let GamePhase::Playing(RoundPhase::Drafting { current_has_drafted, .. }) = &mut self.phase {
                *current_has_drafted = true;
            }
        } else {
            self.advance_drafter();
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
        let eligible_seats: Vec<usize> = self.players.iter().filter_map(|p| {
            let len = p.tableau.len();
            if len < 2 {
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

        // Build draft order: ascending card number of played cards.
        let mut order_pairs: Vec<(usize, u8)> = self.players.iter().map(|p| {
            let played_num = p.tableau.last().map(|c| c.number).unwrap_or(0);
            (p.seat, played_num)
        }).collect();
        order_pairs.sort_by_key(|&(_, num)| num);
        let seat_order: Vec<usize> = order_pairs.into_iter().map(|(s, _)| s).collect();

        // Sort eligible seats in draft order so we deal in the right sequence.
        let sanctuary_waiting: Vec<usize> = seat_order.iter()
            .filter(|s| eligible_seats.contains(s))
            .copied()
            .collect();

        if self.round == 8 {
            self.market.clear();
            if sanctuary_waiting.is_empty() {
                self.finalize_scores();
                return;
            }
            self.phase = GamePhase::Playing(RoundPhase::Drafting {
                order: seat_order,
                current: 0,
                sanctuary_waiting,
                pending_sanctuaries: HashMap::new(),
                current_has_drafted: true,
            });
            self.deal_available_sanctuaries();
            self.skip_non_actionable_drafters();
        } else {
            self.phase = GamePhase::Playing(RoundPhase::Drafting {
                order: seat_order,
                current: 0,
                sanctuary_waiting,
                pending_sanctuaries: HashMap::new(),
                current_has_drafted: false,
            });
            self.deal_available_sanctuaries();
        }
    }

    /// Deal sanctuary cards to as many waiting players as possible (in draft order).
    /// Deal sanctuary cards to waiting players. Gives full draws when the deck
    /// has enough. Only the current drafter gets a partial draw (whatever's left)
    /// since the game can't progress past them — other players wait for discards.
    fn deal_available_sanctuaries(&mut self) {
        loop {
            let (seat, draw_count, is_current_drafter) = match &self.phase {
                GamePhase::Playing(RoundPhase::Drafting { order, current, sanctuary_waiting, .. }) => {
                    if let Some(&seat) = sanctuary_waiting.first() {
                        let count = self.sanctuary_draw_count(seat);
                        let is_current = order[*current] == seat;
                        (seat, count, is_current)
                    } else {
                        return;
                    }
                }
                _ => return,
            };

            if draw_count == 0 {
                // Remove player who needs 0 cards.
                if let GamePhase::Playing(RoundPhase::Drafting { sanctuary_waiting, .. }) = &mut self.phase {
                    sanctuary_waiting.retain(|&s| s != seat);
                }
                continue;
            }

            let deck_size = self.sanctuary_deck.len();
            let anyone_pending = match &self.phase {
                GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) => {
                    !pending_sanctuaries.is_empty()
                }
                _ => false,
            };

            if deck_size == 0 && !anyone_pending {
                // Deck empty and no one will discard. Remove all remaining waiters.
                if let GamePhase::Playing(RoundPhase::Drafting { sanctuary_waiting, .. }) = &mut self.phase {
                    sanctuary_waiting.clear();
                }
                return;
            }
            if deck_size == 0 {
                return; // Deck empty but discards are coming — wait.
            }
            if deck_size < draw_count && !is_current_drafter {
                return; // Not enough for a full draw; wait for discards (unless current drafter).
            }

            // Remove from waiting list and deal.
            if let GamePhase::Playing(RoundPhase::Drafting { sanctuary_waiting, .. }) = &mut self.phase {
                sanctuary_waiting.retain(|&s| s != seat);
            }

            let choices = self.draw_sanctuary_choices(seat);
            if choices.len() == 1 {
                self.players[seat].sanctuaries.push(choices.into_iter().next().unwrap());
            } else if !choices.is_empty() {
                if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) = &mut self.phase {
                    pending_sanctuaries.insert(seat, choices);
                }
            }
        }
    }

    /// How many sanctuary cards a player would draw (1 + clue count).
    fn sanctuary_draw_count(&self, seat: usize) -> usize {
        let clue_count = self.players[seat].tableau.iter().filter(|c| c.clue).count()
            + self.players[seat].sanctuaries.iter().filter(|c| c.clue).count();
        1 + clue_count
    }

    fn draw_sanctuary_choices(&mut self, seat: usize) -> Vec<SanctuaryCard> {
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

    /// Advance to the next drafter, or end the round if all have drafted.
    fn advance_drafter(&mut self) {
        let (order, current) = match &self.phase {
            GamePhase::Playing(RoundPhase::Drafting { order, current, .. }) => {
                (order.clone(), *current)
            }
            _ => return,
        };
        let next = current + 1;
        if next >= order.len() {
            self.end_round().ok();
        } else {
            if let GamePhase::Playing(RoundPhase::Drafting {
                current: ref mut c,
                current_has_drafted: ref mut drafted,
                ..
            }) = &mut self.phase {
                *c = next;
                *drafted = self.round == 8;
            }
            // Try to deal sanctuaries to the new current drafter (may get partial draw).
            self.deal_available_sanctuaries();
            if self.round == 8 {
                self.skip_non_actionable_drafters();
            }
        }
    }

    /// After a sanctuary choice, advance only if the current drafter has
    /// drafted and no longer has pending sanctuaries.
    fn try_advance_drafter(&mut self) {
        let should_advance = match &self.phase {
            GamePhase::Playing(RoundPhase::Drafting {
                order, current, current_has_drafted, pending_sanctuaries, ..
            }) => {
                let seat = order[*current];
                *current_has_drafted && !pending_sanctuaries.contains_key(&seat)
            }
            _ => false,
        };
        if should_advance {
            self.advance_drafter();
        }
    }

    /// On round 8, skip drafters who have no pending sanctuary choices.
    /// On round 8, skip drafters who have no pending sanctuary choice and
    /// aren't waiting for cards.
    fn skip_non_actionable_drafters(&mut self) {
        loop {
            let (order, current, has_pending, is_waiting) = match &self.phase {
                GamePhase::Playing(RoundPhase::Drafting {
                    order, current, pending_sanctuaries, sanctuary_waiting, ..
                }) => {
                    let seat = order[*current];
                    let has_pending = pending_sanctuaries.contains_key(&seat);
                    let is_waiting = sanctuary_waiting.contains(&seat);
                    (order.clone(), *current, has_pending, is_waiting)
                }
                _ => return,
            };
            if has_pending || is_waiting {
                return; // This player needs to choose or is waiting for cards.
            }
            let next = current + 1;
            if next >= order.len() {
                self.end_round().ok();
                return;
            }
            if let GamePhase::Playing(RoundPhase::Drafting { current: ref mut c, .. }) = &mut self.phase {
                *c = next;
            }
        }
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
            let card_number_sum = p.tableau.iter().map(|c| c.number as u32).sum::<u32>();
            PlayerScore {
                seat: p.seat,
                name: p.name.clone(),
                total,
                card_number_sum,
            }
        }).collect();
        self.phase = GamePhase::GameOver { scores };
    }

    /// Create a demo game already in GameOver state with 2 players and real cards.
    pub fn new_demo() -> Self {
        let region_deck = get_region_deck();
        let sanctuary_deck = get_sanctuary_deck();

        // Deal 8 region cards to each player, plus a few sanctuaries.
        let mut gs = Self {
            phase: GamePhase::WaitingForPlayers { needed: 2 },
            round: 8,
            players: vec![
                PlayerState::new(0, "Alice".to_string()),
                PlayerState::new(1, "Bob".to_string()),
            ],
            region_deck: Vec::new(),
            sanctuary_deck: Vec::new(),
            market: Vec::new(),
            player_count: 2,
        };

        // Give Alice first 8 cards, Bob next 8
        gs.players[0].tableau = region_deck[0..8].to_vec();
        gs.players[1].tableau = region_deck[8..16].to_vec();

        // Give each player 2 sanctuaries
        gs.players[0].sanctuaries = sanctuary_deck[0..2].to_vec();
        gs.players[1].sanctuaries = sanctuary_deck[2..4].to_vec();

        gs.finalize_scores();
        gs
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
    /// True when the current drafter has drafted and must choose a sanctuary.
    pub drafter_choosing_sanctuary: bool,
    /// Present only during AdvancedSetup for this player (the 5 dealt cards).
    pub advanced_setup_choices: Option<Vec<RegionCard>>,
    /// Present when this player has pending sanctuary choices (can choose any time during drafting).
    pub sanctuary_choices: Option<Vec<SanctuaryCard>>,
    pub scores: Option<Vec<PlayerScore>>,
    /// Per-card score breakdown for THIS player only. None during play.
    pub my_score_detail: Option<Vec<CardScoreEntry>>,
    /// Per-card score breakdown for ALL players (for scoring table). None during play.
    pub all_score_details: Option<Vec<crate::scoring::PlayerScoreDetail>>,
    /// The card this player played this round (face-up to them only, before reveal).
    pub my_played_card: Option<RegionCard>,
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
    AdvancedSetup,
    ChoosingCards,
    Drafting,
    GameOver,
}

impl GameState {
    pub fn to_client_state(&self, my_seat: usize) -> ClientGameState {
        let (phase, draft_order, current_drafter, drafter_choosing_sanctuary, sanctuary_choices, advanced_setup_choices) = match &self.phase {
            GamePhase::WaitingForPlayers { .. } => {
                (ClientPhase::WaitingForPlayers, vec![], None, false, None, None)
            }
            GamePhase::AdvancedSetup { pending } => {
                let my_choices = pending.get(&my_seat).cloned();
                (ClientPhase::AdvancedSetup, vec![], None, false, None, my_choices)
            }
            GamePhase::Playing(RoundPhase::ChoosingCards) => {
                (ClientPhase::ChoosingCards, vec![], None, false, None, None)
            }
            GamePhase::Playing(RoundPhase::Drafting { order, current, pending_sanctuaries, current_has_drafted, .. }) => {
                let drafter = order.get(*current).copied();
                let my_choices = pending_sanctuaries.get(&my_seat).cloned();
                (ClientPhase::Drafting, order.clone(), drafter, *current_has_drafted, my_choices, None)
            }
            GamePhase::GameOver { .. } => (ClientPhase::GameOver, vec![], None, false, None, None),
        };

        let scores = match &self.phase {
            GamePhase::GameOver { scores } => Some(scores.clone()),
            _ => None,
        };

        // The card this player played this round (visible only to them).
        let my_played_card = self.players.get(my_seat)
            .and_then(|p| p.played_this_round.clone());

        // Live score detail: include the played-but-not-yet-revealed card so
        // scores update immediately when the player places a card.
        let my_score_detail = self.players.get(my_seat)
            .filter(|p| !p.tableau.is_empty() || p.played_this_round.is_some())
            .map(|p| {
                if p.played_this_round.is_some() {
                    let mut tmp = p.clone();
                    if let Some(card) = tmp.played_this_round.take() {
                        tmp.tableau.push(card);
                    }
                    crate::scoring::score_player_detailed(&tmp)
                } else {
                    crate::scoring::score_player_detailed(p)
                }
            });

        let all_score_details = match &self.phase {
            GamePhase::GameOver { .. } => {
                Some(self.players.iter().map(|p| {
                    let entries = crate::scoring::score_player_detailed(p);
                    let total = entries.iter().map(|e| e.points).sum();
                    crate::scoring::PlayerScoreDetail {
                        seat: p.seat,
                        name: p.name.clone(),
                        entries,
                        total,
                    }
                }).collect())
            }
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
            drafter_choosing_sanctuary,
            advanced_setup_choices,
            sanctuary_choices,
            scores,
            my_score_detail,
            all_score_details,
            my_played_card,
            player_count: self.player_count,
        }
    }
}

// ─── Actions ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
pub enum ClientAction {
    StartGame { advanced: bool, #[serde(default)] expansion: bool },
    KeepCards { indices: [usize; 3] },
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
