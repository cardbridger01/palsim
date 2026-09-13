# Palworld OCG Simulator

A deterministic rules engine, ISMCTS agent, and deck optimizer for the
Palworld Official Card Game. Rust core, Python orchestration, PyO3 bridge.

Full design document: `docs/palsim-build-handbook.md`. Read it before
starting a new phase.

\---

## Hard rules for this codebase

These exist because each one has already caused a real error in this project.

1. **Card behaviour comes ONLY from `data/11cardattributes-with-elements.csv`
(the `effect\_text` column) and `data/palworld\_tcg\_qa\_dataset.json`.**
NEVER infer what a card does from its name, subtitle, art, cost, or stats.
If a behaviour is not in `effect\_text` or a QA ruling, STOP and ask.
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
5. **Filter out rows with `soul` in `card\_slug`** before any deck-size,
decklist-hash, or deck-validation work. Soul cards are a separate 10-card
deck and are not part of the 50.
6. **Control for color pair before quoting any card win rate.** Raw rates are
dominated by which archetypes play the card and have reversed several
conclusions in this project's history.
7. **State sample sizes** whenever reporting a result. Many sources here are
thin enough that direction is all the data supports.
8. Derived dumps of the QA dataset (qa\_dump\_\*.txt) are disposable scratch files. data/palworld\_tcg\_qa\_dataset.json is the only source of truth for rulings. Never cite a dump.

\---

## Rules sources, in precedence order

1. **Card text** — comprehensive rules 1.3.1: card text beats the comprehensive rules
2. **Official QA rulings** — `data/palworld\_tcg\_qa\_dataset.json` (97 entries)
3. **Comprehensive Rules v1.00**, 2026-07-30 — `docs/POCGeComprehensive-Rules\_0730.pdf`
4. **Quick Manual**, 2026-06-27 — the SOLE source of the 2-color deck rule

Note on (4): the two-color deck construction rule does **not** appear in the
comprehensive rules despite the comprehensive rules being newer. It is printed
on the box and enforced by every online simulator. Do not drop it from the
deck validator.

\---

## Deck construction constraints (all five)

|Constraint|Source|
|-|-|
|Exactly 50 cards|Comp. rules 6.1.1.1|
|≤4 cards with the same card name|Comp. rules 6.1.1.2|
|≤8 cards with the Lucky icon|Comp. rules 6.1.1.3|
|Soul deck exactly 10|Comp. rules 6.1.2.1|
|≤2 colors; colorless free in any deck|Quick Manual only|

Consequence: **six independent search problems**, one per color pair. No legal
deck crosses pairs. The optimizer should run them as separate jobs.

\---

## Timing model — get this right or everything downstream is wrong

* **Trigger ≠ resolve.** A fulfilled trigger condition puts the ability into
**standby state** immediately (10.8.2), *even mid-resolution*.
* Standby abilities resolve at the next **check timing** (10.5.3.2), turn
player first, then non-turn player.
* **Players MUST play all standby abilities** — not optional (10.8.3.1).
* A standby ability resolves **even if its source card changed zones** (10.8.7).
* Check timing order: all rule actions first, repeated until none remain, then
automatic abilities (10.5.3).
* Every play timing is preceded by a check timing (10.5.4.1).
* Rule actions are **interrupt-type** (immediate — e.g. player damage
resolution) or **check-type** (only at check timing — e.g. lethal damage,
losing verdict). See 11.1.2.

The canonical test: Chillet's On Deploy reveals a Dragon, free-deploys it, and
that Pal's own On Deploy still fires. Confirmed by QA #32, QA #90, and observed
client behaviour in the replay corpus.

\---

## Numeric edge cases — three different behaviours

