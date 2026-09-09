//! Whole-suite entry point: wires in tests/qa_rulings/mod.rs (the API
//! sketch + all 5 category submodules) as a single binary. Not needed
//! for incremental per-category work -- use `cargo test --test
//! <category>` for that -- but this is the one binary that only
//! compiles once every category's engine:: dependency exists, so it
//! doubles as "is the QA-ruling API surface fully realized yet."

#[path = "../qa_rulings/mod.rs"]
mod qa_rulings;
