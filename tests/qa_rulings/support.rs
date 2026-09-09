//! Shared glue for the QA-ruling tests. Everything here just calls into
//! `engine` or builds plain data for it to consume -- no rules logic
//! lives in this file. If a test needs game rules to make sense, that
//! logic belongs in `engine`, not smuggled in here as a "helper."

#![allow(dead_code)]

use engine::deck::DeckEntry;

/// One card-list entry for the deck validator. `number` and `name` are
/// deliberately independent fields (hard rule 3: key on card number,
/// never name) -- the ≤4-copies-per-name cap (QA #2) is checked on
/// `name`, not `number`, so five different printings of the same card
/// still only count as one name.
pub fn deck_entry(number: &str, name: &str) -> DeckEntry {
    DeckEntry {
        number: number.to_string(),
        name: name.to_string(),
    }
}

/// A generic legal 50-card shape for tests that only care about ONE
/// constraint (e.g. "products may mix") and need the rest of the deck to
/// be unremarkable filler: 12 distinct synthetic names at 4 copies each
/// (48), plus 2 more copies of a 13th name (50). Names are clearly
/// synthetic ("Filler <prefix> N") -- never real card names -- so these
/// fixtures can't be mistaken for an asserted real-card interaction.
///
/// The name embeds `number_prefix`, not just a counter, so that two
/// calls with different prefixes (QA #1's "mix Booster Pack and Trial
/// Deck numbering" scenario) never collide on the same generated name --
/// a caller combining two `filler_deck_of` batches under different
/// products already means "these are meant to be distinct cards".
pub fn filler_deck_of(count: usize, number_prefix: &str) -> Vec<DeckEntry> {
    let mut entries = Vec::with_capacity(count);
    let mut name_index = 0;
    let mut copies_of_current = 0;
    for i in 0..count {
        if copies_of_current == 4 {
            name_index += 1;
            copies_of_current = 0;
        }
        entries.push(deck_entry(
            &format!("{number_prefix}-{i:03}"),
            &format!("Filler {number_prefix} {name_index}"),
        ));
        copies_of_current += 1;
    }
    entries
}
