//! Palworld OCG rules engine. Phase 3 of CLAUDE.md's build order.
//!
//! Built one QA-ruling category at a time against `../tests/qa_rulings/`
//! and `../tests/replays/`: implement only what a currently-written test
//! demands, run `cargo test --test <category>` from `../tests`, repeat.
//! Each category compiles as its own binary (see
//! `../tests/tests/*.rs`), so an unimplemented module doesn't block a
//! finished one from reporting real pass/fail counts.
//!
//! Hard rule 1 (CLAUDE.md): card behaviour comes only from `effect_text`
//! (`data/11cardattributes-with-elements.csv`) or an official QA ruling
//! (`data/palworld_tcg_qa_dataset.json`). Every card-specific fact in
//! this crate must trace to one of those two sources in a doc comment
//! -- never a name, a stat, or what seems plausible for a creature with
//! that name.
//!
//! Modules land here as the category that needs them gets built; see
//! `../tests/qa_rulings/mod.rs` for the full API sketch every category
//! was written against.

pub mod cards;
pub mod combat;
pub mod deck;
pub mod selection;
pub mod setup;
pub mod state;
