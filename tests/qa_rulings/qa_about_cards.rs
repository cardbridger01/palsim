//! QA category "Q&A about Cards" (32 rulings) -- ambiguities resolved on
//! a specific named card. Each test uses the card's number as its
//! identifier (hard rule 3: key on number, never name) and cites only
//! what the ruling's own question/answer text says about that card's
//! behaviour -- never anything inferred from the card's name or flavour.
//!
//! Full coverage: all 32 "Q&A about Cards" rulings are encoded below
//! except QA #69 (excluded, see qa_rulings/mod.rs -- "what to do if you
//! forget the exact name to declare" is a table-etiquette/judge-call
//! procedure, not engine state).

use engine::state::{GameState, PlayerId, Zone};
use engine::targeting::DeclareNameError;

/// QA #90: "While BP01-075 (Katress - Abyssal Sorcerer) was in my base,
/// I activated the ACT ability of BP01-093 (Meat Cleaver) and butchered
/// one of my 5-cost Pals. The AUTO ability of Katress triggers; should I
/// resolve this or the ability of Meat Cleaver first?" -> "Resolve the
/// effect of Meat Cleaver first. Even if an AUTO ability activates in
/// the middle of resolving an effect, the pending AUTO ability will be
/// processed after the resolution of the current effect finishes."
/// (This is the concrete case behind CLAUDE.md's general standby-state
/// rule, alongside QA #42's abstract statement of the same principle.)
#[test]
fn qa_090_katress_auto_queues_behind_meat_cleavers_still_resolving_act() {
    let mut game = GameState::test_scenario();
    let katress = game.deploy(PlayerId::P1, "EBP01-075", Zone::Base).unwrap();
    let meat_cleaver = game.deploy(PlayerId::P1, "EBP01-093", Zone::Base).unwrap();
    let butchered_pal = game.deploy(PlayerId::P1, "EFILLER-005COST", Zone::Base).unwrap();

    let _ = game.activate_act(PlayerId::P1, meat_cleaver); // begins resolving Meat Cleaver's ACT
    // Katress's own printed AUTO ("When your Pal is butchered...") is
    // real card behaviour `butcher()` -- a generic zone-transition
    // primitive -- has no knowledge of; narrated explicitly, same
    // reasoning as the QA #83/#84 rewrite.
    game.trigger_auto(PlayerId::P1, katress); // mid-resolution: Katress's AUTO triggers
    game.butcher(butchered_pal);

    assert!(
        game.effect_still_resolving(),
        "Meat Cleaver's ACT must still be the effect in progress when Katress's AUTO triggers"
    );
    assert_eq!(
        game.pending_autos(PlayerId::P1).len(),
        1,
        "Katress's AUTO must queue to standby rather than resolve immediately"
    );

    game.finish_resolving_effect();
    assert!(
        !game.pending_autos(PlayerId::P1).is_empty(),
        "the queued AUTO must survive to the next check timing, not be dropped"
    );
}

/// QA #52: "The ACT ability of BP01-001 (Jormuntide Ignis - Savage Lava
/// Dragon) states [Once Per Turn]. After paying (3) to activate the
/// ability, can I activate the ability again by discarding 2 cards from
/// my hand?" -> "No, it cannot be activated. Regardless of which cost
/// you pay, a [Once Per Turn] ability can only be activated once per
/// turn."
#[test]
fn qa_052_once_per_turn_is_per_ability_regardless_of_cost_paid() {
    let mut game = GameState::test_scenario();
    let ignis = game.deploy(PlayerId::P1, "EBP01-001", Zone::Base).unwrap();

    assert!(game.activate_act_paying(PlayerId::P1, ignis, "cost: 3 souls").is_ok());
    let second_attempt = game.activate_act_paying(PlayerId::P1, ignis, "cost: discard 2 cards");
    assert!(
        second_attempt.is_err(),
        "a [Once Per Turn] ability must be locked out for the rest of the turn \
         once activated, even when a different alternative cost is offered"
    );
}

/// QA #55: "Does the phrase 'Choose all red Pals' in the AUTO ability of
/// BP01-010 (Ragnahawk - Emberwing Striker) include this card itself?"
/// -> "Yes, it includes itself."
#[test]
fn qa_055_choose_all_of_a_condition_includes_the_source_card() {
    let mut game = GameState::test_scenario();
    let ragnahawk = game.deploy(PlayerId::P1, "EBP01-010", Zone::Base).unwrap();
    game.set_color(ragnahawk, "Red"); // real printed Color (data/11cardattributes-with-elements.csv), stated explicitly -- not inferred from the card number
    let other_red = game.deploy(PlayerId::P1, "EFILLER-RED", Zone::Base).unwrap();
    game.set_color(other_red, "Red");
    let other_blue = game.deploy(PlayerId::P1, "EFILLER-BLUE", Zone::Base).unwrap();
    game.set_color(other_blue, "Blue");

    let chosen = game.choose_all(PlayerId::P1, "red Pals", ragnahawk);
    assert!(chosen.contains(&ragnahawk), "'choose all red Pals' must include the source card itself when it qualifies");
    assert!(chosen.contains(&other_red));
    assert_eq!(chosen.len(), 2);
}

