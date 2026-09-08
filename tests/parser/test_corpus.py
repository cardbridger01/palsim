"""Corpus-wide guardrail.

Runs across every *.log file actually present in data/replays/ -- not just
the three known fixtures -- so that when the rest of the ~165-log corpus
(tier1_format_a_all34.zip, replay_logs.zip) is dropped in, any construct
these three logs don't cover (a keyword, a new sentence shape, a card-text
quirk) fails the build loudly as `unparsed`/`unrecognized_line_format`
instead of silently accumulating in the JSON output.

If this test fails on a new log: read the failing line(s), add a pattern
(and builder, if it carries resolution detail) to parser/replay_parser.py,
and extend the fixture tests if the new shape is worth locking down.
"""

from pathlib import Path

import pytest

from parser.replay_parser import parse_log

LOG_DIR = Path(__file__).resolve().parents[2] / "data" / "replays"
LOG_FILES = sorted(LOG_DIR.glob("*.log"))


@pytest.mark.parametrize("log_path", LOG_FILES, ids=lambda p: p.name)
def test_no_unparsed_lines_anywhere_in_the_corpus(log_path):
    result = parse_log(log_path)
    bad = [
        (e["line_index"], e["text"])
        for e in result["events"]
        if e["action"] in ("unparsed", "unrecognized_line_format")
    ]
    assert bad == [], f"{log_path.name}: unrecognized line(s) {bad}"


def test_corpus_is_not_accidentally_empty():
    # Guards against a silently-broken glob (e.g. LOG_DIR moved) making the
    # parametrized test above pass with zero cases.
    assert len(LOG_FILES) >= 3
