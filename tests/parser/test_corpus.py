"""Corpus-wide guardrail for the bracketed `[Tn phase]` format --
replay_parser.py's own three hand-authored fixture logs.

Scoped to BRACKETED_FIXTURE_NAMES (conftest.py), an explicit enumerated
list, not a directory glob. data/replays/ also holds the 44 Format A
logs split from COMBINED_TIER1_TIER2_44_MATCHES.txt (see
test_format_a_corpus.py, which claims those under a different grammar
entirely) -- a glob here would silently re-absorb them and fail loudly
for a reason that carries no information (wrong grammar, not a real
parsing gap). A fourth bracketed log has to be added to the list by
hand; test_corpus_partition.py asserts nothing dropped into
data/replays/ falls through the crack between this file's list and
test_format_a_corpus.py's glob.

If this test fails on one of the three: read the failing line(s), add a
pattern (and builder, if it carries resolution detail) to
parser/replay_parser.py, and extend the fixture tests if the new shape
is worth locking down.
"""

import pytest

from parser.replay_parser import parse_log

from conftest import BRACKETED_FIXTURE_NAMES, LOG_DIR

LOG_FILES = [LOG_DIR / name for name in BRACKETED_FIXTURE_NAMES]


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
    # Guards against BRACKETED_FIXTURE_NAMES being edited down to
    # nothing and this parametrized test silently passing with zero cases.
    assert len(LOG_FILES) >= 3
