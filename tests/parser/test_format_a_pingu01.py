"""Hand-verified fixture test for the Format A parser -- same discipline
as tests/parser/test_battle1.py: every expected event below was built by
reading data/replays/top_players_all20/pingu_01_ca8ae191.txt line by line,
independently of parser/format_a_parser.py's own output, before the parser
was written to match it.

pingu_01 was chosen as the fixture (per the corpus survey) because it is:
  - in the explicitly-"clean" gap group (its own header claims no
    unexplained life gap on either side, cross-checked against the other
    files that name it as a comparison point), not one of the four
    provenance="reconstructed" files;
  - 22 turns, long enough to exercise hand reveals, both interrupt forms
    (bare-form and full-form), a signed (negative) power_buff (Astegon's
    own effect on Relaxaurus), a granted-skill power_buff ("...and the
    explode skill..."), a Chillet reveal-then-free-deploy chain, and
    repeated Lucky-cancel attacks.

`phase` is always None for Format A -- there is no phase-tag concept in
this format, only the block actor (see format_a_parser's own module doc
comment for what `actor` means and when a line overrides it).
"""

from parser.format_a_parser import parse_log

EXPECTED_META = {
    "match_title": "Pingu vs Chylith  [Pingu's match #1 of 10 pulled, most recent]",
    "recordId": "ca8ae191-e71e-4e94-9f86-e0dd0f8a6952",
    "result": 'PINGU WINS - "Chylith\'s life hit zero." Final life: Pingu (WINNER) 4, Chylith 0.',
    "notes": [
        'UNLIKE Ze (whose account name "ze" shows in-game as "bel"), Pingu\'s account name and '
        'in-game/log display name are THE SAME — "Pingu" throughout, no alias. Worth noting as a contrast '
        "with the Ze batch, where the name substitution needed explicit tracking.",
        'Pingu\'s deck HUD reads "BLUE/PURPLE DRAGON CONTROL" — a named archetype label, unlike Ze\'s '
        "generic \"BLUE/PURPLE DECK\" seen across all 10 of Ze's games. Not yet cross-referenced against a "
        "saved Pingu decklist (out of scope for this pass, same as Ze).",
        "Both life totals are fully accounted for by named damage lines — Pingu drops 10->8->6->4 "
        "via three named Chillet hits (turns 1, 4, 17); Chylith drops 10->8->6->4->2->0 via five named "
        "hits (turns 3, 16, 19, 22 x2) — NO unexplained gap. A clean game to open the Pingu dataset.",
        "4 bare-form interrupts (card name on its own line, no player/cost) preserved verbatim "
        "below for decklist cross-check: Turn 3:  Pyrin Noct interrupts Relaxaurus's attack Turn 7:  "
        "Cryolinx interrupts Elphidran's attack (1st of 2 occurrences) Turn 10: Cryolinx interrupts "
        "Elphidran's attack (2nd occurrence, same card/target type) Turn 19: Reindrix interrupts "
        'Chillet\'s attack 2 additional FULL-form interrupts occurred (Turn 5 "Pingu interrupted with '
        'Cryolinx, paid 1 Soul"; Turn 20 "Pingu interrupted with Pyrin Noct, paid 1 Soul") — not counted '
        "in the bare-form total, included in the verbatim body only.",
    ],
    "provenance": "curated",
    "gap_expectation": "clean",
    "opponent_anonymized": False,
    "word_wrap_joins": 0,
}

