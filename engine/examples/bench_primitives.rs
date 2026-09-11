//! Micro-benchmark for `engine::state`'s hot-path primitives, run in
//! release mode:
//!
//!     cargo run --release --example bench_primitives
//!
//! Exists to replace the build handbook's Section 1.4 "~250k games/sec"
//! estimate -- extrapolated from nothing, before any engine code
//! existed -- with a real per-operation number. It is deliberately NOT
//! a games/sec figure: there is no turn-structure loop or legal-move
//! enumeration anywhere in this repo yet (that's Phase 4, `sim/`, which
//! doesn't exist), so there is no "one game" to time. This times the
//! primitives a game would be built out of.
//!
//! The specific question this was written to answer before Phase 4
//! gets built on top of this engine: does per-call cost stay flat as a
//! `GameState` accumulates cards/battles over a game, or does it grow?
//! Three places re-derive from a full scan on every call, deliberately,
//! for correctness -- not bugs, but each is O(size) per call rather
//! than O(1):
//!   - `find_pending_rule_action` (inside `run_check_timing`'s fixpoint
//!     loop) re-scans `self.cards` fresh every call -- QA #74 needs
//!     this: a removed modifier must be visible on the very next check.
//!   - `declare_attack`'s precondition (`self.battles.values().all(...)`,
//!     9.7.4 "attacks are not a batch") scans every battle ever
//!     declared -- `battles` is never pruned.
//!   - `take_top_of_deck` (used by the player-damage mill loop, 3 times
//!     per attack at the default Strike) scans `self.cards` for the
//!     first deck-zoned card belonging to the target -- also never
//!     pruned; a milled card stays in `self.cards` with its zone
//!     changed to Graveyard, not removed.
//!
//! Each sweep below isolates ONE of these by holding the others fixed
//! and using a small timed batch (not thousands of iterations) at each
//! checkpoint, specifically so growth DURING the timed batch doesn't
//! dominate the measurement at low checkpoint values -- an earlier
//! version of this file mixed batch-internal growth into the average
//! and produced numbers that didn't mean what they looked like they
//! meant.

use std::time::{Duration, Instant};

use engine::combat::AttackTarget;
use engine::state::{GameState, PlayerId, Zone};

fn time_it<F: FnMut()>(label: &str, iters: u64, mut f: F) -> Duration {
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    let per_op = elapsed / iters as u32;
    println!("{label:<52} {iters:>7} iters  total {elapsed:>10.2?}  per-op {per_op:>12.2?}");
    per_op
}

/// Fills `game` with `count` additional tracked cards, split across
/// both players and across Base/Hand/Deck, so a "populated board"
/// sweep resembles a real mixed game state rather than one giant pile
/// in a single zone. Plain `place` (not `deploy`) -- these exist only
/// to give a full-`self.cards` scan something to walk past; none are
/// lethal-damaged or otherwise pending anything.
fn populate_board(game: &mut GameState, count: usize) {
    for i in 0..count {
        let owner = if i % 2 == 0 { PlayerId::P1 } else { PlayerId::P2 };
        let zone = match i % 3 {
            0 => Zone::Base,
            1 => Zone::Hand,
            _ => Zone::Deck,
        };
        game.place(owner, "EBENCH-FILLER", zone);
    }
}

fn main() {
    println!("=== deploy (fresh Base card, O(1) expected) ===");
    {
        let mut game = GameState::test_scenario();
        time_it("deploy", 1_000_000, || {
            let _ = game.deploy(PlayerId::P1, "EBENCH-000", Zone::Base);
        });
    }

    println!();
    println!("=== declare_attack + resolve_battle, bounded per-game usage (realistic: ~20 attacks/game, fresh GameState every 20) ===");
    {
        let iters: u64 = 200_000;
        let attacks_per_game = 20u64;
        let start = Instant::now();
        let mut i = 0u64;
        while i < iters {
            let mut game = GameState::test_scenario();
            let attacker = game.deploy(PlayerId::P1, "EBENCH-001", Zone::Base).unwrap();
            for _ in 0..attacks_per_game.min(iters - i) {
                let attack = game.declare_attack(attacker, AttackTarget::Player(PlayerId::P2));
                game.resolve_battle(attack);
                i += 1;
            }
        }
        let elapsed = start.elapsed();
        let per_op = elapsed / iters as u32;
        println!("{:<52} {iters:>7} iters  total {elapsed:>10.2?}  per-op {per_op:>12.2?}", "declare_attack + resolve_battle (bounded)");
    }

    println!();
    println!("=== declare_attack marginal cost vs. accumulated battle-history size ===");
    println!("    (Pal-target combat -- no milling -- isolates the battles.values().all() scan alone)");
    for &history in &[0u64, 500, 2_000, 10_000, 50_000] {
        let mut game = GameState::test_scenario();
        let attacker = game.deploy(PlayerId::P1, "EBENCH-002", Zone::Base).unwrap();
        let defender = game.deploy(PlayerId::P2, "EBENCH-003", Zone::Base).unwrap();
        game.rest(defender); // 9.2.3: only a rested Pal is a legal attack target
        for _ in 0..history {
            let attack = game.declare_attack(attacker, AttackTarget::Pal(defender));
            game.resolve_battle(attack);
        }
        // A small batch relative to `history`, so growth during the
        // timed window itself doesn't swamp the checkpoint's own cost
        // -- except at history=0, which is inherently a ramp from
        // empty and reported as exactly that.
        time_it(&format!("declare_attack (history={history})"), 200, || {
            let attack = game.declare_attack(attacker, AttackTarget::Pal(defender));
            game.resolve_battle(attack);
        });
    }

    println!();
    println!("=== resolve_battle (player target, milling) marginal cost vs. self.cards size ===");
    println!("    (isolates the mill loop's take_top_of_deck scans; battle-history held low and constant)");
    for &cards_size in &[0usize, 500, 2_000, 10_000, 50_000] {
        let mut game = GameState::test_scenario();
        populate_board(&mut game, cards_size);
        let attacker = game.deploy(PlayerId::P1, "EBENCH-004", Zone::Base).unwrap();
        time_it(&format!("resolve_battle, player target (cards~{cards_size})"), 200, || {
            let attack = game.declare_attack(attacker, AttackTarget::Player(PlayerId::P2));
            game.resolve_battle(attack);
        });
    }

    println!();
    println!("=== run_check_timing cost vs. board size (find_pending_rule_action scans self.cards fresh every call) ===");
    for &board_size in &[0usize, 10, 50, 100, 300, 1_000, 3_000] {
        let mut game = GameState::test_scenario();
        populate_board(&mut game, board_size);
        time_it(&format!("run_check_timing (board={board_size})"), 100_000, || {
            game.run_check_timing();
        });
    }
}
