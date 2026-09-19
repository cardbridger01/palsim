//! BP01 "Genesis" card scripts needed by battle1.

use engine::state::{CardId, GameState, GrantedInstanceId, PlayerId, Zone};

use crate::opponent_of;

/// BP01-098, Elphidran -- Gentle Radiance.
///
/// VALIDATION: corpus-validated (bracketed) -- `replay/`'s "reveal" arm
/// calls this function directly for every OnAttack reveal whose
/// resolution carries an "element" key (battle1 x1, battle3 x3),
/// verified by grepping replay/src/lib.rs, not assumed from the card's
/// own presence in a log.
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
/// VALIDATION: corpus-validated (bracketed) -- the canonical Chillet-chain
/// test lives in all three bracketed replays (battle1/2/3).
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
/// VALIDATION: corpus-validated (bracketed) -- `replay/`'s "draw" arm
/// calls this function directly for every one of Jormuntide's OnDeploy
/// draws in all three bracketed replays (4 occurrences total), verified
/// by grepping replay/src/lib.rs. "Choose up to 1 opponent Pal" is
/// still hardcoded to `None` -- checked directly against every
/// occurrence in the corpus, not just battle1: the very next event is
/// always "no_valid_target", so no eligible target ever actually gets
/// exercised by this driver.
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
/// VALIDATION: corpus-validated (bracketed) -- activated in battle1 and
/// battle2's bracketed replays, exercising both the plain and the
/// Pengullet-upgrade branch.
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

/// BP01-020, Pump-Action Shotgun.
///
/// VALIDATION: unvalidated -- appears in 32 Format A logs (card_census's
/// own Tier 1 ranking), but no fixture built so far happens to exercise
/// it, and no QA ruling covers BP01-020 either. Built from effect_text
/// alone; checked only by this module's own unit test below.
///
/// effect_text: "AUTO OnDeploy Choose all of your opponent's Pals, and
/// deal 1200 Damage. ACT [Rest this card] Choose 1 Pal, and it gets
/// Power +200 until end of turn."
///
/// Only the OnDeploy half gets a script. It's unconditional ("all,"
/// not "choose up to N") -- no decision for a caller to make, same
/// shape as `rocket_launcher_granted_sweep`'s own sweep, just without
/// a granted-ability holder to track. The ACT half ("[Rest this card]
/// Choose 1 Pal, +200 Power") is the exact generic "Gear ACT power
/// buff" shape already left unscripted for Grappling Gun/Single-Shot
/// Rifle (see `replay/`'s own module doc comment) -- a replay driver
/// applies it generically at its own "power_buff" event, no
/// card-specific logic needed.
pub fn pump_action_shotgun_on_deploy(game: &mut GameState, owner: PlayerId) {
    for pal in game.pals_in_base(opponent_of(owner)) {
        game.apply_damage(pal, 1200);
    }
}

/// BP01-028, Pengullet -- Yearning for the Sky.
///
/// VALIDATION: corpus-validated (bracketed) -- `replay/`'s "activate" arm
/// calls this function directly after the granted sweep moves this
/// card's holder into the graveyard (battle1), since that zone
/// transition satisfies QA #41's base -> graveyard condition for this
/// card's own AUTO. battle1's own log never shows a "drew 1 card" line
/// for the holder's owner right after this happens, even though the
/// effect text is unconditional (no "may") -- but this is a CONFIRMED
/// client display gap, not an open question: live-client testing (the
/// user played it directly) confirmed the trigger fires and the hand
/// genuinely grows by one; the client just doesn't render this specific
/// draw's log line. See `data/replays/README.md`'s "known anomaly"
/// section and the hand-size assertion in
/// `rocket_launcher_act_then_granted_sweep_...` (replay/src/lib.rs),
/// which documents confirmed-correct behavior rather than flagging a
/// discrepancy.
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
/// VALIDATION: corpus-validated (bracketed) -- `replay/`'s "play_card"
/// arm records that this specific Quick card was just played, and the
/// following "destroyed" event (battle1) calls this function on the
/// named victim instead of a generic graveyard move. Verified by
/// grepping replay/src/lib.rs.
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

