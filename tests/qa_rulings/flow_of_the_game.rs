//! QA category "Flow of the Game" (12 rulings; #4 shuffle-discernibility
//! excluded, see qa_rulings/mod.rs) -- turn structure, zones, and the
//! standby/check-timing resolution order that CLAUDE.md calls out as the
//! part most likely to be silently wrong (10.5, 10.8).

use engine::state::{CardKind, GameState, PlayerId, Timing, Zone};

/// QA #10: "Is there a limit to hand size?" -> "No, there isn't."
#[test]
fn qa_010_no_hand_size_limit() {
    let mut game = GameState::test_scenario();
    for _ in 0..30 {
        game.draw(PlayerId::P1);
    }
    assert_eq!(game.hand_size(PlayerId::P1), 30);
    // No end-of-turn "discard down to N" rule action should exist to
    // enforce a cap that isn't there.
    game.end_phase(PlayerId::P1);
    assert_eq!(game.hand_size(PlayerId::P1), 30);
}

/// QA #11: "When my base has 5 Pals, can I deploy a new Pal?" -> "Yes ...
/// when you deploy the 6th or more Pals, put Pals into your graveyard
/// after deploying until your base has 5." (4.4.5.2; also CLAUDE.md's
/// canonical Chillet-chain test relies on this not being a hard block.)
#[test]
fn qa_011_sixth_pal_deploy_then_bin_down_to_five() {
    let mut game = GameState::test_scenario();
    let mut pals = Vec::new();
    for i in 0..5 {
        pals.push(game.deploy(PlayerId::P1, &format!("EFILLER-{i:03}"), Zone::Base).unwrap());
    }
    assert_eq!(game.pals_in_base(PlayerId::P1).len(), 5);

    // Deploying a 6th is legal -- not rejected outright -- and the base
    // temporarily holds 6.
    let sixth = game.deploy(PlayerId::P1, "EFILLER-005", Zone::Base).unwrap();
    assert_eq!(game.pals_in_base(PlayerId::P1).len(), 6);

    // The controller then chooses which Pal(s) go to the graveyard to
    // get back to 5 -- their choice, not a forced "last in" rule.
    game.bin_down_to_base_limit(PlayerId::P1, vec![pals[0]]);
    assert_eq!(game.pals_in_base(PlayerId::P1).len(), 5);
    assert!(game.zone_of(pals[0]) == Zone::Graveyard);
    assert!(game.zone_of(sixth) == Zone::Base);
}

/// QA #12: "Is there a limit to the number of structures and gears in
/// the base?" -> "No, there isn't. You may deploy any number." Slot
/// numbers in the parsed replay logs are base-wide (README gotcha 5)
/// precisely because of this.
#[test]
fn qa_012_no_structure_or_gear_limit() {
    let mut game = GameState::test_scenario();
    // deploy() defaults to Pal semantics (most callers in this suite are
    // deploying Pals); structures/gears go through this explicit sibling
    // instead of inferring "kind" from the card number. Split 6/6 across
    // both real kinds (Structure/Gear used to be one combined CardKind
    // variant) rather than 12 of one -- both need to be uncapped, not
    // just their sum.
    for i in 0..6 {
        game.deploy_structure_or_gear(PlayerId::P1, &format!("ESTRUCT-{i:03}"), CardKind::Structure).unwrap();
    }
    for i in 0..6 {
        game.deploy_structure_or_gear(PlayerId::P1, &format!("EGEAR-{i:03}"), CardKind::Gear).unwrap();
    }
    assert_eq!(game.structures_in_base(PlayerId::P1).len(), 6);
    assert_eq!(game.gears_in_base(PlayerId::P1).len(), 6);
}

/// QA #22: "Is it possible to choose not to draw a card during the Draw
/// Phase?" -> "No, you cannot. You must always draw a card."
#[test]
fn qa_022_draw_phase_draw_is_mandatory() {
    let mut game = GameState::test_scenario();
    let before = game.hand_size(PlayerId::P1);
    game.draw_phase(PlayerId::P1);
    assert_eq!(
        game.hand_size(PlayerId::P1),
        before + 1,
        "the Draw Phase must draw exactly 1 card; there is no opt-out action"
    );
}

/// QA #23: "Can I deploy a card that has the same card name as a card
/// already in my base?" -> "Yes, you can." (The ≤4-per-name cap is a
/// deck-construction rule, not a base rule -- QA #2 vs #23 are testing
/// different zones on purpose.)
#[test]
fn qa_023_duplicate_names_allowed_in_base() {
    let mut game = GameState::test_scenario();
    let first = game.deploy(PlayerId::P1, "EBP01-004", Zone::Base).unwrap();
    let second = game.deploy(PlayerId::P1, "EBP01-004SR", Zone::Base).unwrap();
    assert_eq!(game.zone_of(first), Zone::Base);
    assert_eq!(game.zone_of(second), Zone::Base);
}