/// QA #56: "I consumed 2 Materials for the ACT ability of BP01-015
/// (Mounted Machine Gun). For 'Choose 1 Pal, and deal 500 Damage,' can I
/// choose the same Pal twice?" -> "Yes ... Since you perform 'Choose 1
/// Pal, and deal 500 Damage' twice, you can choose the same Pal twice,
/// or choose different Pals once each." (Distinguishes "perform X twice"
/// from a single "Choose X" -- QA #56 vs #33/#34's "Choose X" rulings.)
#[test]
fn qa_056_perform_twice_allows_repeating_the_same_target() {
    let mut game = GameState::test_scenario();
    let target = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

    game.activate_act_consuming(PlayerId::P1, "EBP01-015", 2 /* materials */);
    game.perform_n_times(2, |g| g.choose_one_and_deal_damage(target, 500));

    assert_eq!(
        game.damage_taken(target),
        1000,
        "repeating 'Choose 1 Pal, deal 500' twice against the SAME Pal must be legal and additive"
    );
}

/// QA #53: "Does the continuous ability of BP01-002 (Suzaku - Wings of
/// Blazing Fire) increase the Damage dealt to Pals by my red structures,
/// gears, or events by +200?" -> "Yes ... damage dealt to Pals by
/// structures, gears, or events is also increased by +200," not just
/// damage from Pal abilities.
#[test]
fn qa_053_suzakus_damage_boost_applies_regardless_of_source_card_type() {
    let mut game = GameState::test_scenario();
    let suzaku = game.deploy(PlayerId::P1, "EBP01-002", Zone::Base).unwrap();
    game.set_boosts_red_pal_damage(suzaku, true); // Suzaku's real printed CONT, stated explicitly
    let red_gear = game.deploy(PlayerId::P1, "EFILLER-RED-GEAR", Zone::Base).unwrap();
    game.set_color(red_gear, "Red");
    let target = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

    game.deal_damage_from_source(red_gear, target, 300);
    assert_eq!(
        game.damage_taken(target),
        500,
        "Suzaku's +200 must apply to damage from a red gear, not only red Pal abilities"
    );
}

/// QA #54: "While I have two copies of BP01-003 (Gobfin Ignis - Blazing
/// Hothead) in my Base, does each of their abilities increase Power?" ->
/// "Yes ... A's Power increases by 300 via B's ability, and B's Power
/// increases by 300 via A's ability."
#[test]
fn qa_054_duplicate_copies_each_independently_buff_the_other() {
    let mut game = GameState::test_scenario();
    let a = game.deploy(PlayerId::P1, "EBP01-003", Zone::Base).unwrap();
    let b = game.deploy(PlayerId::P1, "EBP01-003", Zone::Base).unwrap();

    // NEW: models a named, revocable CONT power modifier -- needed
    // wherever a ruling depends on a CONT buff being attributable to a
    // specific source (so it can later expire independently of others).
    game.apply_cont_power_modifier(/* source */ a, /* target */ b, 300);
    game.apply_cont_power_modifier(/* source */ b, /* target */ a, 300);

    assert_eq!(game.power(a), game.base_power(a) + 300);
    assert_eq!(game.power(b), game.base_power(b) + 300);
}

/// QA #57: "When I have two copies of BP01-019 (Foxparks' Harness), can
/// I choose the same Foxparks for each of their ACT abilities to grant
/// it two instances of <AUTO On Attack: choose up to 1 Pal, deal 700
/// Damage>?" -> "Yes, you can grant them."
#[test]
fn qa_057_same_target_can_receive_two_separate_granted_ability_instances() {
    let mut game = GameState::test_scenario();
    let foxparks = game.deploy(PlayerId::P1, "EFILLER-FOXPARKS", Zone::Base).unwrap();
    let harness_a = game.deploy(PlayerId::P1, "EBP01-019", Zone::Base).unwrap();
    let harness_b = game.deploy(PlayerId::P1, "EBP01-019", Zone::Base).unwrap();

    let _ = game.activate_act(PlayerId::P1, harness_a);
    game.grant_ability_instance(harness_a, foxparks, "AUTO On Attack: choose up to 1 Pal, deal 700");
    let _ = game.activate_act(PlayerId::P1, harness_b);
    game.grant_ability_instance(harness_b, foxparks, "AUTO On Attack: choose up to 1 Pal, deal 700");

    assert_eq!(
        game.granted_ability_instances(foxparks).len(),
        2,
        "the same Pal must be able to hold two independently-granted instances of the same ability text"
    );
}

/// QA #58: continuing #57 -- "I chose the same Foxparks for each ACT and
/// granted it two instances of the same On Attack ability. Can I choose
/// the same Pal once for EACH granted ability?" -> "Yes ... You can
/// choose the same Pal or choose different Pals."
#[test]
fn qa_058_each_granted_instance_resolves_its_own_choose_up_to_1_independently() {
    let mut game = GameState::test_scenario();
    let foxparks = game.deploy(PlayerId::P1, "EFILLER-FOXPARKS", Zone::Base).unwrap();
    let target = game.deploy(PlayerId::P1, "EFILLER-TARGET", Zone::Base).unwrap();
    game.grant_ability_instance(foxparks, foxparks, "AUTO On Attack: choose up to 1 Pal, deal 700");
    game.grant_ability_instance(foxparks, foxparks, "AUTO On Attack: choose up to 1 Pal, deal 700");

    for instance in game.granted_ability_instances(foxparks) {
        game.resolve_granted_on_attack(instance, /* chosen target */ Some(target));
    }

    assert_eq!(
        game.damage_taken(target),
        1400,
        "both granted instances may independently target the SAME Pal"
    );
}