/// BP01-048, Aurora Guide.
///
/// VALIDATION: corpus-validated (format-a), with a flagged discrepancy --
/// format_a_replay calls this for real at every "play_card" event
/// naming Aurora Guide (pingu_01: 4 occurrences), but only the FIRST
/// shows the resulting "Drew 1."/"Put X on top" text; the other 3 show
/// neither, despite "Draw 1 card" having no "may" (QA #34 is what makes
/// "choose up to 1 [to put back]" itself optional, not the draw). Same
/// pattern independently observed for Pal Sphere below. This driver
/// draws for real regardless of the log's silence and checks the result
/// against the corpus's own ground truth -- the hand contents Format A
/// states at the next "Turn opened" line -- rather than either trusting
/// or ignoring the missing text; see the Open Questions entry this
/// added to CLAUDE.md.
///
/// effect_text: "Quick Draw 1 card, choose up to 1 card from your hand,
/// and put it on the top of the deck."
///
/// `chosen` is `None` for "choose up to 1" declining/finding nothing
/// (QA #34) -- unlike TD01-012 Elphidran Aqua's mandatory "choose 1"
/// (QA #33 lists that card directly; this one is genuinely optional).
pub fn aurora_guide(game: &mut GameState, player: PlayerId, chosen: Option<CardId>) {
    game.draw(player);
    if let Some(card) = chosen {
        game.move_to_top_of_deck(player, card, Zone::Hand);
    }
}

/// BP01-047, Pal Sphere.
///
/// VALIDATION: corpus-validated (format-a) -- format_a_replay calls this
/// for real (pingu_01, line 138), with the same missing-draw-text
/// pattern documented on `aurora_guide` above (no "Drew 3." line follows
/// in the log either).
///
/// effect_text: "Draw 3 cards."
pub fn pal_sphere(game: &mut GameState, player: PlayerId) {
    game.draw(player);
    game.draw(player);
    game.draw(player);
}

/// BP01-026, Relaxaurus -- Hungry Gunner.
///
/// VALIDATION: corpus-validated (format-a) -- format_a_replay's
/// "status_effect" arm calls this for real (pingu_01, line 184: "Chillet
/// won't stand while the source stays in play," immediately following
/// Relaxaurus's own deploy at line 182). Of Relaxaurus's THREE deploys
/// in pingu_01, only this one shows the targeting-rest effect actually
/// firing -- the other two show no target chosen (QA #34: "up to 1"
/// permits 0), matching `target: None` below rather than a forced call.
///
/// effect_text: "AUTO OnDeploy Choose up to 1 ◇6 or less Pal, and rest
/// it. While this card is in the base, that card does not stand."
///
/// QA #61 ("choose up to X (specific condition) Pals, and rest them")
/// and QA #62 -- THIS CARD BY NAME -- both apply directly. `source` is
/// this specific Relaxaurus instance, not a player: QA #62's own ruling
/// ("cannot be stood up even by card effects as long as Relaxaurus
/// remains in the base," CLAUDE.md's timing-model doc) is a lock tied to
/// ONE card staying in the base, not a one-shot suppression -- the
/// opposite duration shape from Jormuntide's `choose_and_rest_wont_stand_next`
/// (one stand phase only), so `lock_from_standing` (QA #62's own tested
/// primitive) is the right one here, not that.
pub fn relaxaurus_on_deploy(game: &mut GameState, target: Option<CardId>, source: CardId) {
    if let Some(card) = target {
        game.rest(card);
        game.lock_from_standing(card, source);
    }
}

