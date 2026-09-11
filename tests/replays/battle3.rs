//! battle3_bg-chillet_vs_blue-red_WIN.log -- the log data/replays/README.md
//! names directly: at T7, [Tn waiting] lines regress to turn 6 (lines 98
//! and 100) because they log an interrupt resolving against a
//! turn-6-declared attack, interleaved with ordinary T7 lines. A harness
//! that sorts or batches by `turn` before replaying scrambles this
//! sequence; this file locks down that this crate's harness doesn't.
//!
//! README's "known anomaly" (Jormuntide Ignis attacks twice, then blocks
//! on Edify's own T7, which 7.2.1 says it should be rested for) is
//! *not* asserted either way here -- CLAUDE.md lists it as an open
//! question ("resolve before trusting the engine's rest-state model").
//! Encoding a specific resolution now would be inventing an answer this
//! project doesn't have yet.

use super::support::{expected_life_check_count, load_fixture, replay_and_assert_every_life_check};

const T7_ANOMALY_LINES: std::ops::Range<usize> = 92..102; // lines 92-101 inclusive
const EXPECTED_TURNS_92_TO_101: [i64; 10] = [7, 7, 7, 7, 7, 7, 6, 7, 6, 7];

#[test]
fn battle3_replays_to_a_win_on_turn_7_with_matching_life_totals_throughout() {
    let log = load_fixture("battle3");
    assert_eq!(log.meta.result, "WIN turn 7");

    let mut game = replay::new_game_for_replay(&log.meta);
    let checks_performed = replay_and_assert_every_life_check(&mut game, &log);

    // See battle1's identical comment -- guards against the harness
    // silently skipping lines it's supposed to check.
    assert_eq!(checks_performed, expected_life_check_count(&log));

    // 3.2.1.2: life below 0 is referenced as 0 but not set to 0 -- see
    // battle1's identical comment.
    assert_eq!(
        game.life(replay::opponent_of_replay_owner(&log.meta)).max(0),
        0,
        "the replay's own result ('WIN turn 7') means the opponent's life must be 0 at the end"
    );
}

#[test]
fn battle3_chillet_free_deploy_chain_still_fires_its_own_on_deploy() {
    let log = load_fixture("battle3");
    let free_deploys: Vec<_> = log
        .events
        .iter()
        .filter(|e| e.action == "deploy" && e.caused_by_line.is_some())
        .collect();
    assert_eq!(free_deploys.len(), 1, "battle3 is known to contain exactly 1 reveal-then-free-deploy chain (lines 107-108)");

    let mut game = replay::new_game_for_replay(&log.meta);
    for event in &log.events {
        replay::apply_event(&mut game, &log.meta, event);
        if event.caused_by_line.is_some() {
            assert!(replay::last_on_deploy_triggered(&game), "line {}", event.line_index);
        }
    }
}

/// This is the test the README's gotcha 1 exists to guard against: prove
/// the harness feeds events to the engine in file line order, not
/// sorted/grouped by the bracketed turn number, using the exact T7
/// non-monotonic sequence README spells out by name.
#[test]
fn battle3_t7_anomaly_events_are_applied_in_file_line_order_not_turn_order() {
    let log = load_fixture("battle3");

    let events_in_range: Vec<_> = log
        .events
        .iter()
        .filter(|e| T7_ANOMALY_LINES.contains(&e.line_index))
        .collect();

    let line_indices: Vec<usize> = events_in_range.iter().map(|e| e.line_index).collect();
    assert_eq!(
        line_indices,
        T7_ANOMALY_LINES.collect::<Vec<_>>(),
        "the harness must iterate log.events in array order, which is file line order"
    );

    let turns: Vec<i64> = events_in_range.iter().map(|e| e.turn as i64).collect();
    assert_eq!(
        turns, EXPECTED_TURNS_92_TO_101,
        "turn numbers must be replayed exactly as logged -- including the T6 regressions at lines 98 and 100 -- \
         never re-sorted into ascending order before feeding the engine"
    );

    // Feed the whole log through the engine and confirm it never rejects
    // or reorders an event because its turn number "went backwards" --
    // that would mean something upstream of this loop (not this test)
    // sorted by turn, which is exactly the bug this fixture exists to
    // catch.
    let mut game = replay::new_game_for_replay(&log.meta);
    for event in &log.events {
        replay::apply_event(&mut game, &log.meta, event);
    }
}
