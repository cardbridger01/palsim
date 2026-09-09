//! QA category "Card Abilities" (45 rulings) -- general ability-timing
//! and zone rules that apply across cards, as opposed to "Q&A about
//! Cards" (qa_about_cards.rs), which resolves ambiguities on a specific
//! named card.
//!
//! Full coverage: all 45 "Card Abilities" rulings are encoded below.

use engine::state::{GameState, PlayerId, Timing, Zone};
// NEW: first real use of engine::selection and engine::combat/engine::cards,
// sketched in qa_rulings/mod.rs but unused by the original 4 worked examples.
use engine::selection::{SelectionError, Selector};

/// QA #32: "Does an On Deploy ability activate even if the card is
/// deployed from a zone other than the hand?" -> "Yes ... activates not
/// only when deployed from the hand, but also when deployed from the
/// deck, graveyard, or other zones by card effects." (CLAUDE.md's
/// canonical Chillet-chain test rests on this; see replays/battle1.rs
/// for the end-to-end replay that exercises it for real.)
#[test]
fn qa_032_on_deploy_fires_regardless_of_source_zone() {
    let mut game = GameState::test_scenario();

    for from_zone in [Zone::Hand, Zone::Deck, Zone::Graveyard] {
        let card = game.place(PlayerId::P1, "EFILLER-000", from_zone);
        let fired = game.deploy_from(PlayerId::P1, card, from_zone, Zone::Base);
        assert!(
            fired.on_deploy_triggered,
            "On Deploy must trigger when deployed from {from_zone:?}, not just from Hand"
        );
    }
}

/// QA #41: "Does an ability that states 'When this card is put into the
/// graveyard' activate even if it is put into the graveyard from the
/// hand or deck?" -> "No ... Unless specified otherwise, it activates
/// only when put into the graveyard from the base." (README gotcha:
/// Leezpunk milled != discarded, per CLAUDE.md's QA table.)
#[test]
fn qa_041_graveyard_trigger_fires_only_from_base() {
    let mut game = GameState::test_scenario();

    let discarded_from_hand = game.place(PlayerId::P1, "EFILLER-000", Zone::Hand);
    game.move_to_graveyard(discarded_from_hand, Zone::Hand);
    assert!(!game.graveyard_trigger_fired(discarded_from_hand));

    let milled_from_deck = game.place(PlayerId::P1, "EFILLER-001", Zone::Deck);
    game.move_to_graveyard(milled_from_deck, Zone::Deck);
    assert!(!game.graveyard_trigger_fired(milled_from_deck));

    let destroyed_from_base = game.deploy(PlayerId::P1, "EFILLER-002", Zone::Base).unwrap();
    game.move_to_graveyard(destroyed_from_base, Zone::Base);
    assert!(
        game.graveyard_trigger_fired(destroyed_from_base),
        "the trigger fires only for the base->graveyard transition"
    );
}

/// QA #92: "A Pal's Power became 0 or less due to a card effect. Is that
/// Pal put into the graveyard?" -> "No ... A Pal is not put into the
/// graveyard simply by its Power becoming 0 or less. It is put into the
/// graveyard when the damage it has taken becomes greater than its
/// Power." (1.3.2.4 -- Power is stored signed and unclamped.)
///
/// Read together with QA #93 and comprehensive rule 11.4.1 (see that
/// test's doc comment for the full resolution), the lethal check is two
/// conditions, not one: `damage_taken > 0 && damage_taken >= power`. This
/// test is the `damage_taken > 0` half -- an undamaged Pal never dies to
/// Power alone, however negative Power gets.
#[test]
fn qa_092_zero_or_negative_power_alone_does_not_kill() {
    let mut game = GameState::test_scenario();
    let pal = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

    game.reduce_power(pal, 10_000); // driven well below 0 by a card effect
    assert!(game.power(pal) <= 0, "Power must be stored signed, not clamped at 0");
    assert_eq!(game.damage_taken(pal), 0);

    game.run_check_timing();
    assert_eq!(
        game.zone_of(pal),
        Zone::Base,
        "Power <= 0 with zero damage taken must not, by itself, send the Pal to the graveyard \
         (damage_taken > 0 is required even when power <= 0)"
    );
}

/// QA #93: "A Pal's Power was reduced by a card effect and became equal
/// to or less than the Damage that Pal had already taken. Is that Pal
/// put into the graveyard?" -> "Yes, it is put into the graveyard."
///
/// RESOLVED (was flagged as a discrepancy against CLAUDE.md; now fixed
/// in both places): the *question* itself poses the equal-to case
/// ("became equal to or less than the Damage") and the answer is
/// unqualified "Yes" -- so equality is lethal. The answer's own
/// follow-up sentence ("If the damage taken becomes greater than its
/// Power...") is loose paraphrase in the explanation, not the ruling;
/// it doesn't get to silently narrow the question it's explaining.
/// Comprehensive rule 11.4.1 settles it independently and precisely:
///
/// > If any Pal or structure takes greater than 0 damage, and it is
/// > equal to or greater than that Pal's power, put that Pal into
/// > their owner's graveyard.
///
/// Two conditions, both required: `damage_taken > 0 && damage_taken >=
/// power`. The first conjunct is QA #92's entire content (Power <= 0
/// with zero damage never dies); the second is what this test covers.
/// This also matches observed play: an even-Power trade in combat (e.g.
/// two 900-Power Pals blocking each other) kills both sides, and it's
/// load-bearing for the Turtles matchup analysis (Seismic 1900 blocking
/// Ignis 1700: Ignis takes 1900 >= 1700 and dies, Seismic takes 1700 <
/// 1900 and lives) -- a strict `>` reading would leave every mirror-power
/// trade in the game one-sided instead.
#[test]
fn qa_093_dies_when_damage_taken_is_at_least_reduced_power() {
    let mut game = GameState::test_scenario();
    let pal = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

    // damage_taken == power (the equality case QA #93's question asks
    // about directly) must be lethal.
    game.apply_damage(pal, 600);
    game.reduce_power(pal, (game.power(pal) - 600) as u32); // Power -> exactly 600
    game.run_check_timing();
    assert_eq!(
        game.zone_of(pal),
        Zone::Graveyard,
        "damage_taken == power must be lethal (rule 11.4.1: 'equal to or greater than'; \
         QA #93's own question poses this exact equality case and the answer is Yes)"
    );
}

