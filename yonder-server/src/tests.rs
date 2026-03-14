/// M2 game logic tests — written before/alongside implementation (TDD).
/// Each test documents expected behavior from RULES.md.
#[cfg(test)]
mod tests {
    use crate::cards::{Biome, Fame, RegionCard, SanctuaryCard, WonderCount};
    use crate::game::{ActionError, GamePhase, GameState, RoundPhase};

    // ─── Helpers ─────────────────────────────────────────────────────────────

    fn region(number: u8) -> RegionCard {
        RegionCard {
            number,
            biome: Biome::Red,
            night: false,
            clue: false,
            wonders: WonderCount::zero(),
            quest: WonderCount::zero(),
            fame: Fame::None,
        }
    }

    fn region_with_clue(number: u8) -> RegionCard {
        RegionCard { clue: true, ..region(number) }
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

    fn sanctuary_with_clue(tile: u8) -> SanctuaryCard {
        SanctuaryCard { clue: true, ..sanctuary(tile) }
    }

    /// Build a 2-player game already in ChoosingCards phase with known cards.
    /// Alice (seat 0) hand: [card_a0, card_a1, card_a2]
    /// Bob   (seat 1) hand: [card_b0, card_b1, card_b2]
    /// Market: 3 cards (player_count+1 = 3)
    fn setup_game(
        alice_hand: Vec<RegionCard>,
        bob_hand: Vec<RegionCard>,
        market: Vec<RegionCard>,
    ) -> GameState {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        // Inject known cards instead of random ones.
        gs.region_deck.clear();
        gs.market.clear();
        gs.players[0].hand = alice_hand;
        gs.players[1].hand = bob_hand;
        gs.market = market;
        gs.round = 1;
        gs.phase = GamePhase::Playing(RoundPhase::ChoosingCards);
        gs
    }

    // ─── start_game ──────────────────────────────────────────────────────────

    #[test]
    fn start_game_deals_3_cards_each_and_market() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        gs.start_game(0, false).unwrap();
        assert_eq!(gs.players[0].hand.len(), 3);
        assert_eq!(gs.players[1].hand.len(), 3);
        assert_eq!(gs.market.len(), 3); // 2 players + 1
    }

