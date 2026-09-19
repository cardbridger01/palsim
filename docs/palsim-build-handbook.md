# Palworld TCG Simulator — Build Handbook

**Version 1.1 · September 3, 2026**
Target machine: Windows 11 · **Ryzen 7 9800X3D (8 cores / 16 threads, 96MB 3D V-Cache)** · RTX 4070 (not required)
Coexisting with: a Palworld game server on the same box

*Changes from v1.0: corrected CPU figures (v1.0 assumed 32 cores), added §7 checkpointing and server coexistence, renumbered the phase plan.*

---

## 0. What this is

Four components:

1. **Rules engine** — deterministic simulator of the Palworld OCG
2. **Agent** — plays under imperfect information
3. **Deck optimizer** — iterates decks to find what beats a field
4. **Move advisor** — given a live board state, recommends the best play

Phases 1–3 are tractable. **Phase 3 is ~80% of the total work** and is where these projects die. There's a useful exit after every phase in §8.

### Scope discipline

- The engine needs to support **150 cards**, not a generic TCG framework.
- The agent needs to be **consistent**, not strong, to rank decks.
- "Optimal play" is underdefined — see §10.3. Decide what you want early.

---

## 1. Prerequisites and setup

### 1.1 Install order matters

Install MSVC build tools **before** Rust, or you'll hit a confusing linker error later.

- [ ] **Visual Studio Build Tools**
  ```powershell
  winget install Microsoft.VisualStudio.2022.BuildTools
  ```
  Then open the Visual Studio Installer and add the **"Desktop development with C++"** workload. The bare Build Tools install does *not* include the linker.

- [ ] **Rust**
  ```powershell
  winget install Rustlang.Rustup
  ```
  Accept the default (option 1) — picks `x86_64-pc-windows-msvc` and detects the build tools.

- [ ] **Restart your terminal**, then verify:
  ```powershell
  rustc --version
  cargo --version
  ```

- [ ] **Git for Windows** (required by Claude Code on Windows)
  ```powershell
  winget install Git.Git
  ```

- [ ] **Python 3.12**
  ```powershell
  winget install Python.Python.3.12
  pip install maturin pandas numpy
  ```

- [ ] **Sysinternals** — for process inspection while tuning
  ```powershell
  winget install Microsoft.Sysinternals
  ```

- [ ] **An RCON client** for the pause watcher (§7). `mcrcon` works with Palworld's RCON.

- [ ] **Claude Code** — current steps at https://code.claude.com/docs/en/setup
  - Native installer needs no Node.js. The old `npm install -g @anthropic-ai/claude-code` route is deprecated and needs Node 18+.
  - Requires a paid plan (Pro / Max / Team / Enterprise / Console). The free claude.ai plan does not include Claude Code, and the install completes either way — this catches people out.
  - `claude doctor` runs a full environment diagnostic.

- [ ] **Use Windows Terminal**, not the legacy console host.

### 1.2 Native Windows vs WSL2

**Use native Windows.** Self-contained Rust + Python, no Linux-specific tooling, and you skip the WSL filesystem boundary. You give up Claude Code's sandboxing (WSL2-only); permission rules are adequate for a single-user project.

### 1.3 Performance configuration

- [ ] `.cargo/config.toml` in the project root:
  ```toml
  [build]
  rustflags = ["-C", "target-cpu=native"]

  [profile.release]
  lto = "fat"
  codegen-units = 1
  panic = "abort"
  ```
  `target-cpu=native` matters more on the 9800X3D than on a generic chip — it has AVX-512, and a generic `x86-64-v3` baseline leaves that on the table. Confirm it's available:
  ```powershell
  rustc --print target-features | findstr avx512
  ```

- [ ] **Windows Defender exclusion for `target/`.**
  Settings → Privacy & security → Windows Security → Virus & threat protection → Exclusions.
  Real-time scanning inspects every object file cargo writes and can double build times.

- [ ] Verify parallelism before writing anything real: `cargo new --lib palsim`, add rayon, time a `par_iter()` over a few million iterations, confirm all cores light up in Task Manager.

### 1.4 Expected throughput (8C / 16T)

| Workload | Rust | Python |
|---|---|---|
| No-search playouts | ~250k games/sec | ~2k games/sec |
| ISMCTS, 100 sims/decision | ~100 games/sec | ~4 games/sec |
| One optimizer generation (435 pairings × 1k games) | **~70 min** | ~5 days |
| 20 generations | **~24 h** | infeasible |
| 20 generations, with pause time for the game server | **~31 h** | — |

**Rust for engine and search, Python for orchestration**, bridged with PyO3/maturin. Cloud cost: $0.

**Unvalidated.** This table is the original pre-engine estimate — extrapolated from nothing, before any engine code existed. It cannot yet be replaced with a real games/sec figure: that needs Phase 4's turn-structure loop and legal-move enumeration, neither of which exists yet (`sim/` is empty). What's now measured is §1.4.1 below — the primitive costs a games/sec figure would be built out of. Re-benchmark this table once Phase 4 exists; until then, treat every row above as a guess.

**The 4070 is not needed.** Only relevant if ISMCTS proves too weak and you train a policy/value net, which is unlikely given the tiny branching factor. Don't let the GPU pull you toward an AlphaZero architecture you don't need.

#### 1.4.1 Measured primitive costs

`engine/examples/bench_primitives.rs` — `cargo run --release --example bench_primitives` (release, `target-cpu=native`, single-threaded, 2026-09-11 on the 9800X3D). Not a games/sec figure; see above.

