"""Hand-verified fixture test for the Format A parser -- same discipline
as test_format_a_pingu01.py/test_format_a_pingu03.py: every expected
event below was built by reading
data/replays/top_players_all20/pingu_02_3e67de53.txt line by line,
independently of parser/format_a_parser.py's own output, before the
parser was confirmed against it.

pingu_02 is the third fixture, chosen deliberately to change the
OPPONENT (redpurple, not Chylith) while pingu_01/03 held it constant --
tests the card-POOL axis pingu_03 was scoped not to touch. 17 turns,
curated provenance, a clean life-gap header claim cross-checked the
same way the other two were.

## A real parser bug this fixture found -- an ACTOR corruption, not just
## a card-name one

Line 121 ("redpurple looked at 5 cards and deployed Foxparks'
Harness.", BP01-005 Reptyro's own ACT narrated as one combined
sentence) has no dedicated pattern -- before this fixture, the closest
thing was `look_and_take`'s three OTHER phrasings ("looked at N and
took M into hand", "Looked at top N: took X to hand[. Graveyard:
...]", "Looked at top N: deployed X."), none of which match "X looked
at N cards and deployed Y." Running the parser directly against this
line, PRE-FIX, showed the line falling through to the generic bare
`deploy` pattern -- and because `_SUBJ`'s character class (word chars,
apostrophe, space, period, hyphen) has no concept of "this isn't a
real subject name," it backtracked across the ENTIRE "redpurple looked
at 5 cards and" prefix to reach this sentence's OWN "deployed", and
`actor` became the literal string "redpurple looked at 5 cards and"
instead of "redpurple". Worse than pingu_03's power_buff bug in one
way: that one corrupted a card name; this one corrupted the actor
field itself, which every other event's ownership resolution depends
on. Fixed with a new, more specific `look_and_take` pattern placed
before the generic `deploy` fallback -- see
`parser/format_a_parser.py`'s own comment on it. Confirmed via the
pre-fix parser run directly (`actor` and `action` both wrong), not
inferred from reading the regex alone; `tests/parser/` stays green
throughout (112 passed).

`phase` is always None for Format A -- there is no phase-tag concept in
this format, only the block actor (see format_a_parser's own module doc
comment for what `actor` means and when a line overrides it).
"""

from parser.format_a_parser import parse_log

EXPECTED_META = {
    "match_title": "Pingu vs redpurple  [Pingu's match #2 of 10 pulled]",
    "recordId": "3e67de53-ec3e-4fc8-a18d-a5a9eab427f8",
    "result": 'PINGU WINS - "redpurple\'s life hit zero." Final life: Pingu (WINNER) 7, redpurple 0.',
    "notes": [
        "Both life totals are fully accounted for by named damage lines — Pingu drops 10->9->8->7 "
        "via three single-point Foxparks hits (turns 1, 3, 15); redpurple drops 10->8->5->3->0 via four "
        "named hits (turns 11 x2, 14, 17) — NO unexplained gap, same as Pingu's match 1. Two for two "
        "clean games to open the Pingu dataset.",
        'Deck labeled "BLUE/PURPLE DRAGON CONTROL" on Pingu\'s HUD, same as match 1.',
        "2 bare-form interrupts (card name on its own line, no player/cost) preserved verbatim "
        "below for decklist cross-check: Turn 5: Pyrin Noct interrupts Jormuntide's attack Turn 8: "
        "Pyrin Noct interrupts Relaxaurus's attack 4 additional FULL-form interrupts occurred (Turn 6 "
        '"Pingu interrupted with Reindrix, discarded Pal Sphere"; Turn 9 "Pingu interrupted with '
        'Reindrix, paid 1 Soul"; Turn 12 x2, "Pingu interrupted with Pyrin Noct, paid 1 Soul" and '
        '"Pingu interrupted with Reindrix, paid 1 Soul") — not counted in the bare-form total, '
        "included in the verbatim body only.",
        "redpurple is the same opponent name seen in two of Ze's games (ze_02, ze_08's context) — "
        "not confirmed to be the literal same account without a profile lookup, but worth flagging as "
        "a recurring name across both Ze's and Pingu's recent opponent pools.",
    ],
    "provenance": "curated",
    "gap_expectation": "clean",
    "opponent_anonymized": False,
    "word_wrap_joins": 0,
}

