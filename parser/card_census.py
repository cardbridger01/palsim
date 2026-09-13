"""Card census: what does the 64-log Format A corpus actually need from
cards/, and in what order should the rest of Phase 3 build it?

    python -m parser.card_census

Parses every Format A log (format_a_parser.parse_log), collects the
distinct card/gear/event names each one references, cross-references
against data/11cardattributes-with-elements.csv (hard rule 3: keyed by
name only as a display lookup, never used to infer behaviour -- see
IMPLEMENTED below), and reports every name NOT yet scripted in cards/,
sorted by how many distinct logs reference it. That ordering -- not
"whatever card looked interesting" -- is the build order the corpus
itself is asking for.

## IMPLEMENTED, and why it's a hand-maintained set

cards/*.rs's own doc comments (`/// BP01-NNN, Name -- Subtitle.`) are
the source of truth for what's actually scripted; this list is derived
from grepping them (see main()), not hand-typed. What's checked in here
as IMPLEMENTED is a snapshot, and needs re-deriving (main() does this
automatically) whenever a card script is added -- the whole point of
this tool is to stay honest about the actual remaining count rather
than reporting a stale one.

## Card-name resolution, and its one known blind spot

Format A logs mostly use the bare main name ("Chillet"), sometimes the
full "Main -- Subtitle" form; both are normalized to the main name (the
part before " -- ") before matching the CSV's own Name column. A
smaller number of parser patterns capture "Owner's Card" as a single
`cards` entry (a possessive-prefixed "destroyed" line, mainly) rather
than splitting the owner out -- `_resolve_possessive` retries the CSV
lookup with a leading "X's " stripped for exactly those, falling back
to the raw name (and showing up in the NOT_IN_CSV list) if that doesn't
resolve either. A residual few dozen 1-log entries are other compound-
sentence artifacts (a numbered choice prefix, a multi-card "to pay a
cost" list) that don't change the build order -- they're already at the
bottom of a frequency-sorted list -- so this tool reports them but
doesn't chase each one down.
"""

from __future__ import annotations

import csv
import re
import sys
from collections import defaultdict
from pathlib import Path

from parser.format_a_parser import parse_log

REPO_ROOT = Path(__file__).resolve().parent.parent
LOG_DIR = REPO_ROOT / "data" / "replays"
CARDS_SRC_DIR = REPO_ROOT / "cards" / "src"
CSV_PATH = REPO_ROOT / "data" / "11cardattributes-with-elements.csv"

_BRACKETED_FIXTURES = {
    "battle1_bg-chillet_vs_blue-purple_WIN.log",
    "battle2_bg-chillet_vs_blue-purple_LOSS.log",
    "battle3_bg-chillet_vs_blue-red_WIN.log",
}
_DOC_HEADER_RE = re.compile(r"^/// (?:BP\d+|TD\d+|PR)-\d+, (?P<name>[^-]+?)(?: -- .*)?\.\s*$")
_POSSESSIVE_RE = re.compile(r"^.+?'s (.+)$")


def format_a_files() -> list[Path]:
    return sorted(
        [p for p in LOG_DIR.glob("*.log") if p.name not in _BRACKETED_FIXTURES]
        + list((LOG_DIR / "top_players_all20").glob("*.txt"))
    )


def implemented_names() -> set[str]:
    """Derived from cards/*.rs's own `/// BP01-NNN, Name -- Subtitle.`
    doc-comment headers, not hand-maintained -- see module doc comment."""
    names = set()
    for path in CARDS_SRC_DIR.glob("*.rs"):
        for line in path.read_text(encoding="utf-8").splitlines():
            m = _DOC_HEADER_RE.match(line.strip())
            if m:
                names.add(m.group("name").strip())
    return names


def load_csv_by_name() -> dict[str, list[tuple[str, str, bool]]]:
    by_name: dict[str, list[tuple[str, str, bool]]] = defaultdict(list)
    with open(CSV_PATH, encoding="utf-8") as f:
        for row in csv.DictReader(f):
            by_name[row["Name"]].append((row["CardNumber"], row["effect_text"], row["Lucky"] == "Y"))
    return by_name


_TRAILING_SUBTITLE_PAREN_RE = re.compile(r"^(.+?) \([^)]+\)$")


def main_name(raw: str) -> str:
    return raw.split(" – ", 1)[0].split(" - ", 1)[0].strip()


def resolve_name(raw_name: str, csv_by_name: dict[str, list[tuple[str, str, bool]]]) -> str:
    """See module doc comment's "Card-name resolution" section."""
    if raw_name in csv_by_name:
        return raw_name
    m = _POSSESSIVE_RE.match(raw_name)
    if m and m.group(1) in csv_by_name:
        return m.group(1)
    # "Foxparks (A Toasty Hug)" -- a card-with-subtitle-in-parens form
    # used to disambiguate between multiple prints of the same main
    # name (Foxparks is both BP01-006 and TD01-004); the parenthetical
    # itself isn't in the CSV's Name column, so strip it and retry.
    m = _TRAILING_SUBTITLE_PAREN_RE.match(raw_name)
    if m and m.group(1) in csv_by_name:
        return m.group(1)
    return raw_name