# (line_index, turn, phase, actor, action, cards, targets, resolution, caused_by_line)
EXPECTED_EVENTS = [
    (26, 1, None, "Chylith", "turn_opened", [], [], {"souls": 5, "souls_cap": 5, "hand": [
        "Aurora Guide", "Elphidran", "Elphidran", "Pengullet", "Pengullet Rocket Launcher",
        "Relaxaurus", "Strike from the Darkness", "Strike from the Darkness",
    ]}, None),
    (27, 1, None, "Chylith", "deploy", ["Pengullet Rocket Launcher"], [], {"slot": 1, "cost": 4}, None),
    (28, 1, None, "Chylith", "deploy", ["Chillet"], [], None, None),
    (29, 1, None, "Chylith", "reveal", ["Pyrin Noct"], [], {"outcome": "added_to_hand"}, None),
    (30, 1, None, "Chylith", "attack_declare", ["Chillet"], ["Pingu"], {"damage": 2}, None),
    (31, 1, None, "Chylith", "attack", ["Chillet"], ["Pingu"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 10, "life_after": 8}, None),
    (34, 2, None, "Chylith", "turn_opened", [], [], {"souls": 7, "souls_cap": 7, "hand": [
        "Aurora Guide", "Chillet", "Elphidran", "Elphidran", "Pengullet", "Relaxaurus",
        "Strike from the Darkness", "Strike from the Darkness",
    ]}, None),
    (35, 2, None, "Chylith", "play_card", ["Aurora Guide"], [], {"cost": 2}, None),
    (36, 2, None, "Chylith", "draw", [], [], {"amount": 1}, None),
    (37, 2, None, "Chylith", "put_on_top", ["Relaxaurus"], [], None, None),
    (38, 2, None, "Chylith", "deploy", ["Chillet"], [], {"slot": 2, "cost": 5}, None),
    (39, 2, None, "Chylith", "reveal", ["Relaxaurus"], [], None, None),
    (40, 2, None, "Chylith", "deploy", ["Relaxaurus"], [], None, 39),
    (43, 3, None, "Pingu", "bare_marker", ["Pyrin Noct"], [], None, None),
    (44, 3, None, "Pingu", "attack_interrupted", ["Relaxaurus"], [], None, 43),
    (45, 3, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 10, "life_after": 8}, None),
    (48, 4, None, "Chylith", "play_card", ["Aurora Guide"], [], None, None),
    (49, 4, None, "Chylith", "deploy", ["Chillet"], [], None, None),
    (50, 4, None, "Chylith", "deploy", ["Astegon"], [], None, None),
    (51, 4, None, "Chylith", "power_buff", ["Relaxaurus"], [], {"amount": -1000}, None),
    (52, 4, None, "Chylith", "attack_declare", ["Astegon"], ["Chillet"], None, None),
    (53, 4, None, "Chylith", "battle_attack", ["Astegon"], ["Chillet"],
     {"attacker_power_expr": "1200", "attacker_power": 1200, "defender_power_expr": "900", "defender_power": 900}, None),
    (54, 4, None, "Chylith", "bare_marker", ["Astegon"], [], None, None),
    (55, 4, None, "Chylith", "destroyed", ["Chillet"], [], {"damage": 1200, "power": 900}, 54),
    (56, 4, None, "Chylith", "attack_declare", ["Chillet"], ["Pingu"], {"damage": 2}, None),
    (57, 4, None, "Chylith", "attack", ["Chillet"], ["Pingu"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 8, "life_after": 6}, None),
    (60, 5, None, "Chylith", "turn_opened", [], [], {"souls": 9, "souls_cap": 9, "hand": [
        "Elphidran", "Elphidran", "Jormuntide", "Pengullet", "Strike from the Darkness",
        "Strike from the Darkness", "Zoe's Strategy",
    ]}, None),
    (61, 5, None, "Chylith", "deploy", ["Pengullet"], [], {"slot": 2, "cost": 4}, None),
    (62, 5, None, "Chylith", "power_buff", ["Pengullet"], [], {"amount": 500, "granted_skill": "explode"}, None),
    (63, 5, None, "Chylith", "draw", [], [], {"amount": 1}, None),
    (64, 5, None, "Chylith", "mass_damage", [], [], {"amount": 1100, "scope": "Chylith", "self_sacrifice": True}, None),
    (65, 5, None, "Chylith", "play_card", ["Strike from the Darkness"], [], {"cost": 4}, None),
    (66, 5, None, "Chylith", "deploy", ["Lyleen Noct"], [], None, None),
    (67, 5, None, "Chylith", "play_card", ["Aurora Guide"], [], None, None),
    (68, 5, None, "Chylith", "activate", ["Lyleen Noct"], [], None, None),
    (69, 5, None, "Chylith", "discard", ["Pengullet"], [], None, None),
    (70, 5, None, "Chylith", "deploy", ["Relaxaurus"], [], None, None),
    (71, 5, None, "Chylith", "attack_declare", ["Relaxaurus"], ["Pingu"], {"damage": 3}, None),
    (72, 5, None, "Pingu", "interrupt", ["Cryolinx"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (75, 6, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Elphidran", "Elphidran", "Jormuntide", "Pengullet", "Strike from the Darkness", "Zoe's Strategy",
    ]}, None),
    (76, 6, None, "Chylith", "play_card", ["Strike from the Darkness"], [], {"cost": 4}, None),
    (77, 6, None, "Chylith", "deploy", ["Elphidran"], [], {"slot": 2, "cost": 4}, None),
    (78, 6, None, "Chylith", "power_buff", ["Elphidran"], [], {"amount": 200}, None),
    (79, 6, None, "Chylith", "reveal", ["Elphidran"], [], {"element": "Dragon", "power_buff": 500}, None),
    (82, 7, None, "Pingu", "bare_marker", ["Cryolinx"], [], None, None),
    (83, 7, None, "Pingu", "attack_interrupted", ["Elphidran"], [], None, 82),
    (86, 8, None, "Chylith", "attack_declare", ["Chillet"], ["Elphidran"], None, None),
    (87, 8, None, "Chylith", "battle_attack", ["Chillet"], ["Elphidran"],
     {"attacker_power_expr": "900", "attacker_power": 900, "defender_power_expr": "600", "defender_power": 600}, None),
    (88, 8, None, "Chylith", "bare_marker", ["Chillet"], [], None, None),
    (89, 8, None, "Chylith", "destroyed", ["Elphidran"], [], {"damage": 900, "power": 600}, 88),
    (90, 8, None, "Chylith", "attack_declare", ["Relaxaurus"], ["Pingu"], {"damage": 3}, None),
    (91, 8, None, "Chylith", "bare_marker", [], [], None, None),
    (92, 8, None, "Chylith", "attack", ["Relaxaurus"], ["Pingu"],
     {"damage_attempted": 3, "damage": 0, "milled": 2, "lucky": True}, 91),
    (93, 8, None, "Chylith", "deploy", ["Chillet"], [], None, None),
    (94, 8, None, "Chylith", "reveal", ["Lyleen Noct"], [], {"outcome": "added_to_hand"}, None),
    (95, 8, None, "Chylith", "attack_declare", ["Chillet"], ["Pingu"], {"damage": 2}, None),
    (96, 8, None, "Chylith", "bare_marker", [], [], None, None),
    (97, 8, None, "Chylith", "attack", ["Chillet"], ["Pingu"],
     {"damage_attempted": 2, "damage": 0, "milled": 2, "lucky": True}, 96),
    (100, 9, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Elphidran", "Jormuntide", "Pengullet", "Zoe's Strategy", "Zoe's Strategy",
    ]}, None),
    (101, 9, None, "Chylith", "deploy", ["Elphidran"], [], {"slot": 2, "cost": 4}, None),
    (102, 9, None, "Chylith", "reveal_declined", [], [], None, None),
    (105, 10, None, "Pingu", "bare_marker", ["Cryolinx"], [], None, None),
    (106, 10, None, "Pingu", "attack_interrupted", ["Elphidran"], [], None, 105),
    (107, 10, None, "Pingu", "deploy", ["Pengullet"], [], {"slot": 3, "cost": 4}, None),
    (108, 10, None, "Pingu", "power_buff", ["Pengullet"], [], {"amount": 500, "granted_skill": "explode"}, None),
    (109, 10, None, "Pingu", "draw", [], [], {"amount": 1}, None),
    (110, 10, None, "Pingu", "mass_damage", [], [], {"amount": 1100, "scope": "Chylith", "self_sacrifice": True}, None),
    (113, 11, None, "Chylith", "attack_declare", ["Relaxaurus"], ["Pingu"], {"damage": 3}, None),
    (114, 11, None, "Chylith", "bare_marker", [], [], None, None),
    (115, 11, None, "Chylith", "attack", ["Relaxaurus"], ["Pingu"],
     {"damage_attempted": 3, "damage": 0, "milled": 3, "lucky": True}, 114),
    (116, 11, None, "Chylith", "play_card", ["Zoe's Strategy"], [], None, None),
    (117, 11, None, "Chylith", "return_to_hand", ["Chillet"], [], None, None),
    (118, 11, None, "Chylith", "discard", ["Zoe's Strategy"], [], None, None),
    (119, 11, None, "Chylith", "deploy", ["Chillet"], [], None, None),
    (120, 11, None, "Chylith", "reveal", ["Pengullet Rocket Launcher"], [], {"outcome": "added_to_hand"}, None),
    (121, 11, None, "Chylith", "attack_declare", ["Chillet"], ["Elphidran"], None, None),
    (122, 11, None, "Chylith", "battle_attack", ["Chillet"], ["Elphidran"],
     {"attacker_power_expr": "900", "attacker_power": 900, "defender_power_expr": "600", "defender_power": 600}, None),
    (123, 11, None, "Chylith", "bare_marker", ["Chillet"], [], None, None),
    (124, 11, None, "Chylith", "destroyed", ["Elphidran"], [], {"damage": 900, "power": 600}, 123),
    (127, 12, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Cryolinx", "Jormuntide", "Zoe's Strategy", "Zoe's Strategy",
    ]}, None),
    (128, 12, None, "Chylith", "deploy", ["Jormuntide"], [], {"slot": 2, "cost": 8}, None),
    (129, 12, None, "Chylith", "draw", [], [], {"amount": 1}, None),
    (132, 13, None, "Pingu", "battle_attack", ["Jormuntide"], ["Chillet"],
     {"attacker_power_expr": "1600", "attacker_power": 1600, "defender_power_expr": "900", "defender_power": 900}, None),
    (133, 13, None, "Pingu", "bare_marker", ["Jormuntide"], [], None, None),
    (134, 13, None, "Pingu", "destroyed", ["Chillet"], [], {"damage": 1600, "power": 900}, 133),
    (137, 14, None, "Chylith", "play_card", ["Aurora Guide"], [], None, None),
    (138, 14, None, "Chylith", "play_card", ["Pal Sphere"], [], None, None),
    (141, 15, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Cryolinx", "Reindrix", "Reindrix", "Zoe's Strategy", "Zoe's Strategy",
    ]}, None),
    (144, 16, None, "Pingu", "battle_attack", ["Jormuntide"], ["Relaxaurus"],
     {"attacker_power_expr": "1600", "attacker_power": 1600, "defender_power_expr": "1200", "defender_power": 1200}, None),
    (145, 16, None, "Pingu", "bare_marker", ["Jormuntide"], [], None, None),
    (146, 16, None, "Pingu", "destroyed", ["Relaxaurus"], [], {"damage": 1600, "power": 1200}, 145),
    (147, 16, None, "Pingu", "play_card", ["Zoe's Strategy"], [], {"cost": 3}, None),
    (148, 16, None, "Pingu", "return_to_hand", ["Chillet"], [], None, None),
    (149, 16, None, "Chylith", "must_discard", [], [], {"amount": 1}, None),
    (150, 16, None, "Chylith", "discard", ["Pengullet Rocket Launcher"], [], None, None),
    (151, 16, None, "Pingu", "deploy", ["Chillet"], [], {"slot": 3, "cost": 5}, None),
    (152, 16, None, "Pingu", "reveal", ["Strike from the Darkness"], [], {"outcome": "added_to_hand"}, None),
    (153, 16, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 8, "life_after": 6}, None),
    (156, 17, None, "Chylith", "deploy", ["Chillet"], [], None, None),
    (157, 17, None, "Chylith", "reveal", ["Reindrix"], [], {"outcome": "added_to_hand"}, None),
    (158, 17, None, "Chylith", "attack_declare", ["Chillet"], ["Pingu"], {"damage": 2}, None),
    (159, 17, None, "Chylith", "attack", ["Chillet"], ["Pingu"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 6, "life_after": 4}, None),
    (160, 17, None, "Chylith", "play_card", ["Zoe's Strategy"], [], None, None),
    (161, 17, None, "Chylith", "destroyed", ["Jormuntide"], [], None, None),
    (164, 18, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Cryolinx", "Pyrin Noct", "Reindrix", "Reindrix", "Strike from the Darkness", "Zoe's Strategy",
    ]}, None),
    (167, 19, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 6, "life_after": 4}, None),
    (168, 19, None, "Pingu", "play_card", ["Zoe's Strategy"], [], {"cost": 3}, None),
    (169, 19, None, "Pingu", "return_to_hand", ["Chillet"], [], None, None),
    (170, 19, None, "Chylith", "must_discard", [], [], {"amount": 1}, None),
    (171, 19, None, "Chylith", "discard", ["Pengullet Rocket Launcher"], [], None, None),
    (172, 19, None, "Pingu", "deploy", ["Chillet"], [], {"slot": 2, "cost": 5}, None),
    (173, 19, None, "Pingu", "reveal", ["Relaxaurus"], [], None, None),
    (174, 19, None, "Pingu", "deploy", ["Relaxaurus"], [], None, 173),
    (175, 19, None, "Pingu", "no_valid_target", [], [], None, None),
    (176, 19, None, "Pingu", "bare_marker", [], [], None, None),
    (177, 19, None, "Pingu", "attack", ["Relaxaurus"], ["them"],
     {"damage_attempted": 3, "damage": 0, "milled": 2, "lucky": True}, 176),
    (178, 19, None, "Pingu", "bare_marker", ["Reindrix"], [], None, None),
    (179, 19, None, "Pingu", "attack_interrupted", ["Chillet"], [], None, 178),
    (182, 20, None, "Chylith", "deploy", ["Relaxaurus"], [], None, None),
    (183, 20, None, "Chylith", "rested", ["Chillet"], [], None, None),
    (184, 20, None, "Chylith", "status_effect", ["Chillet"], [], {"effect": "wont_stand_while_source_in_play"}, None),
    (185, 20, None, "Chylith", "attack_declare", ["Relaxaurus"], ["Chillet"], None, None),
    (186, 20, None, "Pingu", "interrupt", ["Pyrin Noct"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (187, 20, None, "Chylith", "play_card", ["Zoe's Strategy"], [], None, None),
    (188, 20, None, "Chylith", "destroyed", ["Relaxaurus"], [], None, None),
    (191, 21, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Chillet", "Cryolinx", "Reindrix", "Reindrix", "Strike from the Darkness",
    ]}, None),
    (194, 22, None, "Pingu", "bare_marker", [], [], None, None),
    (195, 22, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 1, "lucky": True}, 194),
    (196, 22, None, "Pingu", "bare_marker", [], [], None, None),
    (197, 22, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 2, "lucky": True}, 196),
    (198, 22, None, "Pingu", "deploy", ["Chillet"], [], {"slot": 4, "cost": 5}, None),
    (199, 22, None, "Pingu", "reveal", ["Pyrin Noct"], [], {"outcome": "added_to_hand"}, None),
    (200, 22, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 4, "life_after": 2}, None),
    (201, 22, None, "Pingu", "deploy", ["Reindrix"], [], {"slot": 5, "cost": 5}, None),
    (202, 22, None, "Pingu", "attack", ["Reindrix"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 2, "life_after": 0}, None),
    (204, 22, None, "Pingu", "win_declared", [], [], {"winner": "PINGU"}, None),
    (205, 22, None, "Pingu", "loss_reason", [], [], {"loser": "Chylith", "reason": "life_hit_zero"}, None),
    (206, 22, None, "Pingu", "final_life", [], [], {"winner": "Pingu", "winner_life": 4, "loser": "Chylith", "loser_life": 0}, None),
]