| Primitive | Realistic condition | Per-op cost |
|---|---|---|
| `deploy` | fresh `GameState` | ~350 ns |
| `declare_attack` + `resolve_battle` | bounded, ~20 attacks/game (fresh `GameState` every 20) | ~680 ns |
| `run_check_timing` | board = 100 tracked cards (a plausible full-game ceiling) | ~110 ns |
| `run_check_timing` | board = 300 tracked cards | ~310 ns |

At realistic per-game scale, none of this is the bottleneck the ~250k figure worried about — these are all sub-microsecond.

**But there's a real scaling trap underneath it, found while isolating these numbers.** Three places re-derive from a full scan on every call, deliberately, for correctness, not as bugs:

- `find_pending_rule_action` (inside `run_check_timing`'s fixpoint loop) re-scans `self.cards` every call — QA #74 needs a removed modifier visible on the very next check.
- `declare_attack`'s precondition (`self.battles.values().all(...)`, 9.7.4) scans every battle ever declared on this `GameState` — `battles` is never pruned.
- The player-damage mill loop's `take_top_of_deck` (called once per milled card, 3× per attack at the default Strike) scans `self.cards` for the target's deck — also never pruned; a milled card stays in `self.cards` with its zone changed to Graveyard, not removed.

All three are O(size) per call, fine within one bounded game (a few dozen attacks, a few hundred tracked cards). They stop being fine if a search process **reuses one `GameState` across many simulated games** instead of allocating fresh per playout — `self.cards` and `self.battles` only grow, never shrink, across that instance's lifetime. Isolated measurement:

| Metric | Value |
|---|---|
| `declare_attack` marginal cost, battles-history = 0 | ~203 ns |
| `declare_attack` marginal cost, battles-history = 50,000 | ~30 µs (**148×**) |
| `resolve_battle` (player target, milling) marginal cost, `self.cards` ~ 0 | ~519 ns |
| `resolve_battle` (player target, milling) marginal cost, `self.cards` ~ 50,000 | ~457 µs (**880×**) |

50,000 is roughly what ~800 unreset games' worth of milling would accumulate on one instance — not a realistic single-game size, but a realistic outcome of "for speed, don't reallocate `GameState` between playouts."

**Phase 4 must allocate a fresh `GameState` per playout, never reuse one across games.** That's the one design constraint this benchmark actually changes about the Phase 4 plan — worth stating as a hard requirement there, not just a footnote here.

**If a future Phase 4 wants to reuse state anyway** (allocation pressure at high playout rates, say), **the fix is pruning `self.cards` and `self.battles`** — dropping graveyarded/milled cards and concluded battles that no remaining rule or ability can still reference — **not caching or otherwise short-circuiting the re-derive-fresh scans themselves.** `find_pending_rule_action` re-scanning fresh every call is what makes QA #74 (a removed modifier visible on the very next check) and the 9.6.4/1.2.1 fixpoint correct at all; memoizing it is exactly the kind of change that would pass every currently-green test and then silently misjudge a lethal-damage or losing-verdict check the next time state changes mid-loop. The scans are correct. The maps just need to stop growing forever.

---

## 2. Repository layout

```
palsim/
├── CLAUDE.md              ← read every session; see §3
├── .cargo/config.toml
├── engine/                Rust — zones, phases, check timing, effects
├── cards/                 Rust — 150 card scripts, one module per set
├── sim/                   Rust — ISMCTS, playouts, parallel harness
├── control/               runtime control files (see §7)
│   └── state.txt          RUN | PAUSE | STOP
├── checkpoint/            optimizer state, written between batches
│   └── latest.json
├── scripts/
│   └── watch-server.ps1   RCON poll → writes control/state.txt
├── tests/
│   ├── qa_rulings/        97 official rulings as unit tests
│   └── replays/           64 logs in hand, two grammars — see §4.1
├── parser/                Python — client log → structured move sequence
│                          (replay_parser.py: bracketed grammar; format_a_parser.py:
│                          the site's own narrative export; card_census.py: corpus
│                          card-name census + coverage curve, see §4.1)
├── optimize/              Python — field management, mutation, reporting
└── data/
    ├── 11cardattributes-with-elements.csv
    ├── palworld_tcg_qa_dataset.json
    ├── 40-ladder-meta.json
    ├── 54champdecklistsoneperplayer.csv
    └── replays/
```

**Card scripts as small effect objects, one file per set** — the TCG ONE pattern (their engine is closed, their card DSL is open). Ideal for agentic coding: the agent implements independent effect bodies against a fixed interface without touching the engine.

**`lib` + `rlib` crate types** so you can build a pure-Rust binary for heavy optimizer runs and bypass Python entirely.

---

## 3. CLAUDE.md — write this first

Read every session; the cheapest defence against this project's biggest historical failure mode: **inventing card behaviour**.

```markdown
# Palworld OCG Simulator

## Hard rules for this codebase
- Card behaviour comes ONLY from `effect_text` in data/11cardattributes-with-elements.csv
  and from the official QA rulings in data/palworld_tcg_qa_dataset.json.
- NEVER infer what a card does from its name, subtitle, art, or statistics.
  If a behaviour isn't in effect_text or a QA ruling, STOP and ask.
- Element is a separate field from Color and is NOT derivable from it.
  (Chillet is a Blue card with Ice/Dragon. Warsect is Green with Ground/Grass.)
- Key on card NUMBER, never on name string. The QA feed and the card list use
  different English renderings of the same card (BP01-100 is "The Adventure
  Begins" in the card list and "Beginning of the Adventure" in the QA data).
- Any deck list not sourced from a real tournament or ladder result must be
  labelled a CONSTRUCTION in comments and output.

## Rules sources, in precedence order
1. Card text (comprehensive rules 1.3.1: card text beats comprehensive rules)
2. Official QA rulings (data/palworld_tcg_qa_dataset.json)
3. Comprehensive Rules v1.00, 2026-07-30
4. Quick Manual, 2026-06-27 — SOLE source of the 2-color deck rule (handbook §5.1)

## Long-running jobs
- Any batch job MUST poll control/state.txt between batches and honour PAUSE/STOP
  by checkpointing and exiting cleanly. See handbook §7.
- Checkpoint writes must be atomic (temp file + rename).
- Derive per-unit RNG seeds from a run seed so a resumed run is bit-identical.

## Validation
- Every change must keep `cargo test` green.
- tests/replays/ replays real client logs; a failure means the engine disagrees
  with the actual game. Investigate; do not weaken the test.
```

---

## 4. Ground truth assets

Two things most people building this don't have. **They are the project.**

### 4.1 Replay corpus — 207 URLs catalogued, 64 logs actually in hand, two grammars

**Correcting this section's own earlier estimate.** "~165 replay logs" was a guess made before any log had been parsed. The real numbers, now that Phase 1 has actually happened: 207 match URLs are catalogued as sources; 64 have been pulled and parsed so far (3 hand-authored bracketed fixtures + 44 split from a combined championship-tier extraction + 20 from two top players' personal match history). Parse each into a move sequence, replay through the engine, assert state matches at every line — an automated regression suite for timing bugs, the exact class that produces a simulator that runs fine and ranks decks wrongly.