/// The other half of 11.4.1's two-condition check: damage strictly less
/// than power must NOT be lethal, even though damage_taken > 0. Not
/// itself a numbered QA ruling -- it's the boundary 11.4.1 implies and
/// #92/#93 don't individually cover -- included so the `> 0 && >=`
/// model is pinned down on both sides, not just the equality case.
#[test]
fn rule_11_4_1_damage_taken_below_power_is_not_lethal_even_when_positive() {
    let mut game = GameState::test_scenario();
    let pal = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

    game.apply_damage(pal, 600);
    game.reduce_power(pal, (game.power(pal) - 700) as u32); // Power -> 700, damage_taken (600) < power
    game.run_check_timing();
    assert_eq!(
        game.zone_of(pal),
        Zone::Base,
        "damage_taken (600) < power (700): damage_taken > 0 but NOT >= power, so this must survive"
    );
}

/// 10.5.3.1: "all rule actions resolve, and if that produces new rule
/// actions, repeat" -- a genuine fixpoint, not one pass. Every rule
/// action modeled so far happens to be one-shot on its own (a losing
/// verdict halts everything; resolving one Pal's lethal damage doesn't
/// itself read from anything that could cascade yet), so nothing above
/// has ever forced `run_check_timing`'s loop around more than once. An
/// implementation that resolved a single pending rule action and
/// stopped would still pass every test above.
///
/// This one doesn't let that slide: two Pals are lethal SIMULTANEOUSLY
/// before the call, so `find_pending_rule_action` has to be asked again
/// -- and find the second one -- after resolving the first, within the
/// same `run_check_timing()` call. A single-pass implementation would
/// resolve exactly one and leave the other sitting in the base.
#[test]
fn rule_10_5_3_1_check_timing_drains_multiple_pending_rule_actions_in_one_call() {
    let mut game = GameState::test_scenario();
    let first = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    let second = game.deploy(PlayerId::P1, "EFILLER-001", Zone::Base).unwrap();

    game.apply_damage(first, 600);
    game.reduce_power(first, (game.power(first) - 600) as u32); // lethal: 600 >= 600
    game.apply_damage(second, 700);
    game.reduce_power(second, (game.power(second) - 700) as u32); // lethal: 700 >= 700

    game.run_check_timing();

    assert_eq!(game.zone_of(first), Zone::Graveyard, "the first lethal Pal found must resolve");
    assert_eq!(
        game.zone_of(second),
        Zone::Graveyard,
        "the second must ALSO resolve in the same run_check_timing() call -- \
         a single-pass implementation would leave this one in the base"
    );
}

/// QA #13: "What is night?" -> "This is a game state that changes via
/// card effects. Being 'night' does not inherently trigger any rules by
/// itself, but certain cards have abilities that activate or become
/// active while it is night." A plain, settable state with no built-in
/// side effects of its own.
#[test]
fn qa_013_night_is_a_settable_state_with_no_inherent_trigger() {
    let mut game = GameState::test_scenario();
    assert!(!game.night_active());

    game.set_night(true);
    assert!(game.night_active());
    assert!(
        game.pending_autos(PlayerId::P1).is_empty(),
        "becoming night must not, by itself, trigger any ability -- only cards that reference night react to it"
    );
}

/// QA #14: "What are Material and Ingredient?" -> "These are information
/// of a player. Material and Ingredient by themselves do not hold any
/// special abilities, but they may be gained or consumed by card
/// abilities."
#[test]
fn qa_014_material_and_ingredient_are_plain_player_resources() {
    let mut game = GameState::test_scenario();
    assert_eq!(game.materials(PlayerId::P1), 0);
    assert_eq!(game.ingredients(PlayerId::P1), 0);

    game.gain_materials(PlayerId::P1, 2);
    game.gain_ingredients(PlayerId::P1, 1);
    assert_eq!(game.materials(PlayerId::P1), 2);
    assert_eq!(game.ingredients(PlayerId::P1), 1);

    game.consume_materials(PlayerId::P1, 2);
    assert_eq!(game.materials(PlayerId::P1), 0);
}

