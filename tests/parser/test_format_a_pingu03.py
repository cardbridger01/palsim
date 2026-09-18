"""Hand-verified fixture test for the Format A parser -- same discipline
as test_format_a_pingu01.py: every expected event below was built by
reading data/replays/top_players_all20/pingu_03_edc15d3a.txt line by
line, independently of parser/format_a_parser.py's own output, before
the parser was confirmed against it.

pingu_03 is the second fixture (after pingu_01), chosen deliberately to
hold the OPPONENT constant (Pingu vs Chylith again, same as pingu_01)
while changing the specific game sequence -- isolates whether the
parser/driver generalize across games against the same card pool,
before pingu_04 (or another pingu match against a different opponent,
redpurple) tests the card-pool axis separately. 18 turns, curated
provenance, a clean life-gap header claim cross-checked the same way
pingu_01's was.

## A real parser bug this fixture found, not a fixture-writing mistake

Hand-reading lines 107 and 131 ("Chylith's Jormuntide gets power +200
until end of turn.", "Chylith's Pengullet gets power +500 until end of
turn.") against `format_a_parser.py`'s own power_buff patterns showed
neither pattern stripped a possessive subject the way every other
subject-bearing pattern in this grammar does (deploy, play_card, reveal,
attack_declare, interrupt all separate "NAME's " from the card name via
`_SUBJ` or an explicit `subj` group) -- running the PRE-FIX parser
directly confirmed it: `cards: ["Chylith's Jormuntide"]`, the entire
possessive prefix swallowed into the card field. pingu_01 never
exercised this because none of its own power_buff sentences happened to
carry a possessive prefix (its own analogous lines are bare card names:
"Elphidran gets power +200...", no "Chylith's"). Fixed in
`parser/format_a_parser.py` by adding `_POSSESSIVE_SUBJ` (the same
optional-group shape as `_SUBJ`, adapted for the "NAME's CARD" shape)
to both power_buff patterns -- see that module's own comment on the
constant. Confirmed via `tests/parser/` staying green (102 passed,
including all 8 of pingu_01's own tests) before this fixture's own
expectations were finalized against the corrected parser.

`phase` is always None for Format A -- there is no phase-tag concept in
this format, only the block actor (see format_a_parser's own module doc
comment for what `actor` means and when a line overrides it).
"""

from parser.format_a_parser import parse_log

EXPECTED_META = {
    "match_title": "Pingu vs Chylith  [Pingu's match #3 of 10 pulled — same opponent as match 1]",
    "recordId": "edc15d3a-09a0-4445-9e16-5a32090bfc1f",
    "result": 'PINGU WINS - "Chylith\'s life hit zero." Final life: Pingu (WINNER) 8, Chylith 0.',
    "notes": [
        "Third straight fully clean game for Pingu (with matches 1 and 2) — both life totals are "
        "completely accounted for by named damage lines. Pingu takes exactly one hit all game (10->8, "
        "turn 4) and Chylith drops in three clean chunks late (8->5->2->0, all on turn 18: Relaxaurus, "
        "Jormuntide, Pengullet in sequence) after sitting at 10->8 since turn 12. No unexplained gap.",
        'Deck labeled "BLUE/PURPLE DRAGON CONTROL" on Pingu\'s HUD, same as matches 1-2.',
        "This is Pingu's second game against Chylith in this 10-game pull (see match 1) — worth "
        "noting for anyone doing head-to-head analysis later; Pingu is 2-0 vs Chylith across this batch "
        "so far.",
        "4 bare-form interrupts (card name on its own line, no player/cost) preserved verbatim "
        "below for decklist cross-check: Turn 6:  Pyrin Noct interrupts Elphidran Aqua's attack Turn 9:  "
        "Reindrix interrupts Jormuntide's attack (1st of 3 occurrences) Turn 12: Reindrix interrupts "
        "Jormuntide's attack (2nd occurrence) Turn 15: Reindrix interrupts Jormuntide's attack (3rd "
        'occurrence) 5 additional FULL-form interrupts occurred (Turn 7 Reindrix; Turn 10 Cryolinx; '
        "Turn 13 x2, Cryolinx and Pyrin Noct; Turn 16 Pyrin Noct) — not counted in the bare-form total, "
        "included in the verbatim body only.",
        "Turns 1-2 again show no acting-player header (same recurring pattern seen throughout the Ze "
        "batch) — inferred as Pingu's turns from context (starting hand matches Pingu's later cards) "
        "and labeled PINGU below.",
    ],
    "provenance": "curated",
    "gap_expectation": "clean",
    "opponent_anonymized": False,
    "word_wrap_joins": 0,
}

