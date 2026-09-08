"""Hand-verified fixture test.

battle1_bg-chillet_vs_blue-purple_WIN.log is the shortest replay (68 event
lines after the 5-line header) and every expected event below was built by
reading the log line by line -- this is the file meant to be checked by eye
against data/replays/battle1_bg-chillet_vs_blue-purple_WIN.log.
"""

from parser.replay_parser import parse_log

EXPECTED_META = {
    "recordId": "local-bg-chillet-01",
    "deck": "Blue/Green Chillet (construction, ladder-#16-derived)",
    "player": "Edify (P1)",
    "opponent": "FluffysFury",
    "opponent_colors": "Blue/Purple",
    "result": "WIN turn 5",
    "notes": ["opponent reported poor draw; did nothing until T4"],
}

# (line_index, turn, phase, actor, action, cards, targets, resolution, caused_by_line)
EXPECTED_EVENTS = [
    (6, 1, "main", "player", "setup", [], [], None, None),
    (7, 1, "end", "player", "end_phase", [], [], None, None),
    (8, 2, "stand", "player", "stand_phase", [], [], {"declared_turn": 2}, None),
    (9, 2, "draw", "player", "draw_phase", [], [], None, None),
    (10, 2, "draw", "player", "draw", [], [], {"amount": 1}, None),
    (11, 2, "soul", "player", "place_souls", [], [], {"amount": 2, "total": 4, "cap": 10}, None),
    (12, 2, "main", "player", "main_phase", [], [], {"player": "Edify"}, None),
    (13, 2, "main", "player", "deploy", ["Elphidran – Gentle Radiance"], [], {"slot": 1, "cost": 4}, None),
    (14, 2, "main", "player", "reveal", ["Chillet – Dragon Whisperer"], [], {"element": "Dragon", "power_buff": 500}, None),
    (15, 2, "main", "player", "attack", ["Elphidran – Gentle Radiance"], ["opponent"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 10, "life_after": 8}, None),
    (16, 2, "end", "player", "end_phase", [], [], None, None),
    (17, 3, "waiting", "opponent", "deploy", ["Pengullet Rocket Launcher"], [], None, None),
    (18, 3, "stand", "player", "stand_phase", [], [], {"declared_turn": 3}, None),
    (19, 3, "draw", "player", "draw_phase", [], [], None, None),
    (20, 3, "draw", "player", "draw", [], [], {"amount": 1}, None),
    (21, 3, "soul", "player", "place_souls", [], [], {"amount": 2, "total": 6, "cap": 10}, None),
    (22, 3, "main", "player", "main_phase", [], [], {"player": "Edify"}, None),
    (23, 3, "main", "player", "deploy", ["Chillet – Dragon Whisperer"], [], {"slot": 2, "cost": 5}, None),
    (24, 3, "main", "player", "reveal", ["Jormuntide – Surging Sea Serpent"], [], None, None),
    (25, 3, "main", "player", "deploy", ["Jormuntide – Surging Sea Serpent"], [], None, 24),  # free deploy caused by reveal@24
    (26, 3, "main", "player", "draw", [], [], {"amount": 1}, None),
    (27, 3, "main", "player", "no_valid_target", [], [], None, None),
    (28, 3, "main", "player", "attack_interrupted", ["Jormuntide – Surging Sea Serpent"], [], None, None),
    (29, 3, "main", "player", "attack", ["Chillet – Dragon Whisperer"], ["opponent"],
     {"damage_attempted": 2, "damage": 0, "milled": 1, "lucky": True}, None),
    (30, 3, "main", "player", "reveal_declined", [], [], None, None),
    (31, 3, "main", "player", "attack", ["Elphidran – Gentle Radiance"], ["opponent"],
     {"damage_attempted": 2, "damage": 0, "milled": 1, "lucky": True}, None),
    (32, 3, "end", "player", "end_phase", [], [], None, None),
    (33, 4, "waiting", "opponent", "play_card", ["Strike from the Darkness"], [], None, None),
    (34, 3, "waiting", "opponent", "destroyed", ["Jormuntide – Surging Sea Serpent"], [], None, None),  # turn regresses: gotcha 1
    (35, 4, "waiting", "opponent", "play_card", ["Dark Cannon"], [], None, None),
    (36, 3, "waiting", "opponent", "destroyed", ["Chillet – Dragon Whisperer"], [], None, None),  # turn regresses: gotcha 1
    (37, 4, "stand", "player", "stand_phase", [], [], {"declared_turn": 4}, None),
    (38, 4, "draw", "player", "draw_phase", [], [], None, None),
    (39, 4, "draw", "player", "draw", [], [], {"amount": 1}, None),
    (40, 4, "soul", "player", "place_souls", [], [], {"amount": 2, "total": 8, "cap": 10}, None),
    (41, 4, "main", "player", "main_phase", [], [], {"player": "Edify"}, None),
    (42, 4, "main", "player", "deploy", ["Chillet – Dragon Whisperer"], [], {"slot": 2, "cost": 5}, None),
    (43, 4, "main", "player", "reveal", ["Elphidran Aqua – Gentle Ripples"], [], None, None),
    (44, 4, "main", "player", "deploy", ["Elphidran Aqua – Gentle Ripples"], [], None, 43),  # free deploy caused by reveal@43
    (45, 4, "main", "player", "draw", [], [], {"amount": 2}, None),
    (46, 4, "main", "player", "put_on_top", ["Victor's Strategy"], [], None, None),
    (47, 4, "main", "player", "attack_interrupted", ["Elphidran Aqua – Gentle Ripples"], [], None, None),
    (48, 4, "main", "player", "attack", ["Chillet – Dragon Whisperer"], ["opponent"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 8, "life_after": 6}, None),
    (49, 4, "main", "player", "reveal_declined", [], [], None, None),
    (50, 4, "main", "player", "attack", ["Elphidran – Gentle Radiance"], ["opponent"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 6, "life_after": 4}, None),
    (51, 4, "main", "player", "rest_draw", [], [], {"souls_spent": 3, "drew": 1}, None),
    (52, 4, "end", "player", "end_phase", [], [], None, None),
    (53, 5, "waiting", "opponent", "deploy", ["Pengullet – Yearning for the Sky"], [], None, None),
    (54, 5, "waiting", "opponent", "activate", ["Pengullet Rocket Launcher"], [], None, None),
    (55, 5, "waiting", "opponent", "power_buff", ["Pengullet – Yearning for the Sky"], [], {"amount": 500}, None),
    (56, 5, "waiting", "opponent", "activate", ["Pengullet – Yearning for the Sky"], [], None, None),
    (57, 5, "stand", "player", "stand_phase", [], [], {"declared_turn": 5}, None),
    (58, 5, "draw", "player", "draw_phase", [], [], None, None),
    (59, 5, "draw", "player", "draw", [], [], {"amount": 1}, None),
    (60, 5, "soul", "player", "place_souls", [], [], {"amount": 2, "total": 10, "cap": 10}, None),
    (61, 5, "main", "player", "main_phase", [], [], {"player": "Edify"}, None),
    (62, 5, "main", "player", "deploy", ["Chillet – Dragon Whisperer"], [], {"slot": 1, "cost": 5}, None),
    (63, 5, "main", "player", "reveal", ["Jormuntide – Surging Sea Serpent"], [], None, None),
    (64, 5, "main", "player", "deploy", ["Jormuntide – Surging Sea Serpent"], [], None, 63),  # free deploy caused by reveal@63
    (65, 5, "main", "player", "draw", [], [], {"amount": 1}, None),
    (66, 5, "main", "player", "no_valid_target", [], [], None, None),
    (67, 5, "main", "player", "deploy", ["Chillet – Dragon Whisperer"], [], {"slot": 3, "cost": 5}, None),
    (68, 5, "main", "player", "reveal", ["Jormuntide – Surging Sea Serpent"], [], None, None),
    (69, 5, "main", "player", "deploy", ["Jormuntide – Surging Sea Serpent"], [], None, 68),  # free deploy caused by reveal@68 -- the two-Chillets-one-turn case
    (70, 5, "main", "player", "draw", [], [], {"amount": 1}, None),
    (71, 5, "main", "player", "no_valid_target", [], [], None, None),
    (72, 5, "main", "player", "attack", ["Jormuntide – Surging Sea Serpent"], ["opponent"],
     {"damage": 3, "milled": 3, "lucky": False, "life_before": 4, "life_after": 1}, None),
    (73, 5, "main", "player", "attack", ["Chillet – Dragon Whisperer"], ["opponent"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 1, "life_after": 0}, None),
]