**Two structurally unrelated log grammars, not one:**

- **Bracketed `[Tn phase]` format** — `parser/replay_parser.py`, 3 hand-authored fixture logs (`battle1/2/3`). A fixed "Edify" perspective; phase tag (`main`/`stand`/`draw`/`soul`/`end` vs `waiting`) determines actor.
- **"Format A"** — the site's own narrative export, `parser/format_a_parser.py`, 64 real logs across two on-disk shapes that share one grammar: `data/replays/*.log` (44 files, split from a combined extraction; no hand-reveal data) and `data/replays/top_players_all20/*.txt` (20 files; a per-turn "Turn opened — Souls N/N · Hand: ..." line reveals one seat's hand and soul count). Format A has no fixed perspective: either named player can be the active actor, so nothing in this parser hardcodes a name — actor identity, and the "Opponent"-anonymization case, are both derived from each match's own declared participants. See `data/replays/README.md` for the full grammar writeup, including three attribution traps that cost real time to work out.
  **Correcting this section's own earlier claim that the hand-reveal line makes "strict engine validation — assert hand contents turn by turn, not just life totals — possible": it doesn't, and the reason is structural, not an implementation gap to be fixed later.** Format A's "draw" events never name what was drawn, so any hand the engine builds up call-by-call between two "Turn opened" lines is unverifiable against the log in principle. `format_a_replay` (the driver built against this fixture — see its own module doc comment, "Hand resync, not hand check") instead treats each "Turn opened" line as PREFERRED — not authoritative — and resyncs the engine's hand to match it, every time, rather than deriving a hand independently and comparing. "Preferred" rather than "authoritative" matters here specifically: a live-client test confirmed the line can itself omit a card the player genuinely holds (a card added via Chillet's reveal-declined branch was live-confirmed to enter hand immediately, but didn't appear in this fixture's own hand list for 16 turns), so treating it as ground truth would just be a differently-shaped bug. Still the better of the two known options — the alternative (heuristic identity reconstruction between checkpoints) was tried and produced silent card mislabeling, worse than a resync that occasionally drops a card the engine already had right. Souls turned out to work the same way (`increase_soul` resyncs toward the log's stated value; a matching count at every checkpoint proves the resync is self-consistent, not that souls are independently derived). **Life totals are the only state Format A validates in the stronger sense — derived independently by the engine and then checked against the log, not resynced toward it.** That's a narrower validation claim than this section originally promised; card scripts firing for real (`SCRIPTED_ACTIONS`), board state, combat resolution, and the timing model are all still exercised and still checked, just not hand/soul bookkeeping as an independent signal.
- Both grammars get the same output schema (`line_index, turn, phase, actor, action, cards, targets, resolution, caused_by_line, text`) and the same treatment: `tests/parser/test_corpus.py` (bracketed, explicit 3-file list) and `tests/parser/test_format_a_corpus.py` (Format A, 64-file glob) each assert zero unparsed lines across their own scope; `tests/parser/test_corpus_partition.py` asserts every top-level `.log` is claimed by exactly one of the two, so nothing dropped into `data/replays/` can silently fall outside both guards.

