//! Cargo integration-test entry point for the "Tournaments/Events" QA
//! category. Zero tests (the one ruling in this category is a
//! documented exclusion, see tests/qa_rulings/mod.rs) -- this binary
//! exists so `cargo test --test tournaments_events` still runs cleanly
//! (0 passed; 0 failed) rather than the category being invisible.

#[path = "../qa_rulings/tournaments_events.rs"]
mod tournaments_events;
