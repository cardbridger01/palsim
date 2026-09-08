"""Structural + anomaly regression test for the Blue/Red WIN replay.

battle3 is the file the README calls out by name: at T7, [Tn waiting] lines
carry a LOWER turn number (6) than the [T7 waiting] lines surrounding them,
because they log the resolution of an interrupt cast against a turn-6
declared attack. A parser that sorts or groups by the bracketed turn number
scrambles this sequence; this file locks down that it doesn't.
"""

from parser.replay_parser import parse_log

# Lines 92-101 verbatim (see data/replays/battle3_bg-chillet_vs_blue-red_WIN.log).
# turn 92-97 = 7, then 98 drops to 6, 99 back to 7, 100 drops to 6 again, 101 = 7.
T7_ANOMALY_LINE_RANGE = range(92, 102)
EXPECTED_TURNS_92_TO_101 = [7, 7, 7, 7, 7, 7, 6, 7, 6, 7]


def _events_by_line(result):
    return {e["line_index"]: e for e in result["events"]}


def test_parses_without_error(battle3_path):
    result = parse_log(battle3_path)
    assert result["events"]


def test_meta_has_both_notes(battle3_path):
    result = parse_log(battle3_path)
    meta = result["meta"]
    assert meta["recordId"] == "local-bg-chillet-03"
    assert meta["result"] == "WIN turn 7"
    assert meta["notes"] == [
        "lethal turn was ~37% to connect on both attacks; opponent had milled 10 cards with zero Lucky",
        "ANOMALY at T7 - Ignis attacks twice then blocks on Edify's turn; per 7.2.1 it should "
        "still be rested. Flag for engine test / client behaviour check.",
    ]


def test_no_unparsed_lines(battle3_path):
    result = parse_log(battle3_path)
    bad = [(e["line_index"], e["text"]) for e in result["events"] if e["action"] in ("unparsed", "unrecognized_line_format")]
    assert bad == []


def test_t7_anomaly_turn_numbers_are_preserved_verbatim(battle3_path):
    """The bracketed turn number for each line must be recorded exactly as
    logged, including the two lines that regress to T6."""
    result = parse_log(battle3_path)
    by_line = _events_by_line(result)
    turns = [by_line[i]["turn"] for i in T7_ANOMALY_LINE_RANGE]
    assert turns == EXPECTED_TURNS_92_TO_101


def test_t7_anomaly_ordered_by_line_position_not_turn_number(battle3_path):
    """Event order must follow line_index, never a sort/group by turn --
    the turn sequence above is deliberately non-monotonic."""
    result = parse_log(battle3_path)
    events_in_range = [e for e in result["events"] if e["line_index"] in T7_ANOMALY_LINE_RANGE]
    line_indices = [e["line_index"] for e in events_in_range]
    assert line_indices == list(T7_ANOMALY_LINE_RANGE)
    turns = [e["turn"] for e in events_in_range]
    assert turns != sorted(turns), "turn sequence should be non-monotonic across this range"


def test_t7_anomaly_interrupt_lines_attributed_to_player(battle3_path):
    """Lines 98 and 100 are Edify interrupting the opponent's declared
    attack; despite the 'waiting' tag (opponent's turn), the text names
    Edify as the actor directly."""
    result = parse_log(battle3_path)
    by_line = _events_by_line(result)
    for line_index in (98, 100):
        event = by_line[line_index]
        assert event["phase"] == "waiting"
        assert event["actor"] == "player"
        assert event["action"] == "interrupt"


def test_known_client_anomaly_lines_present(battle3_path):
    """Sanity check the specific card names in the README's 'known anomaly'
    (Ignis attacks twice, then blocks on Edify's own T7) are parsed, not
    silently dropped or misclassified."""
    result = parse_log(battle3_path)
    by_line = _events_by_line(result)
    assert by_line[92]["action"] == "attack_declare"
    assert by_line[92]["cards"] == ["Jormuntide Ignis – Savage Lava Dragon"]
    assert by_line[97]["action"] == "attack_declare"
    assert by_line[97]["cards"] == ["Jormuntide Ignis – Savage Lava Dragon"]
    assert by_line[113]["action"] == "block"
    assert by_line[113]["cards"] == ["Jormuntide Ignis – Savage Lava Dragon"]
    assert by_line[113]["phase"] == "waiting"


def test_events_are_in_strict_line_order(battle3_path):
    result = parse_log(battle3_path)
    line_indices = [e["line_index"] for e in result["events"]]
    assert line_indices == sorted(line_indices)


def test_lethal_line_matches_win_turn_7(battle3_path):
    result = parse_log(battle3_path)
    last = result["events"][-1]
    assert last["line_index"] == 116
    assert last["action"] == "attack"
    assert last["resolution"]["life_after"] == 0


def test_chillet_free_deploy_chain_is_linked(battle3_path):
    result = parse_log(battle3_path)
    by_line = {e["line_index"]: e for e in result["events"]}
    assert by_line[107]["action"] == "reveal"
    assert by_line[108]["action"] == "deploy"
    assert by_line[108]["caused_by_line"] == 107
