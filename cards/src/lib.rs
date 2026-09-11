//! Card scripts, only for the cards a currently-written replay needs --
//! `data/replays/battle1_bg-chillet_vs_blue-purple_WIN.log`'s roster
//! right now (9 named cards, 8 with real behaviour; Victor's Strategy
//! is only ever referenced by name in that log, never activated).
//!
//! Confirmed by grepping the log itself, not by recalling a deck list
//! (Reindrix, Antique Mirror, "Found an Egg!", and Aurora Guide never
//! appear in battle1 at all, despite an earlier, looser estimate of the
//! roster -- "grep the log" beats "recall the deck list").
//!
//! One module per set (CLAUDE.md's convention): `bp01`, `td01`, `td02`.
//! Every function's doc comment quotes its card's `effect_text` column
//! verbatim (hard rule 1) -- never paraphrased, never inferred from a
//! name, subtitle, or stat.

pub mod bp01;
pub mod td01;
pub mod td02;

use engine::state::PlayerId;

/// `PlayerId` is a 2-variant enum with no public "the other one"
/// accessor (`PlayerId::other` is private to `engine::state`) -- this
/// crate computes it locally rather than adding engine surface for
/// something this trivial.
pub(crate) fn opponent_of(player: PlayerId) -> PlayerId {
    match player {
        PlayerId::P1 => PlayerId::P2,
        PlayerId::P2 => PlayerId::P1,
    }
}
