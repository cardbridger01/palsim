# Replay corpus

Client logs from palworldtcg.gg, used as regression tests for the rules engine.
Replay each log through the engine and assert state matches at every line.
A failure means the engine disagrees with the actual game — investigate it,
do not weaken the test.

**Two structurally unrelated log grammars live here**, each with its own
parser and its own corpus-guard test — read the section for whichever one
you're touching, they don't generalize to each other:

- **Bracketed `[Tn phase]` format** (below) — 3 hand-authored fixtures,
  `parser/replay_parser.py`, `tests/parser/test_corpus.py`.
- **Format A** (its own section further down) — the site's own raw
  narrative export, 64 real logs, `parser/format_a_parser.py`,
  `tests/parser/test_format_a_corpus.py`.

207 match URLs are catalogued as sources in total; 64 have been pulled and
parsed so far. More Format A volume is expected — see
`docs/palsim-build-handbook.md` §4.1.

## Bracketed format: files

| File | Deck | Opponent | Result |
|---|---|---|---|
| `battle1_bg-chillet_vs_blue-purple_WIN.log`  | Blue/Green Chillet | Blue/Purple | WIN T5 |
| `battle2_bg-chillet_vs_blue-purple_LOSS.log` | Blue/Green Chillet | Blue/Purple | LOSS T10 |
| `battle3_bg-chillet_vs_blue-red_WIN.log`     | Blue/Green Chillet | Blue/Red    | WIN T7 |

## Bracketed format: parser notes — read before touching replay_parser.py

1. **Turn numbers are not monotonic.** `[Tn waiting]` lines describe the
   OPPONENT's turn and can carry a LOWER turn number than surrounding
   `[Tn main]` lines. Example, battle 3:
       [T7 waiting] FluffysFury's Jormuntide Ignis attacks Edify for 4.
       [T6 waiting] Edify interrupted with Dinossom, paid 1 Soul.
   Order by line position, never by the bracketed turn number.

2. **Two actor perspectives.** `main` / `stand` / `draw` / `soul` / `end` are
   the logging player's turn. `waiting` is the opponent acting.

3. **Damage check format** carries the Lucky outcome:
       "Damage Check: N cards milled, no Lucky. They X to Y life."
       "their Damage Check revealed a Lucky Pal, cancelled, no damage. (N cards milled)"

4. **Chillet resolution** appears as a reveal followed by either a deploy or
   an add-to-hand:
       "Revealed <card>." then "Deployed <card>."      → free deploy
       "Revealed <card>, added to hand."               → declined or not a Dragon
   Any On Deploy of the free-deployed Pal follows immediately
   ("Drew 1.", "No valid target.").

5. **Slot numbers are base-wide**, covering structures and gears as well as
   Pals — slot 6 appears despite the 5-Pal limit (4.4.5.2).

6. **Rule 8.5 draw** logs as "Rested 3 Souls, drew 1 card."

## Bracketed format: known anomalies

Battle 3, T7: Jormuntide Ignis attacks, re-stands via its ACT, attacks again
(nullified), then BLOCKS on Edify's turn 7. Per 7.2.1 the stand phase stands
only the turn player's cards, so Ignis should still have been rested and unable
to block. Either the client stood it somewhere the log doesn't show, or this is
a client bug. Resolve before trusting the engine's rest-state model.

