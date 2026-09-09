//! Cargo integration-test entry point for the "Game Preparation" QA
//! category. Deliberately its own compiled binary (`cargo test --test
//! game_preparation`), separate from every other category, so this
//! category's pass/fail count is meaningful on its own even while other
//! categories don't compile yet. See tests/qa_rulings/mod.rs for why.

#[path = "../qa_rulings/support.rs"]
mod support;

#[path = "../qa_rulings/game_preparation.rs"]
mod game_preparation;
