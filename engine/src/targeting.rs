//! The "declare a card name" mechanic (5.19.1-adjacent; Antique Dresser,
//! BP01-040): a declared name must be a real, released card name -- QA
//! #67 (not yet released) and QA #68 (doesn't exist at all) are two
//! different failure reasons, not one generic rejection.
//!
//! Deliberately NOT backed by reading `data/
//! 11cardattributes-with-elements.csv` at runtime -- that would pull a
//! real card database into the engine crate, the exact coupling
//! `engine::deck`/`engine::cards`' module docs already argue against,
//! and this module has no way to represent "a name from a future,
//! unreleased set" from real data anyway (there is no such data yet).
//! Instead, `GameState` keeps two small explicit registries
//! (`register_known_card_name`, `register_unreleased_card_name`) that a
//! test populates -- the same "state it explicitly" principle as
//! `is_quick`/`set_auto_doubling_cont` elsewhere.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredName(pub(crate) String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclareNameError {
    /// QA #67: the name belongs to a real but not-yet-released card.
    NotYetReleased,
    /// QA #68: the name doesn't correspond to any released OR
    /// upcoming card at all.
    DoesNotExist,
}
