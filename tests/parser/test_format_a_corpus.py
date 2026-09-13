"""Corpus-wide guardrail for the Format A parser -- same spirit as
test_corpus.py, over the disjoint set of files that parser covers
(battle1/2/3's bracketed format is untouched by this module and vice
versa).

Runs across every Format A file actually present: the 44 matches split
from COMBINED_TIER1_TIER2_44_MATCHES.txt (data/replays/*.log, minus the
three bracketed fixtures) plus the 20 top_players_all20/*.txt files.
Zero unparsed/unrecognized lines is the bar -- a failure here means a
sentence shape this module hasn't been taught yet, not a card-behaviour
gap (this parser doesn't interpret card behaviour at all).

Also records, per file, the two things the corpus survey flagged as
worth tracking rather than re-discovering later:

  - provenance: "curated" (from the site's rendered/curated log) vs
    "reconstructed" (rebuilt from the raw tagged event export after a
    site bug -- ze_01, ze_03, ze_08, pingu_06). Reconstructed files are
    closer to source of truth.
  - gap_expectation: "clean" (the file's own header claims no
    unexplained life-total gap) vs "unstated" (it doesn't -- three
    files, ze_01/ze_03/ze_08, are the ones the other files name as the
    exceptions). This is the testable prediction from the corpus
    survey: once something can sum named damage against stated life
    totals, "clean" files should pass that audit and "unstated" files
    should fail it. Not built yet -- this module only structures lines,
    it doesn't audit life totals -- so this test only records the
    expectation, it does not check it.
"""

import pytest

from parser.format_a_parser import parse_log

from conftest import BRACKETED_FIXTURE_NAMES, LOG_DIR

FORMAT_A_FILES = sorted(
    [p for p in LOG_DIR.glob("*.log") if p.name not in BRACKETED_FIXTURE_NAMES]
    + list((LOG_DIR / "top_players_all20").glob("*.txt"))
)


@pytest.mark.parametrize("log_path", FORMAT_A_FILES, ids=lambda p: p.name)
def test_no_unparsed_lines_anywhere_in_the_format_a_corpus(log_path):
    result = parse_log(log_path)
    bad = [
        (e["line_index"], e["text"])
        for e in result["events"]
        if e["action"] in ("unparsed", "unrecognized_line_format")
    ]
    assert bad == [], f"{log_path.name}: unrecognized line(s) {bad}"


def test_corpus_is_not_accidentally_empty():
    # Guards against a silently-broken glob (e.g. either directory
    # moved) making the parametrized test above pass with zero cases.
    # 44 (COMBINED split) + 20 (top_players_all20) = 64.
    assert len(FORMAT_A_FILES) == 64


def test_provenance_and_gap_expectation_survey():
    """Records what the corpus survey found, from the files themselves
    -- not asserted against a hardcoded expected set, so this breaks
    loudly (not silently drifts) if a header's own language changes."""
    reconstructed = []
    gap_unstated = []
    for path in FORMAT_A_FILES:
        result = parse_log(path)
        meta = result["meta"]
        if meta.get("provenance") == "reconstructed":
            reconstructed.append(path.name)
        if meta.get("gap_expectation") == "unstated":
            gap_unstated.append(path.name)

    # The four files the corpus survey found: ze_01, ze_03, ze_08 (site
    # bug: curated log panel evicts early turns) + pingu_06.
    assert len(reconstructed) == 4, f"expected 4 reconstructed-provenance files, found {reconstructed}"

    # gap_expectation is "unstated" for every file that doesn't
    # explicitly claim a clean audit -- this includes the 44 COMBINED
    # matches (which never make the claim either way) alongside the 3
    # named Ze exceptions, so this is a much larger set than just those
    # 3; it is not itself the corpus-survey finding, just the raw signal
    # that finding was read off of for the top_players_all20 subset.
    assert set(gap_unstated) >= {"ze_01_fe1eee24.txt", "ze_03_a0afb991.txt", "ze_08_c99f3835.txt"}