**The card census this corpus produces, and what it means for Phase 3.** `python -m parser.card_census` diffs the CSV's full pool (150 rows, 144 distinct names once reprints are merged, minus Soul (SOUL-001) — no ability text and no place in the 50 — and minus 7 `KEYWORD_ONLY_NAMES` — pure `ACT Interrupt` text with no card-specific behavior to ever script — both excluded per CLAUDE.md's Open Questions/Conventions, so the real pool is **136**) against what `cards/*.rs` actually implements (derived live from its own doc-comment headers, not hand-maintained). **The target is all 136, not the 89 the corpus references and not an earlier "~30 cards the tests need" guess** — the deck optimizer searches the full legal pool per color pair, and a card `cards/` doesn't implement is invisible to that search, so scoping the build to what today's corpus happens to reference would bake the existing meta into the search and guarantee it only ever rediscovers known decks. 14 are built; of the 122 remaining, the tool labels each with a **validation status** — `corpus-validated` (appears in ≥1 of the 64 logs, ranked by how many — 68 cards), `test-only` (zero corpus appearances, but an official QA ruling covers its card number — 36 cards), or `unvalidated` (built from `effect_text` alone, checked against nothing — 18 cards). Build order is still corpus-frequency first, but for a different reason than "covers more logs": those are the cards a Format A replay driver will actually be able to check against real games, so every later, unvalidated card gets written against engine primitives already hammered on by real replays. The `unvalidated` tier is exactly where a wrong `effect_text` reading would hide if the optimizer ever surfaces a strong deck built around one of those cards (§10.1's failure mode) — the census is what tells you that tier even exists for a given card. Also includes each card's `Lucky` flag read straight from the CSV (24 of 150 rows), since the mill loop's damage-check reads it and a wrong flag silently corrupts a life total. **A card needing a script at all is a separate question from its validation tier** — `DEFERRED_LOG_RESOLVED_NAMES` (currently Single-Shot Rifle, Reptyro) flags cards whose replay passes cleanly via generic engine-primitive dispatch (the log states the choice, so there's nothing for replay to resolve) but that still need real logic before the optimizer, which has no log to read a choice off of, can use them — printed in their own census section so a clean replay is never mistaken for done. See CLAUDE.md's Conventions section for the full three-category principle this is one instance of.

**The Format A replay driver exists now** (`format_a_replay/`, `tests/format_a_replays/`) — one fixture so far, `pingu_01` (22 turns, chosen per the corpus survey below). Built with `SCRIPTED_ACTIONS` from the start, matching `replay/`'s retrofit-learned pattern rather than repeating the mistake. It also computes hand SIZE and soul count at every "Turn opened" line, not just life totals — stricter, and what actually drove the corpus-quality findings in CLAUDE.md's Open Questions. The first version of those findings, read from log arithmetic alone, concluded repeat plays of Aurora Guide/Pal Sphere silently don't fire and that Chillet's reveal-declined persistence was Pal-only — a live-client test (played directly, not log-inferred) overturned both: the effects fire every time, the log's silence on repeats is a rendering gap in the export, not the game. That correction is now built into the driver, and it in turn makes the driver's simulated hand size diverge sharply from what pingu_01's own "Turn opened" lines state — so hand size is computed at every checkpoint but not currently asserted, pending a live-client check on whether that specific export field also lags for repeated effects. Two consecutive "TURN N" numbers sometimes belonging to the same player with no logged opposing turn in between is still open. Extending to a second fixture is the natural next step, not a redesign: the driver's own scope (see its module doc comment) is deliberately narrow to this one log so far.

**Every card with a `cards::` script must be called for real, not faked with a generic primitive.** Retagging the first 8 scripts (above) found the bracketed `replay/` driver genuinely invoking only 2 of them, with the other 6's observable effect (a draw, a graveyard move) replicated generically instead — a gap found only by grepping the driver, since every test still passed. Fixed: `replay/` now calls all 8 for real, and a small registry (`SCRIPTED_ACTIONS` in `replay/src/lib.rs`) turns a future card script silently falling back to a generic primitive into a hard panic instead. The retrofit also surfaced a genuine discrepancy, not just a labeling gap: Pengullet – Yearning for the Sky's graveyard-draw trigger now fires per its unconditional AUTO text, but battle1's own client log shows no resulting draw — recorded in `data/replays/README.md`'s known-anomaly section and asserted directly in a `replay/` unit test, rather than left as a silent, unasserted side effect. A Format A driver now exists too (`format_a_replay/`) and was built with this registry-enforcement treatment from the start rather than retrofitted — see below.

Before spending that budget, know what it buys: `card_census.py`'s `coverage_curve()` answers "how many of the 64 logs are fully replayable using only the top N cards" for every N. The curve is close to linear, not steep-early-then-flat — full coverage of one log needs *every* card that log references (an AND across its whole card set, not an OR), so one missing rare card blocks that log regardless of how many common cards are already done. It reaches 64/64 only at N=76 (this number moves down as more corpus-validated cards get built — see `python -m parser.card_census`'s own live output rather than trusting this figure to stay current). There is no natural low-N stopping point; a partial pass (say, the top 20–30 by frequency) buys partial-log validation, not a clean subset of fully-covered games, and that trade should be made deliberately rather than assumed away.

### 4.2 QA dataset (97 official rulings)

Each entry is an ambiguity the designers thought worth clarifying. Turn each into a unit test.

| QA # | Ruling | Test |
|---|---|---|
| **32** | On Deploy fires when deployed from **any** zone, including the deck | Chillet → Jormuntide must draw 1 |
| **90** | An AUTO triggered mid-resolution goes to **standby** and resolves after the current effect finishes — the trigger is not lost | Chillet chains |
| **41** | "When put into the graveyard" triggers fire **only from the base** — damage-check mills never trigger them | Leezpunk milled ≠ discard |
| **11** | You may deploy past 5 Pals, then bin down to 5 (your choice) | Long chains cost bodies |
| **62** | Relaxaurus's lock survives **card effects**, not just the stand phase | — |
| **39** | Deck temporarily hitting 0 *during* resolution is **not** a loss if ≥1 card remains after | Distinct from 11.3.2 |
| **52** | `1/Turn` means once regardless of which cost was paid | — |
| **55** | "Choose all X" includes the source card itself | Penking pumps itself |
| **56** | "Perform 〈…〉 X times" allows repeating a target; "Choose X" does not | Machine Gun vs Antique Dresser |
| **27** | Unzoned references ("Choose all cards") mean **the base**, not hand or deck | — |
| **68** | You may only declare card names that actually exist | — |
| **97** | A granted name can't satisfy a "different card names" requirement | Antique Dresser vs BP01-100 |
| **92/93** | Power ≤ 0 isn't lethal alone; Power ≤ damage already taken **is** | Astegon lines |

