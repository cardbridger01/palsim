//! pingu_01_ca8ae191.txt -- Format A's first fully-replayed log
//! (`tests/parser/test_format_a_pingu01.py`'s own hand-verified,
//! 138-event fixture). Chosen per the corpus survey: 22 turns, curated
//! provenance, a clean life audit in its own header, and it exercises
//! Chillet chains, Astegon's power reduction, both interrupt forms, and
//! Strike from the Darkness.
//!
//! Life is asserted every line the log gives evidence for, same as the
//! bracketed format. Hand SIZE is NOT currently asserted, even though
//! `format_a_replay` computes and returns a `Check::Hand` at every "Turn
//! opened" line -- see the open question below before trying to make
//! this assert again.
//!
//! ## Why hand size isn't asserted here yet
//!
//! Live-client testing (not log-reading) confirmed Aurora Guide, Pal
//! Sphere, and Chillet's reveal-declined-to-hand branch ALL fire every
//! single time they're triggered, including repeats in the same turn --
//! overturning an earlier version of this driver's "only call where the
//! log shows the resulting text" rule, which had been fitted to what
//! turned out to be a wrong reading of pingu_01's own silence on repeat
//! plays. `format_a_replay`'s "reveal"/"play_card" arms were corrected
//! to always call these functions, matching the confirmed real
//! behavior.
//!
//! That correction is almost certainly right about the CARDS. But it
//! makes the driver's simulated hand size diverge, growingly, from what
//! this file's own "Turn opened -- Hand: ..." lines state -- e.g. by
//! turn 15 the two readings are 12 vs. 5. That gap is far too large to
//! be the known per-card-identity limitation (`format_a_replay`'s own
//! module doc comment: hand checks are scoped to size because "draw"
//! events never name what was drawn). It points at one of two things,
//! and only a live-client check can tell them apart:
//!
//! 1. The SITE'S OWN "Turn opened -- Hand:" export text is itself stale
//!    for repeated effects, the same way individual action lines are --
//!    i.e. the live in-game hand counter the user watched go up is
//!    accurate, but this particular export field lags behind it.
//! 2. Something else routinely removes cards from hand that this driver
//!    has no evidence for at all (not a hand-size limit -- QA #10 rules
//!    that out).
//!
//! Re-enable the assertion once that's resolved, rather than forcing a
//! match with another card-behavior heuristic -- the last three tried
//! (a Pal-vs-Gear Chillet persistence split, several "reuse an anonymous
//! drawn card" identity heuristics) were each fitted to this log's own
//! numbers and got overturned by direct client testing, or broke one
//! checkpoint while fixing another. See CLAUDE.md's Open Questions for
//! the full history.

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
