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
}