/// QA #15: "What are terms like 'Kindling' or 'Farming' written on the
/// cards?" -> "They are Work Suitabilities. It is a piece of
/// information possessed by cards, and may be referred to by card
/// abilities."
#[test]
fn qa_015_work_suitability_is_referenceable_card_information() {
    let mut game = GameState::test_scenario();
    let card = game.place(PlayerId::P1, "EFILLER-000", Zone::Hand);
    game.set_work_suitabilities(card, vec!["Kindling".to_string()]);
    assert!(game.has_work_suitability(card, "Kindling"));
}

/// QA #16: "If a card has two or more Work Suitabilities, does it mean
/// it possesses each one of them?" -> "Yes ... if a card has 'Kindling'
/// and 'Crafting', it possesses both individually, and it can be
/// referred to by either."
#[test]
fn qa_016_multiple_work_suitabilities_are_each_individually_possessed() {
    let mut game = GameState::test_scenario();
    let card = game.place(PlayerId::P1, "EFILLER-000", Zone::Hand);
    game.set_work_suitabilities(card, vec!["Kindling".to_string(), "Crafting".to_string()]);
    assert!(game.has_work_suitability(card, "Kindling"));
    assert!(game.has_work_suitability(card, "Crafting"));
}

/// QA #17: "What is the significance of a card's 'Color'?" -> "When
/// constructing a deck, you must build it using up to any two chosen
/// colors plus colorless cards. Additionally, colors may be referred to
/// by card abilities." (The two-color rule itself is the Quick-Manual
/// rule CLAUDE.md flags as easy to lose; this ruling is the ability-
/// reference half of Color's significance.)
#[test]
fn qa_017_color_is_referenceable_by_card_abilities() {
    let mut game = GameState::test_scenario();
    let card = game.deploy(PlayerId::P1, "EFILLER-RED", Zone::Base).unwrap();
    game.set_color(card, "Red");
    assert_eq!(game.color_of(card), "Red");
}

/// QA #18: "What are terms like Fire or Dragon on the cards?" -> "They
/// are elements. It is a piece of information possessed by cards, and
/// may be referred to by card abilities." (Hard rule 2: Element is a
/// separate axis from Color, never derivable from it.)
#[test]
fn qa_018_element_is_referenceable_card_information_independent_of_color() {
    let mut game = GameState::test_scenario();
    let card = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    game.set_color(card, "Blue");
    game.set_elements(card, vec!["Ice".to_string(), "Dragon".to_string()]);
    assert_eq!(game.elements_of(card), vec!["Ice".to_string(), "Dragon".to_string()]);
    assert_eq!(game.color_of(card), "Blue", "setting Element must not imply or overwrite Color");
}

/// QA #19: "What is an 'Owner'?" -> "It refers to the owner of that
/// card."
#[test]
fn qa_019_owner_is_the_player_who_owns_the_card() {
    let mut game = GameState::test_scenario();
    let card = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    assert_eq!(game.owner_of(card), PlayerId::P1);
}

/// QA #20: "Some cards with the same name but different rarities lack
/// the text written in parentheses. Does this mean their effects are
/// different?" -> "No, the effect is the same. The text written in
/// parentheses is explanatory text, so its presence or absence does not
/// change the card's effect."
#[test]
fn qa_020_parenthetical_explanatory_text_does_not_change_a_cards_effect() {
    let with_parens = engine::cards::parse_effect_text("【AUTO】 Deal 300 [Damage] (this ability triggers On Deploy)");
    let without_parens = engine::cards::parse_effect_text("【AUTO】 Deal 300 [Damage]");
    assert_eq!(with_parens, without_parens);
}

/// QA #21: "Some cards that have keywords like Interrupt, Taunt, or
/// Brave lack the text written in parentheses. Does this mean their
/// effects are different?" -> "No, the effect is the same" -- same rule
/// as #20, restated for keyword reminder text specifically.
#[test]
fn qa_021_parenthetical_keyword_reminder_text_does_not_change_a_cards_effect() {
    let with_parens = engine::cards::parse_effect_text("〈Interrupt〉 (nullify the opponent's attack)");
    let without_parens = engine::cards::parse_effect_text("〈Interrupt〉");
    assert_eq!(with_parens, without_parens);
}

/// QA #26: "What is a 'Main Name'?" -> "It is the portion before the '
/// – ' in the card name. It is a piece of information possessed by
/// cards, and may be referred to by card abilities."
#[test]
fn qa_026_main_name_is_the_text_before_the_en_dash() {
    let mut game = GameState::test_scenario();
    let card = game.deploy(PlayerId::P1, "EBP01-053", Zone::Base).unwrap();
    game.set_card_name(card, "Elizabee – Queen of the Flower Garden");
    assert_eq!(game.main_name_of(card), "Elizabee");
}

/// QA #27: "When a card ability does not specify a zone -- such as
/// 'Choose X Pals,' 'The number of your gears,' or 'Choose all cards' --
/// does it refer to cards in other zones like the hand or deck, in
/// addition to the base?" -> "No, it does not. Unless a specific zone is
/// stated, it refers to cards in the base."
#[test]
fn qa_027_unzoned_references_mean_the_base_only() {
    let mut game = GameState::test_scenario();
    let base_card = game.deploy(PlayerId::P1, "EFILLER-BASE", Zone::Base).unwrap();
    game.place(PlayerId::P1, "EFILLER-HAND", Zone::Hand);
    game.place(PlayerId::P1, "EFILLER-DECK", Zone::Deck);

    let chosen = game.choose_all(PlayerId::P1, "cards", base_card);
    assert_eq!(chosen, vec![base_card], "'Choose all cards' with no stated zone must resolve against the base only, not hand/deck too");
}

