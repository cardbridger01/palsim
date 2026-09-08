"""Structural regression test for the longer LOSS replay.

Not hand-verified line by line (172 events) -- battle1 carries the
hand-checked ground truth. This file checks the invariants that must hold
for any log: full line coverage, no unparsed text, and line-position
ordering.
"""

from parser.replay_parser import parse_log


def test_parses_without_error(battle2_path):
    result = parse_log(battle2_path)
    assert result["events"]


def test_meta(battle2_path):
    result = parse_log(battle2_path)
    meta = result["meta"]
    assert meta["recordId"] == "local-bg-chillet-02"
    assert meta["player"] == "Edify (P2, on the draw)"
    assert meta["opponent"] == "FluffysFury"
    assert meta["opponent_colors"] == "Blue/Purple"
    assert meta["result"] == "LOSS turn 10"
    assert meta["notes"] == [
        "pivot at T8 - all 10 souls spent on Relaxaurus + Victor's, Zoe's killed it immediately"
    ]


def test_no_unparsed_lines(battle2_path):
    result = parse_log(battle2_path)
    bad = [(e["line_index"], e["text"]) for e in result["events"] if e["action"] in ("unparsed", "unrecognized_line_format")]
    assert bad == []


def test_events_are_in_strict_line_order(battle2_path):
    result = parse_log(battle2_path)
    line_indices = [e["line_index"] for e in result["events"]]
    assert line_indices == sorted(line_indices)


def test_every_event_has_the_required_fields(battle2_path):
    result = parse_log(battle2_path)
    required = {"line_index", "turn", "phase", "actor", "action", "cards", "targets", "resolution", "caused_by_line", "text"}
    for event in result["events"]:
        assert required <= event.keys()
        assert event["actor"] in ("player", "opponent")
        assert event["phase"] in ("main", "stand", "draw", "soul", "end", "waiting")


def test_game_ends_at_zero_life_on_edify_side_matching_loss(battle2_path):
    result = parse_log(battle2_path)
    last = result["events"][-1]
    assert last["action"] == "attack"
    assert last["resolution"]["life_after"] == 0
    assert last["targets"] == ["Edify"]


def test_chillet_free_deploy_chains_are_linked(battle2_path):
    result = parse_log(battle2_path)
    by_line = {e["line_index"]: e for e in result["events"]}
    for reveal_line, deploy_line in [(36, 37), (85, 86)]:
        assert by_line[reveal_line]["action"] == "reveal"
        assert by_line[deploy_line]["action"] == "deploy"
        assert by_line[deploy_line]["caused_by_line"] == reveal_line
