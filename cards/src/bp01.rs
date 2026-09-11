//! BP01 "Genesis" card scripts needed by battle1.

use engine::state::{CardId, GameState, GrantedInstanceId, PlayerId, Zone};

use crate::opponent_of;

/// BP01-098, Elphidran -- Gentle Radiance.
///
/// effect_text: "AUTO OnAttack You may reveal 1 card from your hand.
/// If you revealed a Dragon Pal this way, this card gets Power +500
/// until end of turn."
///
/// QA #46 (lists EBP01-098 directly) confirms "you may" is optional --
/// `revealed` is `None` when the controller declines (battle1's two
/// "Chose not to reveal a Dragon" lines). "Until end of turn" isn't
/// backed by any duration-expiry mechanism in `engine::` yet (nothing
/// has needed one to expire so far), so this is a plain, permanent
/// `increase_power` -- indistinguishable in every way this replay
/// observes, since nothing after the buffed attack itself resolves
/// depends on the buff having expired.
pub fn elphidran_gr_on_attack(game: &mut GameState, attacker: CardId, revealed: Option<CardId>) {
    let Some(card) = revealed else { return };
    if game.elements_of(card).iter().any(|e| e == "Dragon") {
        game.increase_power(attacker, 500);
    }
}

/// BP01-025, Chillet -- Dragon Whisperer.
///
/// effect_text: "AUTO OnDeploy Reveal the top card of your deck, and
/// if it is a ◇8 or less Dragon Pal, you may deploy it. If you did not
/// deploy it, add it to hand."
///
/// Uses `take_top_of_deck` (not `reveal_top`/`look_at_top`): those two
/// are for the "look and leave it in the deck" shape and decrement the
/// deck's zone count immediately, which would double-decrement once
/// this card's own `deploy_from` call moves the revealed card again.
/// `choose_to_deploy` is the controller's choice when eligible ("you
/// may") -- ineligible or declined both fall through to "add it to
/// hand" identically, matching the card's own "if you did not deploy
/// it" wording covering both cases the same way. Returns the revealed
/// card's id so a caller can chain into ITS OWN On Deploy (QA #32) if
/// it was deployed -- the canonical Chillet-reveals-a-Dragon chain
/// CLAUDE.md calls out by name.
pub fn chillet_on_deploy(game: &mut GameState, player: PlayerId, choose_to_deploy: bool) -> Option<CardId> {
    let card = game.take_top_of_deck(player)?;
    let eligible = game.cost(card) <= 8 && game.elements_of(card).iter().any(|e| e == "Dragon");
    if eligible && choose_to_deploy {
        game.deploy_from(player, card, Zone::Deck, Zone::Base);
    } else {
        game.deploy_from(player, card, Zone::Deck, Zone::Hand);
    }
    Some(card)
}

/// BP01-027, Jormuntide -- Surging Sea Serpent.
///
/// effect_text: "AUTO OnDeploy Draw 1 card, choose up to 1 ◇7 or less
/// Pal, and rest it. That card does not stand during your opponent's
/// next stand phase."
///
/// The target must be the OPPONENT's Pal, not the controller's own --
/// textual, not a guess: "does not stand during YOUR OPPONENT'S next
/// stand phase" is only coherent if the target IS the opponent's card,
/// since the stand phase only ever stands the current turn player's
/// own cards (7.2.1) -- a self-owned target would never stand during
/// the opponent's stand phase regardless of this effect, making the
/// clause vacuous under any other reading. `target` is `None` for "no
/// eligible target" (both of battle1's occurrences resolve this way --
/// QA #34's "choose up to X allows 0" principle) or the caller's
/// already-legal choice among the opponent's cost<=7 base Pals.
/// `choose_and_rest_wont_stand_next` (QA #61, which lists EBP01-027
/// directly) already covers "rest + won't-stand-next" as one generic
/// primitive.
pub fn jormuntide_on_deploy(game: &mut GameState, player: PlayerId, target: Option<CardId>) {
    game.draw(player);
    if let Some(pal) = target {
        game.choose_and_rest_wont_stand_next(player, pal);
    }
}