def test_meta(pingu01_path):
    result = parse_log(pingu01_path)
    assert result["meta"] == EXPECTED_META


def test_event_count(pingu01_path):
    result = parse_log(pingu01_path)
    assert len(result["events"]) == len(EXPECTED_EVENTS)


def test_every_event_matches_by_hand(pingu01_path):
    result = parse_log(pingu01_path)
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


def test_no_unparsed_lines(pingu01_path):
    result = parse_log(pingu01_path)
    bad = [e for e in result["events"] if e["action"] in ("unparsed", "unrecognized_line_format")]
    assert bad == []


def test_chillet_free_deploy_chains_are_linked(pingu01_path):
    result = parse_log(pingu01_path)
    by_line = {e["line_index"]: e for e in result["events"]}
    for reveal_line, deploy_line in [(39, 40), (173, 174)]:
        assert by_line[reveal_line]["action"] == "reveal"
        assert by_line[deploy_line]["action"] == "deploy"
        assert by_line[deploy_line]["caused_by_line"] == reveal_line
        assert by_line[deploy_line]["cards"] == by_line[reveal_line]["cards"]


def test_bare_form_interrupts_link_to_the_attack_they_nullified(pingu01_path):
    """The header's own count: 4 bare-form interrupts (turns 3, 7, 10, 19)."""
    result = parse_log(pingu01_path)
    bare_interrupts = [
        e for e in result["events"]
        if e["action"] == "bare_marker" and any(
            e2["caused_by_line"] == e["line_index"] and e2["action"] == "attack_interrupted"
            for e2 in result["events"]
        )
    ]
    assert len(bare_interrupts) == 4
    assert [e["turn"] for e in bare_interrupts] == [3, 7, 10, 19]


def test_lethal_line_is_last_and_zeroes_opponent_life(pingu01_path):
    result = parse_log(pingu01_path)
    events = [e for e in result["events"] if e["action"] == "attack"]
    last = events[-1]
    assert last["line_index"] == 202
    assert last["resolution"]["life_after"] == 0


def test_provenance_and_gap_expectation_metadata(pingu01_path):
    """The testable prediction from the corpus survey: pingu_01 claims no
    unexplained gap and is not one of the four reconstructed-from-raw-log
    files, so both metadata fields should say so."""
    result = parse_log(pingu01_path)
    assert result["meta"]["provenance"] == "curated"
    assert result["meta"]["gap_expectation"] == "clean"
