# Palworld OCG Simulator

A deterministic rules engine, ISMCTS agent, and deck optimizer for the
Palworld Official Card Game. Rust core, Python orchestration, PyO3 bridge.

Full design document: `docs/palsim-build-handbook.md`. Read it before
starting a new phase.

---

## Hard rules for this codebase

These exist because each one has already caused a real error in this project.

1. **Card behaviour comes ONLY from `data/11cardattributes-with-elements.csv`
   (the `effect_text` column) and `data/palworld_tcg_qa_dataset.json`.**
   NEVER infer what a card does from its name, subtitle, art, cost, or stats.
   If a behaviour is not in `effect_text` or a QA ruling, STOP and ask.

2. **Element is a separate field from Color and is NOT derivable from it.**
   Chillet is a *Blue* card with *Ice/Dragon* elements. Elphidran – Gentle
   Radiance is *Colorless* with *Dragon*. Warsect is *Green* with
   *Ground/Grass*. There is no mapping between the two axes.

3. **Key on card NUMBER, never on name string.** The QA feed and the card
   list use different English renderings of the same card — BP01-100 is
   "The Adventure Begins" in the card list and "Beginning of the Adventure"
   in the QA data. Build an alias table if you need name lookup.

4. **Any deck list not sourced from a real tournament or ladder result must
   be labelled a CONSTRUCTION** in comments, variable names, and any output.
   Never present a constructed list as a verified archetype.

5. **Filter out rows with `soul` in `card_slug`** before any deck-size,
   decklist-hash, or deck-validation work. Soul cards are a separate 10-card
   deck and are not part of the 50.

6. **Control for color pair before quoting any card win rate.** Raw rates are
   dominated by which archetypes play the card and have reversed several
   conclusions in this project's history.

7. **State sample sizes** whenever reporting a result. Many sources here are
   thin enough that direction is all the data supports.

---

## Rules sources, in precedence order

1. **Card text** — comprehensive rules 1.3.1: card text beats the comprehensive rules
2. **Official QA rulings** — `data/palworld_tcg_qa_dataset.json` (97 entries)
3. **Comprehensive Rules v1.00**, 2026-07-30 — `docs/POCGeComprehensive-Rules_0730.pdf`
4. **Quick Manual**, 2026-06-27 — the SOLE source of the 2-color deck rule

Note on (4): the two-color deck construction rule does **not** appear in the
comprehensive rules despite the comprehensive rules being newer. It is printed
on the box and enforced by every online simulator. Do not drop it from the
deck validator.

---

## Deck construction constraints (all five)

| Constraint | Source |
|---|---|
| Exactly 50 cards | Comp. rules 6.1.1.1 |
| ≤4 cards with the same card name | Comp. rules 6.1.1.2 |
| ≤8 cards with the Lucky icon | Comp. rules 6.1.1.3 |
| Soul deck exactly 10 | Comp. rules 6.1.2.1 |
| ≤2 colors; colorless free in any deck | Quick Manual only |

Consequence: **six independent search problems**, one per color pair. No legal
deck crosses pairs. The optimizer should run them as separate jobs.

---

## Timing model — get this right or everything downstream is wrong

- **Trigger ≠ resolve.** A fulfilled trigger condition puts the ability into
  **standby state** immediately (10.8.2), *even mid-resolution*.
- Standby abilities resolve at the next **check timing** (10.5.3.2), turn
  player first, then non-turn player.
- **Players MUST play all standby abilities** — not optional (10.8.3.1).
- A standby ability resolves **even if its source card changed zones** (10.8.7).
- Check timing order: all rule actions first, repeated until none remain, then
  automatic abilities (10.5.3).
- Every play timing is preceded by a check timing (10.5.4.1).
- Rule actions are **interrupt-type** (immediate — e.g. player damage
  resolution) or **check-type** (only at check timing — e.g. lethal damage,
  losing verdict). See 11.1.2.

The canonical test: Chillet's On Deploy reveals a Dragon, free-deploys it, and
that Pal's own On Deploy still fires. Confirmed by QA #32, QA #90, and observed
client behaviour in the replay corpus.

---

## Numeric edge cases — three different behaviours