# (line_index, turn, phase, actor, action, cards, targets, resolution, caused_by_line)
EXPECTED_EVENTS = [
    (28, 1, None, "Pingu", "turn_opened", [], [], {"souls": 1, "souls_cap": 1, "hand": [
        "Elphidran", "Elphidran Aqua", "Pal Sphere", "Pyrin Noct", "Reindrix",
    ]}, None),
    (29, 1, None, "Pingu", "setup", [], [], None, None),
    (32, 2, None, "Pingu", "turn_opened", [], [], {"souls": 3, "souls_cap": 3, "hand": [
        "Elphidran", "Elphidran Aqua", "Pal Sphere", "Pyrin Noct", "Reindrix", "Relaxaurus",
    ]}, None),
    (33, 2, None, "Pingu", "rest_draw", [], [], {"souls_spent": 3, "drew": 1}, None),
    (36, 3, None, "Chylith", "play_card", ["Pal Sphere"], [], None, None),
    (39, 4, None, "Chylith", "turn_opened", [], [], {"souls": 5, "souls_cap": 5, "hand": [
        "Elphidran", "Elphidran Aqua", "Jormuntide", "Pal Sphere", "Pengullet", "Pyrin Noct",
        "Reindrix", "Relaxaurus",
    ]}, None),
    (40, 4, None, "Chylith", "play_card", ["Pal Sphere"], [], {"cost": 4}, None),
    (41, 4, None, "Chylith", "draw", [], [], {"amount": 3}, None),
    (42, 4, None, "Chylith", "deploy", ["Elphidran"], [], None, None),
    (43, 4, None, "Chylith", "attack_declare", ["Elphidran"], ["Pingu"], {"damage": 2}, None),
    (44, 4, None, "Chylith", "attack", ["Elphidran"], ["Pingu"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 10, "life_after": 8}, None),
    (47, 5, None, "Chylith", "turn_opened", [], [], {"souls": 7, "souls_cap": 7, "hand": [
        "Aurora Guide", "Elphidran", "Elphidran Aqua", "Jormuntide", "Pengullet", "Pengullet",
        "Pyrin Noct", "Reindrix", "Reindrix", "Relaxaurus", "Zoe's Strategy",
    ]}, None),
    (48, 5, None, "Chylith", "deploy", ["Elphidran Aqua"], [], {"slot": 1, "cost": 6}, None),
    (49, 5, None, "Chylith", "draw", [], [], {"amount": 2}, None),
    (50, 5, None, "Chylith", "put_on_top", ["Elphidran Aqua"], [], None, None),
    (53, 6, None, "Pingu", "bare_marker", ["Pyrin Noct"], [], None, None),
    (54, 6, None, "Pingu", "attack_interrupted", ["Elphidran Aqua"], [], None, 53),
    (55, 6, None, "Chylith", "reveal", ["Elphidran"], [], {"element": "Dragon"}, None),
    (58, 7, None, "Chylith", "attack_declare", ["Elphidran"], ["Elphidran Aqua"], None, None),
    (59, 7, None, "Pingu", "interrupt", ["Reindrix"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (60, 7, None, "Chylith", "deploy", ["Astegon"], [], None, None),
    (61, 7, None, "Chylith", "power_buff", ["Elphidran Aqua"], [], {"amount": -1000}, None),
    (62, 7, None, "Chylith", "attack_declare", ["Astegon"], ["Pingu"], {"damage": 3}, None),
    (63, 7, None, "Chylith", "bare_marker", [], [], None, None),
    (64, 7, None, "Chylith", "attack", ["Astegon"], ["Pingu"],
     {"damage_attempted": 3, "damage": 0, "milled": 2, "lucky": True}, 63),
    (67, 8, None, "Chylith", "turn_opened", [], [], {"souls": 9, "souls_cap": 9, "hand": [
        "Aurora Guide", "Chillet", "Cryolinx", "Elphidran", "Jormuntide", "Pengullet", "Pengullet",
        "Pyrin Noct", "Reindrix", "Relaxaurus", "Zoe's Strategy",
    ]}, None),
    (68, 8, None, "Chylith", "play_card", ["Aurora Guide"], [], {"cost": 2}, None),
    (69, 8, None, "Chylith", "draw", [], [], {"amount": 1}, None),
    (70, 8, None, "Chylith", "put_on_top", ["Jormuntide"], [], None, None),
    (71, 8, None, "Chylith", "deploy", ["Chillet"], [], {"slot": 1, "cost": 5}, None),
    (72, 8, None, "Chylith", "reveal", ["Jormuntide"], [], None, None),
    (73, 8, None, "Chylith", "deploy", ["Jormuntide"], [], None, 72),
    (74, 8, None, "Chylith", "draw", [], [], {"amount": 1}, None),
    (77, 9, None, "Pingu", "bare_marker", ["Reindrix"], [], None, None),
    (78, 9, None, "Pingu", "attack_interrupted", ["Jormuntide"], [], None, 77),
    (79, 9, None, "Pingu", "battle_attack", ["Chillet"], ["Elphidran"],
     {"attacker_power_expr": "900", "attacker_power": 900, "defender_power_expr": "600", "defender_power": 600}, None),
    (80, 9, None, "Pingu", "bare_marker", ["Chillet"], [], None, None),
    (81, 9, None, "Pingu", "destroyed", ["Elphidran"], [], {"damage": 900, "power": 600}, 80),
    (84, 10, None, "Chylith", "attack_declare", ["Astegon"], ["Chillet"], None, None),
    (85, 10, None, "Pingu", "interrupt", ["Cryolinx"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (86, 10, None, "Chylith", "deploy", ["Pengullet Rocket Launcher"], [], None, None),
    (89, 11, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Chillet", "Cryolinx", "Elphidran", "Pengullet", "Pengullet", "Pyrin Noct", "Reindrix",
        "Relaxaurus", "Strike from the Darkness", "Zoe's Strategy",
    ]}, None),
    (92, 12, None, "Pingu", "bare_marker", ["Reindrix"], [], None, None),
    (93, 12, None, "Pingu", "attack_interrupted", ["Jormuntide"], [], None, 92),
    (94, 12, None, "Pingu", "play_card", ["Strike from the Darkness"], [], {"cost": 4}, None),
    (95, 12, None, "Pingu", "attack", ["Chillet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 10, "life_after": 8}, None),
    (96, 12, None, "Pingu", "deploy", ["Pengullet"], [], {"slot": 3, "cost": 4}, None),
    (97, 12, None, "Pingu", "bare_marker", [], [], None, None),
    (98, 12, None, "Pingu", "attack", ["Pengullet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 2, "lucky": True}, 97),
    (101, 13, None, "Chylith", "play_card", ["Aurora Guide"], [], None, None),
    (102, 13, None, "Chylith", "deploy", ["Chillet"], [], None, None),
    (103, 13, None, "Chylith", "deploy", ["Jormuntide"], [], None, None),
    (104, 13, None, "Chylith", "rested", ["Chillet"], [], None, None),
    (105, 13, None, "Chylith", "status_effect", ["Chillet"], [], {"effect": "wont_stand_next_stand_phase"}, None),
    (106, 13, None, "Chylith", "activate", ["Pengullet Rocket Launcher"], [], None, None),
    (107, 13, None, "Chylith", "power_buff", ["Jormuntide"], [], {"amount": 200}, None),
    (108, 13, None, "Chylith", "attack_declare", ["Jormuntide"], ["Jormuntide"], None, None),
    (109, 13, None, "Pingu", "interrupt", ["Cryolinx"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (110, 13, None, "Chylith", "attack_declare", ["Chillet"], ["Pengullet"], None, None),
    (111, 13, None, "Pingu", "interrupt", ["Pyrin Noct"], [], {"cost": {"type": "soul", "amount": 1}, "result": "nullified"}, None),
    (114, 14, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Chillet", "Elphidran", "Pengullet", "Pyrin Noct", "Reindrix", "Relaxaurus", "Zoe's Strategy",
    ]}, None),
    (115, 14, None, "Chylith", "deploy", ["Relaxaurus"], [], {"slot": 4, "cost": 7}, None),
    (118, 15, None, "Pingu", "bare_marker", [], [], None, None),
    (119, 15, None, "Pingu", "attack", ["Relaxaurus"], ["them"],
     {"damage_attempted": 3, "damage": 0, "milled": 1, "lucky": True}, 118),
    (120, 15, None, "Pingu", "bare_marker", [], [], None, None),
    (121, 15, None, "Pingu", "attack", ["Pengullet"], ["them"],
     {"damage_attempted": 2, "damage": 0, "milled": 2, "lucky": True}, 120),
    (122, 15, None, "Pingu", "play_card", ["Zoe's Strategy"], [], {"cost": 3}, None),
    (123, 15, None, "Pingu", "draw", [], [], {"amount": 1}, None),
    (124, 15, None, "Pingu", "butcher", ["Pengullet"], [], None, None),
    (125, 15, None, "Pingu", "bare_marker", ["Reindrix"], [], None, None),
    (126, 15, None, "Pingu", "attack_interrupted", ["Jormuntide"], [], None, 125),
    (129, 16, None, "Chylith", "deploy", ["Pengullet"], [], None, None),
    (130, 16, None, "Chylith", "activate", ["Pengullet Rocket Launcher"], [], None, None),
    (131, 16, None, "Chylith", "power_buff", ["Pengullet"], [], {"amount": 500}, None),
    (132, 16, None, "Chylith", "activate", ["Pengullet"], [], None, None),
    (133, 16, None, "Chylith", "deploy", ["Pengullet"], [], None, None),
    (134, 16, None, "Chylith", "attack_declare", ["Pengullet"], ["Jormuntide"], None, None),
    (135, 16, None, "Pingu", "interrupt", ["Pyrin Noct"], [], {"cost": {"type": "discard", "card": "Elphidran"}, "result": "nullified"}, None),
    (138, 17, None, "Chylith", "turn_opened", [], [], {"souls": 10, "souls_cap": 10, "hand": [
        "Chillet", "Cryolinx", "Elphidran", "Pengullet", "Reindrix",
    ]}, None),
    (139, 17, None, "Chylith", "deploy", ["Chillet"], [], {"slot": 1, "cost": 5}, None),
    (140, 17, None, "Chylith", "reveal", ["Blazehowl Noct"], [], {"outcome": "added_to_hand"}, None),
    (143, 18, None, "Pingu", "battle_attack", ["Chillet"], ["Pengullet"],
     {"attacker_power_expr": "900", "attacker_power": 900, "defender_power_expr": "600", "defender_power": 600}, None),
    (144, 18, None, "Pingu", "bare_marker", ["Chillet"], [], None, None),
    (145, 18, None, "Pingu", "destroyed", ["Pengullet"], [], {"damage": 900, "power": 600}, 144),
    (146, 18, None, "Pingu", "attack", ["Relaxaurus"], ["them"],
     {"damage": 3, "milled": 3, "lucky": False, "life_before": 8, "life_after": 5}, None),
    (147, 18, None, "Pingu", "attack", ["Jormuntide"], ["them"],
     {"damage": 3, "milled": 3, "lucky": False, "life_before": 5, "life_after": 2}, None),
    (148, 18, None, "Pingu", "deploy", ["Pengullet"], [], {"slot": 3, "cost": 4}, None),
    (149, 18, None, "Pingu", "attack", ["Pengullet"], ["them"],
     {"damage": 2, "milled": 2, "lucky": False, "life_before": 2, "life_after": 0}, None),
    (151, 18, None, "Pingu", "win_declared", [], [], {"winner": "PINGU"}, None),
    (152, 18, None, "Pingu", "loss_reason", [], [], {"loser": "Chylith", "reason": "life_hit_zero"}, None),
    (153, 18, None, "Pingu", "final_life", [], [], {"winner": "Pingu", "winner_life": 8, "loser": "Chylith", "loser_life": 0}, None),
]


def test_meta(pingu03_path):
    result = parse_log(pingu03_path)
    assert result["meta"] == EXPECTED_META


def test_event_count(pingu03_path):
    result = parse_log(pingu03_path)
    assert len(result["events"]) == len(EXPECTED_EVENTS)


def test_every_event_matches_by_hand(pingu03_path):
    result = parse_log(pingu03_path)
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


def test_no_unparsed_lines(pingu03_path):
    result = parse_log(pingu03_path)
    bad = [e for e in result["events"] if e["action"] in ("unparsed", "unrecognized_line_format")]
    assert bad == []


def test_chillet_free_deploy_chain_is_linked(pingu03_path):
    """Chillet's own reveal-then-free-deploy chain, deployed turn 8 (line
    71) revealing and free-deploying Jormuntide (lines 72-73) -- the only
    such chain in this fixture, unlike pingu_01's two."""
    result = parse_log(pingu03_path)
    by_line = {e["line_index"]: e for e in result["events"]}
    assert by_line[72]["action"] == "reveal"
    assert by_line[73]["action"] == "deploy"
    assert by_line[73]["caused_by_line"] == 72
    assert by_line[73]["cards"] == by_line[72]["cards"] == ["Jormuntide"]


def test_bare_form_interrupts_link_to_the_attack_they_nullified(pingu03_path):
    """The header's own count: 4 bare-form interrupts (turns 6, 9, 12, 15)."""
    result = parse_log(pingu03_path)
    bare_interrupts = [
        e for e in result["events"]
        if e["action"] == "bare_marker" and any(
            e2["caused_by_line"] == e["line_index"] and e2["action"] == "attack_interrupted"
            for e2 in result["events"]
        )
    ]
    assert len(bare_interrupts) == 4
    assert [e["turn"] for e in bare_interrupts] == [6, 9, 12, 15]


def test_full_form_interrupts_count(pingu03_path):
    """The header's own count: 5 additional full-form interrupts (turns
    7, 10, 13 x2, 16) -- not counted in the bare-form total above."""
    result = parse_log(pingu03_path)
    full_interrupts = [e for e in result["events"] if e["action"] == "interrupt"]
    assert len(full_interrupts) == 5
    assert [e["turn"] for e in full_interrupts] == [7, 10, 13, 13, 16]


def test_lethal_line_is_last_and_zeroes_opponent_life(pingu03_path):
    result = parse_log(pingu03_path)
    events = [e for e in result["events"] if e["action"] == "attack"]
    last = events[-1]
    assert last["line_index"] == 149
    assert last["resolution"]["life_after"] == 0


def test_provenance_and_gap_expectation_metadata(pingu03_path):
    """The testable prediction from the corpus survey: pingu_03 claims no
    unexplained gap and is not one of the four reconstructed-from-raw-log
    files, so both metadata fields should say so."""
    result = parse_log(pingu03_path)
    assert result["meta"]["provenance"] == "curated"
    assert result["meta"]["gap_expectation"] == "clean"


def test_power_buff_strips_possessive_subject(pingu03_path):
    """The real parser bug this fixture found: lines 107 and 131 name
    their card as "Chylith's Jormuntide"/"Chylith's Pengullet" in the raw
    text -- the possessive subject must be stripped into the actor field,
    not left baked into the card name, the same way every other
    subject-bearing pattern in this grammar already handles it."""
    result = parse_log(pingu03_path)
    by_line = {e["line_index"]: e for e in result["events"]}
    assert by_line[107]["cards"] == ["Jormuntide"]
    assert by_line[107]["actor"] == "Chylith"
    assert by_line[131]["cards"] == ["Pengullet"]
    assert by_line[131]["actor"] == "Chylith"
