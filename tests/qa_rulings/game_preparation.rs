//! QA category "Game Preparation" (7 rulings; #5 sleeves and #6 playmat
//! excluded, see qa_rulings/mod.rs) -- deck construction and pregame
//! setup, before a single card is played.

use super::support::{deck_entry, filler_deck_of};
use engine::deck::{self, DeckError, DeckList};
use engine::setup::{self, MulliganError, SeatChoice};

/// QA #1: "Can I construct a deck with cards from both Trial Deck and
/// Booster Pack cards?" -> "Yes, you can. The deck can be constructed
/// with cards from other products."
#[test]
fn qa_001_deck_may_mix_products() {
    let mut main: Vec<_> = filler_deck_of(46, "EBP01"); // Booster Pack numbering
    main.extend(filler_deck_of(4, "ETD01")); // Trial Deck numbering, mixed in
    let deck = DeckList {
        main,
        soul: filler_deck_of(10, "ESOUL"),
    };
    assert_eq!(
        deck::validate_main_deck(&deck),
        Ok(()),
        "mixing card numbers from different products must never itself be a legality error"
    );
}

/// QA #2: "Can I include 4 pieces each of cards with the same card name
/// but different card number/artwork/rarity in the same deck?" -> "No,
/// you cannot. ... you may only include up to 4 pieces of cards with the
/// same card name in total in a deck."
#[test]
fn qa_002_four_copy_cap_is_by_name_not_by_number() {
    let mut main: Vec<_> = filler_deck_of(45, "EBP01");
    // 5 different printings of the SAME name -- different numbers,
    // artwork, and rarity suffixes, exactly the scenario in the ruling.
    main.push(deck_entry("EBP01-004", "Foxparks Ignis"));
    main.push(deck_entry("EBP01-004SR", "Foxparks Ignis"));
    main.push(deck_entry("EBP01-004SP", "Foxparks Ignis"));
    main.push(deck_entry("EBP01-004OSR", "Foxparks Ignis"));
    main.push(deck_entry("EBP01-004SSP", "Foxparks Ignis"));
    let deck = DeckList {
        main,
        soul: filler_deck_of(10, "ESOUL"),
    };
    let result = deck::validate_main_deck(&deck);
    assert_eq!(
        result,
        Err(vec![DeckError::TooManyCopies {
            card_name: "Foxparks Ignis".to_string(),
            count: 5,
        }])
    );
}

/// QA #3: "Can I include any number of Pal cards, gear cards, structure
/// cards, and event cards in the deck?" -> "Yes ... you may create a
/// deck with 50 Pal cards too."
#[test]
fn qa_003_no_per_card_type_minimums_or_caps() {
    // All-Pal 50-card deck: 12 names at 4 copies (48) + 2 more of a 13th.
    let main = filler_deck_of(50, "EBP01");
    let deck = DeckList {
        main,
        soul: filler_deck_of(10, "ESOUL"),
    };
    assert_eq!(
        deck::validate_main_deck(&deck),
        Ok(()),
        "an all-Pal-card deck must be legal; there is no per-card-type minimum or cap"
    );
}

/// QA #8: "If we're deciding the order with rock paper scissors, can the
/// winner choose whether they want to go first or second?" -> "Yes, they
/// can."
#[test]
fn qa_008_rock_paper_scissors_winner_chooses_seat() {
    let order_first = setup::choose_seat(SeatChoice::GoFirst);
    let order_second = setup::choose_seat(SeatChoice::GoSecond);
    assert_ne!(
        order_first, order_second,
        "the winner's choice of seat must actually determine turn order, both ways"
    );
}

/// QA #9: "When performing mulligan during game preparation, can I
/// choose which cards to redraw instead of all 5 cards?" -> "No, you
/// cannot. If you wish to redraw, all 5 cards must be redrawn."
#[test]
fn qa_009_mulligan_is_all_five_or_none() {
    let opening_hand = filler_deck_of(5, "EHAND")
        .into_iter()
        .map(|e| e.number)
        .collect::<Vec<_>>();

    // Redrawing all 5 is the only legal non-trivial choice.
    assert!(setup::mulligan(&opening_hand, opening_hand.clone()).is_ok());

    // Redrawing a strict subset (keeping some cards) must be rejected --
    // there is no partial mulligan.
    let keep_two = opening_hand[..2].to_vec();
    assert_eq!(
        setup::mulligan(&opening_hand, keep_two),
        Err(MulliganError::MustRedrawAllOrNone)
    );
}
