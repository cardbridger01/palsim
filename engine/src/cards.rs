//! Effect-text parsing, only insofar as QA #20/#21 need it: parenthetical
//! explanatory text (`(this ability triggers On Deploy)`) and keyword
//! reminder text (`(nullify the opponent's attack)`) must parse
//! identically with or without it, because their presence or absence
//! never changes the card's actual effect.
//!
//! NOT a general card database or effect-text interpreter -- resolving a
//! card number to its printed stats/text is `data/
//! 11cardattributes-with-elements.csv`'s job, deliberately kept out of
//! this crate (see `engine::deck`'s module doc comment for the same
//! separation applied to deck validation).

/// Strips every top-level `(...)` segment and collapses the surrounding
/// whitespace, so `"X (explanatory text)"` and `"X"` parse identically
/// (QA #20/#21). Not nested-paren-aware -- no card text in the QA
/// dataset nests parentheses, so a simple first-`(`-to-next-`)` strip is
/// sufficient; this is not a general-purpose parser.
pub fn parse_effect_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut depth = 0u32;
    for ch in text.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}