/// BP01-044, Pengullet Rocket Launcher.
///
/// effect_text: "ACT [Rest this card] Choose 1 Pal, and it gets Power
/// +200 until end of turn. If its main name is 《Pengullet》, instead
/// it gets Power +500 and the skill in 〈〉 until end of turn. 〈ACT
/// [Rest this card] Choose all of your opponent's Pals, and they get X
/// Damage. X is equal to this card's Power. Put this card into the
/// graveyard〉."
///
/// "This card" inside the granted 〈〉 skill refers to whichever card
/// HOLDS the ability once granted (the target, not the launcher) --
/// confirmable from the text itself, not just convention: the launcher
/// is a Gear with no Power at all (its CSV row's Power column is
/// blank), so if "this card" meant the launcher, X would be undefined
/// and the granted skill could never resolve. It only parses under the
/// holder reading, which is also why the upgrade requires main name
/// 《Pengullet》 and grants +500 Power first -- the whole point is
/// giving the carrier a big Power number for the sweep to scale off
/// of. Same "granted text re-scopes to the holder" convention
/// `grant_ability_instance`/`resolve_granted_on_attack` (QA #57/#58,
/// Foxparks' Harness) already assumes. "Until end of turn" is a plain,
/// permanent `increase_power`, same reasoning as Elphidran GR's
/// OnAttack above.
pub fn rocket_launcher_act(game: &mut GameState, launcher: CardId, target: CardId) -> Option<GrantedInstanceId> {
    game.rest(launcher);
    if game.main_name_of(target) == "Pengullet" {
        game.increase_power(target, 500);
        Some(game.grant_ability_instance(
            launcher,
            target,
            "ACT [Rest this card] Choose all of your opponent's Pals, and they get X Damage. X is equal to this card's Power. Put this card into the graveyard",
        ))
    } else {
        game.increase_power(target, 200);
        None
    }
}

/// The 〈〉 skill `rocket_launcher_act` grants, once activated by its
/// holder (`holder`). "This card" = `holder` throughout, per
/// `rocket_launcher_act`'s doc comment -- `holder_owner` is needed only
/// to find "your opponent's Pals" (`pals_in_base` is per-player), not
/// because `holder` could belong to someone else.
pub fn rocket_launcher_granted_sweep(game: &mut GameState, holder: CardId, holder_owner: PlayerId) {
    game.rest(holder);
    let damage = game.power(holder).max(0) as u32;
    for pal in game.pals_in_base(opponent_of(holder_owner)) {
        game.apply_damage(pal, damage);
    }
    game.move_to_graveyard(holder, Zone::Base); // "put this card into the graveyard"
}

/// BP01-028, Pengullet -- Yearning for the Sky.
///
/// effect_text: "AUTO When this card is put into the graveyard, draw 1
/// card."
///
/// QA #41's condition (base -> graveyard only) applies here same as
/// everywhere else -- `graveyard_trigger_fired` is the generic check
/// this function trusts the caller to have already made; it doesn't
/// re-check the zone transition itself, same division of labour as
/// every other trigger-condition-vs-effect-body split in this crate.
pub fn pengullet_graveyard_trigger(game: &mut GameState, owner: PlayerId) {
    game.draw(owner);
}

