//! Pregame procedure: seat choice and mulligan.
//!
//! Implements exactly what QA #8, #9 (`../tests/qa_rulings/
//! game_preparation.rs`) require.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatChoice {
    GoFirst,
    GoSecond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOrder {
    PlayerFirst,
    PlayerSecond,
}

/// QA #8: the rock-paper-scissors winner picks a seat, and that choice
/// actually determines who goes first.
pub fn choose_seat(choice: SeatChoice) -> TurnOrder {
    match choice {
        SeatChoice::GoFirst => TurnOrder::PlayerFirst,
        SeatChoice::GoSecond => TurnOrder::PlayerSecond,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MulliganError {
    /// QA #9: no partial mulligan -- redraw is all 5 cards or none.
    MustRedrawAllOrNone,
}

/// QA #9. `redraw` is the set of cards from `opening_hand` the player
/// chooses to put back and redraw. Legal only at the two endpoints --
/// redraw nothing (keep the hand) or redraw everything -- never a
/// strict subset. (The QA answer only says "if you wish to redraw, all
/// 5 must be redrawn"; it doesn't separately address "redraw nothing"
/// because that's just declining the mulligan, not a case the ruling
/// needed to call out -- legal by the same "all or none" logic, not an
/// added assumption.)
pub fn mulligan(opening_hand: &[String], redraw: Vec<String>) -> Result<(), MulliganError> {
    if redraw.is_empty() || redraw.len() == opening_hand.len() {
        Ok(())
    } else {
        Err(MulliganError::MustRedrawAllOrNone)
    }
}
