//! The 97 official QA rulings (`data/palworld_tcg_qa_dataset.json`) encoded
//! as unit tests, one `#[test]` per ruling, grouped by the dataset's own
//! `category` field into one file per category. Every test's doc comment
//! cites its QA `id` and quotes the ruling's `question`/`answer` verbatim
//! -- hard rule 1 in CLAUDE.md: behaviour comes only from card text or an
//! official QA ruling, never invented from a name or a guess at intent.
//!
//! # Engine API these files assume
//!
//! None of this exists yet. It's sketched here once so ~93 tests spread
//! across five files stay internally consistent, and so Phase 3 has a
//! single place that says what shape `engine` needs to be to make this
//! suite meaningful. Treat it as a proposal, not a contract Phase 3 is
//! locked into -- if a cleaner shape falls out of actually building the
//! engine, the tests should move to match it, not the other way round.
//!
//! - `engine::deck` -- deck-construction validation over caller-supplied
//!   `(number, name, ...)` entries (see `support::deck_entry`). Deliberately
//!   name/number-agnostic: resolving a card number to its name is the
//!   caller's job (that's `data/11cardattributes-with-elements.csv`, kept
//!   out of this crate), the validator only enforces counting rules.
//! - `engine::setup` -- pregame procedure: seat choice, mulligan (all 5
//!   or none).
//! - `engine::state::GameState` -- the mutable game: zones, souls, life,
//!   power/damage, standing/rested, night. `GameState::test_scenario()`
//!   builds a minimal state for a single ruling, not a real 50-card game.
//! - `engine::timing` -- the standby queue and check-timing resolution
//!   order (10.5.3, 10.8 in the comprehensive rules): AUTO abilities
//!   triggered mid-resolution queue rather than interrupt, turn player's
//!   own queue resolves (in whatever order they choose) before the
//!   non-turn player's.
//! - `engine::selection` -- the "Choose X" / "Choose up to X" / "Consume
//!   X" / "Choose all" idioms shared across many card texts, as a
//!   standalone selection-legality checker independent of full game
//!   state.
//! - `engine::targeting` -- target legality (standing vs. rested, Taunt,
//!   Assault) and the "declare a card name" mechanic (Main Name vs. full
//!   card name, existence check against the released card list).
//! - `engine::combat` -- attack declaration, block, Interrupt, and the
//!   keyword traps in CLAUDE.md's keyword table (Breakthrough,
//!   Retaliate, Nocturnal, ...).
//! - `engine::cards` -- effect-text parsing only insofar as QA #20/#21
//!   need it (parenthetical explanatory text must parse identically
//!   with or without it); NOT a general card database. Resolving a card
//!   number to its printed stats is `data/11cardattributes-with-elements.csv`'s
//!   job, deliberately kept out of this crate (see `engine::deck` above).
//!
//! # Explicitly out of scope (not translated into tests)
//!
//! Five of the 97 rulings describe physical equipment, tournament floor
//! rules, or table etiquette, not engine state, and would only ever
//! produce a vacuous `assert!(true)`:
//!
//! - **QA #4** (Flow of the Game) -- shuffle so neither player can
//!   discern card order/position; let your opponent shuffle too at an
//!   event. A physical-shuffling / sportsmanship convention.
//! - **QA #5** (Game Preparation) -- what sleeves are.
//! - **QA #6** (Game Preparation) -- what a playmat is.
//! - **QA #7** (Tournaments/Events) -- per-event time limits are set by
//!   the event, not the game.
//! - **QA #69** (Q&A about Cards) -- if you forget the exact name to
//!   declare, agree with your opponent or call a tournament judge. A
//!   social/procedural fallback, not something a 1v1 simulator resolves.
//!
//! That's documented here rather than left as a silent gap: 97 - 5 = 92
//! rulings become tests below.

mod support;

mod game_preparation; // QA #1, #2, #3, #8, #9 (5 of 7 -- #5, #6 excluded above)
mod flow_of_the_game; // QA #10, #11, #12, #22, #23, #24, #25, #42, #43, #75, #96 (11 of 12 -- #4 excluded above)
mod tournaments_events; // 0 of 1 -- #7 excluded above; file exists only to hold that note
mod card_abilities; // all 45 of 45 "Card Abilities" rulings
mod qa_about_cards; // 31 of 32 "Q&A about Cards" rulings -- #69 excluded above