# (line_index, turn, phase, actor, action, cards, targets, resolution, caused_by_line)
EXPECTED_EVENTS = [
    (24, 1, None, "redpurple", "turn_opened", [], [], {"souls": 1, "souls_cap": 1, "hand": [
        "Chillet", "Pengullet", "Pyrin Noct", "Strike from the Darkness", "Strike from the Darkness",
    ]}, None),
    (25, 1, None, "redpurple", "setup", [], [], None, None),
    (26, 1, None, "redpurple", "mulligan", [], [], None, None),
    (27, 1, None, "redpurple", "deploy", ["Foxparks"], [], None, None),
    (28, 1, None, "redpurple", "attack_declare", ["Foxparks"], ["Pingu"], {"damage": 1}, None),
    (29, 1, None, "redpurple", "attack", ["Foxparks"], ["Pingu"],
     {"damage": 1, "milled": 1, "lucky": False, "life_before": 10, "life_after": 9}, None),
    (32, 2, None, "redpurple", "turn_opened", [], [], {"souls": 3, "souls_cap": 3, "hand": [
        "Aurora Guide", "Chillet", "Pal Sphere", "Pyrin Noct", "Reindrix", "Relaxaurus",
    ]}, None),
    (33, 2, None, "redpurple", "rest_draw", [], [], {"souls_spent": 3, "drew": 1}, None),
    (34, 2, None, "redpurple", "deploy", ["Foxparks' Harness"], [], None, None),
    (37, 3, None, "redpurple", "turn_opened", [], [], {"souls": 5, "souls_cap": 5, "hand": [
        "Aurora Guide", "Chillet", "Jormuntide", "Pal Sphere", "Pal Sphere", "Pyrin Noct", "Reindrix",
        "Relaxaurus",
    ]}, None),
    (38, 3, None, "redpurple", "play_card", ["Pal Sphere"], [], {"cost": 4}, None),
    (39, 3, None, "redpurple", "draw", [], [], {"amount": 3}, None),
    (40, 3, None, "redpurple", "attack_declare", ["Foxparks"], ["Pingu"], {"damage": 1}, None),
    (41, 3, None, "redpurple", "attack", ["Foxparks"], ["Pingu"],
     {"damage": 1, "milled": 1, "lucky": False, "life_before": 9, "life_after": 8}, None),
    (44, 4, None, "redpurple", "turn_opened", [], [], {"souls": 7, "souls_cap": 7, "hand": [
        "Aurora Guide", "Aurora Guide", "Chillet", "Jormuntide", "Pal Sphere", "Pyrin Noct", "Reindrix",
        "Reindrix", "Reindrix", "Relaxaurus", "Strike from the Darkness",
    ]}, None),
    (45, 4, None, "redpurple", "play_card", ["Aurora Guide"], [], {"cost": 2}, None),
    (46, 4, None, "redpurple", "draw", [], [], {"amount": 1}, None),
    (47, 4, None, "redpurple", "put_on_top", ["Jormuntide"], [], None, None),
    (48, 4, None, "redpurple", "deploy", ["Chillet"], [], {"slot": 1, "cost": 5}, None),
    (49, 4, None, "redpurple", "reveal", ["Jormuntide"], [], None, None),
    (50, 4, None, "redpurple", "deploy", ["Jormuntide"], [], None, 49),
    (51, 4, None, "redpurple", "draw", [], [], {"amount": 1}, None),
    (54, 5, None, "Pingu", "bare_marker", ["Pyrin Noct"], [], None, None),
    (55, 5, None, "Pingu", "attack_interrupted", ["Jormuntide"], [], None, 54),
    (56, 5, None, "Pingu", "bare_marker", [], [], None, None),
    (57, 5, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 2, "lucky": True}, 56),
    (60, 6, None, "redpurple", "deploy", ["Foxparks"], [], None, None),
    (61, 6, None, "redpurple", "activate", ["Foxparks' Harness"], [], None, None),
    (62, 6, None, "redpurple", "deploy", ["Single-Shot Rifle"], [], None, None),
    (63, 6, None, "redpurple", "damage_taken", ["Jormuntide"], [], {"damage": 1500}, None),
    (64, 6, None, "redpurple", "activate", ["Single-Shot Rifle"], [], None, None),
    (65, 6, None, "redpurple", "power_buff", ["Foxparks"], [], {"amount": 200}, None),
    (66, 6, None, "redpurple", "destroyed", ["Jormuntide"], [], {"damage": 700}, None),
    (67, 6, None, "redpurple", "attack_declare", ["Foxparks"], ["Chillet"], None, None),
    (68, 6, None, "Pingu", "interrupt", ["Reindrix"], [], {"cost": {"type": "discard", "card": "Pal Sphere"}, "result": "nullified"}, None),
    (71, 7, None, "redpurple", "turn_opened", [], [], {"souls": 9, "souls_cap": 9, "hand": [
        "Aurora Guide", "Chillet", "Elphidran", "Elphidran", "Pyrin Noct", "Reindrix", "Reindrix",
        "Relaxaurus", "Strike from the Darkness",
    ]}, None),
    (74, 8, None, "Pingu", "battle_attack", ["Chillet"], ["Foxparks"],
     {"attacker_power_expr": "900", "attacker_power": 900, "defender_power_expr": "200", "defender_power": 200}, None),
    (75, 8, None, "Pingu", "bare_marker", ["Chillet"], [], None, None),
    (76, 8, None, "Pingu", "destroyed", ["Foxparks"], [], {"damage": 900, "power": 200}, 75),
    (77, 8, None, "Pingu", "play_card", ["Aurora Guide"], [], {"cost": 2}, None),
    (78, 8, None, "Pingu", "draw", [], [], {"amount": 1}, None),
    (79, 8, None, "Pingu", "put_on_top", ["Relaxaurus"], [], None, None),
    (80, 8, None, "Pingu", "deploy", ["Chillet"], [], {"slot": 2, "cost": 5}, None),
    (81, 8, None, "Pingu", "reveal", ["Relaxaurus"], [], None, None),
    (82, 8, None, "Pingu", "deploy", ["Relaxaurus"], [], None, 81),
    (83, 8, None, "Pingu", "bare_marker", ["Pyrin Noct"], [], None, None),
    (84, 8, None, "Pingu", "attack_interrupted", ["Relaxaurus"], [], None, 83),
    (85, 8, None, "Pingu", "battle_attack", ["Chillet"], ["Foxparks"],
     {"attacker_power_expr": "900", "attacker_power": 900, "defender_power_expr": "200", "defender_power": 200}, None),
    (86, 8, None, "Pingu", "bare_marker", ["Chillet"], [], None, None),
    (87, 8, None, "Pingu", "destroyed", ["Foxparks"], [], {"damage": 900, "power": 200}, 86),
    (90, 9, None, "redpurple", "deploy", ["Menasting"], [], None, None),
    (91, 9, None, "redpurple", "activate", ["Single-Shot Rifle"], [], None, None),
    (92, 9, None, "redpurple", "power_buff", ["Menasting"], [], {"amount": 200}, None),
    (93, 9, None, "redpurple", "activate", ["Foxparks' Harness"], [], None, None),
    (94, 9, None, "redpurple", "attack_declare", ["Menasting"], ["Chillet"], None, None),
    (95, 9, None, "Pingu", "interrupt", ["Reindrix"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (96, 9, None, "redpurple", "deploy", ["Menasting"], [], None, None),
    (99, 10, None, "redpurple", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Elphidran", "Elphidran", "Pengullet", "Pyrin Noct", "Reindrix", "Strike from the Darkness",
        "Zoe's Strategy",
    ]}, None),
    (102, 11, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 10, "life_after": 8}, None),
    (103, 11, None, "Pingu", "bare_marker", [], [], None, None),
    (104, 11, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 2, "lucky": True}, 103),
    (105, 11, None, "Pingu", "play_card", ["Strike from the Darkness"], [], {"cost": 4}, None),
    (106, 11, None, "redpurple", "return_to_hand", ["Reptyro"], [], None, None),
    (107, 11, None, "Pingu", "attack", ["Relaxaurus"], ["them"],
     {"damage": 3, "milled": 3, "lucky": False, "life_before": 8, "life_after": 5}, None),
    (108, 11, None, "Pingu", "deploy", ["Elphidran"], [], {"slot": 4, "cost": 4}, None),
    (109, 11, None, "Pingu", "reveal", ["Elphidran"], [], {"element": "Dragon", "power_buff": 500}, None),
    (110, 11, None, "Pingu", "battle_attack", ["Elphidran"], ["Menasting"],
     {"attacker_power_expr": "600+500 = 1100", "attacker_power": 1100, "defender_power_expr": "700", "defender_power": 700}, None),
    (111, 11, None, "Pingu", "retaliate", ["Elphidran"], [], {"source": "Menasting"}, None),
    (112, 11, None, "Pingu", "bare_marker", ["Elphidran"], [], None, None),
    (113, 11, None, "Pingu", "destroyed", ["Menasting"], [], {"damage": 1100, "power": 700}, 112),
    (114, 11, None, "Pingu", "destroyed", ["Elphidran"], [], {"damage": 700, "power": 1100}, None),
    (115, 11, None, "redpurple", "return_to_hand", ["Foxparks"], [], None, None),
    (118, 12, None, "redpurple", "deploy", ["Reptyro"], [], None, None),
    (119, 12, None, "redpurple", "activate", ["Reptyro"], [], None, None),
    (120, 12, None, "redpurple", "reveal_and_deploy", ["Foxparks' Harness"], [], {"deployed": True}, None),
    (121, 12, None, "redpurple", "look_and_take", ["Foxparks' Harness"], [], {"looked": 5, "outcome": "deployed"}, None),
    (122, 12, None, "redpurple", "deploy", ["Foxparks"], [], None, None),
    (123, 12, None, "redpurple", "activate", ["Foxparks' Harness"], [], None, None),
    (124, 12, None, "redpurple", "activate", ["Foxparks' Harness"], [], None, None),
    (125, 12, None, "redpurple", "activate", ["Single-Shot Rifle"], [], None, None),
    (126, 12, None, "redpurple", "power_buff", ["Foxparks"], [], {"amount": 200}, None),
    (127, 12, None, "redpurple", "damage_taken", ["Relaxaurus"], [], {"damage": 700}, None),
    (128, 12, None, "redpurple", "destroyed", ["Relaxaurus"], [], {"damage": 700}, None),
    (129, 12, None, "redpurple", "attack_declare", ["Foxparks"], ["Chillet"], None, None),
    (130, 12, None, "Pingu", "interrupt", ["Pyrin Noct"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (131, 12, None, "redpurple", "attack_declare", ["Reptyro"], ["Chillet"], None, None),
    (132, 12, None, "Pingu", "interrupt", ["Reindrix"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (135, 13, None, "redpurple", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Elphidran", "Pengullet", "Zoe's Strategy", "Zoe's Strategy",
    ]}, None),
    (138, 14, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 5, "life_after": 3}, None),
    (139, 14, None, "Pingu", "bare_marker", [], [], None, None),
    (140, 14, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 1, "lucky": True}, 139),
    (141, 14, None, "Pingu", "deploy", ["Pengullet"], [], {"slot": 3, "cost": 4}, None),
    (142, 14, None, "Pingu", "bare_marker", [], [], None, None),
    (143, 14, None, "Pingu", "attack", ["Pengullet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 1, "lucky": True}, 142),
    (144, 14, None, "Pingu", "play_card", ["Zoe's Strategy"], [], {"cost": 3}, None),
    (145, 14, None, "Pingu", "draw", [], [], {"amount": 1}, None),
    (146, 14, None, "Pingu", "butcher", ["Pengullet"], [], None, None),
    (147, 14, None, "Pingu", "play_card", ["Zoe's Strategy"], [], {"cost": 3}, None),
    (148, 14, None, "Pingu", "return_to_hand", ["Jormuntide"], [], None, None),
    (149, 14, None, "redpurple", "must_discard", [], [], {"amount": 1}, None),
    (150, 14, None, "redpurple", "discard", ["Foxparks"], [], None, None),
    (153, 15, None, "redpurple", "play_card", ["Medical Supplies"], [], None, None),
    (154, 15, None, "redpurple", "activate", ["Foxparks' Harness"], [], None, None),
    (155, 15, None, "redpurple", "activate", ["Foxparks' Harness"], [], None, None),
    (156, 15, None, "redpurple", "damage_taken", ["Chillet"], [], {"damage": 700}, None),
    (157, 15, None, "redpurple", "destroyed", ["Chillet"], [], {"damage": 700}, None),
    (158, 15, None, "redpurple", "damage_taken", ["Chillet"], [], {"damage": 700}, None),
    (159, 15, None, "redpurple", "destroyed", ["Chillet"], [], {"damage": 700}, None),
    (160, 15, None, "redpurple", "attack_declare", ["Foxparks"], ["Pingu"], {"damage": 1}, None),
    (161, 15, None, "redpurple", "attack", ["Foxparks"], ["Pingu"],
     {"damage": 1, "milled": 1, "lucky": False, "life_before": 8, "life_after": 7}, None),
    (162, 15, None, "redpurple", "deploy", ["Leezpunk"], [], None, None),
    (165, 16, None, "redpurple", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Aurora Guide", "Elphidran", "Elphidran Aqua", "Jormuntide",
    ]}, None),
    (166, 16, None, "redpurple", "play_card", ["Aurora Guide"], [], {"cost": 2}, None),
    (167, 16, None, "redpurple", "draw", [], [], {"amount": 1}, None),
    (168, 16, None, "redpurple", "deploy", ["Relaxaurus"], [], {"slot": 1, "cost": 7}, None),
    (171, 17, None, "Pingu", "attack", ["Relaxaurus"], ["them"],
     {"damage": 3, "milled": 3, "lucky": False, "life_before": 3, "life_after": 0}, None),
    (173, 17, None, "Pingu", "win_declared", [], [], {"winner": "PINGU"}, None),
    (174, 17, None, "Pingu", "loss_reason", [], [], {"loser": "redpurple", "reason": "life_hit_zero"}, None),
    (175, 17, None, "Pingu", "final_life", [], [], {"winner": "Pingu", "winner_life": 7, "loser": "redpurple", "loser_life": 0}, None),
]