def test_meta(battle1_path):
    result = parse_log(battle1_path)
    assert result["meta"] == EXPECTED_META


def test_event_count(battle1_path):
    result = parse_log(battle1_path)
    assert len(result["events"]) == len(EXPECTED_EVENTS)


def test_every_event_matches_by_hand(battle1_path):
    result = parse_log(battle1_path)
    events = result["events"]
    assert len(events) == len(EXPECTED_EVENTS)
    for event, expected in zip(events, EXPECTED_EVENTS):
        line_index, turn, phase, actor, action, cards, targets, resolution, caused_by_line = expected
        assert event["line_index"] == line_index
        assert event["turn"] == turn
        assert event["phase"] == phase
        assert event["actor"] == actor
        assert event["action"] == action, f"line {line_index}: {event['text']!r}"
        assert event["cards"] == cards, f"line {line_index}: {event['text']!r}"
        assert event["targets"] == targets, f"line {line_index}: {event['text']!r}"
        assert event["resolution"] == resolution, f"line {line_index}: {event['text']!r}"
        assert event["caused_by_line"] == caused_by_line, f"line {line_index}: {event['text']!r}"


def test_chillet_free_deploy_chains_are_linked(battle1_path):
    """The four reveal-then-free-deploy pairs in this log (T3, T4, and two
    in T5 -- the two-Chillets-in-one-turn case) must each have the deploy's
    caused_by_line pointing back at its triggering reveal."""
    result = parse_log(battle1_path)
    by_line = {e["line_index"]: e for e in result["events"]}
    for reveal_line, deploy_line in [(24, 25), (43, 44), (63, 64), (68, 69)]:
        assert by_line[reveal_line]["action"] == "reveal"
        assert by_line[deploy_line]["action"] == "deploy"
        assert by_line[deploy_line]["caused_by_line"] == reveal_line
        assert by_line[deploy_line]["cards"] == by_line[reveal_line]["cards"]


def test_no_unparsed_lines(battle1_path):
    result = parse_log(battle1_path)
    bad = [e for e in result["events"] if e["action"] in ("unparsed", "unrecognized_line_format")]
    assert bad == []


def test_lethal_line_is_last_and_zeroes_opponent_life(battle1_path):
    result = parse_log(battle1_path)
    last = result["events"][-1]
    assert last["line_index"] == 73
    assert last["resolution"]["life_after"] == 0