    #[test]
    fn start_game_sets_round_1_and_choosing_phase() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        gs.start_game(0, false).unwrap();
        assert_eq!(gs.round, 1);
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::ChoosingCards)));
    }

    #[test]
    fn start_game_solo_works() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.start_game(0, false).unwrap();
        assert_eq!(gs.players[0].hand.len(), 3);
        assert_eq!(gs.market.len(), 2); // 1 player + 1
        assert_eq!(gs.round, 1);
    }

    #[test]
    fn start_game_only_seat_0_can_start() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        let err = gs.start_game(1, false).unwrap_err();
        assert!(matches!(err, ActionError::NotYourTurn));
    }

    #[test]
    fn start_game_cannot_start_twice() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        gs.start_game(0, false).unwrap();
        let err = gs.start_game(0, false).unwrap_err();
        assert!(matches!(err, ActionError::GameAlreadyStarted));
    }

    // ─── advanced setup ───────────────────────────────────────────────────────

    #[test]
    fn advanced_start_deals_5_and_enters_setup_phase() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        gs.start_game(0, true).unwrap();
        assert!(matches!(gs.phase, GamePhase::AdvancedSetup { .. }));
        // Players have no hand yet; choices are in pending.
        assert_eq!(gs.players[0].hand.len(), 0);
        assert_eq!(gs.players[1].hand.len(), 0);
        let client = gs.to_client_state(0);
        assert!(client.advanced_setup_choices.is_some());
        assert_eq!(client.advanced_setup_choices.unwrap().len(), 5);
    }

    #[test]
    fn advanced_setup_keep_cards_transitions_to_choosing_when_all_done() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        gs.start_game(0, true).unwrap();
        gs.keep_cards(0, &[0, 1, 2]).unwrap();
        // Bob still pending — still in AdvancedSetup.
        assert!(matches!(gs.phase, GamePhase::AdvancedSetup { .. }));
        gs.keep_cards(1, &[0, 2, 4]).unwrap();
        // Both done — should be in ChoosingCards now.
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::ChoosingCards)));
        assert_eq!(gs.players[0].hand.len(), 3);
        assert_eq!(gs.players[1].hand.len(), 3);
        assert_eq!(gs.round, 1);
    }

    #[test]
    fn advanced_setup_reject_duplicate_indices() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        gs.start_game(0, true).unwrap();
        let err = gs.keep_cards(0, &[0, 0, 1]).unwrap_err();
        assert!(matches!(err, ActionError::InvalidCardIndex));
    }

    // ─── play_card ────────────────────────────────────────────────────────────

    #[test]
    fn play_card_removes_card_from_hand() {
        let mut gs = setup_game(
            vec![region(5), region(10), region(15)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap();
        assert_eq!(gs.players[0].hand.len(), 2);
    }

    #[test]
    fn play_card_rejects_invalid_index() {
        let mut gs = setup_game(
            vec![region(5), region(10), region(15)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        let err = gs.play_card(0, 5).unwrap_err();
        assert!(matches!(err, ActionError::InvalidCardIndex));
    }

    #[test]
    fn play_card_rejects_playing_twice_in_same_round() {
        let mut gs = setup_game(
            vec![region(5), region(10), region(15)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap();
        let err = gs.play_card(0, 0).unwrap_err();
        assert!(matches!(err, ActionError::AlreadyPlayedThisRound));
    }

    #[test]
    fn play_card_advances_to_drafting_when_both_play_round_1() {
        // Round 1: no sanctuary eligibility (no previous card).
        // After both play → should go straight to Drafting.
        let mut gs = setup_game(
            vec![region(5), region(10), region(15)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap(); // Alice plays card 5
        gs.play_card(1, 0).unwrap(); // Bob plays card 3
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::Drafting { .. })));
    }

    #[test]
    fn play_card_commits_card_to_tableau_on_reveal() {
        let mut gs = setup_game(
            vec![region(5), region(10), region(15)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 0).unwrap();
        // After both play, cards go to tableau.
        assert_eq!(gs.players[0].tableau.len(), 1);
        assert_eq!(gs.players[1].tableau.len(), 1);
        assert_eq!(gs.players[0].tableau[0].number, 5);
        assert_eq!(gs.players[1].tableau[0].number, 3);
    }

    // ─── Sanctuary eligibility ────────────────────────────────────────────────

    #[test]
    fn no_sanctuary_in_round_1() {
        // First card played — no previous card to compare against.
        let mut gs = setup_game(
            vec![region(5), region(10), region(15)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap(); // Alice plays 5
        gs.play_card(1, 0).unwrap(); // Bob plays 3
        // Should be Drafting, not SanctuaryChoice.
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::Drafting { .. })));
    }

    #[test]
    fn sanctuary_triggered_when_played_number_greater_than_previous() {
        // Alice's tableau already has card 5. She plays card 10 (10 > 5 → eligible).
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = (1..=5).map(|i| sanctuary(i)).collect();
        gs.players[0].tableau.push(region(5)); // previous card
        gs.players[1].tableau.push(region(7)); // Bob's previous

        gs.play_card(0, 0).unwrap(); // Alice plays 10 (> 5 ✓)
        gs.play_card(1, 0).unwrap(); // Bob plays 3 (< 7 ✗)
        // In Drafting. Alice eligible, 1 sanctuary (no clues) → auto-assigned eagerly.
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::Drafting { .. })));
        assert_eq!(gs.players[0].sanctuaries.len(), 1);
    }

    #[test]
    fn no_sanctuary_when_played_number_equal_to_previous() {
        let mut gs = setup_game(
            vec![region(5), region(10), region(15)],
            vec![region(3), region(8), region(20)],
            vec![region(1), region(2), region(4)],
        );
        gs.players[0].tableau.push(region(5)); // previous = 5
        gs.players[1].tableau.push(region(3)); // previous = 3

        gs.play_card(0, 0).unwrap(); // Alice plays 5 (= 5, not >)
        gs.play_card(1, 0).unwrap(); // Bob plays 3 (= 3, not >)
        // Neither eligible → straight to Drafting.
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::Drafting { .. })));
    }

    #[test]
    fn no_sanctuary_when_played_number_less_than_previous() {
        let mut gs = setup_game(
            vec![region(3), region(10), region(15)],
            vec![region(2), region(8), region(20)],
            vec![region(1), region(4), region(6)],
        );
        gs.players[0].tableau.push(region(10)); // previous = 10
        gs.players[1].tableau.push(region(8));  // previous = 8

        gs.play_card(0, 0).unwrap(); // Alice plays 3 (< 10 ✗)
        gs.play_card(1, 0).unwrap(); // Bob plays 2 (< 8 ✗)
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::Drafting { .. })));
    }

    // ─── Sanctuary draw count ─────────────────────────────────────────────────

    #[test]
    fn sanctuary_draw_count_is_1_with_no_clues_auto_assigned() {
        // With no clues, only 1 sanctuary drawn → auto-assigned on drafter's turn.
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = (1..=5).map(|i| sanctuary(i)).collect();
        gs.players[0].tableau.push(region(5));
        gs.players[1].tableau.push(region(50));

        gs.play_card(0, 0).unwrap(); // Alice plays 10 (> 5 ✓)
        gs.play_card(1, 1).unwrap(); // Bob plays 8 (< 50 ✗)

        // Draft order: Bob (3) first, Alice (10) second.
        // Alice draws 1 sanctuary (no clues) → auto-assigned eagerly.
        assert_eq!(gs.players[0].sanctuaries.len(), 1);
        assert_eq!(gs.players[0].sanctuaries[0].tile, 5); // top of deck
        if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) = &gs.phase {
            assert!(!pending_sanctuaries.contains_key(&0));
        }
    }

    #[test]
    fn sanctuary_draw_count_includes_tableau_clues() {
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = (1..=10).map(|i| sanctuary(i)).collect();
        // Alice has 2 clue cards in her tableau already.
        gs.players[0].tableau.push(region_with_clue(2));
        gs.players[0].tableau.push(region_with_clue(4));
        gs.players[0].tableau.push(region(5)); // non-clue previous card
        gs.players[1].tableau.push(region(50));

        gs.play_card(0, 0).unwrap(); // Alice plays 10 (> 5 ✓)
        gs.play_card(1, 1).unwrap(); // Bob plays 8 (< 50 ✗)

        // Draft order: Bob (8) first, Alice (10) second.
        // Sanctuary drawn on Alice's turn.
        gs.draft_card(1, 0).unwrap(); // Bob drafts
        // Now Alice's turn — should have 1 + 2 clues = 3 sanctuary choices.
        if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) = &gs.phase {
            let choices = pending_sanctuaries.get(&0).expect("Alice should have pending choices");
            assert_eq!(choices.len(), 3);
        } else {
            panic!("Expected Drafting, got {:?}", gs.phase);
        }
    }

    #[test]
    fn sanctuary_draw_count_includes_sanctuary_clues() {
        // Clues on already-held sanctuary cards also count.
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = (1..=10).map(|i| sanctuary(i)).collect();
        gs.players[0].tableau.push(region(5)); // previous, no clue
        gs.players[0].sanctuaries.push(sanctuary_with_clue(1)); // 1 clue from sanctuary
        gs.players[1].tableau.push(region(50));

        gs.play_card(0, 0).unwrap(); // Alice plays 10 (> 5 ✓)
        gs.play_card(1, 1).unwrap(); // Bob plays 8 (< 50 ✗)

        // Draft order: Bob (8) first, Alice (10) second.
        gs.draft_card(1, 0).unwrap(); // Bob drafts
        // Now Alice's turn — should draw 1 + 1 sanctuary clue = 2 choices.
        if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) = &gs.phase {
            let choices = pending_sanctuaries.get(&0).expect("Alice should have pending choices");
            assert_eq!(choices.len(), 2);
        } else {
            panic!("Expected Drafting, got {:?}", gs.phase);
        }
    }

    // ─── choose_sanctuary ─────────────────────────────────────────────────────

    #[test]
    fn choose_sanctuary_keeps_chosen_card() {
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = vec![sanctuary(7), sanctuary(8), sanctuary(9)];
        // Give Alice a clue so she draws 2 sanctuaries (needs a choice, not auto-assign).
        gs.players[0].tableau.push(region_with_clue(5));
        gs.players[1].tableau.push(region(50));

        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 1).unwrap();

        // Draft order: Bob (8) first, Alice (10) second.
        gs.draft_card(1, 0).unwrap(); // Bob drafts
        // Alice's turn — sanctuary drawn, she must draft then choose.
        gs.draft_card(0, 0).unwrap(); // Alice drafts
        gs.choose_sanctuary(0, 0).unwrap();
        assert_eq!(gs.players[0].sanctuaries.len(), 1);
        assert_eq!(gs.players[0].sanctuaries[0].tile, 9); // deck is popped from end
    }

    #[test]
    fn choose_sanctuary_wrong_player_rejected() {
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = (1..=5).map(|i| sanctuary(i)).collect();
        gs.players[0].tableau.push(region_with_clue(5)); // Alice eligible with clue
        gs.players[1].tableau.push(region(50)); // Bob won't qualify

        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 1).unwrap();

        // Draft order: Bob (8) first. Bob tries to choose sanctuary but he has none.
        let err = gs.choose_sanctuary(1, 0).unwrap_err();
        assert!(matches!(err, ActionError::NotYourTurn));
    }

    #[test]
    fn sanctuary_dealt_eagerly_when_deck_has_enough() {
        // Alice is eligible with clue (needs 2 cards). Deck has plenty — dealt immediately.
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = (1..=10).map(|i| sanctuary(i)).collect();
        gs.players[0].tableau.push(region_with_clue(5)); // Alice eligible, has clue → 2 choices
        gs.players[1].tableau.push(region(50));

        gs.play_card(0, 0).unwrap(); // Alice plays 10 (> 5 ✓)
        gs.play_card(1, 1).unwrap(); // Bob plays 8 (< 50 ✗)

        // Alice's sanctuary dealt eagerly — she can choose before her draft turn.
        if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) = &gs.phase {
            let choices = pending_sanctuaries.get(&0).expect("Alice should have pending choices");
            assert_eq!(choices.len(), 2);
        }
        gs.choose_sanctuary(0, 0).unwrap();
        assert_eq!(gs.players[0].sanctuaries.len(), 1);
    }

    #[test]
    fn sanctuary_partial_draw_when_deck_low() {
        // Alice (needs 2) and Bob (needs 2) both eligible, but deck only has 3.
        // Both get dealt: Bob gets 2, Alice gets 1 (auto-assigned).
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(8), region(15), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = vec![sanctuary(1), sanctuary(2), sanctuary(3)]; // only 3
        gs.players[0].tableau.push(region_with_clue(5)); // Alice: 10 > 5, 1 clue → needs 2
        gs.players[1].tableau.push(region_with_clue(3)); // Bob: 8 > 3, 1 clue → needs 2

        gs.play_card(0, 0).unwrap(); // Alice plays 10
        gs.play_card(1, 0).unwrap(); // Bob plays 8

        // Draft order: Bob (8) first, Alice (10) second.
        // Bob dealt 2, Alice dealt 1 (only 1 left) → auto-assigned.
        if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, .. }) = &gs.phase {
            assert!(pending_sanctuaries.contains_key(&1), "Bob should have 2 choices");
        }
        assert_eq!(gs.players[0].sanctuaries.len(), 1, "Alice got auto-assigned the 1 remaining card");
    }

    #[test]
    fn sanctuary_deferred_when_deck_empty() {
        // Alice (needs 2) and Bob (needs 2) both eligible, but deck only has 2.
        // Bob dealt first (draft order), gets 2. Alice must wait (deck empty).
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(8), region(15), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = vec![sanctuary(1), sanctuary(2)]; // only 2
        gs.players[0].tableau.push(region_with_clue(5)); // Alice: 10 > 5, 1 clue → needs 2
        gs.players[1].tableau.push(region_with_clue(3)); // Bob: 8 > 3, 1 clue → needs 2

        gs.play_card(0, 0).unwrap(); // Alice plays 10
        gs.play_card(1, 0).unwrap(); // Bob plays 8

        // Draft order: Bob (8) first, Alice (10) second.
        // Bob dealt 2, deck now empty. Alice waiting.
        if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, sanctuary_waiting, .. }) = &gs.phase {
            assert!(pending_sanctuaries.contains_key(&1), "Bob should have choices");
            assert!(sanctuary_waiting.contains(&0), "Alice should be waiting");
        }

        // Bob chooses → returns 1 card to deck. Alice can now draw.
        gs.choose_sanctuary(1, 0).unwrap();
        if let GamePhase::Playing(RoundPhase::Drafting { pending_sanctuaries, sanctuary_waiting, .. }) = &gs.phase {
            // Alice gets 1 card (auto-assigned) since only 1 returned.
            assert!(sanctuary_waiting.is_empty());
        }
        assert_eq!(gs.players[0].sanctuaries.len(), 1, "Alice got auto-assigned");
    }

    #[test]
    fn draft_then_sanctuary_on_same_turn() {
        // Current drafter must choose sanctuary after drafting if they have pending ones.
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(8), region(15), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.sanctuary_deck = (1..=10).map(|i| sanctuary(i)).collect();
        gs.players[0].tableau.push(region_with_clue(5)); // Alice: 10 > 5 ✓, has clue
        gs.players[1].tableau.push(region_with_clue(3)); // Bob: 8 > 3 ✓, has clue

        gs.play_card(0, 0).unwrap(); // Alice plays 10
        gs.play_card(1, 0).unwrap(); // Bob plays 8

        // Draft order: Bob (8) first, Alice (10) second.
        if let GamePhase::Playing(RoundPhase::Drafting { order, pending_sanctuaries, .. }) = &gs.phase {
            assert_eq!(order[0], 1); // Bob first
            // Bob's sanctuary dealt immediately (he's first drafter and eligible).
            assert!(pending_sanctuaries.contains_key(&1));
        }

        // Bob drafts from market.
        gs.draft_card(1, 0).unwrap();
        // Bob still has pending sanctuary → must choose before Alice can draft.
        if let GamePhase::Playing(RoundPhase::Drafting { current_has_drafted, .. }) = &gs.phase {
            assert!(*current_has_drafted);
        }
        // Alice can't draft yet (Bob's turn still, choosing sanctuary).
        let err = gs.draft_card(0, 0).unwrap_err();
        assert!(matches!(err, ActionError::NotYourTurn));

        // Bob chooses sanctuary → advances to Alice's turn (her sanctuary drawn now).
        gs.choose_sanctuary(1, 0).unwrap();
        assert_eq!(gs.players[1].sanctuaries.len(), 1);

        // Now it's Alice's turn — sanctuary already drawn.
        gs.draft_card(0, 0).unwrap();
        // Alice has pending sanctuary → must choose.
        gs.choose_sanctuary(0, 0).unwrap();
        assert_eq!(gs.players[0].sanctuaries.len(), 1);

        // Round should advance.
        assert_eq!(gs.round, 2);
    }

    // ─── draft_card ───────────────────────────────────────────────────────────

    #[test]
    fn draft_order_is_ascending_by_played_card_number() {
        // Alice played 10, Bob played 3 → Bob drafts first (lower number).
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap(); // Alice plays 10
        gs.play_card(1, 0).unwrap(); // Bob plays 3
        // Round 1 → straight to Drafting.
        if let GamePhase::Playing(RoundPhase::Drafting { order, current, .. }) = &gs.phase {
            assert_eq!(order[*current], 1); // Bob (seat 1) drafts first
        } else {
            panic!("Expected Drafting phase");
        }
    }

    #[test]
    fn draft_card_adds_card_to_hand() {
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 0).unwrap(); // Bob plays 3, drafts first
        gs.draft_card(1, 0).unwrap(); // Bob drafts market[0]
        assert_eq!(gs.players[1].hand.len(), 3); // had 2 after playing, now 3
    }

    #[test]
    fn draft_card_wrong_player_is_rejected() {
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 0).unwrap(); // Bob drafts first
        let err = gs.draft_card(0, 0).unwrap_err(); // Alice tries to draft first
        assert!(matches!(err, ActionError::NotYourTurn));
    }

    #[test]
    fn draft_card_advances_round_after_last_drafter() {
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 0).unwrap(); // Bob first, Alice second
        gs.draft_card(1, 0).unwrap(); // Bob drafts
        gs.draft_card(0, 0).unwrap(); // Alice drafts (last)
        // Round should advance to 2, market refilled, back to ChoosingCards.
        assert_eq!(gs.round, 2);
        assert!(matches!(gs.phase, GamePhase::Playing(RoundPhase::ChoosingCards)));
    }

    #[test]
    fn market_refilled_to_player_count_plus_1_after_round() {
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        // Give the deck enough cards for refill.
        gs.region_deck = (30..=40).map(|i| region(i as u8)).collect();
        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 0).unwrap();
        gs.draft_card(1, 0).unwrap();
        gs.draft_card(0, 0).unwrap();
        // Market should have 3 new cards (2 players + 1).
        assert_eq!(gs.market.len(), 3);
    }

    // ─── Round 8 special case ─────────────────────────────────────────────────

    #[test]
    fn round_8_transitions_to_game_over_without_drafting() {
        // Round 8: after both players play, game goes directly to GameOver (no draft).
        let mut gs = setup_game(
            vec![region(10), region(15), region(20)],
            vec![region(3), region(8), region(25)],
            vec![region(1), region(2), region(4)],
        );
        gs.round = 8; // Jump to round 8.
        gs.play_card(0, 0).unwrap();
        gs.play_card(1, 0).unwrap();
        // No drafting in round 8 — should go straight to GameOver.
        assert!(matches!(gs.phase, GamePhase::GameOver { .. }));
    }

    // ─── join ─────────────────────────────────────────────────────────────────

    #[test]
    fn join_assigns_sequential_seats() {
        let mut gs = GameState::new_waiting(3);
        assert_eq!(gs.join("Alice").unwrap(), 0);
        assert_eq!(gs.join("Bob").unwrap(), 1);
        assert_eq!(gs.join("Carol").unwrap(), 2);
    }

    #[test]
    fn join_same_name_returns_existing_seat() {
        let mut gs = GameState::new_waiting(2);
        assert_eq!(gs.join("Alice").unwrap(), 0);
        assert_eq!(gs.join("Alice").unwrap(), 0); // same seat
    }

    #[test]
    fn join_rejects_when_room_full() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        let err = gs.join("Carol").unwrap_err();
        assert!(matches!(err, ActionError::RoomFull));
    }

    #[test]
    fn join_rejected_after_game_starts() {
        let mut gs = GameState::new_waiting(2);
        gs.join("Alice").unwrap();
        gs.join("Bob").unwrap();
        gs.start_game(0, false).unwrap();
        let err = gs.join("Carol").unwrap_err();
        assert!(matches!(err, ActionError::GameAlreadyStarted));
    }
}