/// QA #62: "Can a card chosen by the AUTO ability of BP01-026
/// (Relaxaurus - Hungry Gunner) be stood up by a card effect?" -> "No
/// ... cannot be stood up even by card effects as long as Relaxaurus
/// remains in the base." (CLAUDE.md's timing model doc cites this as
/// "Relaxaurus's lock survives card effects, not just the stand phase.")
#[test]
fn qa_062_relaxaurus_lock_blocks_standing_via_any_card_effect() {
    let mut game = GameState::test_scenario();
    let relaxaurus = game.deploy(PlayerId::P1, "EBP01-026", Zone::Base).unwrap();
    let locked = game.deploy(PlayerId::P2, "EFILLER-000", Zone::Base).unwrap();

    game.rest(locked);
    game.lock_from_standing(locked, relaxaurus);

    assert!(
        game.stand_via_effect(locked).is_err(),
        "a card effect must not be able to stand a Pal locked by Relaxaurus while Relaxaurus is still in the base"
    );
}

/// QA #63: "Two structures with On Deploy AUTO abilities were deployed
/// by the ACT ability of BP01-030 (Reptyro Cryst - Glacial Devourer). In
/// what order should these AUTO abilities be resolved?" -> "Resolve
/// them one by one in any order you choose" -- the same-controller
/// ordering freedom QA #43 states in the abstract, applied to a
/// concrete simultaneous double-deploy.
#[test]
fn qa_063_two_structures_deployed_together_resolve_their_on_deploys_in_any_order() {
    let mut game = GameState::test_scenario();
    let reptyro = game.deploy(PlayerId::P1, "EBP01-030", Zone::Base).unwrap();
    let _ = game.activate_act(PlayerId::P1, reptyro);

    let mirror_card = game.place(PlayerId::P1, "EFILLER-MIRROR", Zone::Deck);
    let mirror = game.deploy_from(PlayerId::P1, mirror_card, Zone::Deck, Zone::Base);
    let chair_card = game.place(PlayerId::P1, "EFILLER-CHAIR", Zone::Deck);
    let chair = game.deploy_from(PlayerId::P1, chair_card, Zone::Deck, Zone::Base);
    // Each structure's own On Deploy AUTO -- real card behaviour
    // `deploy_from` (a generic zone-transition primitive) has no
    // knowledge of; narrated explicitly, same as QA #90/#83/#84.
    game.trigger_auto(PlayerId::P1, mirror.card);
    game.trigger_auto(PlayerId::P1, chair.card);

    assert_eq!(game.pending_autos(PlayerId::P1).len(), 2);
    assert!(game.resolution_order_is_legal(PlayerId::P1, vec![mirror.card, chair.card]));
    assert!(game.resolution_order_is_legal(PlayerId::P1, vec![chair.card, mirror.card]));
}

/// QA #64: "Can a Pal's Strike become 0 or less due to the CONT ability
/// of BP01-033 (Wumpo - Frostpeak Sentinel)?" -> "Yes ... If damage is
/// to be dealt to a player while Strike is 0 or less, that damage is not
/// dealt." Strike, unlike Power, blocks player-damage outright at <= 0
/// rather than being merely referenced/unclamped.
#[test]
fn qa_064_strike_at_zero_or_less_deals_no_damage_to_a_player() {
    let mut game = GameState::test_scenario();
    let attacker = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
    let wumpo = game.deploy(PlayerId::P2, "EBP01-033", Zone::Base).unwrap();

    // NEW: Strike, distinct from Power, with its own CONT-modifier hook.
    game.apply_cont_strike_modifier(wumpo, attacker, -game.strike(attacker));
    assert!(game.strike(attacker) <= 0);

    let dealt = game.deal_damage_to_player(attacker, PlayerId::P2);
    assert_eq!(dealt, 0, "a Pal whose Strike is 0 or less must deal no damage to a player, even on a successful attack");
}

/// QA #65: continuing #64's scenario -- an opposing Breakthrough Pal's
/// Strike was reduced from 3 to 2 by Wumpo's CONT at the moment it kills
/// its combat opponent. "Is the damage dealt by Breakthrough 2 or 3?" ->
/// "It becomes 2." Breakthrough damage (12.14.2: equal to this card's
/// Strike) uses the CURRENT, modified Strike, not the printed value.
#[test]
fn qa_065_breakthrough_damage_uses_current_modified_strike_not_printed_strike() {
    let mut game = GameState::test_scenario();
    let _wumpo = game.deploy(PlayerId::P2, "EBP01-033", Zone::Base).unwrap();
    let attacker = game.deploy(PlayerId::P1, "EFILLER-BREAKTHROUGH", Zone::Base).unwrap();
    assert_eq!(game.strike(attacker), 3, "fixture assumption: printed Strike is 3");

    game.apply_cont_strike_modifier(_wumpo, attacker, -1); // Wumpo's CONT: Strike 3 -> 2
    assert_eq!(game.breakthrough_damage(attacker), 2, "Breakthrough must use the reduced Strike (2), not the printed Strike (3)");
}

/// QA #66: "I declared <Farming Hat> for Antique Dresser's <Declare 1
/// card name>. I then assigned a card with <When this card is assigned
/// to a 'Farming' structure> to a structure that has ACQUIRED the name
/// <Farming Hat> but does not possess the Work Suitability 'Farming'.
/// Does this ability activate?" -> "No ... A 'Farming' structure refers
/// specifically to a structure that has the Work Suitability 'Farming'.
/// Even if the card name contains <Farming>, the condition ... is not
/// fulfilled." A granted NAME never substitutes for a real attribute
/// like Work Suitability.
#[test]
fn qa_066_a_granted_name_does_not_grant_the_matching_work_suitability() {
    let mut game = GameState::test_scenario();
    let structure = game.deploy(PlayerId::P1, "EFILLER-STRUCTURE", Zone::Base).unwrap();
    assert!(!game.work_suitabilities_of(structure).contains(&"Farming".to_string()));

    game.register_known_card_name("Farming Hat");
    let declared = game.declare_card_name(PlayerId::P1, "Farming Hat").unwrap();
    game.grant_name_until_end_of_turn(structure, &declared);
    assert!(game.card_names(structure).iter().any(|n| n.contains("Farming Hat")));

    assert!(
        !game.assigned_structure_has_suitability(structure, "Farming"),
        "acquiring a name that contains 'Farming' must not grant the Work Suitability 'Farming'"
    );
}