/// QA #28: "Can I play a card with Quick during the Main Phase of my
/// turn?" -> "Yes, you can." Quick cards are playable at ordinary timing
/// too, not only the opponent's-turn window.
#[test]
fn qa_028_quick_may_be_played_during_your_own_main_phase() {
    let game = GameState::test_scenario();
    assert!(game.can_play_card(PlayerId::P1, true, Timing::OwnMain));
}

/// QA #29: "Can I play a card with Quick during a battle on my turn?" ->
/// "No, you cannot. Cards with Quick can only be used during battle on
/// the opponent's turn." Together with #28 and #30 this resolves
/// CLAUDE.md's open question about QA #29's literal wording: Quick has
/// TWO windows -- your own Main Phase (like an ordinary card, per #28)
/// and battle on the opponent's turn (per #30) -- but never during a
/// battle on your OWN turn.
#[test]
fn qa_029_quick_cannot_be_played_during_a_battle_on_your_own_turn() {
    let game = GameState::test_scenario();
    assert!(!game.can_play_card(PlayerId::P1, true, Timing::OwnTurnDuringBattle));
}

/// QA #30: "Can I play multiple cards with Quick during a battle on the
/// opponent's turn?" -> "Yes ... First, choose 1 card you wish to play,
/// play it, and resolve its effect. Then, if there is another card you
/// wish to play, you may play it and resolve its effect." One at a
/// time, fully resolved before the next.
#[test]
fn qa_030_multiple_quick_cards_may_be_played_one_after_another_on_the_opponents_turn() {
    let mut game = GameState::test_scenario();
    assert!(game.can_play_card(PlayerId::P2, true, Timing::OpponentTurnDuringBattle));
    game.play_quick(PlayerId::P2, "EQUICK-001", Timing::OpponentTurnDuringBattle).unwrap();

    assert!(game.can_play_card(PlayerId::P2, true, Timing::OpponentTurnDuringBattle));
    game.play_quick(PlayerId::P2, "EQUICK-002", Timing::OpponentTurnDuringBattle).unwrap();
}

/// QA #31: "Does a card's ability activate regardless of which zone the
/// card is currently in?" -> "No ... Unless specified otherwise, it is
/// effective only while in the Base. Certain abilities, such as
/// Interrupt, can become effective outside of the Base." (QA #51 pins
/// down that Interrupt's specific exception is "in the hand", not
/// "anywhere outside the base".)
#[test]
fn qa_031_abilities_are_effective_only_in_the_base_by_default() {
    let mut game = GameState::test_scenario();
    let in_hand = game.place(PlayerId::P1, "EFILLER-CONT", Zone::Hand);
    game.set_cont_ability(in_hand, "your Pals get Power +100");
    assert!(!game.cont_ability_active(in_hand), "a CONT ability must not apply while its card is only in the hand");

    let in_base = game.deploy(PlayerId::P1, "EFILLER-CONT-2", Zone::Base).unwrap();
    game.set_cont_ability(in_base, "your Pals get Power +100");
    assert!(game.cont_ability_active(in_base));
}

/// QA #33: "With an ability that states 'Choose X cards,' can I choose a
/// number fewer than X?" -> "No ... You must choose targets as much as
/// possible. For 'Choose 2 Pals', with 2+ Pals in the base you must
/// choose exactly 2; with only 1 Pal, choose that 1 and resolve the
/// rest."
#[test]
fn qa_033_choose_x_requires_as_many_as_possible_up_to_x() {
    let selector = Selector::choose_exact(2);
    assert_eq!(selector.validate(&[1, 2], &[1, 2, 3]), Ok(()));
    assert_eq!(
        selector.validate(&[1], &[1, 2, 3]),
        Err(SelectionError::MustChooseAsManyAsPossible { required: 2, available: 3 })
    );
    // Only 1 eligible total: choosing that 1 satisfies "choose 2 as much as possible".
    assert_eq!(selector.validate(&[1], &[1]), Ok(()));
}

/// QA #34: "With an ability that states 'Choose up to X cards,' can I
/// choose 0 cards?" -> "Yes ... by choosing 0 cards, you can effectively
/// choose not to select any."
#[test]
fn qa_034_choose_up_to_x_allows_choosing_zero() {
    let selector: Selector<i32> = Selector::choose_up_to(3);
    assert_eq!(selector.validate(&[], &[1, 2, 3]), Ok(()));
}

/// QA #35: "With an ability that states 'Consume X [resources],' can I
/// choose 0?" -> "Yes, you can."
#[test]
fn qa_035_consume_x_allows_choosing_zero() {
    let selector: Selector<i32> = Selector::consume_up_to(2);
    assert_eq!(selector.validate(&[], &[1, 2]), Ok(()));
}

/// QA #36: "With an ability that states 'Choose all,' can I choose to
/// exclude some of them?" -> "No, you cannot."
#[test]
fn qa_036_choose_all_cannot_exclude_any_eligible_card() {
    let selector = Selector::choose_all();
    assert_eq!(
        selector.validate(&[1, 2], &[1, 2, 3]),
        Err(SelectionError::MustChooseAllEligible { available: 3 })
    );
    assert_eq!(selector.validate(&[1, 2, 3], &[1, 2, 3]), Ok(()));
}

