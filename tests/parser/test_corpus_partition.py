"""Partition check between the two corpus guards.

test_corpus.py claims an explicit enumerated list (the bracketed
fixtures); test_format_a_corpus.py claims everything else it can glob.
That split only stays correct as a whole if nothing in data/replays/
falls outside both -- a file claimed by neither would silently sit
outside every zero-unparsed-lines guard this project has, and a file
claimed by both would mean one of the two lists needs narrowing. Either
is a real gap in corpus coverage, not an accounting nicety, so it's
asserted directly rather than left to be noticed by absence.

Scoped to top-level data/replays/*.log only, matching what test_corpus.py
and test_format_a_corpus.py actually contend over -- top_players_all20's
*.txt files live in a different directory and extension, unambiguously
test_format_a_corpus.py's alone, and were never in question.
"""

from conftest import LOG_DIR

from test_corpus import LOG_FILES as BRACKETED_FILES
from test_format_a_corpus import FORMAT_A_FILES


def test_every_top_level_log_is_claimed_by_exactly_one_corpus_guard():
    all_logs = set(LOG_DIR.glob("*.log"))
    bracketed = set(BRACKETED_FILES)
    format_a = {p for p in FORMAT_A_FILES if p.parent == LOG_DIR}

    unclaimed = all_logs - bracketed - format_a
    double_claimed = bracketed & format_a

    assert unclaimed == set(), f"claimed by neither corpus guard: {sorted(p.name for p in unclaimed)}"
    assert double_claimed == set(), f"claimed by both corpus guards: {sorted(p.name for p in double_claimed)}"