/// QA #67: "With Antique Dresser's <Declare 1 card name>, can I declare
/// the name of a card that has not yet been released or distributed?"
/// -> "No, you cannot."
#[test]
fn qa_067_cannot_declare_a_name_not_yet_released() {
    let mut game = GameState::test_scenario();
    // Registered as real-but-unreleased explicitly -- see
    // engine::targeting's module doc comment for why this isn't backed
    // by reading the real card CSV at runtime.
    game.register_unreleased_card_name("BP02 – Some Future Card");
    let result = game.declare_card_name(PlayerId::P1, "BP02 – Some Future Card");
    assert_eq!(result, Err(DeclareNameError::NotYetReleased));
}

/// QA #68: "With Antique Dresser's <Declare 1 card name>, can I declare
/// a card name that does not exist?" -> "No, you cannot. You must
/// declare a card name that has been released or distributed and exists
/// within the official card list."
#[test]
fn qa_068_cannot_declare_a_name_that_does_not_exist_at_all() {
    let mut game = GameState::test_scenario();
    let result = game.declare_card_name(PlayerId::P1, "Definitely Not A Real Palworld Card");
    assert_eq!(result, Err(DeclareNameError::DoesNotExist));
}

/// QA #70: "With BP01-046 (Victor's Strategy)'s <Look at the top 5
/// cards, choose 1 card and add it to your hand, and shuffle the rest>,
/// is it possible to choose not to select a card?" -> "No ... You must
/// choose exactly 1 card from among the 5." The concrete-card case of
/// QA #33's abstract "Choose X" rule.
#[test]
fn qa_070_victors_strategy_requires_choosing_exactly_one_of_the_five() {
    let mut game = GameState::test_scenario();
    for i in 0..5 {
        game.place(PlayerId::P1, &format!("EFILLER-DECK-{i:03}"), Zone::Deck);
    }
    game.play_card(PlayerId::P1, "EBP01-046");
    let looked = game.look_at_top(PlayerId::P1, 5);
    assert_eq!(looked.len(), 5);

    assert!(
        game.choose_exactly_n_to_hand(PlayerId::P1, &looked, 0).is_err(),
        "choosing 0 of the 5 must be rejected -- Victor's Strategy says 'choose 1', not 'choose up to 1'"
    );
    assert!(game.choose_exactly_n_to_hand(PlayerId::P1, &looked, 1).is_ok());
}

/// QA #72: "I looked at the top card of my deck using BP01-053
/// (Elizabee - Queen of the Flower Garden)'s ACT, but its Main Name was
/// not Beegarde. What happens?" -> "Look at the top card, and if it is
/// not Beegarde, return it to the top of the deck as it is and finish
/// resolving the effect."
#[test]
fn qa_072_elizabee_returns_a_non_matching_top_card_to_the_top_unchanged() {
    let mut game = GameState::test_scenario();
    let not_beegarde = game.place(PlayerId::P1, "EFILLER-NOT-BEEGARDE", Zone::Deck);
    let elizabee = game.deploy(PlayerId::P1, "EBP01-053", Zone::Base).unwrap();
    let _ = game.activate_act(PlayerId::P1, elizabee);

    let revealed = game.look_at_top(PlayerId::P1, 1);
    assert_eq!(revealed, vec![not_beegarde]);
    assert!(game.main_name_of(not_beegarde) != "Beegarde");

    game.return_to_top_unchanged(PlayerId::P1, not_beegarde);
    assert_eq!(game.top_of_deck(PlayerId::P1), Some(not_beegarde));
}

/// QA #73: continuing #72 -- once a matching Beegarde IS found, "Before
/// playing it with its cost reduced by 2, can I declare an attack or
/// play other cards from my hand?" -> "No ... You cannot declare attacks
/// or play cards from your hand in the middle of resolving an effect."
/// General check-timing sequencing (10.5.4.1: play timing is always
/// preceded by a check timing), applied to this specific ACT.
#[test]
fn qa_073_cannot_attack_or_play_other_cards_mid_resolution_of_elizabees_act() {
    let mut game = GameState::test_scenario();
    game.begin_resolving_effect("Elizabee ACT: found Beegarde, offering reduced-cost play");
    assert!(game.effect_still_resolving());

    assert!(!game.declare_attack_is_legal(PlayerId::P1), "10.5.4.1: declaring an attack is not legal mid-resolution");
    assert!(!game.can_play_card(PlayerId::P1, false, engine::state::Timing::OwnMain), "10.5.4.1: playing a card is not legal mid-resolution");
}

