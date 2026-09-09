//! Deck-construction validation.
//!
//! Implements exactly what QA #1, #2, #3
//! (`../tests/qa_rulings/game_preparation.rs`) require: the <=4-copies
//! cap is keyed on card **name**, not card number (QA #2's entire
//! point -- five different printings of one name still count as one
//! name, hard rule 3: key on number, but *this specific cap* is a name
//! rule, not a number rule), and mixing products or card types is never
//! itself a legality error (QA #1, #3).
//!
//! NOT yet implemented, because no currently-written test exercises it
//! (CLAUDE.md's 5-constraint table is sourced from the comprehensive
//! rules directly, not from a QA id, so Phase 2's QA-ruling harness
//! never encoded it as its own test): the exact-50-card size, the
//! <=8-Lucky-icon cap, the <=2-color-pair rule, or the separate
//! 10-card soul deck's own size check. Add those with their own tests
//! before the deck optimizer (Phase 6) leans on this module -- their
//! absence here is a real gap, not a settled "not needed."

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckEntry {
    pub number: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckList {
    pub main: Vec<DeckEntry>,
    pub soul: Vec<DeckEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckError {
    /// QA #2: more than 4 copies of one card name in the main deck,
    /// counting every printing (differing card number / artwork /
    /// rarity suffix) of that name together.
    TooManyCopies { card_name: String, count: usize },
}

/// QA #1, #2, #3. `deck.soul` is accepted but not validated here (no
/// current test exercises the soul deck's own 10-card rule). Returns
/// every over-the-cap name found, not just the first, in first-seen
/// order.
pub fn validate_main_deck(deck: &DeckList) -> Result<(), Vec<DeckError>> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut first_seen_order: Vec<&str> = Vec::new();

    for entry in &deck.main {
        let count = counts.entry(entry.name.as_str()).or_insert(0);
        if *count == 0 {
            first_seen_order.push(entry.name.as_str());
        }
        *count += 1;
    }

    let errors: Vec<DeckError> = first_seen_order
        .into_iter()
        .filter_map(|name| {
            let count = counts[name];
            (count > 4).then(|| DeckError::TooManyCopies {
                card_name: name.to_string(),
                count,
            })
        })
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