### 4.3 Behavioural oracle

Two working simulators exist (palworldtcg.gg, palworldtcgsim.com). **Both are closed-source and the logic is almost certainly server-side** — there is no engine to pull, and decompiling is a copyright and ToS problem.

But they are excellent *oracles*. Game rules and mechanics are not copyrightable, and Palworld published the comprehensive rules themselves.

- [ ] Build a list of ~50 contested interactions
- [ ] Play each through a sim manually, record what happens
- [ ] Turn each into a unit test

- [ ] **Also: contact the developers.** Reasonable asks: a rules-clarification list they've compiled, an export of your own match history, whether structured replay data exists.

---

## 5. Rules reference — the parts that bite

### 5.1 Deck construction (full constraint set)

| Constraint | Source |
|---|---|
| Exactly 50 cards | Comp. rules 6.1.1.1 |
| ≤4 cards with the same card name | Comp. rules 6.1.1.2 |
| ≤8 cards with the Lucky icon | Comp. rules 6.1.1.3 |
| Soul deck exactly 10 | Comp. rules 6.1.2.1 |
| **≤2 colors; colorless free in any deck** | **Quick Manual only — NOT in comp. rules** |

**Do not lose the color rule.** Absent from §6.1.1 of the newer comprehensive rules but printed on the box and enforced by every simulator. An optimizer that doesn't know it will generate four-color piles and rank them highly.

**Consequence: six independent search problems, not one.** No legal deck crosses color pairs. Run all six separately; play the six champions against each other for the final field.

| Pair | Distinct cards | Lucky-eligible |
|---|---|---|
| Red/Blue | 79 | 12 |
| Red/Green | 79 | 12 |
| Red/Purple | 80 | 12 |
| Blue/Green | 79 | 12 |
| Blue/Purple | 80 | 12 |
| Green/Purple | 80 | 12 |

The set is deliberately symmetric — 35/35/35/36 + 9 colorless, and **exactly 12 Lucky-eligible cards in every pair**. The Lucky-cap decision (which 8 of 12, at what average cost) is structurally identical in all six.

### 5.2 Turn structure

- Phases: Stand → Draw → Soul → Main → End
- **Player going first skips their first draw phase** (7.3.1)
- Souls +2/turn, cap 10. P2 places 1 at setup (6.2.1.5)
  - P1: 2 / 4 / 6 / 8 / 10 · P2: 3 / 5 / 7 / 9 / 11 → capped at 10
- **Souls cap out around turn 5.** After that the only sink is the draw action below.
- **Rest 3 souls → draw 1. Once per turn** (8.5), both players, every turn. QA #96: this does *not* count as "playing a card."
- Mulligan: all-or-nothing, **redraw 5 (no penalty)**, once per game, P1 declares first (6.2.1.7)
- **Stand phase stands only the turn player's cards** (7.2.1) — a Pal that ends your opponent's turn rested is attackable on yours

### 5.3 Damage and the Lucky check (11.2.2)

Attacking a player with Strike N:
1. Mill top card to graveyard
2. **If it has the Lucky icon → stop, cancel ALL remaining damage, set damage to 0**
3. If cards milled ≥ N, or deck is empty → reduce life by N
4. Otherwise return to step 1

Overall cancel rate ~30%, rising with Strike. Strike 3 is cancelled ~37% of the time.

### 5.4 Combat (§9)