Battle 1: Pengullet – Yearning for the Sky (BP01-028) has its own AUTO
"When this card is put into the graveyard, draw 1 card" — unconditional, no
"may." Its holder self-destructs via the Pengullet Rocket Launcher's granted
sweep skill (a base -> graveyard transition, QA #41's condition), so the
trigger should fire — but the log records no "drew 1 card" line for the
holder's owner anywhere near that point. `replay/`'s driver fires the trigger
for real anyway (following the rules text, not the log's silence) and a unit
test (`rocket_launcher_act_then_granted_sweep_via_power_buff_and_activate_events`,
`replay/src/lib.rs`) asserts the resulting hand-size increase directly, so
this divergence from the log is a checked, documented fact rather than an
unasserted side effect. Either the client doesn't display this specific
draw, or doesn't implement it — same category of client anomaly as the
Jormuntide Ignis case above, not evidence the card's own implementation is
wrong.

---

## Format A: files

64 real site logs across two on-disk shapes that share one grammar:

- `*.log` directly in this directory (44 files) — split from
  `COMBINED_TIER1_TIER2_44_MATCHES.txt` (`parser/split_combined_matches.py`,
  a one-off tool; the combined file's own delimiter, not assumed, is
  documented in that script). No hand-reveal data.
- `top_players_all20/*.txt` (20 files, plus `match_list.tsv`, a manifest of
  player/recordId/opponent/date/listed_result) — 10 games each from two
  players ("pingu", "ze"). Every turn opens with `Turn opened — Souls N/N ·
  Hand: card, card, ...`, revealing one seat's hand and soul count in full —
  the richest thing in this corpus, and the reason it's the right fixture set
  for engine-side hand/draw validation and eventual hidden-information search
  work, not just life-total checks.

Shape: bare `TURN N` lines — an ALL-CAPS (or otherwise bare) actor-name line
opens a block, every subsequent line belongs to that actor until the next
such line; a line with its own explicit subject ("Pingu interrupted
with...") overrides the block actor for that one line. No phase tag exists —
unlike the bracketed format, there is no fixed "logging player" perspective;
either named participant can be the block actor, in either order, and the
same participant can go by more than one name (see traps below). Full
grammar, patterns, and the reasoning behind every judgment call live in
`parser/format_a_parser.py`'s own module doc comment — this section covers
only what cost real time to work out and would otherwise have to be
rediscovered from scratch.

## Format A: three attribution traps

Actor identity in Format A is NOT safe to derive by pattern-matching which
name gets used as a line-prefix most often, or by matching a name against
`match_list.tsv`'s own `player`/`opponent` columns. Three real, observed
reasons why, found by working through the `top_players_all20` batch:

1. **Prefix-majority looks like a rule but isn't one.** In most files, the
   *opponent's* name is the one that appears constantly as a line-prefix
   (`"Chylith deployed Chillet."`, `"Chylith's Chillet attacks Pingu for
   2."`), while the recording player's own actions are usually bare
   (`"Deployed Pengullet Rocket Launcher..."`, no subject — inherited from
   the block header instead). Treat this as an observed tendency to sanity-
   check against, never as the actual identity rule: an explicit subject in
   the text always overrides the block header, in *either* direction, and
   nothing stops a game from being unusually one-sided in which player's
   name gets narrated explicitly.

2. **The account name and the in-game display name can differ.** Ze's
   leaderboard/profile name is `"ze"`; every log line, and the in-game HUD,
   shows the account's display name `"bel"` instead. `match_list.tsv`'s
   `player` column says `ze`; the log body never says `ze` once. A parser
   matching actor identity against the manifest's own player name would find
   zero occurrences and misattribute every line. Pingu has no such alias
   (account name and display name are both "Pingu") — the difference isn't
   visible from the file *names*, only from each match's own header line
   (`MATCH: Ze (in-game name "bel") vs ...`).

3. **The opponent can be shown as the literal word "Opponent."** Some
   accounts have a site-side anonymization/privacy setting that replaces
   their real name with the literal English word `"Opponent"` — in the log
   body, not just the UI. Confirmed in both the Pingu batch (`pingu_07`, vs
   "poi") and the Ze batch (`ze_04`, `ze_09`, vs "amata" and "兔绣袖"), so
   it's a site-wide behavior, not tied to one account or one player's own
   games. The real name from `match_list.tsv` will never appear in the body
   at all for these — `"Opponent"` has to be a recognized, expected actor
   identity in its own right, not an unresolved failure.

**The fix common to all three:** derive the seat from the log body itself —
each match's own declared participants, read off its header line, plus the
literal `"Opponent"` placeholder — never by matching against
`match_list.tsv`. Cross-check against the manifest by `recordId` afterward,
and treat a mismatch as an error to investigate, not a silently-accepted
fallback. `format_a_parser.py`'s `_participant_aliases()` is exactly this:
it reads both the leading name and any quoted alias off the `MATCH:` line
per match, so `bel`, `Ze`, and `Opponent` are all valid identities for the
same two-participant match without any name ever being hardcoded.

## Format A: provenance matters, and it's recorded per file

Four files — `ze_01`, `ze_03`, `ze_08`, `pingu_06` — were reconstructed from
the raw `[T<N> phase]`-tagged event export after a site bug (the curated log
panel silently evicts early turns once a long game is scrubbed to its end),
rather than transcribed from the normal rendered/curated view the other 60
files use. They're closer to source of truth, and they're also the only
files where a sentence can be physically word-wrapped across two source
lines — `format_a_parser.py`'s word-wrap join is deliberately gated to
`provenance == "reconstructed"` for exactly this reason, not applied
corpus-wide. Every parsed file's `meta` carries `provenance`
(`"curated"`/`"reconstructed"`) and `gap_expectation` (`"clean"` if the
file's own header claims every life-total change is accounted for by named
damage lines, `"unstated"` otherwise) — both derived from each file's own
header text, not hardcoded, so re-parsing stays honest as more logs are
added. `ze_01`/`ze_03`/`ze_08` are exactly the three files the *other* files
name as the "unlike these, this game has no unexplained gap" comparison
cases — a testable prediction for whenever something can sum named damage
against stated life totals: those three should fail that audit, the
`gap_expectation == "clean"` files should pass it.

## Format A: what the corpus needs from `cards/` — and what it doesn't decide

`python -m parser.card_census` diffs the CSV's full 150-row pool (144
distinct names) against `cards/*.rs`. **The build target is all 144, not
just what this corpus references** — the deck optimizer searches the full
legal pool per color pair, and a card `cards/` doesn't implement is
invisible to that search, so scoping to the corpus's own 89 referenced
names would bake the existing meta into the search and guarantee it only
ever rediscovers known decks.

What the corpus *does* decide is build **order**, and validation
**confidence** — every not-yet-implemented name is labeled
`corpus-validated` (appears in ≥1 of the 64 logs — 81 cards, build these
first, since they're the ones an eventual Format A replay driver can check
against real games), `test-only` (zero corpus appearances but an official
QA ruling covers its card number — 36 cards), or `unvalidated` (built from
`effect_text` alone, checked against nothing — 19 cards). The `unvalidated`
tier is exactly where a wrong `effect_text` reading would hide if the
optimizer ever surfaces a strong deck built around one of those cards
(handbook §10.1) — worth knowing which tier a card is in before trusting
the optimizer's read on it.

`coverage_curve()` (tier 1 only, i.e. corpus-validated cards) answers "how
many logs are fully replayable with only the top N of those" — close to
linear with no natural low-N elbow, since full coverage of one log needs
*every* card it references. That's a statement about replay-validation
depth, not about how many cards the optimizer needs built — that number is
144 regardless of how many are ever exercised by a real log.
