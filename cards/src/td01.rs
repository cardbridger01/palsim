//! TD01 trial deck card scripts needed by battle1.

use engine::state::{CardId, GameState, PlayerId, Zone};

/// TD01-012, Elphidran Aqua -- Gentle Ripples.
///
/// VALIDATION: corpus-validated (bracketed), corpus-validated (format-a)
/// -- `replay/`'s "put_on_top" arm calls this function directly (both
/// draws and the top-of-deck move together, as one call) for every
/// occurrence in all three bracketed replays (4 total: battle1 x1,
/// battle2 x2, battle3 x1); `format_a_replay`'s own "deploy" arm calls
/// it too, via a deferred draw+put_on_top lookahead (same shape as
/// Aurora Guide's own), for pingu_03's one occurrence (lines 48-50).
/// Verified by grepping both drivers' src/lib.rs, not assumed.
///
/// effect_text: "AUTO OnDeploy Draw 2 cards, choose 1 card from your
/// hand, and put it on the top of the deck."
///
/// QA #33 (lists ETD01-012 directly): "choose 1" (not "up to 1") is
/// mandatory -- `chosen` is a real card the caller must supply, never
/// optional here, unlike Chillet's "you may" or Jormuntide's "up to 1"
/// above it in bp01.rs. `move_to_top_of_deck` does the real Hand ->
/// Deck zone bookkeeping this needs (unlike `return_to_top_unchanged`,
/// which is only for putting back a card that never actually left the
/// deck).
pub fn elphidran_aqua_on_deploy(game: &mut GameState, player: PlayerId, chosen: CardId) {
    game.draw(player);
    game.draw(player);
    game.move_to_top_of_deck(player, chosen, Zone::Hand);
}

/// TD01-010, Single-Shot Rifle.
///
/// VALIDATION: corpus-validated (format-a) -- `format_a_replay`'s own
/// "damage_taken" arm calls this function for real (discriminated by
/// `last_deployed` + amount==1500, same shape as Jormuntide's own
/// OnDeploy draw), for pingu_02's one occurrence (line 63, right after
/// this card's own deploy at line 62). Grep-verified against
/// `format_a_replay/src/lib.rs`, not inferred from the card's presence
/// in a log. Previously tracked in `parser/card_census.py`'s
/// `DEFERRED_LOG_RESOLVED_NAMES` on the theory that "the log already
/// resolves the choice" made deferring costless -- removed from that
/// list in the same commit as this function, since the real cost was
/// never the choice (`target` is a parameter the caller supplies, not
/// something this function resolves itself), only the missing
/// function.
///
/// effect_text: "AUTO OnDeploy Choose up to 1 Pal, and deal 1500
/// Damage; ACT [Rest this card] Choose 1 Pal, and it gets Power +200
/// until end of turn."
///
/// Only the OnDeploy half gets a script. The ACT half ("[Rest this
/// card] Choose 1 Pal, +200 Power") is the generic "Gear ACT power
/// buff" shape `replay/`'s own module doc comment already leaves
/// unscripted for this exact card and Grappling Gun.
///
/// This is a Red card (BP01-002 Suzaku's own +200 non-battle-damage
/// boost applies to it -- QA #53), so damage goes through
/// `deal_damage_from_source(source, ...)`, never `apply_damage`
/// directly -- caught only by review, not by this crate's own tests,
/// since no fixture deploys Suzaku alongside any of these cards yet.
/// `source` is `self` (this card is the one dealing the damage), not
/// the caller's Suzaku.
pub fn single_shot_rifle_on_deploy(game: &mut GameState, source: CardId, target: Option<CardId>) {
    if let Some(target) = target {
        game.deal_damage_from_source(source, target, 1500);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elphidran_aqua_on_deploy_draws_two_then_puts_a_chosen_hand_card_on_top_where_the_next_draw_finds_it() {
        let mut game = GameState::test_scenario();
        let starting_hand = game.hand_size(PlayerId::P1);
        let victors_strategy = game.place(PlayerId::P1, "EBP01-046", Zone::Hand); // already in hand, battle1's actual case

        elphidran_aqua_on_deploy(&mut game, PlayerId::P1, victors_strategy);

        assert_eq!(
            game.hand_size(PlayerId::P1),
            starting_hand + 2, // +1 pre-existing, +2 drawn, -1 put back on top = net +2 from before this call
            "draw 2, then move exactly 1 back out of hand"
        );
        assert_eq!(game.zone_of(victors_strategy), Zone::Deck);
        assert_eq!(game.top_of_deck(PlayerId::P1), Some(victors_strategy));

        // The load-bearing assertion: "put on top of the deck" must not
        // be cosmetic -- the very next draw must return THIS card.
        let next_drawn = game.draw(PlayerId::P1);
        assert_eq!(next_drawn, victors_strategy);
    }

    #[test]
    fn single_shot_rifle_on_deploy_deals_1500_when_a_target_is_chosen() {
        let mut game = GameState::test_scenario();
        let source = game.deploy(PlayerId::P1, "EFILLER-RIFLE", Zone::Base).unwrap();
        game.set_color(source, "Red");
        let target = game.deploy(PlayerId::P2, "EFILLER-A", Zone::Base).unwrap();

        single_shot_rifle_on_deploy(&mut game, source, Some(target));

        assert_eq!(game.damage_taken(target), 1500, "no Suzaku on the field -- unboosted");
    }

    #[test]
    fn single_shot_rifle_on_deploy_with_no_target_does_nothing() {
        let mut game = GameState::test_scenario();
        let source = game.deploy(PlayerId::P1, "EFILLER-RIFLE", Zone::Base).unwrap();
        game.set_color(source, "Red");
        let bystander = game.deploy(PlayerId::P1, "EFILLER-A", Zone::Base).unwrap();

        single_shot_rifle_on_deploy(&mut game, source, None);

        assert_eq!(game.damage_taken(bystander), 0);
    }

    #[test]
    fn single_shot_rifle_on_deploy_gets_suzakus_plus_200_since_the_source_is_red() {
        // QA #53: Suzaku's CONT boosts non-battle damage from ANY red
        // card the controller has in base, gears included -- the exact
        // interaction this function's own doc comment flags as
        // uncaught by any fixture. `deal_damage_from_source` (not
        // `apply_damage`) is what makes this pass.
        let mut game = GameState::test_scenario();
        let suzaku = game.deploy(PlayerId::P1, "EFILLER-SUZAKU", Zone::Base).unwrap();
        game.set_boosts_red_pal_damage(suzaku, true);
        let source = game.deploy(PlayerId::P1, "EFILLER-RIFLE", Zone::Base).unwrap();
        game.set_color(source, "Red");
        let target = game.deploy(PlayerId::P2, "EFILLER-A", Zone::Base).unwrap();

        single_shot_rifle_on_deploy(&mut game, source, Some(target));

        assert_eq!(game.damage_taken(target), 1700, "1500 + Suzaku's +200");
    }
}
