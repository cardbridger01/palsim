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

**9.2.3 (a standing Pal cannot be targeted by an attack, only chosen as a
blocker) confirmed against the live client**, not just against rules text
and the Quick Manual Q&A — same live-client session as the two Format A
confirmations below. Load-bearing for the whole combat model and for deck
analysis built on top of it (`rule_9_2_3_standing_pals_are_not_legal_attack_targets`
in `tests/qa_rulings/card_abilities.rs` already encodes it); now also
client-verified, not solely derived.

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
* **Does a card need a script at all? Three categories, not two — and
this is a different question from what SCRIPTED_ACTIONS governs.**
SCRIPTED_ACTIONS answers "if a script exists, is it actually being
called, or is a generic primitive silently standing in for it" (the
drift the bullet above describes). It says nothing about whether a
script needs to exist in the first place — that's this principle, and
the two aren't in tension, just answering different questions:

  | Category | Script? | Why |
  |-|-|-|
  | **Keyword-only** | Never | The effect IS an engine mechanic (Interrupt 12.8.2, Nocturnal, Taunt) — no card-specific behavior exists to script. Confirmed permanently free: Reindrix, Cryolinx, Blazehowl, Blazehowl Noct, Wumpo Botan, Foxparks, Dinossom (pure `ACT Interrupt (Hand Quick...)` text), plus Pyrin Noct (that same Interrupt text with the Nocturnal reminder prepended, checked to confirm no clause beyond either keyword) — `parser/card_census.py` excludes all 8 from the build-target pool, same treatment as Soul. |
  | **Log-resolved choice** | Usually cheap to resolve, not free | A replay driver reads the choice off the log (which Pal was targeted, how much damage, what was drawn) — generic dispatch is correct and sufficient FOR REPLAY. But Phase 6's optimizer has no log to read; it has to MAKE that choice itself, so the card isn't actually usable by the optimizer until it gets real logic, however cleanly its replay passes. Turned out, for a straight "choose up to 1 Pal, deal N Damage" shape, to cost almost nothing to resolve: `single_shot_rifle_on_deploy` was written taking `target: Option<CardId>` as a parameter (the caller's problem, not the function's) and immediately stopped being deferred — the missing piece was never the choice, only the function. Reptyro's ACT (look at 5, choose a gear, deploy it, shuffle the rest — real deck-manipulation state, not one primitive call) is the harder remaining case and stays in `DEFERRED_LOG_RESOLVED_NAMES`. |
  | **Branching / conditional** | Yes, now | Real conditional logic no generic dispatch could resolve even during replay (Zoe's Strategy's multiple bullets, Chillet's reveal-then-maybe-free-deploy). Scripted as soon as they're built, same as always. |

  The enforcement gap this leaves: SCRIPTED_ACTIONS only catches a
  script being BYPASSED (a category-3 card quietly falling back to a
  primitive). Nothing catches the other direction — a category-2 card
  mistaken for done because its replay passes cleanly with no script at
  all. If ~20 cards got filed that way, Phase 6 would start with a
  fifth of the pool silently inert, and the failure mode is the
  optimizer quietly never building decks around them — which looks
  like a result, not a bug. `parser/card_census.py` prints category-2
  cards in their own section (hand-maintained the same way
  `BRACKETED_FIXTURE_NAMES` is) so they can't be lost in either the
  "done" pile or the ordinary "not yet built" pile.
* **CORRECTION to an earlier version of this entry: Suzaku is not
blocked, and the batch of 8 cards below it had a real bug because of
that wrong assumption.** `deal_damage_from_source(source, target,
amount)` and `set_boosts_red_pal_damage` already exist
(`engine/src/state.rs`), built specifically for QA #53 (Suzaku: +200 to
non-battle damage from any red card, gears included) — that QA-ruling
test passes today. The mistake was assuming "no engine hook" from a
grep for "Suzaku"/"Brave"/"Retaliate" that never checked for the
GENERIC primitive Suzaku's own ruling produced. **Real consequence, not
just a documentation error**: Kitsun, Makeshift Handgun, Mounted
Machine Gun, Single-Shot Rifle, and Pump-Action Shotgun (all five Red,
all dealing non-battle damage) called `apply_damage` directly instead
of `deal_damage_from_source` — every one of them would have dealt the
WRONG amount, silently, the moment a fixture deployed Suzaku alongside
any of them, and none of the three existing fixtures do, so nothing
caught it. Fixed: all five now take a `source: CardId` parameter and
route through `deal_damage_from_source`; each has its own unit test
deploying a `boosts_red_pal_damage` source and asserting the +200
actually lands. A second, deeper layer of the same bug: `deal_damage_
from_source`'s own red-check reads the SOURCE card's `color` field, and
`format_a_replay`'s own `materialize()` never called `set_color` at
all — every driver-materialized card had `color: None`, so even with
the scripts fixed, the boost would still have silently never fired for
anything this driver deploys. `CardMeta` gained a `color` field
(populated from the CSV for all 18 existing entries) and `materialize`
now calls `set_color`. `replay/` (the bracketed driver) has the same
gap and was NOT fixed in this pass — flagged, not silently carried
forward: none of its three fixtures deploy Suzaku either, so it's
dormant there too, but worth the same fix before it isn't.
* **A separate, genuine blocker found working down the frequency list,
that isn't either "needs a script" or "doesn't" (Suzaku, corrected
above, turned out not to be one of these): some Tier 1 cards need a new
ENGINE primitive before any card script could be written at all.**
Distinct from all three categories above — those all assume the
engine already has whatever primitive the script would call
(`apply_damage`, `deploy_from`, `grant_keyword`...). A few of the
highest-frequency remaining cards don't clear that bar yet — worth a
scoped pass of their own before the frequency list reaches them
naturally, since each one is a keyword or type distinction that
unlocks every OTHER card using it, not just the one that surfaced it:
  - **Grappling Gun** (BP01-045, 15 logs) — its ACT grants the
    Vigilance skill (`〈AUTO Vigilance (At the end of your turn, stand
    this card)〉`). `grant_keyword` can attach the keyword itself, but
    nothing in the engine's end phase currently checks for it and
    stands the card — the keyword's own EFFECT isn't implemented yet.
  - **Menasting** (BP01-084, 11 logs) — Retaliate itself ("when this
    card is put into the graveyard during battle, put the opposing
    combat Pal into the graveyard too") has no engine implementation
    at all (unlike Breakthrough, Interrupt, Nocturnal, Taunt, all of
    which do) — confirmed by grep, not assumed. Needs battle-context
    tracking (which Pal was this one's opponent in the battle that
    just killed it), not just a keyword flag.
  - **Univolt** (BP01-009, 10 logs) and **Foxparks**'s own Brave-only
    print — Brave is a REAL keyword (QA #21 lists it alongside
    Interrupt/Taunt as one whose parenthetical reminder text is
    non-binding, same ruling that makes Interrupt/Nocturnal
    keyword-only), but it has no engine implementation either.
  - **Victor's Strategy** (BP01-046, 18 logs) — one of its three
    bullets draws X cards where X = the caller's own STRUCTURE count.
    `CardKind` only distinguishes `Pal` from `StructureOrGear` — the
    engine cannot currently tell a Structure from a Gear, so this
    bullet can't be counted correctly, only approximated (and
    approximating a number hard rule 1 says to get from data, not
    guesswork, is exactly the mistake this file exists to prevent). The
    same Structure-vs-Gear gap is probably WHY Reptyro (`parser/
    card_census.py`'s `DEFERRED_LOG_RESOLVED_NAMES`) stays deferred too
    -- its own ACT chooses specifically among "gear," not any card, off
    the top 5 -- worth confirming when this distinction gets built,
    since Reptyro might turn out to need only that, not the harder
    look-then-shuffle mechanic its current deferral reason emphasizes.

  None of these were built this session — writing a card script around
  a missing primitive would either silently misrepresent what's
  implemented or require inventing engine behavior without a rules
  citation, both worse than leaving the card unbuilt with the reason
  on record. Worth a scoped pass of their own (new engine primitives,
  not card scripts) before the frequency list reaches them naturally.
* **A Format A driver now exists** (`format_a_replay/`, three fixtures —
`pingu_01`, `pingu_03`, `pingu_02`, `tests/format_a_replays/pingu_01.rs` /
`pingu_03.rs` / `pingu_02.rs`), built with the `SCRIPTED_ACTIONS` registry from the
start rather than retrofitted — `astegon_on_deploy`, `aurora_guide`,
`pal_sphere`, `relaxaurus_on_deploy`, `elphidran_aqua_on_deploy`, and
the three `zoes_strategy_*` functions are tagged `corpus-validated
(format-a)`, grep-verified against `format_a_replay/src/lib.rs`, not
inferred. Life totals are the only state this driver independently
derives and checks; hand and souls are resynced from each "Turn
opened" line rather than compared against it — see
`format_a_replay`'s own module doc comment, "Hand resync, not hand
check," for why, and the SETTLED entry below for the correction that
established it.
* **`pingu_03` — second fixture, same opponent (Pingu vs Chylith again)
on purpose, to isolate whether the driver generalizes across GAMES
before testing whether it generalizes across card POOLS.** Got to
green with only additive changes: a `"setup"`/`"rest_draw"` pair for
turn-1-only narration pingu_01 never exercised, a standalone
`"butcher"` arm for Zoe's Strategy's butcher bullet phrased as
"Butchered X." (pingu_01's own two occurrences were phrased "X
destroyed," a different literal sentence for the same bullet), and
wiring the already-implemented, already-bracketed-validated
`elphidran_aqua_on_deploy` into the "deploy" arm's own deferred-
lookahead, same shape as Aurora Guide's. `take_from_hand`, the resync
logic, and the stand-phase detection's `last_stood_owner` check needed
ZERO changes — and the last one is a harder test than it sounds:
pingu_03's own turn sequence has 7 consecutive same-owner turn
transitions (pingu_01 had 2), and it held. A real PARSER bug (not a
driver bug) surfaced too: `power_buff`'s two patterns had no
possessive-subject stripping, unlike every other subject-bearing
pattern in the grammar — "Chylith's Jormuntide gets power +200..."
parsed as card name `"Chylith's Jormuntide"` literally, caught only
because pingu_03 has multiple players' Pals getting power_buff lines
in the same fixture (pingu_01's own analogous lines never carried a
possessive prefix). Fixed with `_POSSESSIVE_SUBJ` in
`parser/format_a_parser.py`; see `tests/parser/test_format_a_pingu03.py`'s
own doc comment for the full account, confirmed via the pre-fix parser
run directly, not inferred.
* **Open question this fixture raised, not resolved: a possible
Elphidran -- Gentle Radiance OnAttack mis-attribution that this test
discipline wouldn't necessarily catch.** pingu_03 line 55 ("Chylith
revealed Elphidran (Dragon).", turn 6, PINGU's own turn) hits
`format_a_replay`'s "reveal" arm's `element`-present branch and fires
Elphidran's OnAttack power buff on Chylith's own Elphidran (deployed
turn 4) — the same code path pingu_01's own two occurrences of this
shape use, and those are both genuinely mid-Chylith's-own-turn. This
one fires during the OPPONENT's turn, with no attack_declare/
battle_attack naming Chylith's Elphidran anywhere nearby, so there's no
textual evidence this really is an OnAttack trigger rather than some
other reveal being misattributed to it. It didn't break pingu_03's
life-total checks — a stray "until end of turn" power buff on a card
that isn't destroyed before its own end phase has no checkable effect
— which is exactly the concern: a wrong attribution here is SILENT,
not caught by this test's own pass/fail signal. Left as-is rather than
guessed at (hard rule 1) — see `format_a_replay`'s own module doc
comment for where this is flagged in the code. **pingu_02 added one
data point, not a resolution:** its own Elphidran reveal (line 109) is
a textbook-clean occurrence — Pingu's own Elphidran, revealed on
Pingu's OWN turn, immediately followed by that same Elphidran
attacking (line 110) — so it doesn't recur in pingu_03's specific
ambiguous shape (opponent's turn, no nearby attack) and doesn't settle
the question. Useful as a contrast case for what this code path looks
like when it's unambiguously right, nothing more.
* **`pingu_02` — third fixture, opponent changed to redpurple on
purpose, testing the card-POOL axis `pingu_03` was scoped not to
touch.** A genuinely different card pool (Foxparks, Foxparks' Harness,
Single-Shot Rifle, Menasting, Reptyro, Leezpunk, Medical Supplies) —
and NONE of them needed a new `cards::` script; `parser/card_census.py`'s
"already in cards/: 13" is unchanged. Every new event shape dispatches
through existing generic engine primitives (`apply_damage` for
Single-Shot Rifle's flat-damage OnDeploy, `materialize` for Reptyro's
own combined-sentence ACT, plain no-ops for "mulligan"/"setup"/
"retaliate"), the same discipline `find_by_name_or_materialize` already
uses for combat — a script gets written when a card's effect needs
real, checked logic, not for every card a log happens to name. One
real, NEW-CLASS driver bug did surface: `find_defender_or_materialize`
picked an ILLEGAL (standing) attack target when two live copies of the
same name existed under one owner at once (redpurple's two Menastings,
turns 9-11) — a different shape than the already-documented Chillet-
revival case (one card reused across zones over time, not two
coexisting at once), and the existing "most recently deployed, else
most recently created" heuristic has no way to know a freshly-deployed
duplicate that never attacked isn't the one a later line means. Fixed
by using 9.2.3 itself as the disambiguator: prefer whichever same-named
candidate IS resting (the log's own declaration is already proof the
real target was legal) before falling back to the old heuristic — see
`find_defender_or_materialize`'s own doc comment. A second, less severe
bug (an ACTOR-corrupting gap in the parser's own `look_and_take`
patterns, not a driver bug — see `tests/parser/test_format_a_pingu02.py`'s
own doc comment) was fixed at the parser layer before this fixture's
driver work even started.

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
* **SETTLED by live-client test (the user played it directly): Pengullet
– Yearning for the Sky's graveyard trigger (BP01-028) fires and draws,
confirming the engine's reading.** Once the Pengullet Rocket Launcher's
granted sweep moves this card base -> graveyard (QA #41's condition),
the AUTO (unconditional, no "may") draws a card for real -- hand
genuinely grows by one. battle1's own log shows no resulting "drew 1
card" line for the holder's owner anywhere nearby, but that's now
confirmed a client display gap, not a rules divergence: the client
simply doesn't render this specific draw. `replay/`'s driver (already
following the rules text over the log's silence) and its hand-size
assertion in `rocket_launcher_act_then_granted_sweep_...`
(replay/src/lib.rs) were already correct -- see
`data/replays/README.md`'s known-anomaly entry, now updated to record
this as confirmed rather than open.
* **SETTLED by live-client test (the user played it directly, not
inferred from a log): Aurora Guide and Pal Sphere draw every single time
they're cast, including twice in the same turn ("you could draw 6").
Chillet's "add it to hand" branch persists every time too, Pal or
Gear, and stacks across multiple Chillets in one turn. The Rocket
Launcher's granted sweep targets the holder's opponent, exactly as
printed — confirming `rocket_launcher_granted_sweep`'s existing
implementation was already right, and that pingu_01's own line 64 ("every
Chylith Pal" naming the WRONG side) is that one occurrence's log text
being misleading, not a rules exception.** This overturned an earlier
(wrong) reading built from pingu_01's own hand-size arithmetic alone,
which had concluded the opposite for the first three: that repeat plays
silently don't fire, and that Chillet's persistence was Pal-only.
`format_a_replay` now calls all of these unconditionally, matching the
confirmed behavior — see its own module doc comment for the retraction
in detail. The general lesson: this log's missing text on repeat plays
is confirmed to be a RENDERING gap in the export, not evidence about
what the game did, and log-only arithmetic reached the wrong conclusion
by itself twice in one session — a live-client check is worth running
before trusting a log-inferred behavioral claim, not just after.
* **SETTLED, structurally rather than by resolving which data source to
trust: `format_a_replay` resyncs hand (and souls) from each "Turn
opened" line instead of deriving them independently and comparing.**
The prior entry here asked whether the export's own hand-count TEXT
lags on repeated effects, or something else silently removes cards.
Neither needed answering: the underlying reason hand size can't be an
independent check is structural, not a bug to chase — Format A's "draw"
events never name what was drawn, so any hand the engine builds up
call-by-call between two "Turn opened" lines is unverifiable against
the log in principle, however carefully it's tracked. Confirmed the
hard way: a targeted fix (prefer renaming an existing anonymous-draw
placeholder over fabricating a new card on a name-lookup miss) was
implemented and measured directly — it reduced but did NOT close the
size drift, and worse, it silently mislabeled real cards (a genuine
Jormuntide, once deployed under the name "Chillet" because an untracked
second Chillet copy grabbed the wrong placeholder first) — an identity
corruption bug, strictly worse than the visible, bounded size drift it
replaced. See `format_a_replay`'s own module doc comment ("Hand resync,
not hand check" and "Why not reconstruct identity between checkpoints
instead") for the full mechanism. `tests/format_a_replays/pingu_01.rs`
does not assert hand size, by design, not pending anything further.
**Corrected in the same pass: souls are NOT an independently-verified
check either**, despite an earlier report in this project's history
treating "souls matched while hand drifted" as evidence the "Turn
opened" line itself was trustworthy. `increase_soul` only ever resyncs
the tracked total UP to match the log's own `souls` field — matching at
every checkpoint proves that resync is self-consistent, not that souls
are derived independently. That inference is retracted; the line may
still be trustworthy, but not for that reason, and this codebase
doesn't currently have one. **Life totals are now the only state Format
A validates in the stronger, independently-derived-then-compared
sense** — see `docs/palsim-build-handbook.md` §4.1, corrected to match.
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
* **SETTLED by live-client test, and the answer is the bad one: the
"Turn opened" line can omit cards the player genuinely holds.** Chillet's
reveal-declined branch was confirmed to go to hand immediately (the
same session that confirmed Pengullet's trigger, above) — so Pyrin Noct
really was in Chylith's hand from turn 2 onward (added at line 29), and
the log's own "Turn opened" line simply didn't list it until turn 18.
That means the line is a **partial view, not ground truth** — resyncing
the engine's hand to it can silently DROP a real card the engine
correctly held, for as long as the log's omission lasts. **This does
not reverse the resync decision.** The alternative (heuristic
reconciliation between checkpoints) was tried and measured directly: it
produced silent card-IDENTITY corruption (a real Jormuntide mislabeled
"Chillet"), which is strictly worse than a resync occasionally dropping
a card the engine already had right. It does change the documentation:
`format_a_replay`'s module doc comment and this file previously called
the line "authoritative" — corrected to "preferred over an unverifiable
derived hand," a weaker and more honest claim that doesn't invite a
future reader to trust the line somewhere it can mislead. It may also
explain part of the original hand-size drift this whole investigation
started from: an engine correctly holding a card the log never lists
looks, from the outside, exactly like over-drawing.
  **Chased one level further, per the standing question of whether this
  is a characterizable rule or an arbitrary display gap:** grepped
  every event in pingu_01 mentioning "Pyrin Noct" by name. Between line
  29 (added to Chylith's hand) and line 164 (first appears in Chylith's
  own "Turn opened" hand list), there is exactly zero intervening
  reference to it for Chylith — no play, deploy, discard, interrupt, or
  reveal. (Line 43's bare-marker "Pyrin Noct" at turn 3, and lines 186/199
  later, are all Pingu's own separate copy, a different player's card
  entirely — confirmed by actor, not assumed from the name.) Nothing
  correlates with turn 18 specifically; the card just starts appearing.
  Sample size of one occurrence in one log — hard rule 7 applies, this
  doesn't support inferring a rule ("cards added outside a draw don't
  show until X") over "arbitrary client display bug the resync has to
  live with." Treat as the latter until a second occurrence says
  otherwise.

