//! battle1_bg-chillet_vs_blue-purple_WIN.log -- the shortest replay (68
//! event lines), fully hand-verified at the parser level in
//! tests/parser/test_battle1.py. This is the one CLAUDE.md's canonical
//! test lives in: T3 and T5 both deploy Chillet, reveal a Dragon, and
//! free-deploy it, and that freely-deployed Pal's own On Deploy still
//! fires (QA #32, QA #90) -- lines 24-27, 63-66, and 68-71 in the log,
//! linked by the parser's `caused_by_line` field.

use super::support::{load_fixture, replay_and_assert_every_life_check};
use engine::state::PlayerId;

#[test]
fn battle1_replays_to_a_win_on_turn_5_with_matching_life_totals_throughout() {
    let log = load_fixture("battle1");
    assert_eq!(log.meta.result, "WIN turn 5");

    let mut game = engine::replay::new_game_for_replay(&log.meta);
    replay_and_assert_every_life_check(&mut game, &log);

    assert_eq!(
        game.life(PlayerId::opponent_of_replay_owner(&log.meta)),
        0,
        "the replay's own result ('WIN turn 5') means the opponent's life must be 0 at the end"
    );
}

#[test]
fn battle1_chillet_free_deploy_chains_still_fire_their_own_on_deploy() {
    let log = load_fixture("battle1");

    // Every deploy the parser linked via caused_by_line (a bare "Revealed
    // X." immediately followed by a same-named free "Deployed X.") is a
    // Chillet-style free deploy. QA #32 says that freely-deployed Pal's
    // own On Deploy must still fire, from the deck, exactly as if it had
    // been deployed from hand.
    let free_deploys: Vec<_> = log
        .events
        .iter()
        .filter(|e| e.action == "deploy" && e.caused_by_line.is_some())
        .collect();
    assert_eq!(
        free_deploys.len(),
        4,
        "expected exactly the 4 reveal-then-free-deploy chains battle1 is known to contain"
    );

    let mut game = engine::replay::new_game_for_replay(&log.meta);
    for event in &log.events {
        engine::replay::apply_event(&mut game, event);
        if event.caused_by_line.is_some() {
            assert!(
                engine::replay::last_on_deploy_triggered(&game),
                "line {}: a Chillet-style free deploy must still trigger the deployed Pal's own On Deploy",
                event.line_index
            );
        }
    }
}