/// BP01-095, Zoe's Strategy.
///
/// Three small functions, one per bullet this card's printed text
/// lists, rather than one function branching on a caller-supplied
/// "which bullet" flag.
///
/// VALIDATION: corpus-validated (format-a) for exactly the shapes below
/// -- format_a_replay calls the matching function at each of pingu_01's
/// 6 occurrences. Flagged, not resolved: twice (of 6), `return_to_hand`
/// and `force_discard` are BOTH exercised from the SAME play (log lines
/// 147+149 and 168+170), never `return_to_hand` alone paired with
/// nothing else there. Once (line 116), `return_to_hand` fires ALONE.
/// Twice (lines 160, 187), only `butcher` fires. Whether "return a Pal
/// to hand" and "your opponent discards" are truly two separate
/// bullets of "choose 1 of the following" (and this game just always
/// happened to pick the second one right after the first) or the CSV's
/// flattened effect_text lost a bullet-grouping the physical card
/// actually prints as one combined option is an open question -- see
/// CLAUDE.md. The resource-loss bullet (up to 5 Material/Ingredient) is
/// never exercised in pingu_01 and has no function here.
///
/// effect_text: "Choose 1 of the following: Choose up to 5 of your
/// opponent's Material and/or Ingredient in total, and your opponent
/// loses them. Choose up to 1 Pal from your graveyard, and return it to
/// hand. Your opponent chooses 1 card from their hand, and discards it.
/// Choose 1 of your Pals, and butcher it. If you butchered 1 or more
/// cards this way, choose 1 of your opponent's Pals, and put it into
/// the graveyard."
pub fn zoes_strategy_return_to_hand(game: &mut GameState, player: PlayerId, returned: CardId) {
    game.deploy_from(player, returned, Zone::Graveyard, Zone::Hand);
}

/// See `zoes_strategy_return_to_hand`. `opponent` is the player who is
/// forced to discard (the printed text's "your opponent," relative to
/// this card's controller) -- `discarded` is whichever card of THEIRS
/// the log already names.
pub fn zoes_strategy_force_discard(game: &mut GameState, opponent: PlayerId, discarded: CardId) {
    game.deploy_from(opponent, discarded, Zone::Hand, Zone::Graveyard);
}

/// See `zoes_strategy_return_to_hand`. QA #81 defines Butcher as
/// choosing N of your OWN base Pals and graveyarding them -- always a
/// base -> graveyard move, so `graveyard_trigger_fired` reads true for
/// a butchered card, same as any other base-originating death. None of
/// pingu_01's two occurrences (the "X destroyed, N damage vs M Power."
/// phrasing) or pingu_03's one (the "Butchered X." phrasing -- a
/// different literal sentence for the same bullet, dispatched via
/// `format_a_replay`'s own standalone top-level "butcher" arm rather
/// than through "play_card"'s Zoe's Strategy lookahead, since pingu_03's
/// occurrence has an unrelated draw event breaking that lookahead's
/// simple adjacency) shows the "if you butchered ... put 1 of your
/// opponent's Pals into the graveyard" bonus firing, so that half isn't
/// implemented here.
pub fn zoes_strategy_butcher(game: &mut GameState, own_pal: CardId) {
    game.butcher(own_pal);
}

/// BP01-007, Kitsun.
///
/// VALIDATION: unvalidated -- appears in 8 Format A logs, but no
/// fixture built so far happens to exercise it; no QA ruling covers
/// BP01-007 either.
///
/// effect_text: "AUTO OnDeploy Choose up to 1 <7-diamond-or-greater>
/// Pal, and deal 1200 Damage."
///
/// The <7-diamond-or-greater> cost restriction on the legal target is
/// enforced by whoever calls this (the same division of labor
/// `find_defender_or_materialize`'s own doc comment already describes
/// for 9.2.3 -- a caller that hands this function an illegal target is
/// this driver's own bug, not something the card script re-validates).
/// `target` mirrors `single_shot_rifle_on_deploy`'s own shape exactly
/// -- same "choose up to 1 Pal, deal N Damage" effect, different N.
pub fn kitsun_on_deploy(game: &mut GameState, target: Option<CardId>) {
    if let Some(target) = target {
        game.apply_damage(target, 1200);
    }
}