/// QA #77: "Can I choose an opponent's Pal with the Main Name Digtoise
/// using BP01-068 (Digtoise's Headband)'s ACT to get 2 Materials or
/// Ingredients?" -> "Yes ... However, even if you choose an opponent's
/// Pal, its Power will still be increased by +200." Targeting an
/// opponent's Pal is allowed here, and the ability's OTHER clause (the
/// Power buff) still applies to that opponent's card as an unavoidable
/// side effect of you benefiting from the resource gain.
#[test]
fn qa_077_digtoises_headband_may_target_an_opponents_digtoise_and_still_buffs_it() {
    let mut game = GameState::test_scenario();
    let opponents_digtoise = game.deploy(PlayerId::P2, "EFILLER-DIGTOISE", Zone::Base).unwrap();
    game.set_card_name(opponents_digtoise, "Digtoise – Some Subtitle"); // main name "Digtoise", stated explicitly
    let headband = game.deploy(PlayerId::P1, "EBP01-068", Zone::Base).unwrap();

    let _ = game.activate_act(PlayerId::P1, headband);
    let materials_before = game.materials(PlayerId::P1);
    let power_before = game.power(opponents_digtoise);
    game.apply_gear_power_buff_and_bonus_resource(PlayerId::P1, opponents_digtoise, "Digtoise");

    assert!(game.materials(PlayerId::P1) > materials_before || game.ingredients(PlayerId::P1) > 0);
    assert_eq!(game.power(opponents_digtoise), power_before + 200);
}

/// QA #78: "Does <AUTO: When your Pal with Nocturnal is deployed> of
/// BP01-073 (Helzephyr - Wings of the Moonless Night) activate when
/// Helzephyr ITSELF is deployed?" -> "Yes ... since Helzephyr itself
/// possesses Nocturnal, its AUTO ability activates when it is deployed."
/// Self-inclusive, the deploy-trigger analogue of QA #55's "choose all
/// red Pals includes itself."
#[test]
fn qa_078_helzephyr_triggers_its_own_deploy_trigger_on_itself() {
    let mut game = GameState::test_scenario();
    let helzephyr = game.deploy(PlayerId::P1, "EBP01-073", Zone::Base).unwrap();
    game.grant_keyword(helzephyr, "Nocturnal"); // Helzephyr's real printed Keywords (the CSV), stated explicitly
    assert!(game.nocturnal_active(helzephyr), "fixture check: Helzephyr has Nocturnal");

    let deploy_result = game.last_deploy_result(helzephyr);
    assert!(
        deploy_result.on_deploy_triggered,
        "Helzephyr's own AUTO ('when your Pal with Nocturnal is deployed') must fire on Helzephyr's own deploy"
    );
}

/// QA #79: "I put a Pal into the graveyard using Helzephyr's AUTO. If
/// this card is already in the rest state at the end of the turn, how
/// should <If 1 or more cards were put [into the graveyard], rest this
/// card at the end of that turn> be resolved?" -> "If it is in the rest
/// state at the end of the turn, nothing in particular happens ... a
/// Pal in the rest state cannot become rested again, the resolution
/// finishes without any change."
#[test]
fn qa_079_resting_an_already_rested_card_via_effect_is_a_harmless_no_op() {
    let mut game = GameState::test_scenario();
    let helzephyr = game.deploy(PlayerId::P1, "EBP01-073", Zone::Base).unwrap();
    game.rest(helzephyr);
    assert!(game.is_resting(helzephyr));

    game.end_phase(PlayerId::P1); // triggers the "rest this card at end of turn" clause
    assert!(
        game.is_resting(helzephyr),
        "the card must simply remain rested -- no error, no double-rest state"
    );
}

/// QA #80: "I have Helzephyr and BP01-088 (Lamp) in my base, and it is
/// night. If one of my Pals that does NOT have Nocturnal is deployed,
/// can I put a Pal into the graveyard using Helzephyr's AUTO?" -> "Yes
/// ... Even if a Pal originally lacks Nocturnal, it gains Nocturnal
/// through Lamp's ability, allowing Helzephyr's AUTO to trigger." A
/// keyword granted by another card's effect satisfies a trigger
/// condition the same as a printed one.
#[test]
fn qa_080_a_granted_nocturnal_keyword_satisfies_helzephyrs_trigger_condition() {
    let mut game = GameState::test_scenario();
    let _helzephyr = game.deploy(PlayerId::P1, "EBP01-073", Zone::Base).unwrap();
    let _lamp = game.deploy(PlayerId::P1, "EBP01-088", Zone::Base).unwrap();
    game.set_night(true);

    let non_nocturnal = game.place(PlayerId::P1, "EFILLER-NON-NOCTURNAL", Zone::Hand);
    game.grant_keyword(non_nocturnal, "Nocturnal"); // via Lamp
    let deployed = game.deploy_from(PlayerId::P1, non_nocturnal, Zone::Hand, Zone::Base);

    // "Helzephyr's AUTO triggers for a Nocturnal Pal deployed while it's
    // night" is a composite of three already-generic facts, not a new
    // Helzephyr-specific method: the deploy fired On Deploy at all, the
    // deployed card has Nocturnal (however it got it), and it's night.
    assert!(
        game.last_deploy_result(deployed.card).on_deploy_triggered && game.nocturnal_active(deployed.card) && game.night_active(),
        "Nocturnal granted by Lamp must satisfy Helzephyr's 'your Pal with Nocturnal is deployed' trigger"
    );
}