def census(files: list[Path], csv_by_name: dict[str, list[tuple[str, str, bool]]]) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    """Returns (appears_in, names_by_log): appears_in maps resolved_name
    -> {log filenames referencing it}; names_by_log is the inverse, log
    filename -> {resolved names it references} -- needed for the
    coverage curve, which asks a per-log question ("does this log's
    full card set fit inside the top N?"), not a per-card one."""
    appears_in: dict[str, set[str]] = defaultdict(set)
    names_by_log: dict[str, set[str]] = {}
    for path in files:
        result = parse_log(path)
        names_this_file = {resolve_name(main_name(raw), csv_by_name) for e in result["events"] for raw in e["cards"] if raw.strip()}
        names_by_log[path.name] = names_this_file
        for name in names_this_file:
            appears_in[name].add(path.name)
    return appears_in, names_by_log


def coverage_curve(
    names_by_log: dict[str, set[str]],
    ranked_real_names: list[str],
) -> list[tuple[int, int]]:
    """For N = 0..len(ranked_real_names), how many of the logs are FULLY
    replayable using only the top N names from `ranked_real_names`
    (already frequency-sorted) plus whatever's already implemented?
    Already-implemented and noise (non-card) names never appear in
    `ranked_real_names`, so intersecting each log's referenced names
    with it already leaves exactly "what this log still needs" -- no
    separate implemented/noise exclusion required. Returns only the
    (N, covered_count) rows where covered_count actually changes, i.e.
    the step function's steps -- N values where nothing newly becomes
    coverable are collapsed rather than printed redundantly."""
    real_name_set = set(ranked_real_names)
    needs_by_log = {log: (names & real_name_set) for log, names in names_by_log.items()}

    steps: list[tuple[int, int]] = []
    last_covered = -1
    top_n: set[str] = set()
    for n in range(0, len(ranked_real_names) + 1):
        if n > 0:
            top_n.add(ranked_real_names[n - 1])
        covered = sum(1 for needs in needs_by_log.values() if needs <= top_n)
        if covered != last_covered:
            steps.append((n, covered))
            last_covered = covered
    return steps


def main() -> None:
    # Card effect_text uses circled-digit Soul-cost glyphs (①②③) that
    # Windows' default console codepage can't encode -- this tool is
    # meant to actually be run, not just imported, so make stdout UTF-8
    # rather than let a card's own printed cost crash the report.
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    files = format_a_files()
    csv_by_name = load_csv_by_name()
    implemented = implemented_names()
    appears_in, names_by_log = census(files, csv_by_name)

    not_implemented = {n: f for n, f in appears_in.items() if n not in implemented}
    real = {n: f for n, f in not_implemented.items() if n in csv_by_name}
    noise = {n: f for n, f in not_implemented.items() if n not in csv_by_name}
    ranked_real_names = [n for n, _ in sorted(real.items(), key=lambda kv: -len(kv[1]))]

    print(f"Parsed {len(files)} Format A logs.")
    print(f"Distinct names referenced: {len(appears_in)}  |  already in cards/: {len(implemented)}")
    print(f"Real, CSV-matched, not-yet-implemented: {len(real)}  |  unresolved 1-log artifacts: {len(noise)}")
    print()
    # Lucky is read straight from the CSV per card, never assumed --
    # every damage check in the corpus (the mill loop's is_lucky read)
    # depends on it, and a wrong flag would silently corrupt a life
    # total in exactly the way the replay life-check assertions are
    # built to catch, but only if the flag actually comes from data.
    # 24 of ~150 CSV rows are Lucky; shown here so it's visible at
    # build time, not discovered later.
    print(f"{'#':<4}{'name':<32}{'logs':>5}  {'CardNumber':<14}{'Lucky':<6}effect_text")
    for idx, name in enumerate(ranked_real_names, start=1):
        rows = csv_by_name[name]
        num = "+".join(n for n, _, _ in rows)
        effect = rows[0][1]
        lucky = "Y" if any(is_lucky for _, _, is_lucky in rows) else ""
        effect_short = (effect[:80] + "...") if len(effect) > 80 else effect
        print(f"{idx:<4}{name:<32}{len(real[name]):>5}  {num:<14}{lucky:<6}{effect_short}")

    if noise:
        print()
        print(f"=== {len(noise)} unresolved 1-log artifacts (not in build order; see module doc comment) ===")
        for name, logs in sorted(noise.items(), key=lambda kv: -len(kv[1])):
            print(f"  {name!r} -- e.g. {sorted(logs)[0]}")

    print()
    print(f"=== coverage curve: N cards (top of the build order) -> logs fully replayable with only those N + the {len(implemented)} already implemented ===")
    print(f"{'N':>4}{'logs covered':>15}  / {len(files)}")
    for n, covered in coverage_curve(names_by_log, ranked_real_names):
        print(f"{n:>4}{covered:>15}  / {len(files)}")


if __name__ == "__main__":
    main()