/// BP01-096, Dark Cannon.
///
/// effect_text: "Quick Choose 1 ◇5 or less Pal, and put it into the
/// graveyard."
///
/// Same shape as TD02-021 (Strike from the Darkness) -- no owner
/// qualifier printed, `target` is whichever legal base Pal (cost<=5)
/// the caller already chose. The Quick keyword itself (this can be
/// played during a battle on the opponent's turn) is a legality/timing
/// fact `engine::state::can_play_card`/`Timing` already models
/// generically (QA #28/#29/#30); this function is only the effect
/// body, not the play-legality check.
pub fn dark_cannon(game: &mut GameState, target: CardId) {
    game.move_to_graveyard(target, Zone::Base);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elphidran_gr_on_attack_buffs_power_when_a_revealed_card_is_dragon() {
        let mut game = GameState::test_scenario();
        let attacker = game.deploy(PlayerId::P1, "EBP01-098", Zone::Base).unwrap();
        let starting = game.power(attacker);
        let dragon = game.place(PlayerId::P1, "EFILLER-DRAGON", Zone::Hand);
        game.set_elements(dragon, vec!["Dragon".to_string()]);

        elphidran_gr_on_attack(&mut game, attacker, Some(dragon));

        assert_eq!(game.power(attacker), starting + 500);
    }

    #[test]
    fn elphidran_gr_on_attack_does_not_buff_when_declined() {
        let mut game = GameState::test_scenario();
        let attacker = game.deploy(PlayerId::P1, "EBP01-098", Zone::Base).unwrap();
        let starting = game.power(attacker);

        elphidran_gr_on_attack(&mut game, attacker, None);

        assert_eq!(game.power(attacker), starting, "'you may' -- declining must not buff");
    }

    #[test]
    fn elphidran_gr_on_attack_does_not_buff_a_non_dragon_reveal() {
        let mut game = GameState::test_scenario();
        let attacker = game.deploy(PlayerId::P1, "EBP01-098", Zone::Base).unwrap();
        let starting = game.power(attacker);
        let not_dragon = game.place(PlayerId::P1, "EFILLER-NOT-DRAGON", Zone::Hand);
        game.set_elements(not_dragon, vec!["Water".to_string()]);

        elphidran_gr_on_attack(&mut game, attacker, Some(not_dragon));

        assert_eq!(game.power(attacker), starting, "the buff requires a DRAGON reveal specifically");
    }

    #[test]
    fn chillet_on_deploy_deploys_an_eligible_dragon_when_chosen() {
        let mut game = GameState::test_scenario();
        let revealed = game.place(PlayerId::P1, "EBP01-027", Zone::Deck); // top of deck: lowest tracked id
        game.set_cost(revealed, 8);
        game.set_elements(revealed, vec!["Dragon".to_string(), "Water".to_string()]);

        let result = chillet_on_deploy(&mut game, PlayerId::P1, true);

        assert_eq!(result, Some(revealed));
        assert_eq!(game.zone_of(revealed), Zone::Base, "an eligible reveal, deployed, must land in the base");
    }

    #[test]
    fn chillet_on_deploy_declining_an_eligible_reveal_goes_to_hand() {
        let mut game = GameState::test_scenario();
        let revealed = game.place(PlayerId::P1, "EBP01-027", Zone::Deck);
        game.set_cost(revealed, 8);
        game.set_elements(revealed, vec!["Dragon".to_string()]);

        chillet_on_deploy(&mut game, PlayerId::P1, false);

        assert_eq!(game.zone_of(revealed), Zone::Hand, "'if you did not deploy it, add it to hand' -- covers declining too");
    }

    #[test]
    fn chillet_on_deploy_an_ineligible_reveal_always_goes_to_hand() {
        let mut game = GameState::test_scenario();
        let revealed = game.place(PlayerId::P1, "EFILLER-TOO-EXPENSIVE", Zone::Deck);
        game.set_cost(revealed, 9); // > 8
        game.set_elements(revealed, vec!["Dragon".to_string()]);

        chillet_on_deploy(&mut game, PlayerId::P1, true); // "you may" flag ignored: not eligible at all

        assert_eq!(game.zone_of(revealed), Zone::Hand);
    }

    #[test]
    fn chillet_on_deploy_with_an_empty_deck_reveals_nothing() {
        let mut game = GameState::test_scenario();
        game.set_deck_size_for_test(PlayerId::P1, 0);

        assert_eq!(chillet_on_deploy(&mut game, PlayerId::P1, true), None);
    }

    #[test]
    fn jormuntide_on_deploy_draws_and_rests_the_targeted_opponent_pal_without_it_standing_next() {
        let mut game = GameState::test_scenario();
        let starting_hand = game.hand_size(PlayerId::P1);
        let opponent_pal = game.deploy(PlayerId::P2, "EFILLER-000", Zone::Base).unwrap();
        game.set_cost(opponent_pal, 7);

        jormuntide_on_deploy(&mut game, PlayerId::P1, Some(opponent_pal));

        assert_eq!(game.hand_size(PlayerId::P1), starting_hand + 1, "'draw 1 card' always happens");
        assert!(game.is_resting(opponent_pal));
        assert!(game.wont_stand_next_stand_phase(opponent_pal));
    }

    #[test]
    fn jormuntide_on_deploy_with_no_target_still_draws() {
        let mut game = GameState::test_scenario();
        let starting_hand = game.hand_size(PlayerId::P1);

        jormuntide_on_deploy(&mut game, PlayerId::P1, None); // battle1's actual case, both occurrences

        assert_eq!(game.hand_size(PlayerId::P1), starting_hand + 1);
    }

    #[test]
    fn rocket_launcher_act_gives_plus_200_to_a_non_pengullet_target() {
        let mut game = GameState::test_scenario();
        let launcher = game.deploy(PlayerId::P2, "EBP01-044", Zone::Base).unwrap();
        let target = game.deploy(PlayerId::P2, "EFILLER-000", Zone::Base).unwrap();
        let starting = game.power(target);

        let granted = rocket_launcher_act(&mut game, launcher, target);

        assert_eq!(game.power(target), starting + 200);
        assert_eq!(granted, None);
        assert!(game.is_resting(launcher), "[Rest this card] is the ACT's own cost");
    }

    #[test]
    fn rocket_launcher_act_upgrades_a_pengullet_target_to_plus_500_and_grants_the_sweep() {
        let mut game = GameState::test_scenario();
        let launcher = game.deploy(PlayerId::P2, "EBP01-044", Zone::Base).unwrap();
        let target = game.deploy(PlayerId::P2, "EBP01-028", Zone::Base).unwrap();
        game.set_card_name(target, "Pengullet – Yearning for the Sky");
        let starting = game.power(target);

        let granted = rocket_launcher_act(&mut game, launcher, target);

        assert_eq!(game.power(target), starting + 500, "main name Pengullet upgrades +200 to +500");
        assert!(granted.is_some());
        assert_eq!(game.granted_ability_instances(target).len(), 1);
    }

    #[test]
    fn rocket_launcher_granted_sweep_deals_damage_equal_to_holders_power_to_all_opponent_pals_then_graveyards_holder() {
        let mut game = GameState::test_scenario();
        let holder = game.deploy(PlayerId::P2, "EBP01-028", Zone::Base).unwrap();
        game.increase_power(holder, 500); // the +500 upgrade from rocket_launcher_act, applied directly for this test's focus
        let holder_power = game.power(holder).max(0) as u32;
        let victim_a = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();
        let victim_b = game.deploy(PlayerId::P1, "EFILLER-B", Zone::Base).unwrap();

        rocket_launcher_granted_sweep(&mut game, holder, PlayerId::P2);

        assert_eq!(game.damage_taken(victim_a), holder_power, "X = this card's [the holder's] Power");
        assert_eq!(game.damage_taken(victim_b), holder_power);
        assert_eq!(game.zone_of(holder), Zone::Graveyard, "'put this card into the graveyard' -- the holder destroys itself");
    }

    #[test]
    fn pengullet_graveyard_trigger_draws_one_card_for_its_owner() {
        let mut game = GameState::test_scenario();
        let starting_hand = game.hand_size(PlayerId::P2);

        pengullet_graveyard_trigger(&mut game, PlayerId::P2);

        assert_eq!(game.hand_size(PlayerId::P2), starting_hand + 1);
    }

    #[test]
    fn dark_cannon_puts_the_chosen_pal_into_the_graveyard() {
        let mut game = GameState::test_scenario();
        let target = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
        game.set_cost(target, 5);

        dark_cannon(&mut game, target);

        assert_eq!(game.zone_of(target), Zone::Graveyard);
    }
}