* **Power is unclamped.** Store signed; never clamp at 0 (1.3.2.4).
A Pal at negative Power survives with 0 damage taken, but dies the moment it
has any damage ≥ its Power (QA #92, #93).
* **Life** below 0 is *referenced* as 0 but not set to 0 (3.2.1.2).
* **Resources** below 0 *become* 0 (3.3.3.1).

Test all three separately.

\---

## Keyword traps

|Keyword|Trap|
|-|-|
|**Interrupt**|Nullifies **attacks only** (12.8.2). Blank against On Deploy damage, ACT abilities, and events. Cost is `\[①, discard this]` OR `\[discard this + 1 other card]` — a player at zero souls can still interrupt.|
|**Taunt**|Only compels targeting if the card **can** be targeted (12.10.2), and standing Pals cannot be (9.2.3). A standing Taunt Pal taunts nothing.|
|**Breakthrough**|Damage equals **this card's Strike** (12.14.2), NOT excess/overflow damage.|
|**Retaliate**|Triggers on being put in the graveyard **during a battle** (12.12).|
|**Assault**|May target **standing** Pals — the exception to 9.2.3.|
|**Nocturnal**|+300 Power only while the "night" specific state is active (5.3, 12.13).|

\---

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

\---

## Long-running jobs

* Any batch job MUST poll `control/state.txt` between batches and honour
`PAUSE` / `STOP` by checkpointing and exiting cleanly (exit 0).
* Checkpoint writes MUST be atomic: write to `.tmp`, then rename.
* Derive per-unit RNG seeds from a single run seed so a resumed run is
bit-identical to an uninterrupted one. Without this, a bug found in game
#418,203 is not reproducible after a pause cycle.
* Size batches to a few seconds of work so pause latency stays low.

Rationale: this machine also runs a Palworld game server. Handbook §7.

\---

## Validation

* `cargo test` must stay green. No exceptions.
* `tests/qa\_rulings/` encodes the 97 official rulings as unit tests.
* `tests/replays/` replays real client logs through the engine and asserts
state at every line. **A failure here means the engine disagrees with the
actual game — investigate it. Do not weaken the test.**

\---

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

\---

## Conventions

* Rust for the engine and search; Python for orchestration and analysis.
* Card scripts: small effect objects, **one module per set** (`cards/bp01.rs`,
`cards/td01.rs`, `cards/td02.rs`). BP02 "Legends Awaken" releases 2026-10-30
and must be addable as a new module, not a refactor.
* Release builds use `target-cpu=native` — this box is a Ryzen 7 9800X3D with
AVX-512 and 96MB of 3D V-Cache.
* Seedable PRNG per worker (`rand\_pcg`), never thread-local RNG.
* Every card script's doc comment carries a validation tag, `/// VALIDATION:
<tier>`, where `<tier>` is one of `corpus-validated (bracketed)`,
`corpus-validated (format-a)`, `test-only`, or `unvalidated` — see
`parser/card_census.py`, which computes the same three-way split (plus
`corpus-validated`, further split by which replay corpus actually exercises
it) for every card NOT yet in `cards/`. The point is the same either way:
when the optimizer calls some unplayed card broken, the tag is what tells
you, without re-deriving it, whether that implementation was ever checked
against a real game, an official ruling, or nothing at all. Tag every new
card script when it's written; don't leave it for a census run to notice.
* **`corpus-validated` means the specific function is called by a replay
driver and its result feeds an assertion — not "the card's name appears in
a corpus log."** Caught retagging the first 8 `cards/` scripts: `replay/`
(the bracketed-format driver) only calls two of them
(`chillet_on_deploy`, `rocket_launcher_act`/`rocket_launcher_granted_sweep`)
directly. The other six — `elphidran_gr_on_attack`, `jormuntide_on_deploy`,
`pengullet_graveyard_trigger`, `dark_cannon`, `elphidran_aqua_on_deploy`,
`strike_from_the_darkness` — have their *observable* effect (a draw, a
graveyard move, a top-of-deck placement) replicated with a generic engine
primitive instead, because the driver only needs the visible result to
match, not the real function to run. Those cards genuinely deploy and
attack in the replays, and that participation is real (their Cost/Strike
feed real life-total math) — but the scripted ability itself is validated
only by its own unit test, same confidence tier as if the corpus never
existed. Expect the same gap once a Format A driver exists: a card
appearing in a Format A log is not, by itself, evidence its script was
ever called. Verify by grepping the driver for the function name before
tagging anything `corpus-validated (format-a)` — don't infer it from
the card's presence in a log.

\---

## Open questions — ask before assuming

* **Declaring a bare main name.** 5.19.1 permits declaring "a card name, main
name, or sub name," but Antique Dresser says "Declare 1 card name."
Unresolved; does not affect currently known cards.
* **Quick cards on your own turn — SETTLED.** QA #29's summary sentence, read
alone, would bar Quick entirely on your own turn. Read together with QA #28
and QA #30 it doesn't: Quick has exactly two legal windows — your own Main
Phase (QA #28, played like an ordinary card) and battle on the opponent's
turn (QA #29/#30, one card at a time, each fully resolved before the next
may be played) — but never during a battle on your own turn. Encoded in
`tests/qa_rulings/card_abilities.rs` (`qa_028_*`, `qa_029_*`, `qa_030_*`).
* **Whether the online client implements Chillet chaining correctly.** Rules
and QA say yes; the replay corpus has not yet produced a clean observation
of a chained On Deploy firing.
* **Alarm Bell (BP01-018)'s mandatory-attack scope, found during the human
read of the 19 `unvalidated`-tier cards.** "ACT 1/Turn [①, assign 1 Pal]
Stand all Pals assigned this turn. Until end of turn, your Pals cannot be
assigned, and must attack as much as possible (Includes Pals deployed after
activating this ability)." Two things effect_text doesn't settle: (1) does
"all Pals assigned this turn" include the Pal spent as this very ACT's own
`[assign 1 Pal]` cost, or only Pals assigned by some other effect earlier in
the turn — a self-reference timing question the same shape as trigger-vs-
resolve elsewhere in this document; (2) what "must attack as much as
possible" actually requires — every Pal that has a legal target, once each,
regardless of a player's tactical preference? The QA dataset's only nearby
ruling (#46) covers the opposite case ("may [do X]" is never mandatory) and
says nothing about how a mandatory-attack requirement is itself enforced.
Do not infer an enforcement mechanism; ask before scripting this one.
* **Soul (SOUL-001)'s place in the 144-card build target.** effect_text is
literally "Generic Soul card - colorless resource, no ability text." Hard
rule 5 already excludes Soul cards from deck-validation work as a separate
10-card deck outside the 50; unclear whether `cards/` needs a script for it
at all, or whether `parser/card_census.py`'s pool should exclude it (143,
not 144). Flagging rather than silently deciding either way.

