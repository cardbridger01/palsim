"""One-off splitter: data/replays/COMBINED_TIER1_TIER2_44_MATCHES.txt ->
one raw .log file per match in data/replays/, named by recordId.

Run once:

    python -m parser.split_combined_matches

Not part of the test suite or any regular workflow -- the combined file
gets split once, the resulting .log files are what actually get used
(by tests/parser/test_corpus.py and, eventually, the parser itself)
from then on.

## Delimiter, found in the file rather than assumed

Every match in the combined file opens with a line of the shape

    ----- TIER 1 MATCH 1/34 (recordId 120756fe-f516-430c-8e98-ee8e239a7d48) -----
    ----- TIER 2 MATCH 1/10 done (worklist row 1, recordId 01090a7d-..., Sonken vs Skkias) -----

Tier 1 and Tier 2 rows are NOT the same shape (Tier 2 adds "done" and
"worklist row N, ... vs ..."), so a splitter that assumed one exact
string would miss half the file. What both share -- confirmed by
grepping the whole file before writing this, not assumed -- is
"----- TIER <n> MATCH " at the start and a bare "recordId <uuid>"
somewhere in the line; that shared shape is the actual delimiter this
matches on. Between tiers, a line of 10+ '#' characters marks a
section banner (e.g. "# TIER 2 -- 10/102 matches (PARTIAL...)"); those
get excluded from the preceding match's file rather than silently
appended to it.

This is NOT a claim that every real Format A log in the wild shares
this delimiter -- it's specific to how THIS combined file concatenates
matches it already extracted. A different combined file would need its
own delimiter re-derived from itself the same way.
"""

from __future__ import annotations

import re
from pathlib import Path

REPLAYS_DIR = Path(__file__).resolve().parent.parent / "data" / "replays"
COMBINED_FILE = REPLAYS_DIR / "COMBINED_TIER1_TIER2_44_MATCHES.txt"
EXPECTED_MATCH_COUNT = 44

DELIMITER_RE = re.compile(r"^----- TIER \d+ MATCH .*recordId ([0-9a-fA-F-]{36}).*-----\s*$")
BANNER_RE = re.compile(r"^#{10,}\s*$")


def split_matches(text: str) -> list[tuple[str, str]]:
    """Returns [(recordId, match_text), ...] in file order. `match_text`
    runs from the match's own delimiter line up to (but not including)
    whichever comes first: the next delimiter, a tier-section banner
    line, or end of file."""
    lines = text.splitlines(keepends=True)
    matches: list[tuple[str, str]] = []
    current_id: str | None = None
    current_lines: list[str] = []

    def flush() -> None:
        if current_id is not None:
            matches.append((current_id, "".join(current_lines)))

    for line in lines:
        stripped = line.rstrip("\n").rstrip("\r")
        delimiter_match = DELIMITER_RE.match(stripped)
        if delimiter_match:
            flush()
            current_id = delimiter_match.group(1)
            current_lines = [line]
            continue
        if current_id is not None and BANNER_RE.match(stripped):
            flush()
            current_id = None
            current_lines = []
            continue
        if current_id is not None:
            current_lines.append(line)

    flush()
    return matches


def write_matches(matches: list[tuple[str, str]], out_dir: Path) -> list[Path]:
    """Names each file by recordId (every match in this combined file
    carries one -- see module docstring); would fall back to a
    sequential `match_NNN.log` name, and disambiguate a real duplicate
    recordId with a `_2`/`_3`/... suffix, if either ever happened here,
    but neither does."""
    seen: dict[str, int] = {}
    written: list[Path] = []
    for index, (record_id, match_text) in enumerate(matches, start=1):
        name = record_id or f"match_{index:03d}"
        seen[name] = seen.get(name, 0) + 1
        if seen[name] > 1:
            name = f"{name}_{seen[name]}"
        out_path = out_dir / f"{name}.log"
        out_path.write_text(match_text, encoding="utf-8", newline="\n")
        written.append(out_path)
    return written


def main() -> None:
    text = COMBINED_FILE.read_text(encoding="utf-8")
    matches = split_matches(text)
    written = write_matches(matches, REPLAYS_DIR)

    assert len(written) == EXPECTED_MATCH_COUNT, (
        f"expected exactly {EXPECTED_MATCH_COUNT} matches (README: "
        f"'Total matches in this file: 44'), split out {len(written)}"
    )
    record_ids = [record_id for record_id, _ in matches]
    assert len(set(record_ids)) == len(record_ids), "duplicate recordId across matches -- naming would have collided"

    print(f"Split {COMBINED_FILE.name} into {len(written)} match logs in {REPLAYS_DIR}:")
    for path in written:
        print(f"  {path.name}")


if __name__ == "__main__":
    main()
