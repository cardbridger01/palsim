//! The "Choose X" / "Choose up to X" / "Consume up to X" / "Choose all"
//! idioms shared across many card texts, as a standalone selection-
//! legality checker independent of full game state (deliberately: these
//! four cardinality rules are the same regardless of what's being
//! chosen -- Pals, cards, resources -- so this doesn't need `CardId` or
//! `GameState` at all, just counts).
//!
//! Implements exactly what QA #33, #34, #35, #36
//! (`../tests/qa_rulings/card_abilities.rs`) require.

use std::marker::PhantomData;

#[derive(Debug, Clone, Copy)]
enum SelectorKind {
    /// QA #33: "Choose X" requires choosing as many as possible, up to
    /// X -- with X or more eligible, exactly X; with fewer, all of them.
    ChooseExact(usize),
    /// QA #34/#35: "Choose/Consume up to X" allows choosing 0.
    ChooseUpTo(usize),
    /// QA #36: "Choose all" permits no exclusions.
    ChooseAll,
}

/// `T` is never inspected -- only counts matter -- so there's no trait
/// bound on it; the type parameter exists purely so a call site can
/// write `Selector<CardId>` (or, in these tests, `Selector<i32>`
/// standing in generically) and get a selector shaped for what it's
/// selecting, without this module needing to know what a `CardId` is.
pub struct Selector<T> {
    kind: SelectorKind,
    _marker: PhantomData<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    /// QA #33: `chosen.len()` didn't equal `min(required, available)`.
    MustChooseAsManyAsPossible { required: usize, available: usize },
    /// QA #36: `chosen.len()` didn't equal `available`.
    MustChooseAllEligible { available: usize },
}

impl<T> Selector<T> {
    pub fn choose_exact(n: usize) -> Self {
        Selector { kind: SelectorKind::ChooseExact(n), _marker: PhantomData }
    }

    pub fn choose_up_to(n: usize) -> Self {
        Selector { kind: SelectorKind::ChooseUpTo(n), _marker: PhantomData }
    }

    /// Same cardinality rule as `choose_up_to` -- QA #35 ("consume up to
    /// X, can I choose 0?") answers identically to QA #34's "choose up
    /// to X". A separate constructor rather than reusing `choose_up_to`
    /// directly so a call site states which idiom it's modeling.
    pub fn consume_up_to(n: usize) -> Self {
        Selector { kind: SelectorKind::ChooseUpTo(n), _marker: PhantomData }
    }

    pub fn choose_all() -> Self {
        Selector { kind: SelectorKind::ChooseAll, _marker: PhantomData }
    }

    pub fn validate(&self, chosen: &[T], available: &[T]) -> Result<(), SelectionError> {
        match self.kind {
            SelectorKind::ChooseExact(required) => {
                let effective_required = required.min(available.len());
                if chosen.len() == effective_required {
                    Ok(())
                } else {
                    Err(SelectionError::MustChooseAsManyAsPossible { required, available: available.len() })
                }
            }
            SelectorKind::ChooseUpTo(max) => {
                if chosen.len() <= max.min(available.len()) {
                    Ok(())
                } else {
                    Err(SelectionError::MustChooseAsManyAsPossible { required: max, available: available.len() })
                }
            }
            SelectorKind::ChooseAll => {
                if chosen.len() == available.len() {
                    Ok(())
                } else {
                    Err(SelectionError::MustChooseAllEligible { available: available.len() })
                }
            }
        }
    }
}