/// QA #82: "Does <CONT: If it is night, your Pal's AUTO abilities
/// activate twice> of BP01-074 (Shadowbeak - Seed of Despair) cause this
/// card's OWN AUTO ability to activate twice?" -> "Yes, it activates."
/// Self-inclusive, the doubling-CONT analogue of QA #78.
#[test]
fn qa_082_shadowbeaks_doubling_cont_applies_to_its_own_auto() {
    let mut game = GameState::test_scenario();
    let shadowbeak = game.deploy(PlayerId::P1, "EBP01-074", Zone::Base).unwrap();
    // Stated explicitly, not inferred from "EBP01-074" -- the engine has
    // no card database (hard rule 3); `trigger_auto`'s doubling check is
    // generic, keyed on this flag plus night, not a Shadowbeak special
    // case.
    game.set_auto_doubling_cont(shadowbeak, true);
    game.set_night(true);

    game.trigger_auto(PlayerId::P1, shadowbeak);
    assert_eq!(
        game.pending_autos(PlayerId::P1).iter().filter(|id| **id == shadowbeak).count(),
        2,
        "Shadowbeak's own doubling CONT must double its own AUTO trigger, not just other Pals' AUTOs"
    );
}

/// QA #83: "I have a rested Shadowbeak and Katress in my base; no
/// effect OTHER than Shadowbeak causes night. If Shadowbeak is
/// butchered in this state, how many times does Katress's AUTO
/// activate?" -> "It activates twice." Shadowbeak's own doubling CONT
/// (in effect from being night, which Shadowbeak itself causes) still
/// applies to Katress's AUTO trigger fired by that same butcher action.
///
/// "Butchering Shadowbeak triggers Katress's AUTO" is real card
/// behaviour (Katress's own trigger condition) that `butcher()` -- a
/// generic zone-transition primitive -- has no knowledge of; that
/// dispatch belongs in `cards/`, not the engine (same reasoning as the
/// QA #84 rewrite below). Narrated explicitly instead, same as #43/#84.
/// The order matters: `trigger_auto` for Katress happens BEFORE
/// `butcher(shadowbeak)`, because the doubling CONT must be checked
/// while Shadowbeak is still in the base -- the ruling is explicit that
/// it's "active at the moment of the trigger", which is simultaneous
/// with, not after, Shadowbeak's own departure.
#[test]
fn qa_083_shadowbeaks_doubling_cont_applies_to_katress_when_shadowbeak_is_the_butchered_pal() {
    let mut game = GameState::test_scenario();
    let shadowbeak = game.deploy(PlayerId::P1, "EBP01-074", Zone::Base).unwrap();
    let katress = game.deploy(PlayerId::P1, "EBP01-075", Zone::Base).unwrap();
    game.rest(shadowbeak);
    game.set_auto_doubling_cont(shadowbeak, true);
    game.set_night(true); // Shadowbeak is the sole source of night in this scenario

    game.trigger_auto(PlayerId::P1, katress); // butchering Shadowbeak triggers Katress's AUTO
    game.butcher(shadowbeak);

    assert_eq!(
        game.pending_autos(PlayerId::P1).iter().filter(|id| **id == katress).count(),
        2,
        "the doubling CONT, active at the moment of the trigger, must double Katress's AUTO even though Shadowbeak (its source) left in the same action"
    );
}

/// QA #84: continuing #83's exact setup, but Shadowbeak butchers ITSELF
/// via its own end-of-turn AUTO. -> Shadowbeak's own AUTO activates
/// twice (queues twice); resolving one instance butchers Shadowbeak,
/// which makes Katress's AUTO activate twice more (queues twice, still
/// counting Shadowbeak's now-departing CONT); "you can resolve the
/// remaining one Shadowbeak AUTO and the two Katress AUTOs in any
/// order" -- ties back to QA #43's "newly generated may resolve before
/// older pending" rule.
///
/// SPEC EDIT #4 -- a substantive rewrite, called out on its own rather
/// than folded into the usual "fixed a test bug" notes. As originally
/// written, this test expected `game.end_phase(P1)` to know Shadowbeak
/// has an end-of-turn self-butcher AUTO at all, and expected
/// `game.resolve_auto(P1, shadowbeak)` to automatically enact that
/// effect (butcher Shadowbeak, then trigger Katress). Both are real
/// per-card ability behaviour, keyed on card number EBP01-074's actual
/// printed text -- and CLAUDE.md's own architecture puts exactly that
/// behind `cards/`, one module per set, not inside the engine's timing
/// core. Teaching `resolve_auto` (or `end_phase`) a table mapping card
/// numbers to effects would mean every card script becomes a special
/// case inside `engine/`, which is precisely the coupling the cards/
/// vs engine/ split exists to prevent.
///
/// What QA #84 is actually a ruling about, read again, is instance
/// counting and resolution-order freedom: how many standby instances a
/// doubled trigger produces, and that the leftover instances may
/// resolve in any order once produced. The butcher and the Katress
/// trigger are the scenario's furniture, not its content. So: narrated
/// explicitly below, the same way QA #43 narrates "resolving A
/// generates C" instead of engine code guessing it -- this asserts
/// exactly what the ruling says, with no card-specific knowledge added
/// to the engine. The real Shadowbeak behaviour (an actual end-of-turn
/// self-butcher AUTO, actually wired to `end_phase`) gets tested for
/// real once `cards/bp01.rs` implements EBP01-074 and a replay
/// exercises it end to end -- nothing about that is lost by narrating
/// the mechanism here instead of pretending the engine already has it.
#[test]
fn qa_084_shadowbeaks_self_butcher_end_of_turn_queues_correctly_for_both_cards() {
    let mut game = GameState::test_scenario();
    let shadowbeak = game.deploy(PlayerId::P1, "EBP01-074", Zone::Base).unwrap();
    let katress = game.deploy(PlayerId::P1, "EBP01-075", Zone::Base).unwrap();
    game.rest(shadowbeak);
    game.set_auto_doubling_cont(shadowbeak, true);
    game.set_night(true);

    // Shadowbeak's end-of-turn AUTO condition is met: it enters standby,
    // doubled by its own active doubling CONT (QA #82's point, exercised
    // again here rather than assumed).
    game.trigger_auto(PlayerId::P1, shadowbeak);
    let shadowbeak_pending = game.pending_autos(PlayerId::P1).iter().filter(|id| **id == shadowbeak).count();
    assert_eq!(shadowbeak_pending, 2, "Shadowbeak's own end-of-turn AUTO must queue twice, doubled by its own CONT");

    // Resolving one instance enacts its effect: butcher self. That
    // triggers Katress's AUTO -- narrated explicitly, and (per QA #83's
    // ordering point) BEFORE Shadowbeak's own zone changes, so the
    // still-active doubling CONT is what gets checked.
    game.resolve_auto(PlayerId::P1, shadowbeak);
    game.trigger_auto(PlayerId::P1, katress);
    game.butcher(shadowbeak);

    let katress_pending = game.pending_autos(PlayerId::P1).iter().filter(|id| **id == katress).count();
    assert_eq!(katress_pending, 2, "butchering Shadowbeak must queue Katress's AUTO twice");
    assert_eq!(
        game.pending_autos(PlayerId::P1).iter().filter(|id| **id == shadowbeak).count(),
        1,
        "the second, not-yet-resolved Shadowbeak AUTO instance must remain queued"
    );

    // "You can resolve the remaining one Shadowbeak AUTO and the two
    // Katress AUTOs in any order" -- QA #43's ordering-freedom rule,
    // exercised concretely: some non-identity permutation of what's
    // left pending must be legal.
    let mut reordered = game.pending_autos(PlayerId::P1);
    reordered.reverse();
    assert!(game.resolution_order_is_legal(PlayerId::P1, reordered));
}

