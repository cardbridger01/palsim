//! pingu_03_edc15d3a.txt -- the second Format A driver fixture, chosen
//! to hold the OPPONENT constant (Pingu vs Chylith, same as pingu_01)
//! while testing a different game sequence: does the driver generalize,
//! or was it fitted to pingu_01 alone? 18 turns, curated provenance, a
//! clean life-gap header claim (see `tests/parser/test_format_a_pingu03.py`
//! for the parser-level hand-verification this fixture was built from).
//!
//! Life is asserted every line the log gives evidence for, same
//! discipline as pingu_01. Hand SIZE is not asserted here either, for
//! the same structural reason (see `format_a_replay`'s own module doc
//! comment, "Hand resync, not hand check") -- `Check::Hand` results are
//! still counted as a regression guard against a skipped checkpoint.

use format_a_replay::{apply_event, new_driver_for_replay, new_game_for_replay, Check};

use super::support::load_fixture;

#[test]
fn pingu03_replays_with_matching_life_totals_throughout() {
    let log = load_fixture("pingu_03");
    assert_eq!(log.meta.record_id.as_deref(), Some("edc15d3a-09a0-4445-9e16-5a32090bfc1f"));

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
                    hand_checks += 1;
                }
            }
        }
    }

    assert_eq!(life_checks, format_a_replay::expected_life_check_count(&log));
    assert_eq!(hand_checks, format_a_replay::expected_hand_check_count(&log));

    // "PINGU WINS ... Final life: Pingu (WINNER) 8, Chylith 0."
    assert_eq!(game.life(participants.seat_of_actor("Chylith")).max(0), 0);
    assert_eq!(game.life(participants.seat_of_actor("Pingu")).max(0), 8);
}
