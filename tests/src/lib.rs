//! Phase 2 test harness for the Palworld OCG rules engine.
//!
//! This crate depends on `engine` (`../engine`), which does not exist
//! yet: per CLAUDE.md's build order, the test harness (Phase 2) comes
//! *before* the rules engine (Phase 3). `cargo test` against this
//! package is EXPECTED to fail to compile right now. That failure is
//! the point, not a bug to fix here: these files are an executable
//! specification -- CLAUDE.md's own words, "a specification of correct
//! behaviour, in executable form" -- not a regression suite for code
//! that exists.
//!
//! A passing `cargo test` on this crate before `engine/` is built would
//! mean stub/mock logic snuck in somewhere. Don't add any. Phase 3
//! implements `engine` for real, and these tests turn green one real
//! piece of behaviour at a time -- that's the whole reason the harness
//! is written first.
//!
//! Only two things live directly under `src/`: this file, and nothing
//! else. The actual test content lives at `tests/qa_rulings/` and
//! `tests/replays/` (matching the repo layout in CLAUDE.md /
//! docs/palsim-build-handbook.md) and is pulled in below via explicit
//! `#[path]` so the on-disk layout doesn't have to nest under `src/`.

#[cfg(test)]
#[path = "../qa_rulings/mod.rs"]
mod qa_rulings;

#[cfg(test)]
#[path = "../replays/mod.rs"]
mod replays;
