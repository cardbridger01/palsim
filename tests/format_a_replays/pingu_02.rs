//! pingu_02_3e67de53.txt -- the third Format A driver fixture, chosen to
//! change the OPPONENT (redpurple, not Chylith) while pingu_01/03 held
//! it constant -- tests the card-POOL axis pingu_03 was scoped not to
//! touch. 17 turns, curated provenance (see
//! `tests/parser/test_format_a_pingu02.py` for the parser-level
//! hand-verification this fixture was built from).
//!
//! Life is asserted every line the log gives evidence for, same
//! discipline as pingu_01/03. Hand SIZE is not asserted here either,
//! for the same structural reason (see `format_a_replay`'s own module
//! doc comment, "Hand resync, not hand check").

use format_a_replay::{apply_event, new_driver_for_replay, new_game_for_replay, Check};

use super::support::load_fixture;

#[test]
fn pingu02_replays_with_matching_life_totals_throughout() {
    let log = load_fixture("pingu_02");
    assert_eq!(log.meta.record_id.as_deref(), Some("3e67de53-ec3e-4fc8-a18d-a5a9eab427f8"));

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

    // "PINGU WINS ... Final life: Pingu (WINNER) 7, redpurple 0."
    assert_eq!(game.life(participants.seat_of_actor("redpurple")).max(0), 0);
    assert_eq!(game.life(participants.seat_of_actor("Pingu")).max(0), 7);
}