def test_meta(pingu02_path):
    result = parse_log(pingu02_path)
    assert result["meta"] == EXPECTED_META


def test_event_count(pingu02_path):
    result = parse_log(pingu02_path)
    assert len(result["events"]) == len(EXPECTED_EVENTS)


def test_every_event_matches_by_hand(pingu02_path):
    result = parse_log(pingu02_path)
    events = result["events"]
    assert len(events) == len(EXPECTED_EVENTS)
    for event, expected in zip(events, EXPECTED_EVENTS):
        line_index, turn, phase, actor, action, cards, targets, resolution, caused_by_line = expected
        assert event["line_index"] == line_index
        assert event["turn"] == turn, f"line {line_index}: {event['text']!r}"
        assert event["phase"] == phase
        assert event["actor"] == actor, f"line {line_index}: {event['text']!r}"
        assert event["action"] == action, f"line {line_index}: {event['text']!r}"
        assert event["cards"] == cards, f"line {line_index}: {event['text']!r}"
        assert event["targets"] == targets, f"line {line_index}: {event['text']!r}"
        assert event["resolution"] == resolution, f"line {line_index}: {event['text']!r}"
        assert event["caused_by_line"] == caused_by_line, f"line {line_index}: {event['text']!r}"


def test_no_unparsed_lines(pingu02_path):
    result = parse_log(pingu02_path)
    bad = [e for e in result["events"] if e["action"] in ("unparsed", "unrecognized_line_format")]
    assert bad == []