- **Power is unclamped.** Store signed; never clamp at 0 (1.3.2.4).
  A Pal at negative Power survives with 0 damage taken, but dies the moment it
  has any damage ≥ its Power (QA #92, #93).
- **Life** below 0 is *referenced* as 0 but not set to 0 (3.2.1.2).
- **Resources** below 0 *become* 0 (3.3.3.1).

Test all three separately.

---

## Keyword traps

| Keyword | Trap |
|---|---|
| **Interrupt** | Nullifies **attacks only** (12.8.2). Blank against On Deploy damage, ACT abilities, and events. Cost is `[①, discard this]` OR `[discard this + 1 other card]` — a player at zero souls can still interrupt. |
| **Taunt** | Only compels targeting if the card **can** be targeted (12.10.2), and standing Pals cannot be (9.2.3). A standing Taunt Pal taunts nothing. |
| **Breakthrough** | Damage equals **this card's Strike** (12.14.2), NOT excess/overflow damage. |
| **Retaliate** | Triggers on being put in the graveyard **during a battle** (12.12). |
| **Assault** | May target **standing** Pals — the exception to 9.2.3. |
| **Nocturnal** | +300 Power only while the "night" specific state is active (5.3, 12.13). |

---

## Combat sequencing

Each attack is a **complete battle**: attack declaration (9.3) → block
declaration (9.4) → quick step (9.5) → damage step (9.6) → end of battle (9.7).
Only then does 9.7.4 return the turn player to their main-phase play timing to
decide on another attack.

This means **the defender commits a blocker before the attacker decides on a
second attack.** Do not model attacks as a simultaneous batch.

Damage accumulates on a Pal within a turn and resets at end phase (5.14.1, 7.6.3).

The stand phase stands **only the turn player's cards** (7.2.1) — a Pal that
ends the opponent's turn rested is a legal attack target on yours.

---

## Long-running jobs

- Any batch job MUST poll `control/state.txt` between batches and honour
  `PAUSE` / `STOP` by checkpointing and exiting cleanly (exit 0).
- Checkpoint writes MUST be atomic: write to `.tmp`, then rename.
- Derive per-unit RNG seeds from a single run seed so a resumed run is
  bit-identical to an uninterrupted one. Without this, a bug found in game
  #418,203 is not reproducible after a pause cycle.
- Size batches to a few seconds of work so pause latency stays low.

Rationale: this machine also runs a Palworld game server. Handbook §7.

---

## Validation

- `cargo test` must stay green. No exceptions.
- `tests/qa_rulings/` encodes the 97 official rulings as unit tests.
- `tests/replays/` replays real client logs through the engine and asserts
  state at every line. **A failure here means the engine disagrees with the
  actual game — investigate it. Do not weaken the test.**

---

## Build order (do not reorder)

1. **Log parser** — `parser/`. Client log → structured move sequence.
2. **Test harness** — `tests/`. QA rulings + replay regression.
3. **Rules engine + card scripts** — `engine/`, `cards/`.
4. **ISMCTS agent** — `sim/`.
5. **Checkpointing + server coexistence** — `control/`, `scripts/`.
6. **Deck optimizer** — `optimize/`.

The harness comes before the engine deliberately. Without it, plausible-looking
engine code passes review and the error only surfaces as bad deck rankings
weeks later.

---

## Conventions

- Rust for the engine and search; Python for orchestration and analysis.
- Card scripts: small effect objects, **one module per set** (`cards/bp01.rs`,
  `cards/td01.rs`, `cards/td02.rs`). BP02 "Legends Awaken" releases 2026-10-30
  and must be addable as a new module, not a refactor.
- Release builds use `target-cpu=native` — this box is a Ryzen 7 9800X3D with
  AVX-512 and 96MB of 3D V-Cache.
- Seedable PRNG per worker (`rand_pcg`), never thread-local RNG.

---

## Open questions — ask before assuming

- **Declaring a bare main name.** 5.19.1 permits declaring "a card name, main
  name, or sub name," but Antique Dresser says "Declare 1 card name."
  Unresolved; does not affect currently known cards.
- **Quick cards on your own turn.** QA #29's summary sentence read literally
  would bar it entirely; it almost certainly means "during a battle." Resolve
  before hard-coding.
- **Whether the online client implements Chillet chaining correctly.** Rules
  and QA say yes; the replay corpus has not yet produced a clean observation
  of a chained On Deploy firing.
