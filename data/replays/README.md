# Replay corpus

Client logs from palworldtcg.gg, used as regression tests for the rules engine.
Replay each log through the engine and assert state matches at every line.
A failure means the engine disagrees with the actual game — investigate it,
do not weaken the test.

## Files

| File | Deck | Opponent | Result |
|---|---|---|---|
| `battle1_bg-chillet_vs_blue-purple_WIN.log`  | Blue/Green Chillet | Blue/Purple | WIN T5 |
| `battle2_bg-chillet_vs_blue-purple_LOSS.log` | Blue/Green Chillet | Blue/Purple | LOSS T10 |
| `battle3_bg-chillet_vs_blue-red_WIN.log`     | Blue/Green Chillet | Blue/Red    | WIN T7 |

Add `tier1_format_a_all34.zip` and `replay_logs.zip` (~165 logs) here when extracted.

## Parser notes — read before writing the parser

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

## Known anomaly

Battle 3, T7: Jormuntide Ignis attacks, re-stands via its ACT, attacks again
(nullified), then BLOCKS on Edify's turn 7. Per 7.2.1 the stand phase stands
only the turn player's cards, so Ignis should still have been rested and unable
to block. Either the client stood it somewhere the log doesn't show, or this is
a client bug. Resolve before trusting the engine's rest-state model.
