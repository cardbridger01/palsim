//! Attack declaration and the battle sequence (9.2-9.7).
//!
//! Two rules this module exists specifically to get right, because
//! CLAUDE.md flags both as places this project has already been wrong:
//!
//! - **Each attack is a complete battle** (9.3 attack declaration -> 9.4
//!   block declaration -> 9.5 quick step -> 9.6 damage step -> 9.7 end
//!   of battle), and only THEN (9.7.4) does the turn player return to
//!   their play timing to decide on another attack. Attacks are never a
//!   batch: `GameState::declare_attack` refuses to start a second battle
//!   while an earlier one hasn't concluded (see
//!   `rule_9_7_4_cannot_declare_a_second_attack_before_the_first_battle_concludes`
//!   in card_abilities.rs). On Attack fires AT declaration (9.3), before
//!   block declaration (9.4) ever happens -- QA #71 depends on exactly
//!   this ordering, and `declare_attack` records the mechanical fact of
//!   what was declared (`on_attacks_structure_triggered`, etc.)
//!   immediately, not deferred to block resolution.
//! - **Attack targets are the opposing player, one of their structures,
//!   or one of their RESTED Pals (9.2.3) -- never a standing Pal.**
//!   `GameState::declare_attack` enforces this for real (a panic, not a
//!   comment) rather than trusting every future caller to remember it --
//!   see `rule_9_2_3_standing_pals_are_not_legal_attack_targets`. This is
//!   also exactly what makes Taunt conditional (12.10.2/3):
//!   `taunt_compelled_targets` (built in Card Abilities batch 1) already
//!   filters to resting Taunt-havers only, so a standing Taunt Pal
//!   compels nothing -- see
//!   `rule_12_10_2_a_standing_taunt_pal_compels_nothing`.
//!
//! Implements exactly what QA #45, #49, #50, #71
//! (`../tests/qa_rulings/card_abilities.rs`) require, plus the two
//! supplementary rule tests above.

use crate::state::{CardId, PlayerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttackId(pub(crate) u32);

/// 9.2.3: what a declared attack can target. Constructing one doesn't
/// itself check legality -- `GameState::declare_attack` does, since
/// that's the only place a target actually gets used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackTarget {
    Player(PlayerId),
    Pal(CardId),
    Structure(CardId),
}

pub(crate) struct Battle {
    pub(crate) attacker: CardId,
    /// `None` once the declared target is removed mid-battle (QA #49) --
    /// distinct from "never had one", which can't happen since
    /// `declare_attack` always takes a target.
    pub(crate) target: Option<AttackTarget>,
    /// 9.7: true once this battle has reached end of battle, by any path
    /// (interrupted, resolved with a target, or resolved with none).
    /// `declare_attack` refuses to start a new battle while any exist
    /// with this false -- 9.7.4, attacks are not a batch.
    pub(crate) concluded: bool,
}