/// QA #37: "Can I choose not to activate an automatic ability whose
/// trigger conditions have been met?" -> "No, you cannot. It must always
/// be activated."
#[test]
fn qa_037_a_triggered_auto_cannot_be_declined() {
    let mut game = GameState::test_scenario();
    let source = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    game.trigger_auto(PlayerId::P1, source);
    assert!(
        game.decline_auto(PlayerId::P1, source).is_err(),
        "an AUTO whose trigger condition is met must always be activated -- there is no 'choose not to' option"
    );
}

/// QA #38: "I played an ability that states 'Look at the top X cards of
/// your deck,' but the number of cards in my deck was fewer than X. Can
/// the effect still be resolved?" -> "Yes ... Look at the remaining
/// cards as much as possible ... even if the deck temporarily becomes 0
/// cards during resolution, as long as 1+ card remains in the deck after
/// resolution finishes, you do not lose the game."
#[test]
fn qa_038_look_at_top_x_with_fewer_than_x_cards_resolves_with_what_is_there() {
    let mut game = GameState::test_scenario();
    game.set_deck_size_for_test(PlayerId::P1, 2); // fewer than the 5 the ability asks for
    let looked = game.look_at_top(PlayerId::P1, 5);
    assert_eq!(looked.len(), 2, "must look at as many as are actually in the deck, not fail or look at 0");
    assert_eq!(game.zone_count(PlayerId::P1, Zone::Deck), 0);
}

/// QA #39: "I played an ability that states 'Reveal the top X cards of
/// your deck,' but the number of cards in my deck was fewer than X."
/// -> same resolution as #38, for Reveal instead of Look. (Same rule as
/// battle handbook §9.6/CLAUDE.md's "Known unknowns" deck-out entry:
/// temporarily hitting 0 mid-resolution is not itself a loss.)
#[test]
fn qa_039_reveal_top_x_with_fewer_than_x_cards_resolves_with_what_is_there() {
    let mut game = GameState::test_scenario();
    game.set_deck_size_for_test(PlayerId::P1, 1);
    let revealed = game.reveal_top(PlayerId::P1, 3);
    assert_eq!(revealed.len(), 1);
    assert_eq!(game.zone_count(PlayerId::P1, Zone::Deck), 0);

    game.refill_deck_for_test(PlayerId::P1, 1); // resolution finishes with >=1 card in deck
    game.run_check_timing();
    assert!(!game.has_lost(PlayerId::P1));
}

/// QA #40: "If there are multiple cards with Taunt, which card should I
/// prioritize attacking?" -> "The attacking player can choose the target
/// of the attack from among the cards that possess 'Taunt'."
#[test]
fn qa_040_attacker_chooses_which_taunt_card_to_target_among_several() {
    let mut game = GameState::test_scenario();
    let taunt_a = game.deploy(PlayerId::P2, "EFILLER-TAUNT-A", Zone::Base).unwrap();
    let taunt_b = game.deploy(PlayerId::P2, "EFILLER-TAUNT-B", Zone::Base).unwrap();
    game.rest(taunt_a);
    game.rest(taunt_b);
    game.grant_keyword(taunt_a, "Taunt");
    game.grant_keyword(taunt_b, "Taunt");

    let legal_targets = game.taunt_compelled_targets(PlayerId::P1);
    assert_eq!(legal_targets.len(), 2, "with multiple Taunt cards eligible, the attacker chooses among all of them");
}

/// QA #44: "I have multiple copies of the same card with a [Once Per
/// Turn] ability in my base. Can the ability be activated once for each
/// individual card?" -> "Yes ... if you have multiple copies of TD01-008
/// (Stone Pit), even after activating one copy's ACT, you can still
/// activate the other copy's."
#[test]
fn qa_044_once_per_turn_is_tracked_per_card_instance_not_per_name() {
    let mut game = GameState::test_scenario();
    let stone_pit_a = game.deploy(PlayerId::P1, "ETD01-008", Zone::Base).unwrap();
    let stone_pit_b = game.deploy(PlayerId::P1, "ETD01-008", Zone::Base).unwrap();

    assert!(game.activate_act(PlayerId::P1, stone_pit_a).is_ok());
    assert!(
        game.activate_act(PlayerId::P1, stone_pit_b).is_ok(),
        "a second copy of the same once-per-turn card must have its own independent activation"
    );
}

/// QA #45: "Does an ability that states 'At the end of the battle this
/// card attacked' activate even if the attack was Interrupted?" -> "Yes,
/// it activates."
#[cfg(any())] // batch 4 (combat: engine::combat, declare_attack/block/interrupt_attack) -- excluded from compilation, not just skipped, since the body references a module that does not exist yet. Not this session; see qa_rulings/mod.rs. Remove this attribute once engine::combat exists.
#[test]
fn qa_045_end_of_battle_this_card_attacked_fires_even_if_interrupted() {
    let mut game = GameState::test_scenario();
    let attacker = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    game.declare_attack(attacker, engine::combat::AttackTarget::Player(PlayerId::P2));
    game.interrupt_attack(attacker);
    assert!(game.attack_was_interrupted(attacker));

    assert!(
        game.end_of_battle_this_card_attacked_triggered(attacker),
        "the 'end of the battle this card attacked' trigger must still fire even when the attack was nullified by Interrupt"
    );
}