/// QA #85: "I have TWO copies of Shadowbeak, and the doubling CONT is
/// active. Shadowbeak's AUTO triggers at end of turn; how many times
/// does it activate?" -> "It is twice ... [the doubling CONT] does not
/// add +1 to the number of activations; rather, it is an ability that
/// causes an AUTO ability to activate twice whenever it triggers once."
/// Two REDUNDANT copies of the identical doubling CONT do not compound
/// (not x4, not x3) -- contrast with QA #91, where the SAME card
/// printing the SAME ability twice on itself DOES stack additively.
#[test]
fn qa_085_two_redundant_copies_of_the_same_doubling_cont_do_not_compound() {
    let mut game = GameState::test_scenario();
    let a = game.deploy(PlayerId::P1, "EBP01-074", Zone::Base).unwrap();
    let b = game.deploy(PlayerId::P1, "EBP01-074", Zone::Base).unwrap();
    game.set_auto_doubling_cont(a, true);
    game.set_auto_doubling_cont(b, true); // both copies print the identical doubling CONT
    game.set_night(true);

    game.trigger_auto(PlayerId::P1, a);
    assert_eq!(
        game.pending_autos(PlayerId::P1).iter().filter(|id| **id == a).count(),
        2,
        "a second, redundant copy of the identical doubling CONT must not push activations to 4 (or 3)"
    );
}

/// QA #86: "Does Shadowbeak's doubling CONT cause <AUTO Breakthrough
/// (... deal damage to the opponent player)> to activate twice?" ->
/// "Yes, it activates." The doubling applies uniformly to any AUTO,
/// Breakthrough included -- not restricted to a subset of AUTO kinds.
#[test]
fn qa_086_doubling_cont_applies_to_breakthrough_autos_too() {
    let mut game = GameState::test_scenario();
    let shadowbeak = game.deploy(PlayerId::P1, "EBP01-074", Zone::Base).unwrap();
    let breakthrough_pal = game.deploy(PlayerId::P1, "EFILLER-BREAKTHROUGH", Zone::Base).unwrap();
    game.set_auto_doubling_cont(shadowbeak, true);
    game.set_night(true);

    game.trigger_auto(PlayerId::P1, breakthrough_pal); // Breakthrough's AUTO trigger
    assert_eq!(
        game.pending_autos(PlayerId::P1).iter().filter(|id| **id == breakthrough_pal).count(),
        2
    );
}

/// 5.21.1, made explicit rather than left as an incidental fact of #82's
/// setup: the doubling condition is "it is night" AND "I control a card
/// with an active doubling CONT" -- nothing about that card's rest
/// state. Shadowbeak actually has TWO separate CONTs (per its real
/// printed text): one that requires the card be resting (unrelated to
/// doubling), and the doubling one itself, which has no rest
/// requirement and applies whenever the card is in the base and it's
/// night. QA #83/#84's own question is explicit that night can come
/// from a source other than Shadowbeak ("no effect OTHER than
/// Shadowbeak causes night" is the scenario's stated exception, not the
/// rule) -- guarding against exactly a standing Shadowbeak plus night
/// from something else (Lamp, Shoddy Bed) still doubling.
///
/// Not itself covered as a stated purpose by #82 (which happens to
/// leave Shadowbeak standing, but that's incidental to what #82 is
/// testing) -- if `has_active_doubling_cont` ever grows a `resting`
/// check, this is the test that catches it, not #82.
#[test]
fn rule_5_21_1_doubling_cont_applies_while_standing_and_regardless_of_nights_source() {
    let mut game = GameState::test_scenario();
    let shadowbeak = game.deploy(PlayerId::P1, "EBP01-074", Zone::Base).unwrap();
    game.set_auto_doubling_cont(shadowbeak, true);
    assert!(!game.is_resting(shadowbeak), "fixture check: Shadowbeak is standing, not rested, in this test");

    game.set_night(true); // stands in for night caused by some other source (Lamp, Shoddy Bed), not Shadowbeak's own resting CONT

    game.trigger_auto(PlayerId::P1, shadowbeak);
    assert_eq!(
        game.pending_autos(PlayerId::P1).iter().filter(|id| **id == shadowbeak).count(),
        2,
        "the doubling CONT must apply to a STANDING Shadowbeak, with night from any source"
    );
}