- Attack targets: the opposing player, their structures, or their **rested** Pals (9.2.3)
- **Standing Pals cannot be attacked.** Confirmed in the Quick Manual Q&A.
- Blocking: non-turn player rests a standing Pal to become the target (9.4.2)
- Pals **can attack the turn they are deployed** (9.2.2.3)
- Pal limit 5; no limit on structures and gears (4.4.5.2, QA #12)
- **Each attack is a complete battle** (9.3 → 9.7); only then does 9.7.4 return the turn player to their play timing. The defender commits a blocker *before* the attacker decides on a second attack.
- Damage accumulates on a Pal within a turn and resets at end phase (5.14.1, 7.6.3)

### 5.5 Keyword traps

| Keyword | The trap |
|---|---|
| **Interrupt** | Nullifies **attacks only** (12.8.2). Blank against On Deploy damage, ACT abilities, and events. Cost is `[①, discard this]` **OR** `[discard this + 1 other card]` — a player with zero souls can still interrupt. |
| **Taunt** | Only compels targeting if the card **can** be targeted (12.10.2), and standing Pals can't be (9.2.3). **A standing Taunt Pal taunts nothing.** |
| **Breakthrough** | Deals damage equal to **this card's Strike** (12.14.2) — NOT excess/overflow damage. |
| **Retaliate** | Triggers on being put in the graveyard **during a battle** (12.12) |
| **Assault** | Lets this card target **standing** Pals — the exception to 9.2.3 |
| **Nocturnal** | +300 power only while the "night" specific state is active (5.3, 12.13) |

### 5.6 Timing model (must be right)

- **Trigger ≠ resolve.** A fulfilled trigger condition puts the ability into **standby state** immediately (10.8.2), even mid-resolution.
- Standby abilities resolve at the next **check timing** (10.5.3.2), turn player first.
- **Players MUST play all standby abilities** — not optional (10.8.3.1)
- A standby ability resolves even if its source card changed zones (10.8.7)
- Check timing order: rule actions first, repeat until none, then automatic abilities (10.5.3)
- Play timing is always preceded by a check timing (10.5.4.1)
- Rule actions are **interrupt-type** (immediate, e.g. player damage) or **check-type** (only at check timing, e.g. lethal damage, losing verdict) — 11.1.2

### 5.7 Numeric edge cases

- **Power is unclamped** — store signed, never clamp at 0 (1.3.2.4, QA #92/93)
- **Life** below 0 is *referenced* as 0 (3.2.1.2)
- **Resources** below 0 *become* 0 (3.3.3.1)

Three different behaviours in one rules document. Test all three.

### 5.8 Element data

All 92 Pals classified in `11cardattributes-with-elements.csv`. Nine elements: Neutral, Fire, Water, Grass, Electric, Ground, Ice, Dark, Dragon.

**Only 9 Dragons exist**, all ◇8 or less (so Chillet can deploy any of them): Elphidran – Gentle Radiance ◇4, Dinossom ◇4, Chillet ◇5, Azurobe ◇6, Elphidran Aqua ◇6, Relaxaurus ◇7, Jormuntide Ignis ◇8, Jormuntide ◇8, Astegon ◇8.

---

## 6. Data hygiene rules

Each of these has already caused an error.

- [ ] **Filter out rows with `soul` in `card_slug`** before any deck-size or decklist-hash work
- [ ] **Control for color pair before quoting any card win rate.** Raw rates are dominated by which archetypes play the card and have reversed several conclusions
- [ ] **The JSON ladder dataset beats the ladder xlsx** wherever they disagree
- [ ] **State sample sizes.** Many sources are thin enough that direction is all the data supports
- [ ] Never present a constructed decklist as a verified archetype list

---

## 7. Checkpointing and server coexistence

The box also runs a Palworld game server. A ~24-hour optimizer run must yield to players and survive a power cut without losing the run.

**Cooperative checkpointing, not OS-level suspend.** `pssuspend` freezes a process but leaves everything in volatile memory — a power cut loses the whole run. Writing state to disk between batches survives anything.

### 7.1 Control protocol

One file, `control/state.txt`, containing one word:

| Value | Meaning |
|---|---|
| `RUN` | proceed normally |
| `PAUSE` | checkpoint and exit cleanly (0); the watcher restarts it later |
| `STOP` | checkpoint and exit; do not auto-restart |

The simulation polls this **between batches**, never mid-batch. Size a batch to a few seconds of work so pause latency stays low — for the optimizer that's a chunk of pairings, not a whole generation.

### 7.2 Rust side

```rust
use std::fs;
use rayon::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Checkpoint {
    pub run_seed: u64,
    pub generation: u32,
    pub pairing_index: usize,
    pub field: Vec<DeckId>,
    pub results: Vec<MatchResult>,
}

#[derive(PartialEq)]
pub enum Control { Run, Pause, Stop }

pub fn read_control() -> Control {
    match fs::read_to_string("control/state.txt")
        .unwrap_or_else(|_| "RUN".into())
        .trim().to_uppercase().as_str()
    {
        "PAUSE" => Control::Pause,
        "STOP"  => Control::Stop,
        _       => Control::Run,
    }
}

/// Atomic write: temp file + rename. A crash mid-write leaves the previous
/// checkpoint intact rather than a half-written one.
pub fn save_checkpoint(cp: &Checkpoint) -> std::io::Result<()> {
    fs::create_dir_all("checkpoint")?;
    let tmp = "checkpoint/latest.json.tmp";
    fs::write(tmp, serde_json::to_vec_pretty(cp)?)?;
    fs::rename(tmp, "checkpoint/latest.json")?;   // atomic on NTFS
    Ok(())
}

pub fn load_checkpoint() -> Option<Checkpoint> {
    fs::read_to_string("checkpoint/latest.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn run_optimizer() -> anyhow::Result<()> {
    let mut cp = load_checkpoint().unwrap_or_else(Checkpoint::fresh);
    println!("resuming at gen {} pairing {}", cp.generation, cp.pairing_index);

    while cp.generation < TARGET_GENERATIONS {
        let pairings = build_pairings(&cp.field);

        while cp.pairing_index < pairings.len() {
            match read_control() {
                Control::Pause => { save_checkpoint(&cp)?; println!("paused");  return Ok(()); }
                Control::Stop  => { save_checkpoint(&cp)?; println!("stopped"); return Ok(()); }
                Control::Run   => {}
            }

            let end = (cp.pairing_index + BATCH_SIZE).min(pairings.len());
            let batch = &pairings[cp.pairing_index..end];

            // seed per pairing from the run seed so results are reproducible
            // across resumes — a resumed run is bit-identical to an unbroken one
            let out: Vec<MatchResult> = batch.par_iter().enumerate()
                .map(|(i, p)| play_matches(p, cp.run_seed ^ ((cp.pairing_index + i) as u64)))
                .collect();

            cp.results.extend(out);
            cp.pairing_index = end;
            save_checkpoint(&cp)?;
        }

        cp = advance_generation(cp);
        save_checkpoint(&cp)?;
    }
    Ok(())
}
```

Three things make this outage-proof:

- **Atomic writes** — temp + rename, so a crash never leaves a corrupt checkpoint
- **Per-pairing seeds derived from a run seed** — a resumed run produces identical results to an unbroken one, so a bug found in game #418,203 stays reproducible
- **Checkpoint after every batch** — worst-case loss is one batch, not one generation

### 7.3 The watcher script

`scripts/watch-server.ps1`. Requires `RCONEnabled=True` in `PalWorldSettings.ini`.

```powershell
$RconHost = "127.0.0.1"; $RconPort = 25575; $RconPass = "yourpassword"
$SimExe   = ".\target\release\palsim.exe"
$Control  = "control\state.txt"
$PollSec  = 30

New-Item -ItemType Directory -Force -Path "control" | Out-Null

function Get-PlayerCount {
    try {
        $out = & mcrcon -H $RconHost -P $RconPort -p $RconPass "ShowPlayers" 2>$null
        # first line is the CSV header; each further non-empty line is a player
        return (($out -split "`n") | Where-Object { $_.Trim() } | Measure-Object).Count - 1
    } catch {
        return 1    # RCON unreachable: fail safe, assume players are on
    }
}