/// QA #46: "For an ability that states 'You may [do X],' is it
/// mandatory to [do X]?" -> "No, it is not mandatory. You can choose
/// whether or not to do it."
#[test]
fn qa_046_you_may_do_x_is_never_mandatory() {
    let mut game = GameState::test_scenario();
    let source = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    assert!(game.optional_effect_may_be_declined(source, "you may deploy a Pal from your hand"));
}

/// QA #47: "Do abilities that refer to 'all red Pals' or 'all blue
/// Pals' include colorless Pals?" -> "No, it does not include them.
/// Colorless Pals do not have a color."
#[test]
fn qa_047_all_of_a_color_excludes_colorless_pals() {
    let mut game = GameState::test_scenario();
    let red = game.deploy(PlayerId::P1, "EFILLER-RED", Zone::Base).unwrap();
    game.set_color(red, "Red");
    let colorless = game.deploy(PlayerId::P1, "EFILLER-COLORLESS", Zone::Base).unwrap();
    game.set_color(colorless, "Colorless");

    let chosen = game.choose_all(PlayerId::P1, "red Pals", red);
    assert!(chosen.contains(&red));
    assert!(!chosen.contains(&colorless), "'all red Pals' must not include a Colorless Pal");
}

/// QA #48: "If a card has two or more ACT abilities that are [Once Per
/// Turn], can each ACT ability be activated once?" -> "Yes ... each one
/// can be activated once."
#[test]
fn qa_048_two_distinct_once_per_turn_act_abilities_on_one_card_each_activate_once() {
    let mut game = GameState::test_scenario();
    let card = game.deploy(PlayerId::P1, "EFILLER-DUAL-ACT", Zone::Base).unwrap();

    assert!(game.activate_act_named(PlayerId::P1, card, "ACT-1").is_ok());
    assert!(
        game.activate_act_named(PlayerId::P1, card, "ACT-2").is_ok(),
        "a SEPARATE once-per-turn ACT ability on the same card must have its own independent lockout"
    );
    assert!(
        game.activate_act_named(PlayerId::P1, card, "ACT-1").is_err(),
        "but re-activating ACT-1 again this turn must still be blocked"
    );
}

/// QA #49: "My Pal attacked an opponent's Pal. During this, the target
/// Pal left the base due to an On Attack ability of my attacking Pal.
/// Can I change the target to the opponent player or another Pal?" ->
/// "No ... the battle proceeds as normal. However, because there is no
/// battle opponent, the battle ends without the attacking Pal dealing or
/// taking any damage."
#[cfg(any())] // batch 4 (combat: engine::combat, declare_attack/block/interrupt_attack) -- excluded from compilation, not just skipped, since the body references a module that does not exist yet. Not this session; see qa_rulings/mod.rs. Remove this attribute once engine::combat exists.
#[test]
fn qa_049_target_removed_mid_battle_by_attackers_own_on_attack_cannot_be_retargeted() {
    let mut game = GameState::test_scenario();
    let attacker = game.deploy(PlayerId::P1, "EFILLER-ON-ATTACK-REMOVES-TARGET", Zone::Base).unwrap();
    let target = game.deploy(PlayerId::P2, "EFILLER-000", Zone::Base).unwrap();
    game.rest(target);

    let attack = game.declare_attack(attacker, engine::combat::AttackTarget::Pal(target));
    game.trigger_on_attack_removes_target(attacker); // the attacker's own On Attack removes `target`
    assert!(game.battle_target(attack).is_none());

    assert!(
        game.retarget_attack(attack, PlayerId::P2).is_err(),
        "there must be no legal action that retargets the battle onto the player or another Pal"
    );
    game.resolve_battle(attack);
    assert_eq!(
        game.damage_taken(attacker),
        0,
        "with no battle opponent, the battle must end with neither side dealing or taking damage"
    );
}

/// QA #50: "Does an ability that states 'When this card is attacked'
/// activate even if I block using the card that has this ability?" ->
/// "No, it does not activate. It activates when that card is attacked
/// by an opponent's Pal [i.e. as the declared target, not as a
/// blocker]."
#[cfg(any())] // batch 4 (combat: engine::combat, declare_attack/block/interrupt_attack) -- excluded from compilation, not just skipped, since the body references a module that does not exist yet. Not this session; see qa_rulings/mod.rs. Remove this attribute once engine::combat exists.
#[test]
fn qa_050_when_this_card_is_attacked_does_not_fire_for_a_card_used_to_block() {
    let mut game = GameState::test_scenario();
    let attacker = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    let blocker = game.deploy(PlayerId::P2, "EFILLER-WHEN-ATTACKED", Zone::Base).unwrap();
    game.rest(blocker);

    let attack = game.declare_attack(attacker, engine::combat::AttackTarget::Player(PlayerId::P2));
    game.block(attack, blocker);

    assert!(
        !game.on_attacked_triggered(blocker),
        "'when this card is attacked' must not fire for a card that becomes the battle opponent by blocking"
    );
}

