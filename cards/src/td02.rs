//! TD02 trial deck card scripts needed by battle1.

use engine::state::{CardId, GameState, Zone};

/// TD02-021, Strike from the Darkness.
///
/// VALIDATION: unit-tested only. Played in battle1's bracketed replay
/// (the only one of the three that references it), but `replay/` treats
/// "play_card" as pure narration and replicates the observable destroy
/// generically (`move_to_graveyard` off the following "destroyed"
/// event) rather than calling this function. Confirmed by grepping
/// replay/src/lib.rs, not assumed.
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

#[cfg(test)]
mod tests {
    use super::*;
    use engine::state::PlayerId;

    #[test]
    fn strike_from_the_darkness_puts_the_chosen_pal_into_the_graveyard() {
        let mut game = GameState::test_scenario();
        let target = game.deploy(PlayerId::P1, "EFILLER-000", Zone::Base).unwrap();

        strike_from_the_darkness(&mut game, target);

        assert_eq!(game.zone_of(target), Zone::Graveyard);
    }
}
