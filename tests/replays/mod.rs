//! Replay regression tests (CLAUDE.md / build handbook Phase 2): feed
//! every one of the three known client logs, as already parsed into JSON
//! by the Phase 1 parser (`parser/replay_parser.py` ->
//! `tests/replays/fixtures/*.json`, checked in so this crate never has
//! to shell out to Python), through the engine and assert state at every
//! line that the log itself states a life total for.
//!
//! `replay::ReplayLog` / `ReplayEvent` are meant to be exactly the JSON
//! schema `parser/replay_parser.py` documents in its module docstring
//! (line_index, turn, phase, actor, action, cards, targets, resolution,
//! caused_by_line, text) -- they derive `Deserialize` from that schema
//! directly rather than re-describing it.
//!
//! Events are replayed in `events` array order, i.e. file line order,
//! **never** sorted or grouped by the `turn` field -- battle3's T7
//! [Tn waiting] turn-regression (see data/replays/README.md and
//! parser/replay_parser.py's gotcha 1) is deliberately still present in
//! these fixtures and must stay that way. A harness that re-sorts by
//! turn before replaying would silently pass while re-ordering an
//! interrupt's resolution around the attack it answers -- see
//! `battle3::t7_events_are_applied_in_file_line_order_not_turn_order`.

mod support;

mod battle1;
mod battle2;
mod battle3;