/// QA #51: "Can an ability that states 'ACT Interrupt ([Hand][Quick]
/// [cost] Nullify the opponent's attack ...)' be activated even if the
/// card with this ability is in the base?" -> "No ... Interrupt is an
/// ability that can be activated only while the card is in the hand."
#[test]
fn qa_051_interrupt_cannot_be_activated_from_the_base() {
    let mut game = GameState::test_scenario();
    let in_base = game.deploy(PlayerId::P1, "EFILLER-INTERRUPT", Zone::Base).unwrap();
    assert!(
        game.activate_interrupt(in_base).is_err(),
        "Interrupt can only be activated from the HAND, not the base -- the specific case of #31's general rule"
    );

    let in_hand = game.place(PlayerId::P1, "EFILLER-INTERRUPT-2", Zone::Hand);
    assert!(game.activate_interrupt(in_hand).is_ok());
}

/// QA #59: "I played a card that states 'Look at the top X cards of your
/// deck, choose up to Y (specific) cards and add them to your hand, and
/// shuffle the rest with the deck.' What happens if there are no such
/// specific cards among the cards I looked at?" -> "Choose 0 cards and
/// resolve the rest of the effect."
#[test]
fn qa_059_look_choose_up_to_y_specific_to_hand_with_no_matches_chooses_zero() {
    let mut game = GameState::test_scenario();
    for i in 0..5 {
        game.place(PlayerId::P1, &format!("EFILLER-NONMATCH-{i}"), Zone::Deck);
    }
    let looked = game.look_at_top(PlayerId::P1, 5);
    let matching: Vec<_> = looked.iter().copied().filter(|c| game.main_name_of(*c) == "SpecificWanted").collect();
    assert!(matching.is_empty());

    assert!(
        game.choose_up_to_n_to_hand(PlayerId::P1, &matching, 1).is_ok(),
        "choosing 0 from an empty matching set must be legal; the rest of the effect (shuffle) still resolves"
    );
}

/// QA #60: same shape as #59, for a "choose up to Y (specific) cards and
/// DEPLOY them" ability -> "Choose 0 cards and resolve the rest of the
/// effect."
#[test]
fn qa_060_look_choose_up_to_y_specific_to_deploy_with_no_matches_chooses_zero() {
    let mut game = GameState::test_scenario();
    for i in 0..5 {
        game.place(PlayerId::P1, &format!("EFILLER-NONMATCH-{i}"), Zone::Deck);
    }
    let looked = game.look_at_top(PlayerId::P1, 5);
    let matching: Vec<_> = looked.iter().copied().filter(|c| game.main_name_of(*c) == "SpecificWanted").collect();
    assert!(game.choose_up_to_n_to_deploy(PlayerId::P1, &matching, 1).is_ok());
}

/// QA #61: "I played a card that states 'Choose up to X (specific
/// condition) Pals, and rest them. During (specific period), they do not
/// stand.' Can I choose a Pal that is already in the rest state?" ->
/// "Yes ... it will not become rested again, but the chosen card will
/// not stand for the duration specified."
#[test]
fn qa_061_choosing_an_already_rested_pal_still_applies_the_wont_stand_duration() {
    let mut game = GameState::test_scenario();
    let pal = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    game.rest(pal);
    assert!(game.is_resting(pal));

    game.choose_and_rest_wont_stand_next(PlayerId::P1, pal);
    assert!(game.is_resting(pal), "it must simply remain rested -- resting an already-rested card is a no-op");
    assert!(
        game.wont_stand_next_stand_phase(pal),
        "the 'does not stand during (period)' clause still applies, independent of whether resting it did anything"
    );
}

/// QA #71: "I attacked a structure with a Pal that has 'AUTO When this
/// card attacks a structure.' If that attack is blocked by an opponent's
/// Pal, does the ability still activate?" -> "Yes ... it activates at
/// the exact moment the card declares an attack on the structure, which
/// occurs BEFORE the opponent designates a blocking Pal."
#[cfg(any())] // batch 4 (combat: engine::combat, declare_attack/block/interrupt_attack) -- excluded from compilation, not just skipped, since the body references a module that does not exist yet. Not this session; see qa_rulings/mod.rs. Remove this attribute once engine::combat exists.
#[test]
fn qa_071_on_attacks_a_structure_fires_at_declaration_even_if_later_blocked() {
    let mut game = GameState::test_scenario();
    let attacker = game.deploy(PlayerId::P1, "EBP01-052", Zone::Base).unwrap();
    let structure = game.deploy(PlayerId::P2, "EFILLER-STRUCTURE", Zone::Base).unwrap();
    let blocker = game.deploy(PlayerId::P2, "EFILLER-BLOCKER", Zone::Base).unwrap();
    game.rest(blocker);

    let attack = game.declare_attack(attacker, engine::combat::AttackTarget::Structure(structure));
    assert!(game.on_attacks_structure_triggered(attacker), "the trigger fires the instant the attack is declared");

    game.block(attack, blocker);
    assert!(
        game.on_attacks_structure_triggered(attacker),
        "being blocked afterward must not retroactively un-fire an already-fired trigger"
    );
}

