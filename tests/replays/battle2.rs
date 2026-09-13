//! battle2_bg-chillet_vs_blue-purple_LOSS.log -- the longer LOSS replay
//! (172 event lines). Structural coverage only, mirroring
//! tests/parser/test_battle2.py: battle1 carries the deeply-verified
//! ground truth for both the parser and (via that shared fixture) the
//! engine replay harness.

use super::support::{expected_life_check_count, load_fixture, replay_and_assert_every_life_check};

#[test]
fn battle2_replays_to_a_loss_on_turn_10_with_matching_life_totals_throughout() {
    let log = load_fixture("battle2");
    assert_eq!(log.meta.result, "LOSS turn 10");

    let mut game = replay::new_game_for_replay(&log.meta);
    let checks_performed = replay_and_assert_every_life_check(&mut game, &log);

    // See battle1's identical comment -- guards against the harness
    // silently skipping lines it's supposed to check.
    assert_eq!(checks_performed, expected_life_check_count(&log));

    // 3.2.1.2: life below 0 is referenced as 0 but not set to 0 -- see
    // battle1's identical comment.
    assert_eq!(
        game.life(replay::replay_owner(&log.meta)).max(0),
        0,
        "the replay's own result ('LOSS turn 10') means the log owner's life must be 0 at the end"
    );
}

#[test]
fn battle2_chillet_free_deploy_chains_still_fire_their_own_on_deploy() {
    let log = load_fixture("battle2");
    let free_deploys: Vec<_> = log
        .events
        .iter()
        .filter(|e| e.action == "deploy" && e.caused_by_line.is_some())
        .collect();
    assert_eq!(free_deploys.len(), 2, "battle2 is known to contain exactly 2 reveal-then-free-deploy chains");

    let mut game = replay::new_game_for_replay(&log.meta);
    let mut driver = replay::ReplayDriver::default();
    for event in &log.events {
        replay::apply_event(&mut driver, &mut game, &log.meta, event);
        if event.caused_by_line.is_some() {
            assert!(replay::last_on_deploy_triggered(&game), "line {}", event.line_index);
        }
    }
}
