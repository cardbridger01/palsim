//! pingu_01_ca8ae191.txt -- Format A's first fully-replayed log
//! (`tests/parser/test_format_a_pingu01.py`'s own hand-verified,
//! 138-event fixture). Chosen per the corpus survey: 22 turns, curated
//! provenance, a clean life audit in its own header, and it exercises
//! Chillet chains, Astegon's power reduction, both interrupt forms, and
//! Strike from the Darkness.
//!
//! Life is asserted every line the log gives evidence for, same as the
//! bracketed format. Hand SIZE is NOT asserted, even though
//! `format_a_replay` still returns a `Check::Hand` at every "Turn
//! opened" line -- and it never will be, by design, not pending
//! anything. See `format_a_replay`'s own module doc comment ("Hand
//! resync, not hand check") for the full reasoning: each "Turn opened"
//! line resyncs the engine's hand for that player from the log rather
//! than being compared against it, which makes an equality assertion
//! here tautological. `Check::Hand` results are still counted against
//! `expected_hand_check_count` below purely as a regression guard --
//! proof this harness didn't silently skip a checkpoint -- not as a
//! hand-size assertion.

use format_a_replay::{apply_event, new_driver_for_replay, new_game_for_replay, Check};

use super::support::load_fixture;

#[test]
fn pingu01_replays_with_matching_life_totals_throughout() {
    let log = load_fixture("pingu_01");
    assert_eq!(log.meta.record_id.as_deref(), Some("ca8ae191-e71e-4e94-9f86-e0dd0f8a6952"));

    let mut game = new_game_for_replay(&log.meta);
    let (participants, mut driver) = new_driver_for_replay(&log);

    let mut life_checks = 0;
    let mut hand_checks = 0;
    for i in 0..log.events.len() {
        if let Some(check) = apply_event(&mut driver, &mut game, &participants, &log.events, i) {
            let line_index = log.events[i].line_index;
            match check {
                Check::Life(l) => {
                    life_checks += 1;
                    assert_eq!(
                        game.life(l.player).max(0) as i64,
                        l.expected_life,
                        "line {line_index}: engine life diverges from the real client log"
                    );
                }
                Check::Hand(_) => {
                    // Not asserted -- see the module doc comment.
                    hand_checks += 1;
                }
            }
        }
    }

    // Same regression guard as the bracketed format's own tests: proves
    // this harness didn't silently skip a line it was supposed to check
    // (an unrecognized action falling through to a no-op instead of a
    // panic).
    assert_eq!(life_checks, format_a_replay::expected_life_check_count(&log));
    assert_eq!(hand_checks, format_a_replay::expected_hand_check_count(&log));

    // "PINGU WINS ... Final life: Pingu (WINNER) 4, Chylith 0."
    assert_eq!(game.life(participants.seat_of_actor("Chylith")).max(0), 0);
    assert_eq!(game.life(participants.seat_of_actor("Pingu")).max(0), 4);
}