/// QA #74: "A Pal had its Power increased by a CONT ability, but the
/// condition for the CONT ability is no longer met, causing its Power to
/// return to normal. If the Pal has already taken Damage equal to or
/// greater than its returned Power, what happens?" -> "Since it has
/// taken Damage equal to or greater than its Power, it is put into the
/// graveyard."
///
/// This ruling's own wording ("equal to or greater than") differs from
/// QA #93's ("greater than", strict) for what reads like the same
/// general lethal-damage rule -- a discrepancy in the official QA data
/// itself, not a paraphrase issue. Encoded per THIS ruling's literal
/// wording since it's answering its own distinct scenario (a CONT buff
/// expiring); flagged for a human to reconcile against the comprehensive
/// rules rather than silently picking one interpretation for both.
#[test]
fn qa_074_power_reverting_to_normal_can_be_immediately_lethal() {
    let mut game = GameState::test_scenario();
    let pal = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    let modifier = game.apply_cont_power_modifier(pal, pal, 500);
    game.apply_damage(pal, game.power(pal) as u32); // damage taken == the CURRENT (buffed) Power

    game.remove_cont_power_modifier(modifier); // the CONT's condition stops being met; Power reverts
    game.run_check_timing();

    assert_eq!(
        game.zone_of(pal),
        Zone::Graveyard,
        "once Power reverts, damage already taken that now meets/exceeds the reverted Power must be lethal immediately"
    );
}

/// QA #76: "I played an ability that states 'Increase your soul by X (in
/// the rest state).' Can I increase the number of my souls beyond 10?"
/// -> "No, you cannot. The maximum limit for souls is 10."
#[test]
fn qa_076_increase_soul_cannot_push_souls_beyond_ten() {
    let mut game = GameState::test_scenario();
    for _ in 0..8 {
        game.increase_soul(PlayerId::P1, 1);
    }
    assert_eq!(game.zone_count(PlayerId::P1, Zone::SoulArea), 8);

    game.increase_soul(PlayerId::P1, 5); // would be 13 if uncapped
    assert_eq!(
        game.zone_count(PlayerId::P1, Zone::SoulArea),
        10,
        "an 'increase soul by 5' effect starting at 8 souls must not overshoot the 10 cap"
    );
}

/// QA #81: "What is Butcher?" -> "It means putting the specified number
/// of Pals in the Base into the graveyard. For 'Choose 1 of your Pals,
/// and butcher it,' choose 1 Pal in your Base and put it into the
/// graveyard."
#[test]
fn qa_081_butcher_means_choose_n_of_your_base_pals_and_graveyard_them() {
    let mut game = GameState::test_scenario();
    let pal = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    game.butcher(pal);
    assert_eq!(game.zone_of(pal), Zone::Graveyard);
}

/// QA #87: "I played a card that states 'Choose up to X (specific) cards
/// from your graveyard and return them to your hand' or '... and deploy
/// them.' What happens if there are no such specific cards in the
/// graveyard?" -> "Choose 0 cards and resolve the rest of the effect."
/// Same rule as #59/#60, sourced from the graveyard instead of the deck.
#[test]
fn qa_087_choose_up_to_x_specific_from_graveyard_with_no_matches_chooses_zero() {
    let mut game = GameState::test_scenario();
    let non_matching = game.deploy(PlayerId::P1, "EFILLER-NONMATCH", Zone::Base).unwrap();
    game.move_to_graveyard(non_matching, Zone::Base);

    let matching: Vec<_> = game
        .cards_in_zone(PlayerId::P1, Zone::Graveyard)
        .into_iter()
        .filter(|c| game.main_name_of(*c) == "SpecificWanted")
        .collect();
    assert!(matching.is_empty());
    assert!(game.choose_up_to_n_to_hand(PlayerId::P1, &matching, 1).is_ok());
}

/// QA #94: "On my turn, I played an ability that states 'It becomes
/// night until the end of the opponent's next turn.' If the card that
/// played this ability is put into the graveyard during the opponent's
/// next turn, does it stop being night?" -> "No, it does not stop being
/// night. Even if the card that played this ability leaves the base, it
/// is night until the end of the opponent's next turn."
#[test]
fn qa_094_night_persists_after_its_source_card_leaves_the_base() {
    let mut game = GameState::test_scenario();
    let source = game.deploy(PlayerId::P1, "EFILLER-NIGHT-SOURCE", Zone::Base).unwrap();
    game.set_night_until(engine::state::DurationSpec::EndOfOpponentsNextTurn, source);
    assert!(game.night_active());

    game.move_to_graveyard(source, Zone::Base); // the card that caused night leaves
    assert!(
        game.night_active(),
        "night set 'until end of the opponent's next turn' must persist even after its source card leaves the base"
    );
}

/// QA #95: "I played a card that states 'Choose up to 1 (specific) card
/// from your hand and deploy it.' What happens if there is no such
/// specific card in my hand?" -> "Choose 0 cards and resolve the rest of
/// the effect." Same rule as #59/#60/#87, sourced from the hand.
#[test]
fn qa_095_choose_up_to_1_specific_from_hand_with_no_match_chooses_zero() {
    let mut game = GameState::test_scenario();
    let non_matching = game.place(PlayerId::P1, "EFILLER-NONMATCH", Zone::Hand);
    assert_ne!(game.main_name_of(non_matching), "SpecificWanted");

    assert!(
        game.choose_up_to_n_to_deploy(PlayerId::P1, &[], 1).is_ok(),
        "with zero matching cards in hand, choosing 0 and resolving the rest of the effect must be legal"
    );
}