/// QA #24: "Can I deploy one of my cards to the opponent's base?" ->
/// "No, you cannot."
#[test]
fn qa_024_cannot_deploy_into_opponents_base() {
    let mut game = GameState::test_scenario();
    let result = game.deploy_to_opponents_base(PlayerId::P1, "EBP01-004");
    assert!(result.is_err(), "there must be no legal action that deploys a card into the opponent's base");
}

/// QA #25: "Can I play cards from my hand, activate abilities, or attack
/// with my Pals during the opponent's turn?" -> "No ... However, cards
/// with Quick can be played during battle on the opponent's turn.
/// Additionally, automatic and continuous abilities will activate/apply
/// during the opponent's turn as long as their conditions are met."
#[test]
fn qa_025_only_quick_and_auto_cont_work_on_opponents_turn() {
    let mut game = GameState::test_scenario();
    game.set_turn_player(PlayerId::P1);

    // P2 (non-turn player) may not play an ordinary card outside battle.
    // `is_quick` is passed explicitly, not inferred from a card number --
    // the engine has no card database to look "EQUICK-001" up in, and
    // hard rule 3 says never derive behaviour from a number/name pattern.
    assert!(!game.can_play_card(PlayerId::P2, false, Timing::OpponentTurnOutsideBattle));

    // P2 may play a Quick card during a battle on P1's turn.
    assert!(game.can_play_card(PlayerId::P2, true, Timing::OpponentTurnDuringBattle));

    // AUTO/CONT still activate/apply for the non-turn player regardless.
    assert!(game.auto_and_cont_apply_on_opponents_turn(PlayerId::P2));
}

/// QA #42: "An AUTO ability activated in the middle of resolving an
/// effect. Should I resolve the effect currently in progress or the
/// activated AUTO ability first?" -> resolve the in-progress effect
/// first; the AUTO goes to standby and resolves after (10.8.2 / the
/// general form of the Chillet-chain rule CLAUDE.md calls out by name).
#[test]
fn qa_042_auto_triggered_mid_resolution_queues_not_interrupts() {
    let mut game = GameState::test_scenario();
    let source = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

    game.begin_resolving_effect("some in-progress effect");
    game.trigger_auto(PlayerId::P1, source); // fires mid-resolution
    assert!(
        game.effect_still_resolving(),
        "the AUTO must not interrupt the effect already resolving"
    );
    assert_eq!(game.pending_autos(PlayerId::P1), vec![source]);

    game.finish_resolving_effect();
    assert!(!game.effect_still_resolving());
    assert_eq!(
        game.next_check_timing_actions(PlayerId::P1),
        vec![source],
        "the queued AUTO must resolve at the next check timing, after the effect finished"
    );
}

/// QA #43: "If multiple AUTO abilities activate at the same time, in
/// what order should they be resolved?" -> turn player resolves all of
/// their own pending AUTOs in any order they choose, then the non-turn
/// player resolves all of theirs in any order; a newly-generated AUTO
/// can be resolved before an older pending one (no forced FIFO); and if
/// a turn-player AUTO triggers while the non-turn player's queue is
/// resolving, priority returns to the turn player before continuing the
/// non-turn player's queue.
#[test]
fn qa_043_turn_player_queue_resolves_before_non_turn_player_queue() {
    let mut game = GameState::test_scenario();
    game.set_turn_player(PlayerId::P1);
    let p1_source = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    let p2_source = game.deploy(PlayerId::P2, "EFILLER-001", Zone::Base).unwrap();

    game.trigger_auto(PlayerId::P2, p2_source);
    game.trigger_auto(PlayerId::P1, p1_source);

    assert_eq!(
        game.priority_player(),
        PlayerId::P1,
        "the turn player's pending AUTOs must be offered before the non-turn player's"
    );
}

#[test]
fn qa_043_newly_generated_auto_may_resolve_before_older_pending_one() {
    let mut game = GameState::test_scenario();
    game.set_turn_player(PlayerId::P1);
    let a = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    let b = game.deploy(PlayerId::P1, "EFILLER-001", Zone::Base).unwrap();

    game.trigger_auto(PlayerId::P1, a);
    game.trigger_auto(PlayerId::P1, b); // B is pending alongside A

    // Resolving A generates a new AUTO, C. Stated explicitly here (deploy
    // a fresh source, trigger it) rather than as an implicit side effect
    // of resolve_auto -- what resolving a given ability generates is
    // real card-specific behaviour (Card Abilities/Q&A about Cards'
    // job), not something this abstract ordering test should hardcode
    // into the engine as "resolving anything always spawns something".
    game.resolve_auto(PlayerId::P1, a);
    let c = game.deploy(PlayerId::P1, "EFILLER-002", Zone::Base).unwrap();
    game.trigger_auto(PlayerId::P1, c);
    assert!(game.pending_autos(PlayerId::P1).contains(&b));
    assert!(game.pending_autos(PlayerId::P1).contains(&c));

    // C -> B is a legal resolution order; C need not go last.
    assert!(game.resolution_order_is_legal(PlayerId::P1, vec![c, b]));
}