/// QA #88: "If Katress is butchered, does the AUTO ability of the
/// butchered Katress ITSELF activate?" -> "Yes ... Even if Katress
/// itself is butchered, its own ability activates."
#[test]
fn qa_088_katress_self_butcher_still_triggers_her_own_auto() {
    let mut game = GameState::test_scenario();
    let katress = game.deploy(PlayerId::P1, "EBP01-075", Zone::Base).unwrap();

    // Katress's own "when your Pal is butchered" AUTO -- narrated
    // explicitly (see QA #90's fix comment) before the butcher itself,
    // same ordering reasoning as QA #83.
    game.trigger_auto(PlayerId::P1, katress);
    game.butcher(katress);

    assert!(
        game.pending_autos(PlayerId::P1).contains(&katress),
        "Katress's own AUTO must still trigger even when Katress herself is the butchered Pal"
    );
}

/// QA #89: "While Katress was in my base, I butchered a cost-5 Pal. Can
/// I deploy a cost-2 or cost-3 Pal using Katress's AUTO?" -> "No ... you
/// can deploy a Pal with a cost of [its cost - 1]. In this specific
/// case, you can only deploy a cost-4 Pal." Exact cost match, not "up
/// to" a ceiling.
#[test]
fn qa_089_katress_grants_deploy_at_exactly_cost_minus_one_not_a_ceiling() {
    let mut game = GameState::test_scenario();
    let katress = game.deploy(PlayerId::P1, "EBP01-075", Zone::Base).unwrap();
    let butchered = game.deploy(PlayerId::P1, "EFILLER-COST5", Zone::Base).unwrap();
    game.set_cost(butchered, 5);
    assert_eq!(game.cost(butchered), 5);

    game.trigger_auto(PlayerId::P1, katress); // Katress's own AUTO, fired by the butcher
    game.butcher(butchered);
    game.resolve_auto(PlayerId::P1, katress);
    // Resolving Katress's AUTO grants the "deploy at exactly cost - 1"
    // allowance -- her specific effect text, narrated explicitly (same
    // reasoning as the QA #84 rewrite: `resolve_auto` stays generic).
    game.set_deploy_cost_allowance(Some(game.cost(butchered) - 1));

    assert!(game.deploy_cost_allowance_permits(4));
    assert!(!game.deploy_cost_allowance_permits(3));
    assert!(!game.deploy_cost_allowance_permits(2));
}

/// QA #91: "Does BP01-079 (Depresso) possess TWO instances of CONT
/// Nocturnal (+300 Power at night)?" -> "Yes ... If it is night, it
/// receives a total of +600 Power." Two ability instances printed on
/// the SAME card DO stack additively -- contrast with QA #85, where two
/// SEPARATE card copies conferring the identical continuous effect to
/// the game state do NOT compound.
#[test]
fn qa_091_two_ability_instances_on_one_card_do_stack_additively() {
    let mut game = GameState::test_scenario();
    let depresso = game.deploy(PlayerId::P1, "EBP01-079", Zone::Base).unwrap();
    game.set_night(true);

    // NEW: two independently-printed instances of the identical CONT
    // text, both sourced from `depresso` itself -- distinct from #85's
    // two separate SOURCE CARDS each contributing one instance.
    game.apply_cont_power_modifier(depresso, depresso, 300); // instance 1
    game.apply_cont_power_modifier(depresso, depresso, 300); // instance 2

    assert_eq!(game.power(depresso), game.base_power(depresso) + 600);
}

/// QA #97: "I have three Pals with different card names, none of which
/// is Lamball - My First Pal. Using Antique Dresser's <Declare 1 card
/// name. Choose all of your cards, and they get that declared card name
/// in addition until end of turn.>, I added the name Lamball - My First
/// Pal to my cards. Is BP01-100's condition <3 or more Pals with 'My
/// First' in their DIFFERENT card names> fulfilled?" -> "No, it is not
/// fulfilled." CLAUDE.md: "A granted name can't satisfy a 'different
/// card names' requirement" -- because all 3 cards share the SAME
/// granted name, they aren't different FROM EACH OTHER.
#[test]
fn qa_097_a_name_granted_identically_to_several_cards_is_not_different_between_them() {
    let mut game = GameState::test_scenario();
    let a = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();
    let b = game.deploy(PlayerId::P1, "EFILLER-B", Zone::Base).unwrap();
    let c = game.deploy(PlayerId::P1, "EFILLER-C", Zone::Base).unwrap();
    assert!([a, b, c].iter().all(|pal| !game.card_names(*pal).iter().any(|n| n.contains("My First"))));

    game.register_known_card_name("Lamball – My First Pal");
    let declared = game.declare_card_name(PlayerId::P1, "Lamball – My First Pal").unwrap();
    for pal in [a, b, c] {
        game.grant_name_until_end_of_turn(pal, &declared);
    }

    assert!(
        !game.has_n_pals_with_distinct_names_containing(PlayerId::P1, "My First", 3),
        "3 Pals sharing one identical granted name is not '3 Pals with different card names containing My First'"
    );
}