while ($true) {
    $players = Get-PlayerCount
    $running = Get-Process -Name palsim -ErrorAction SilentlyContinue

    if ($players -gt 0) {
        if ($running) {
            Set-Content $Control "PAUSE"
            Write-Host "$(Get-Date -f HH:mm:ss)  $players online -> PAUSE"
            $running.WaitForExit(60000) | Out-Null   # give it time to checkpoint
        }
    } else {
        Set-Content $Control "RUN"
        if (-not $running) {
            Write-Host "$(Get-Date -f HH:mm:ss)  server empty -> resuming sim"
            Start-Process $SimExe -ArgumentList "optimize" -PriorityClass Idle
        }
    }
    Start-Sleep -Seconds $PollSec
}
```

Note the **fail-safe on RCON error**: if the query fails, assume players are online and pause. Better to lose sim time than to degrade someone's game.

### 7.4 Belt and braces

The watcher has up to a 30-second blind spot between polls. Cover it so the server never starves even in that window:

```powershell
$env:RAYON_NUM_THREADS = "12"     # of 16, leaves headroom on 8C/16T
```

Or pin explicitly:

```powershell
(Get-Process palsim).ProcessorAffinity    = 0x0FFC   # sim off threads 0-1
(Get-Process PalServer).ProcessorAffinity = 0x0003   # server on threads 0-1
```

The sim already starts at `Idle` priority in the script above, so the scheduler favours the server under any contention.

- [ ] Register `watch-server.ps1` as a Scheduled Task at logon so it survives reboots
- [ ] Test the loop end to end: connect to the server, confirm the sim checkpoints and exits; disconnect, confirm it resumes at the right pairing
- [ ] Test the outage path: kill the sim mid-batch, restart, confirm it resumes and that `latest.json` was never corrupt

### 7.5 Expected impact

Light server use means most of a 24-hour run proceeds uninterrupted. Budget **~31 hours wall-clock** for 20 generations with typical pausing. All six color pairs at 10 generations each, run sequentially, is about **3 days** — which is why the two-tier approach matters: screen broadly with cheap playouts, spend ISMCTS only on finalists.

---

## 8. Phase plan

Each phase has a real exit point. Stopping after Phase 3 still delivers value.

### Phase 1 — Log parser (days) — DONE for the 64 logs in hand, two grammars

**Do this first, before the engine.** Output: turn, phase, actor, action, targets, resolution.

Independently useful — turns replays into analysable data regardless of whether the engine ships, and forces you to nail down the state representation before writing rules code. Turned out to be two independent parsers, not one — see §4.1 for why, and it's not over: 207 URLs are catalogued against 64 logs actually parsed, so more Format A volume is still expected, and each new batch is one `python -m pytest tests/parser/` away from telling you whether it needs new grammar or just slots in.

- [x] Bracketed `[Tn phase]` grammar (`replay_parser.py`) — start with the three known logs; you know what correct output looks like
- [x] Handle interleaved `[Tn waiting]` / `[Tn main]` tagging — turn numbers don't always increase monotonically
- [x] Format A grammar (`format_a_parser.py`) for the site's own narrative export — a second grammar, not an extension of the first; see §4.1's three attribution traps before touching this one
- [x] Card census (`card_census.py`) — what Phase 3 actually needs to build, and in what order; see §4.1
- [ ] **Exit value:** structured replay data, queryable — met for 64 logs; re-run as more of the 207 catalogued URLs get pulled

### Phase 2 — Test harness (days)

- [ ] 97 QA rulings → unit tests
- [ ] Replay corpus → regression tests (currently: 3 bracketed fixtures fully engine-validated; the 64 Format A logs are parsed and structurally clean, and 1 of them — `pingu_01` — is now run through the engine too, via `format_a_replay/`; the other 63 aren't yet)
- [ ] ~50 oracle observations from a live sim → unit tests

**Build this before the engine.** With it, agentic coding works: write effect, run suite, see failure, fix. Without it, the agent writes plausible code and you don't find out it's wrong until the optimizer recommends a deck that loses to everything.

- [ ] **Exit value:** a specification of correct behaviour, in executable form

### Phase 3 — Rules engine + card scripts (weeks)

**The real commitment, and where enthusiasm dies.** Budget accordingly.

- [ ] Zones, phases, souls, check timing, standby state
- [ ] Damage resolution with Lucky mill
- [ ] Quick step and interrupts
- [ ] Card effect scripts — **the target is all 136 distinct names (150 CSV rows, 144 distinct names minus Soul (SOUL-001) — no ability text, no place in the 50 — minus 7 `KEYWORD_ONLY_NAMES`, pure `ACT Interrupt` text with no card-specific behavior to ever script), not the 89 the corpus references** — the optimizer searches the full legal pool, and an unbuilt card is invisible to that search, guaranteeing only known decks get rediscovered. `python -m parser.card_census` ranks the remaining 122 into `corpus-validated` (68, build first — checkable against real games) / `test-only` (36, a QA ruling covers the number but no replay evidence) / `unvalidated` (18, checked against nothing, §10.1's failure mode lives here). Its coverage curve (how many of the 64 logs replay fully at N corpus-validated cards) is close to linear with no low-N elbow — that curve measures replay-validation depth, not the optimizer's legal-pool requirement, which is all 136 regardless of what's validated. **Whichever driver replays these must call each card's real script, not a generic primitive that fakes its visible result** — both `replay/` and `format_a_replay/` build this in via a `SCRIPTED_ACTIONS` registry (`replay/src/lib.rs`, `format_a_replay/src/lib.rs`): a registered card reaching a generic fallback path is a hard panic, so "corpus-validated" stays structurally true rather than aspirational. That's a different question from whether a card needs a script AT ALL, though — `DEFERRED_LOG_RESOLVED_NAMES` (Single-Shot Rifle, Reptyro so far) tracks cards whose replay passes cleanly with no script because the log itself resolves the choice, which the optimizer won't have; see CLAUDE.md's Conventions section
- [ ] **Exit value:** with a random-playout agent, deck screening plus Monte Carlo answers to open questions

### Phase 4 — ISMCTS agent (1–2 weeks)

| Approach | Cost | Quality |
|---|---|---|
| PIMC / determinization | Low | Strategy fusion, but often fine |
| **ISMCTS** | Moderate | **Probably the sweet spot** |
| ReBeL-style (belief + CFR) | High | Overkill here |

Branching factor is small — 5 Pal slots, hand of 5–8, souls capped at 10, games end by turn 10. **ISMCTS with random rollouts and no neural network is likely strong enough.**

- [ ] Seedable PRNG per worker
- [ ] **Exit value:** the move advisor becomes real

### Phase 5 — Checkpointing and server coexistence (1–2 days)

Build this **before** the first long optimizer run, not after losing one.

- [ ] `control/state.txt` protocol and `read_control()`
- [ ] Atomic checkpoint save/load, batches sized to a few seconds
- [ ] Per-pairing RNG seeds derived from a run seed, so resumed runs are bit-identical
- [ ] `watch-server.ps1` plus a Scheduled Task at logon
- [ ] Test the pause path (connect / disconnect) and the outage path (kill mid-batch)
- [ ] **Exit value:** you can start a multi-day job and walk away

### Phase 6 — Deck optimizer (days, given 1–5)

- [ ] **Seed the population with real decks** — ~100 championship lists and 56 ladder archetypes. Don't search from scratch.
- [ ] Iterative best-response / double oracle: maintain a field, find the deck that beats it, add it, repeat
- [ ] Mutation operators: swap a card, adjust counts 3↔4, respect the Lucky cap as a hard constraint
- [ ] Run the six color pairs as separate jobs
- [ ] **Exit value:** the original goal

---

## 9. Known unknowns

- [ ] **Deck-out vs life-zero** — settled: both are check-type rule actions (11.3.3) resolving simultaneously (11.1.2.4). Same player hitting both loses either way; both players at once = draw (1.2.2). Encode explicitly.
- [ ] **Where is the 2-color rule formally published?** Box and Quick Manual confirmed. Check tournament regulations in case other format rules are missing.
- [ ] **Declaring a bare main name** — 5.19.1 permits declaring "a card name, main name, or sub name," but Antique Dresser says "Declare 1 card name." Unresolved; doesn't matter for known cards.
- [ ] **Quick cards on your own turn** — QA #29's summary sentence, read literally, would bar it entirely. Almost certainly means "during a battle." Resolve before hard-coding.
- [ ] **BP02 "Legends Awaken" lands 2026-10-30.** Design card scripts and the element schema so a new set is a new module, not a refactor.

---

## 10. Failure modes

### 10.1 The optimizer overfits to the agent's weaknesses

**The most common way these projects produce confident garbage.** If the agent times interrupts badly, it systematically undervalues interrupt-heavy decks and recommends lists that lose to real people.

**Mitigation:** sanity-check output against championship data. If it declares a deck great that no real player has ever run, **suspect the agent before believing the result.**

### 10.2 A plausible engine that's subtly wrong about timing

A skipped check timing or dropped standby trigger produces a simulator that runs fine and ranks decks wrongly. No symptom until the recommendations are bad.

**Mitigation:** the Phase 2 harness. This is why it comes first.

### 10.3 "Optimal" is underdefined

In a two-player zero-sum imperfect-information game, an equilibrium strategy is **unexploitable**, not **maximally winning**. Against a known field, an exploitative agent beats the equilibrium one.

**Your real question is probably not "what's the best deck in the abstract."** It's "what beats the Blue/Purple and Blue/Red Suzaku decks I actually face." Modelling a five-deck field is dramatically cheaper, converges in hours rather than days, and answers the question you have.

---

## 11. First session checklist

- [ ] Build tools → Rust → Git → Python → Sysinternals → mcrcon → Claude Code installed and verified
- [ ] Defender exclusion on `target/`
- [ ] `.cargo/config.toml` in place; AVX-512 confirmed via `rustc --print target-features`
- [ ] Rayon smoke test saturates all cores
- [ ] Repo scaffolded per §2, including `control/` and `checkpoint/`
- [ ] `CLAUDE.md` written per §3
- [ ] `data/` populated with the CSVs, QA JSON, and replay logs
- [ ] RCON enabled on the Palworld server; `mcrcon` verified from the command line
- [ ] Point Claude Code at the three known replay logs and build the parser

---

## Appendix — legal boundaries

- **Game rules and mechanics are not copyrightable.** Palworld published the comprehensive rules. Implementing a simulator is fine.
- **Their code is copyrighted.** Do not decompile client bundles to lift effect implementations. Likely a ToS violation as well.
- **Card names, text, and art are Bushiroad / Pocketpair IP.** Fine for private analysis; don't redistribute.
- **Bulk scraping likely violates ToS.** The official QA API you already used is a documented public endpoint — that's the good case.
- Observing a public simulator's behaviour by playing it is just playing a game.