#[test]
fn qa_043_turn_player_auto_mid_non_turn_queue_regains_priority() {
    let mut game = GameState::test_scenario();
    game.set_turn_player(PlayerId::P1);
    let d = game.deploy(PlayerId::P2, "EFILLER-000", Zone::Base).unwrap();
    let e = game.deploy(PlayerId::P2, "EFILLER-001", Zone::Base).unwrap();

    game.trigger_auto(PlayerId::P2, d);
    game.trigger_auto(PlayerId::P2, e); // non-turn player's queue is active

    let f = game.deploy(PlayerId::P1, "EFILLER-002", Zone::Base).unwrap();
    game.resolve_auto(PlayerId::P2, d); // resolving D triggers F, the turn player's
    game.trigger_auto(PlayerId::P1, f);

    assert_eq!(
        game.priority_player(),
        PlayerId::P1,
        "F must be offered before returning to resolve E"
    );
}

/// QA #75: "What does 'increase soul' mean?" -> placing the specified
/// number of souls from the Soul Deck into the Soul Area. Souls are a
/// finite, real resource (a Soul Deck), not an abstract counter.
#[test]
fn qa_075_increase_soul_moves_cards_soul_deck_to_soul_area() {
    let mut game = GameState::test_scenario();
    let before_deck = game.zone_count(PlayerId::P1, Zone::SoulDeck);
    let before_area = game.zone_count(PlayerId::P1, Zone::SoulArea);

    game.increase_soul(PlayerId::P1, 2);

    assert_eq!(game.zone_count(PlayerId::P1, Zone::SoulDeck), before_deck - 2);
    assert_eq!(game.zone_count(PlayerId::P1, Zone::SoulArea), before_area + 2);
}

/// QA #96: "Does '[Rest 3 Souls] Draw 1 card' count as playing a card?"
/// -> "No, it does not." (README gotcha 6; QA cites this rule for
/// BP01-100's ability specifically, but the answer is the general rule.)
#[test]
fn qa_096_rest_souls_draw_does_not_count_as_playing_a_card() {
    let mut game = GameState::test_scenario();
    for _ in 0..3 {
        game.increase_soul(PlayerId::P1, 1);
    }
    let played_before = game.cards_played_this_turn(PlayerId::P1);

    game.rest_souls_and_draw(PlayerId::P1, 3);

    assert_eq!(
        game.cards_played_this_turn(PlayerId::P1),
        played_before,
        "rule-8.5 rest-3-draw-1 must not increment the 'played a card' count \
         that Once-Per-Turn 'play a card' abilities key off of"
    );
}

/// Comprehensive rules 1.2.1 (a losing verdict ends the game
/// immediately) and 11.3.3 (life <= 0 is a check-type rule action) --
/// not itself a numbered QA ruling, but the same kind of boundary the
/// #92/#93 fix pinned down: rule actions drain BEFORE any AUTO ever
/// gets priority (10.5.3), so a player hitting 0 life loses with their
/// opponent's (or their own) pending AUTOs still unresolved. This is
/// why a lethal Shadowbeak self-butcher (QA #84) can end the game
/// before the second queued instance ever resolves.
#[test]
fn rule_1_2_1_losing_verdict_ends_the_game_with_pending_autos_still_unresolved() {
    let mut game = GameState::test_scenario();
    game.set_turn_player(PlayerId::P1);
    let source = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    game.trigger_auto(PlayerId::P1, source); // queued, not yet resolved

    game.reduce_life(PlayerId::P2, 10); // P2's life hits 0
    assert!(
        game.game_over().is_none(),
        "the losing verdict is itself a rule action -- it doesn't resolve until check timing runs"
    );

    game.run_check_timing();

    assert_eq!(
        game.game_over(),
        Some(PlayerId::P2),
        "P2 must be the loser once check timing resolves the losing verdict"
    );
    assert_eq!(
        game.pending_autos(PlayerId::P1),
        vec![source],
        "the game must end before P1's pending AUTO ever gets priority -- it stays queued, unresolved"
    );
}