/// BP01-021, Makeshift Handgun.
///
/// VALIDATION: unvalidated -- appears in 9 Format A logs, but no
/// fixture built so far happens to exercise it; no QA ruling covers
/// BP01-021 either.
///
/// effect_text: "AUTO OnDeploy Choose up to 1 Pal, and deal 500
/// Damage. ACT [Rest this card] Choose 1 Pal, and it gets Power +200
/// until end of turn."
///
/// Only the OnDeploy half gets a script, same reasoning as
/// `single_shot_rifle_on_deploy`'s own doc comment: the ACT half is
/// the generic "Gear ACT power buff" shape already left unscripted for
/// Grappling Gun/Single-Shot Rifle.
pub fn makeshift_handgun_on_deploy(game: &mut GameState, target: Option<CardId>) {
    if let Some(target) = target {
        game.apply_damage(target, 500);
    }
}

/// BP01-015, Mounted Machine Gun.
///
/// VALIDATION: unvalidated -- appears in 8 Format A logs, but no
/// fixture built so far happens to exercise it; no QA ruling covers
/// BP01-015 either.
///
/// effect_text: "ACT 1/Turn [Consume X Material, assign 1 Pal] Perform
/// <Choose 1 Pal, and deal 500 Damage.> X times."
///
/// X (how much Material to consume, and therefore how many times the
/// bracketed effect repeats) and each iteration's own target are both
/// the caller's choice, same division of labor as everywhere else in
/// this crate -- `targets` is the already-resolved list, one chosen
/// Pal per repetition (the same Pal may legally appear more than
/// once). Once-per-turn tracking and the Material cost payment itself
/// are the caller's responsibility too, not re-validated here.
pub fn mounted_machine_gun_act(game: &mut GameState, targets: &[CardId]) {
    for &target in targets {
        game.apply_damage(target, 500);
    }
}

/// BP01-001, Jormuntide Ignis.
///
/// VALIDATION: unvalidated -- appears in 21 Format A logs, but no
/// fixture built so far happens to exercise it; QA #92/#93 cover this
/// card's own unclamped-Power edge cases, not this ACT, so this
/// specific ability has no ruling either.
///
/// effect_text: "ACT 1/Turn [(3)] OR [Discard 2 cards from hand] Stand
/// this card."
///
/// The either/or cost (3 Souls, or discard 2 cards) is the caller's
/// to pay -- already-generic engine primitives (soul spend, discard)
/// handle either path; this function is only the effect itself.
/// `stand_via_effect` (not `rest`'s own inverse, which doesn't exist)
/// is QA #61/Relaxaurus's own lock-respecting stand path -- the
/// correct one for a card-effect-driven stand, as opposed to the
/// stand PHASE's own unconditional one.
pub fn jormuntide_ignis_stand(game: &mut GameState, this_card: CardId) -> Result<(), String> {
    game.stand_via_effect(this_card)
}

/// BP01-078, Lyleen Noct.
///
/// VALIDATION: unvalidated -- appears in 15 Format A logs, but no
/// fixture built so far happens to exercise it; no QA ruling covers
/// BP01-078 either.
///
/// effect_text: "ACT [Rest this card, discard 1 card from hand] Choose
/// 1 <6-diamond-or-less> Pal from your graveyard, and deploy it in the
/// rest state."
///
/// This card's own [Rest this card, discard 1 card from hand] cost and
/// the <6-diamond-or-less> restriction on `chosen` are both the
/// caller's responsibility, same division of labor as every other
/// script in this crate. "Deploy it in the rest state" is the one
/// thing worth getting right here: an ordinary `deploy_from` leaves a
/// card standing by default (QA #11's usual case), so `rest` is called
/// immediately after, not left to whatever the next stand phase would
/// do anyway.
pub fn lyleen_noct_act(game: &mut GameState, owner: PlayerId, chosen: CardId) {
    game.deploy_from(owner, chosen, Zone::Graveyard, Zone::Base);
    game.rest(chosen);
}