def test_no_actor_corruption(pingu02_path):
    """The specific regression this fixture's own real bug produced --
    every event's actor must be one of the two real participants, never
    a fragment of the sentence itself."""
    result = parse_log(pingu02_path)
    bad = [e for e in result["events"] if e["actor"] not in ("Pingu", "redpurple")]
    assert bad == []


def test_chillet_free_deploy_chain_is_linked(pingu02_path):
    """Chillet's own reveal-then-free-deploy chain, deployed turn 4
    (line 48) revealing and free-deploying Jormuntide (lines 49-50) --
    the only such chain in this fixture. redpurple's own turn-8 Chillet
    (line 80) reveals a Relaxaurus (lines 81-82) too."""
    result = parse_log(pingu02_path)
    by_line = {e["line_index"]: e for e in result["events"]}
    assert by_line[50]["caused_by_line"] == 49
    assert by_line[50]["cards"] == by_line[49]["cards"] == ["Jormuntide"]
    assert by_line[82]["caused_by_line"] == 81
    assert by_line[82]["cards"] == by_line[81]["cards"] == ["Relaxaurus"]


def test_bare_form_interrupts_link_to_the_attack_they_nullified(pingu02_path):
    """The header's own count: 2 bare-form interrupts (turns 5, 8)."""
    result = parse_log(pingu02_path)
    bare_interrupts = [
        e for e in result["events"]
        if e["action"] == "bare_marker" and any(
            e2["caused_by_line"] == e["line_index"] and e2["action"] == "attack_interrupted"
            for e2 in result["events"]
        )
    ]
    assert len(bare_interrupts) == 2
    assert [e["turn"] for e in bare_interrupts] == [5, 8]


def test_full_form_interrupts_count(pingu02_path):
    """The header's own count: 4 additional full-form interrupts (turns
    6, 9, 12 x2)."""
    result = parse_log(pingu02_path)
    full_interrupts = [e for e in result["events"] if e["action"] == "interrupt"]
    assert len(full_interrupts) == 4
    assert [e["turn"] for e in full_interrupts] == [6, 9, 12, 12]


def test_lethal_line_is_last_and_zeroes_opponent_life(pingu02_path):
    result = parse_log(pingu02_path)
    events = [e for e in result["events"] if e["action"] == "attack"]
    last = events[-1]
    assert last["line_index"] == 171
    assert last["resolution"]["life_after"] == 0


def test_provenance_and_gap_expectation_metadata(pingu02_path):
    """The testable prediction from the corpus survey: pingu_02 claims no
    unexplained gap and is not one of the four reconstructed-from-raw-log
    files, so both metadata fields should say so."""
    result = parse_log(pingu02_path)
    assert result["meta"]["provenance"] == "curated"
    assert result["meta"]["gap_expectation"] == "clean"
