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
(the bracketed-format driver) originally called only two of them
(`chillet_on_deploy`, `rocket_launcher_act`/`rocket_launcher_granted_sweep`)
directly, faking the other six's *observable* effect (a draw, a graveyard
move, a top-of-deck placement) with a generic engine primitive instead,
because the driver only needed the visible result to match, not the real
function to run — so the whole tiering system's "corpus-validated" label
wasn't actually backed by anything for those six, discovered only by
grepping the driver, not by anything the tests caught. **Fixed**: `replay/`
now calls all eight for real (`ReplayDriver`/`SCRIPTED_ACTIONS` in
`replay/src/lib.rs`) — a card with a script registered in
`SCRIPTED_ACTIONS` reaching a generic-primitive fallback path is now a hard
panic, not a silent substitution, so a *future* dispatch gap surfaces
immediately instead of needing another deliberate grep to notice. The
retrofit also surfaced a real discrepancy, not just a labeling gap:
Pengullet – Yearning for the Sky's own graveyard-draw trigger (BP01-028)
now fires for real when its granted-sweep self-destruct occurs, but
battle1's own client log never shows the resulting "drew 1 card" line —
see that function's doc comment and `data/replays/README.md`'s known-
anomaly section. Verify by grepping the driver for the function name
before tagging anything `corpus-validated (format-a)` — don't infer it
from the card's presence in a log.
* **A Format A driver now exists** (`format_a_replay/`, one fixture so
far — `pingu_01`, `tests/format_a_replays/pingu_01.rs`), built with the
`SCRIPTED_ACTIONS` registry from the start rather than retrofitted —
`astegon_on_deploy`, `aurora_guide`, `pal_sphere`, `relaxaurus_on_deploy`,
and the three `zoes_strategy_*` functions are the first cards genuinely
tagged `corpus-validated (format-a)`, grep-verified against
`format_a_replay/src/lib.rs`, not inferred. It also adds hand-SIZE and
soul-count checks at every "Turn opened" line — stricter than life
totals alone, and what actually surfaced the findings in the next few
Open Questions entries.

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
* **Soul (SOUL-001) — SETTLED: excluded from the build-target pool, which is
143, not 144.** effect_text is literally "Generic Soul card - colorless
resource, no ability text." Hard rule 5 already excludes Soul cards from
deck-validation work as a separate 10-card deck outside the 50; a card with
no ability text and no place in the 50 isn't something the deck optimizer
can ever put in a deck, so `parser/card_census.py`'s pool excludes it
(`all_names`/`names_by_log` and every "144" in its output and docstring
became "143"), and `cards/` needs no script for it. Decided, not merely
flagged — see `docs/palsim-build-handbook.md` §4.1 and
`data/replays/README.md`'s "what the corpus needs from `cards/`" section
for where 143 is cited downstream.
* **Pengullet – Yearning for the Sky's graveyard trigger (BP01-028) —
client behavior unconfirmed, needs a live test, not just log-reading.**
The engine fires this AUTO (unconditional, no "may") once the Pengullet
Rocket Launcher's granted sweep moves this card base -> graveyard,
satisfying QA #41's condition — but battle1's own log shows no resulting
"drew 1 card" line for the holder's owner anywhere nearby (see
`replay/src/lib.rs`'s `pengullet_graveyard_trigger` call site and
`data/replays/README.md`'s known-anomaly entry). Two explanations fit
the log equally well: the client doesn't render/log this specific draw,
or the client doesn't implement the trigger at all — only the second is
an actual rules divergence, and reading the log harder won't distinguish
them. Testable cheaply, same behavioral-oracle approach as handbook
§4.3: play a game on the site, get a Pengullet with a Rocket Launcher's
granted sweep, fire it, and watch hand size directly. Hand grows -> the
client just doesn't log it, engine's reading stands. Hand doesn't grow
-> either a client bug or this rules reading is wrong, and that needs
resolving before trusting this card's implementation. Not yet run.
* **Repeat plays of the same drawing effect within one Format A game
appear to genuinely not draw, not just skip rendering the text —
confirmed empirically, not just suspected.** pingu_01: Aurora Guide
(BP01-048, "Draw 1 card...") shows the resulting "Drew 1."/"Put X on
top" text on its first play only; the other 3 of 4 plays, and Pal
Sphere's (BP01-047, "Draw 3 cards") one play, show neither. An earlier
version of `format_a_replay` called the real function anyway for the
silent occurrences (the text-omission theory); the hand-size checks this
driver is built around drifted -1 then -2, exactly matching those silent
plays. Both functions are now only called where the log gives direct
evidence, and are flagged in their own doc comments. Whether this is a
client bug (the effect really doesn't fire) or an undocumented rule
("draw" abilities are once-per-game, or similar) is unresolved — a
live-client test analogous to the Pengullet entry above would settle it,
and would also settle the next two entries at the same time, since all
three look like instances of one broader phenomenon in how this site
renders/executes repeated identical effects within a game.
* **Chillet's own "if you did not deploy it, add it to hand" branch:
persists in the tracked hand only when the revealed card is a Pal, not a
Gear/Spell — confirmed against 3 of pingu_01's occurrences, not yet
checked against the other 3 in that same file.** Revealing a Pal
(Pyrin Noct line 29, Lyleen Noct line 94) and adding it to hand matches
the next "Turn opened" checkpoint; revealing a Gear (Pengullet Rocket
Launcher, line 120) and adding it to hand does not — the card is simply
absent from the next checkpoint's stated hand, with no discard/play line
accounting for its departure (and QA #10 already rules out a hand-size
limit as the explanation). `format_a_replay`'s "reveal" arm calls the
real function every time either way, but only lets a Pal's result
persist. Not yet checked against pingu_01's other 3 "added to hand"
reveals (lines 152, 157, 199) or any other Format A file — this is the
narrowest claim the data supports so far, not a general rule.
* **Two consecutive Format A "TURN N" numbers can belong to the same
player, with no logged turn for their opponent in between.** pingu_01:
"TURN 1"/"TURN 2" and "TURN 11"/"TURN 12" are both Chylith back to back.
`format_a_replay` treats this as at most one real stand phase for that
player, not two (standing already-standing cards again exposed a real
Pal as an illegal attack target three lines later, in the "TURN 12"
case) — but whether this reflects the actual game (Pingu genuinely gets
skipped, e.g. an extra-turn effect neither player's deck obviously has)
or is another rendering gap in this export is unresolved. Cross-checking
against `top_players_all20/match_list.tsv` or another file with the same
pattern would help.
* **One hand-size checkpoint in pingu_01 (turn 21, line 191) is one card
short and resisted every specific fix tried.** Not asserted in
`tests/format_a_replays/pingu_01.rs` — see that file's own doc comment
for what was tried (a stand-phase bug, a Pal-vs-Gear persistence rule,
several "reuse an anonymous drawn card" heuristics) and why each was
rejected (either it didn't fix this checkpoint, or it fixed this one by
breaking an earlier one). Likely the same general class as the
anonymous-draw-identity limitation `format_a_replay`'s own module doc
comment already documents (hand checks are scoped to size, not
name-for-name identity, specifically because Format A's "draw" events
never name what was drawn) — but not confirmed as that specific cause.
* **Zoe's Strategy's (BP01-095) "return a Pal to hand" and "your
opponent discards" bullets always co-occur in pingu_01, never
separately.** Twice (of 6 total plays), both fire from the same play;
once, "return to hand" fires alone; twice, only the butcher bullet
fires; the resource-loss bullet never fires. Whether the physical
card's real bulleted structure groups "return + opponent discards" as
ONE combined option (and the CSV's flattened `effect_text` lost that
line break) or these are genuinely two separate "choose 1 of the
following" bullets that this game's 2 relevant plays both happened to
pick together is unresolved — see `cards::bp01`'s own doc comment on
`zoes_strategy_return_to_hand`. Exactly a QA-question-pile item, not
something to invent an answer for.

