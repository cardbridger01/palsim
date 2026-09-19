//! TD02 trial deck card scripts needed by battle1.

use engine::state::{CardId, GameState, PlayerId, Zone};

/// TD02-021, Strike from the Darkness.
///
/// VALIDATION: corpus-validated (bracketed) -- played in battle1's
/// bracketed replay (the only one of the three that references it).
/// `replay/`'s "play_card" arm records that this specific Quick card was
/// just played, and the following "destroyed" event calls this function
/// on the named victim instead of a generic graveyard move. Verified by
/// grepping replay/src/lib.rs, not assumed.
///
/// effect_text: "Choose 1 Pal, and put it into the graveyard."
///
/// No zone/owner qualifier in the printed text -- QA #27 ("unless a
/// specific zone is stated, it refers to cards in the base") settles
/// ZONE (the target must be a base Pal, hence `Zone::Base` below), but
/// says nothing about OWNER. Nothing restricts this to the opponent's
/// Pal; `target` is whichever legal base Pal the caller already chose
/// (in battle1, always the opponent's, since that's the only sane
/// play -- but that's a player decision, not a rule this function
/// enforces).
pub fn strike_from_the_darkness(game: &mut GameState, target: CardId) {
    game.move_to_graveyard(target, Zone::Base);
}

/// TD02-012, Astegon.
///
/// VALIDATION: corpus-validated (format-a) -- format_a_replay's "deploy"
/// arm calls this for real (pingu_01, line 51: "Relaxaurus gets power
/// -1000 until end of turn," immediately following Astegon's own
/// deploy). Only the OnDeploy half is implemented -- pingu_01's one
/// Astegon attack (line 53) kills its direct Pal-vs-Pal target through
/// ordinary Strike/Power combat, not through the OnAttack sweep below;
/// nothing in this replay exercises "choose all Pals with Power 300 or
/// less," so that half is left unbuilt rather than shipped unvalidated.
///
/// effect_text: "AUTO OnDeploy Choose up to 1 Pal, and it gets Power
/// -1000 until end of turn (It is not put into the graveyard even if
/// Power becomes 0 or less); AUTO OnAttack Choose all Pals with Power
/// 300 or less, and put them into the graveyard (Choose your Pals
/// too)."
///
/// No owner qualifier on "choose up to 1 Pal" -- pingu_01's own
/// occurrence targets its OWN controller's Relaxaurus, not the
/// opponent's, so this must not assume an opponent-only target.
/// `reduce_power` is deliberately unclamped (1.3.2.4, QA #92/#93): the
/// parenthetical "not put into the graveyard even if Power becomes 0 or
/// less" is confirming the general engine rule already applies here, not
/// adding a special case.
pub fn astegon_on_deploy(game: &mut GameState, target: CardId) {
    game.reduce_power(target, 1000);
}

/// TD02-014, Leezpunk.
///
/// VALIDATION: unvalidated -- appears in 13 Format A logs, but no
/// fixture built so far happens to exercise it; no QA ruling covers
/// TD02-014 either.
///
/// effect_text: "AUTO When this card is put into the graveyard, your
/// opponent chooses 1 card from their hand, and discards it."
///
/// A base -> graveyard-triggered forced discard of the OPPONENT'S own
/// hand card -- same zone move as `zoes_strategy_force_discard`
/// (`deploy_from(..., Zone::Hand, Zone::Graveyard)`), just this card's
/// own trigger instead of Zoe's Strategy's. `discarded` is the
/// opponent's already-chosen card (QA #10 rules out any hand-size
/// limit forcing a specific choice; "their" own pick is the caller's
/// to resolve, not this function's). Callers are responsible for
/// checking `graveyard_trigger_fired` (QA #41: base -> graveyard only)
/// before calling this, same division of labor as every other
/// graveyard-trigger script in this crate.
pub fn leezpunk_graveyard_trigger(game: &mut GameState, opponent: PlayerId, discarded: CardId) {
    game.deploy_from(opponent, discarded, Zone::Hand, Zone::Graveyard);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strike_from_the_darkness_puts_the_chosen_pal_into_the_graveyard() {
        let mut game = GameState::test_scenario();
        let target = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

        strike_from_the_darkness(&mut game, target);

        assert_eq!(game.zone_of(target), Zone::Graveyard);
    }

    #[test]
    fn astegon_on_deploy_reduces_power_by_1000_even_below_zero_without_dying() {
        let mut game = GameState::test_scenario();
        let target = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();
        game.set_power(target, 500);

        astegon_on_deploy(&mut game, target);

        assert_eq!(game.power(target), -500, "unclamped -- 1.3.2.4");
        assert_eq!(game.zone_of(target), Zone::Base, "negative Power alone must not kill (QA #92/#93)");
    }

    #[test]
    fn leezpunk_graveyard_trigger_discards_the_chosen_opponent_hand_card() {
        let mut game = GameState::test_scenario();
        let discarded = game.place(PlayerId::P2, "EFILLER-000", Zone::Hand);

        leezpunk_graveyard_trigger(&mut game, PlayerId::P2, discarded);

        assert_eq!(game.zone_of(discarded), Zone::Graveyard);
    }
}