/// BP01-004, Bushi.
///
/// VALIDATION: unvalidated -- appears in 9 Format A logs, but no
/// fixture built so far happens to exercise it; no QA ruling covers
/// BP01-004 either.
///
/// effect_text: "AUTO At the end of the battle this card attacked, you
/// may return this card to hand. ACT Interrupt (Hand Quick [(1),
/// discard this card] OR [Discard this card and 1 other card from
/// hand] Nullify the opponent's attack. Battle damage does not occur)"
///
/// Only the end-of-battle half gets a script -- the ACT Interrupt half
/// is the fully generic keyword mechanic `KEYWORD_ONLY_NAMES`'s own 8
/// cards already cover (`parser/card_census.py`), just paired here with
/// a second, real ability the way Pyrin Noct's Nocturnal ISN'T (Pyrin
/// Noct has no second clause beyond its two keywords; Bushi does).
/// "You may" (QA #46: never mandatory) makes `returned` the caller's
/// own choice, not resolved here -- `false` is a legal, common choice,
/// not a missing one.
pub fn bushi_end_of_battle(game: &mut GameState, owner: PlayerId, this_card: CardId, returned: bool) {
    if returned {
        game.deploy_from(owner, this_card, Zone::Base, Zone::Hand);
    }
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
    fn pump_action_shotgun_on_deploy_deals_1200_to_every_opponent_pal_and_leaves_owners_own_pals_untouched() {
        let mut game = GameState::test_scenario();
        let opponent_a = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();
        let opponent_b = game.deploy(PlayerId::P1, "EFILLER-B", Zone::Base).unwrap();
        let own_pal = game.deploy(PlayerId::P2, "EFILLER-C", Zone::Base).unwrap();

        pump_action_shotgun_on_deploy(&mut game, PlayerId::P2);

        assert_eq!(game.damage_taken(opponent_a), 1200);
        assert_eq!(game.damage_taken(opponent_b), 1200);
        assert_eq!(game.damage_taken(own_pal), 0, "\"your opponent's Pals\" -- the controller's own Pals are untouched");
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

    #[test]
    fn aurora_guide_draws_one_and_puts_the_chosen_card_on_top() {
        let mut game = GameState::test_scenario();
        let starting_hand = game.hand_size(PlayerId::P1);
        let chosen = game.place(PlayerId::P1, "EFILLER-000", Zone::Hand);

        aurora_guide(&mut game, PlayerId::P1, Some(chosen));

        assert_eq!(game.hand_size(PlayerId::P1), starting_hand + 1, "+1 pre-existing, +1 drawn, -1 put back on top");
        assert_eq!(game.top_of_deck(PlayerId::P1), Some(chosen));
    }

    #[test]
    fn aurora_guide_with_nothing_chosen_still_draws() {
        let mut game = GameState::test_scenario();
        let starting_hand = game.hand_size(PlayerId::P1);

        aurora_guide(&mut game, PlayerId::P1, None);

        assert_eq!(game.hand_size(PlayerId::P1), starting_hand + 1, "'choose up to 1' declining must not block the unconditional draw");
    }

    #[test]
    fn pal_sphere_draws_three() {
        let mut game = GameState::test_scenario();
        let starting_hand = game.hand_size(PlayerId::P1);

        pal_sphere(&mut game, PlayerId::P1);

        assert_eq!(game.hand_size(PlayerId::P1), starting_hand + 3);
    }

    #[test]
    fn relaxaurus_on_deploy_rests_and_locks_the_chosen_target_from_standing() {
        let mut game = GameState::test_scenario();
        let relaxaurus = game.deploy(PlayerId::P1, "EBP01-026", Zone::Base).unwrap();
        let target = game.deploy(PlayerId::P2, "EFILLER-000", Zone::Base).unwrap();

        relaxaurus_on_deploy(&mut game, Some(target), relaxaurus);

        assert!(game.is_resting(target));
        assert!(game.stand_via_effect(target).is_err(), "QA #62: the lock survives card effects, not just the stand phase");
    }

    #[test]
    fn relaxaurus_on_deploy_with_no_target_does_nothing() {
        let mut game = GameState::test_scenario();
        let relaxaurus = game.deploy(PlayerId::P1, "EBP01-026", Zone::Base).unwrap();
        let target = game.deploy(PlayerId::P2, "EFILLER-000", Zone::Base).unwrap();

        relaxaurus_on_deploy(&mut game, None, relaxaurus);

        assert!(!game.is_resting(target), "'up to 1' declining must not rest anything (QA #34)");
    }

    #[test]
    fn zoes_strategy_return_to_hand_moves_a_graveyard_pal_to_hand() {
        let mut game = GameState::test_scenario();
        let card = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
        game.move_to_graveyard(card, Zone::Base);
        let starting_hand = game.hand_size(PlayerId::P1);

        zoes_strategy_return_to_hand(&mut game, PlayerId::P1, card);

        assert_eq!(game.zone_of(card), Zone::Hand);
        assert_eq!(game.hand_size(PlayerId::P1), starting_hand + 1);
    }

    #[test]
    fn zoes_strategy_force_discard_moves_the_named_hand_card_to_the_graveyard() {
        let mut game = GameState::test_scenario();
        let card = game.place(PlayerId::P2, "EFILLER-000", Zone::Hand);

        zoes_strategy_force_discard(&mut game, PlayerId::P2, card);

        assert_eq!(game.zone_of(card), Zone::Graveyard);
    }

    #[test]
    fn zoes_strategy_butcher_graveyards_the_chosen_own_pal_from_the_base() {
        let mut game = GameState::test_scenario();
        let card = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

        zoes_strategy_butcher(&mut game, card);

        assert_eq!(game.zone_of(card), Zone::Graveyard);
        assert!(game.graveyard_trigger_fired(card), "QA #81: Butcher is always a base -> graveyard move");
    }

    #[test]
    fn kitsun_on_deploy_deals_1200_when_a_target_is_chosen() {
        let mut game = GameState::test_scenario();
        let target = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();

        kitsun_on_deploy(&mut game, Some(target));

        assert_eq!(game.damage_taken(target), 1200);
    }

    #[test]
    fn kitsun_on_deploy_with_no_target_does_nothing() {
        let mut game = GameState::test_scenario();
        let bystander = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();

        kitsun_on_deploy(&mut game, None);

        assert_eq!(game.damage_taken(bystander), 0);
    }

    #[test]
    fn makeshift_handgun_on_deploy_deals_500_when_a_target_is_chosen() {
        let mut game = GameState::test_scenario();
        let target = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();

        makeshift_handgun_on_deploy(&mut game, Some(target));

        assert_eq!(game.damage_taken(target), 500);
    }

    #[test]
    fn mounted_machine_gun_act_deals_500_per_target_including_a_repeated_one() {
        let mut game = GameState::test_scenario();
        let a = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();
        let b = game.deploy(PlayerId::P1, "EFILLER-B", Zone::Base).unwrap();

        mounted_machine_gun_act(&mut game, &[a, b, a]);

        assert_eq!(game.damage_taken(a), 1000, "X = 3 repetitions, two of them targeting the same Pal");
        assert_eq!(game.damage_taken(b), 500);
    }

    #[test]
    fn jormuntide_ignis_stand_stands_a_rested_card() {
        let mut game = GameState::test_scenario();
        let card = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();
        game.rest(card);

        let result = jormuntide_ignis_stand(&mut game, card);

        assert!(result.is_ok());
        assert!(!game.is_resting(card));
    }

    #[test]
    fn lyleen_noct_act_deploys_the_chosen_graveyard_pal_already_rested() {
        let mut game = GameState::test_scenario();
        let card = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();
        game.move_to_graveyard(card, Zone::Base);

        lyleen_noct_act(&mut game, PlayerId::P1, card);

        assert_eq!(game.zone_of(card), Zone::Base);
        assert!(game.is_resting(card), "\"deploy it in the rest state\" -- not left standing by an ordinary deploy's own default");
    }

    #[test]
    fn bushi_end_of_battle_returns_to_hand_only_when_chosen() {
        let mut game = GameState::test_scenario();
        let stays = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();
        let returns = game.deploy(PlayerId::P1, "EFILLER-B", Zone::Base).unwrap();

        bushi_end_of_battle(&mut game, PlayerId::P1, stays, false);
        bushi_end_of_battle(&mut game, PlayerId::P1, returns, true);

        assert_eq!(game.zone_of(stays), Zone::Base, "\"you may\" -- declining is a legal, common choice");
        assert_eq!(game.zone_of(returns), Zone::Hand);
    }
}
