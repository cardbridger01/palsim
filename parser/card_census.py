"""Card census: what does cards/ still need to build, in what order, and
-- separately -- how much of each has ever actually been checked against
anything?

    python -m parser.card_census

## The target is all 150 CSV rows (144 distinct names), not the 89 the
## corpus references

An earlier version of this tool scoped "what to build" to the corpus's
own 89 referenced names. That's wrong for this project's actual use: the
deck optimizer searches the full legal card pool per color pair (~80
cards), and a card cards/ doesn't implement is invisible to that search
-- any deck built around it can't be found. Restricting the build target
to what today's corpus happens to reference would bake the existing meta
into the search and guarantee the optimizer only ever rediscovers known
decks. So this tool reports against the full CSV pool (150 rows, 144
distinct names once reprints are merged), not just the corpus subset.

## Build order is still corpus-frequency, for a different reason

Frequency in the corpus doesn't mean "matters more to the metagame" --
150 - 89 = 61 cards have zero corpus appearances and may well be exactly
the ones a "secret deck" search would want. It means "checkable": a card
appearing in 40 logs is a card whose implementation gets exercised, and
disagreed with, by real recorded games the moment there's an engine-side
Format A replay driver. Building those first means every later,
unvalidated card is written against engine primitives (deploy, attack,
damage, the mill loop) that have already been hammered on hard. So the
build order is three tiers, corpus-validated first (ranked by log count,
same ordering as before) then test-only then unvalidated -- see
VALIDATION STATUS below.

## VALIDATION STATUS -- the real point of this tool now

Every not-yet-implemented name gets one of three labels:

  - **corpus-validated**: appears in >=1 Format A log. Once an engine-side
    replay driver exists for Format A (it doesn't yet -- only the
    bracketed-format `replay/` crate does), this card's behaviour gets
    checked against real recorded games, the same way the 8 already-
    implemented cards are checked by the bracketed fixtures today.
  - **test-only**: zero corpus appearances, but its CardNumber appears in
    `data/palworld_tcg_qa_dataset.json`'s `related_cards` for at least
    one ruling (hard rule 3: matched by number, never name -- see
    `qa_covered_card_numbers()`). Checked against an official ruling,
    never against an actual game.
  - **unvalidated**: neither. Built from `effect_text` and general rules
    knowledge alone, checked against nothing.

This distinction is where CLAUDE.md's own worry about the optimizer
finding a "secret deck" actually lands: if the search surfaces a strong
deck built around an unvalidated card, the first question is whether
that card's effect is really that strong or whether it's a wrong
`effect_text` reading that's never been exercised by anything -- and the
census is what tells you, at a glance, which of those two is even
plausible for that specific card.

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
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

from parser.format_a_parser import parse_log

REPO_ROOT = Path(__file__).resolve().parent.parent
LOG_DIR = REPO_ROOT / "data" / "replays"
CARDS_SRC_DIR = REPO_ROOT / "cards" / "src"
CSV_PATH = REPO_ROOT / "data" / "11cardattributes-with-elements.csv"
QA_DATASET_PATH = REPO_ROOT / "data" / "palworld_tcg_qa_dataset.json"

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


# The QA dataset's own related_cards IDs carry a leading "E" (an
# extraction/print marker, not part of the card number) and sometimes a
# trailing rarity-variant suffix (OSR/SP/SR/SSP/TSP/TSR) for a specific
# print -- neither is in the CSV's CardNumber column, so both are
# stripped before comparing. A few refs (e.g. "BT01-039") are a product
# ("BT01") not in this CSV at all; those just never match, correctly.
_QA_REF_RE = re.compile(r"^E?(?P<set>BP\d+|TD\d+|BT\d+|PR)-(?P<num>\d+)[A-Za-z]*$")


def qa_covered_card_numbers() -> set[str]:
    """Base CardNumbers (e.g. "BP01-073") referenced by at least one QA
    ruling's related_cards -- hard rule 3: matched by number, extracted
    from the dataset's own IDs, never by name."""
    data = json.loads(QA_DATASET_PATH.read_text(encoding="utf-8"))
    numbers: set[str] = set()
    for item in data["items"]:
        for ref in item.get("related_cards", []):
            m = _QA_REF_RE.match(ref)
            if m:
                numbers.add(f"{m.group('set')}-{m.group('num')}")
    return numbers


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


def _print_tier(title: str, names: list[str], csv_by_name, real_appears_in, start: int) -> int:
    if not names:
        return start
    print()
    print(f"=== {title} ({len(names)}) ===")
    print(f"{'#':<4}{'name':<32}{'logs':>5}  {'CardNumber':<14}{'Lucky':<6}effect_text")
    idx = start
    for name in names:
        rows = csv_by_name[name]
        num = "+".join(n for n, _, _ in rows)
        effect = rows[0][1]
        lucky = "Y" if any(is_lucky for _, _, is_lucky in rows) else ""
        effect_short = (effect[:80] + "...") if len(effect) > 80 else effect
        logs = len(real_appears_in.get(name, ()))
        print(f"{idx:<4}{name:<32}{logs:>5}  {num:<14}{lucky:<6}{effect_short}")
        idx += 1
    return idx


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
    qa_numbers = qa_covered_card_numbers()
    appears_in, names_by_log = census(files, csv_by_name)

    noise = {n: f for n, f in appears_in.items() if n not in csv_by_name and n not in implemented}
    all_names = set(csv_by_name)  # the full 144-distinct-name target, per the module doc comment
    remaining = sorted(all_names - implemented)

    def is_qa_covered(name: str) -> bool:
        card_numbers = {num for num, _, _ in csv_by_name[name]}
        return bool(card_numbers & qa_numbers)

    corpus_validated = [n for n in remaining if n in appears_in]
    test_only = [n for n in remaining if n not in appears_in and is_qa_covered(n)]
    unvalidated = [n for n in remaining if n not in appears_in and not is_qa_covered(n)]
    corpus_validated.sort(key=lambda n: -len(appears_in[n]))
    test_only.sort()
    unvalidated.sort()

    print(f"Parsed {len(files)} Format A logs.")
    print(f"CSV: {sum(len(v) for v in csv_by_name.values())} rows, {len(all_names)} distinct names  |  already in cards/: {len(implemented)}")
    print(f"Remaining {len(remaining)}: {len(corpus_validated)} corpus-validated, {len(test_only)} test-only, {len(unvalidated)} unvalidated")
    if noise:
        print(f"(plus {len(noise)} unresolved corpus-parse artifacts, not real cards -- see below)")

    # Lucky is read straight from the CSV per card, never assumed -- every
    # damage check in the corpus (the mill loop's is_lucky read) depends
    # on it, and a wrong flag would silently corrupt a life total in
    # exactly the way the replay life-check assertions are built to
    # catch, but only if the flag actually comes from data. 24 of 150 CSV
    # rows are Lucky; shown here so it's visible at build time.
    next_idx = 1
    next_idx = _print_tier(
        "TIER 1 -- corpus-validated (build these first: checkable against real games once a Format A replay driver exists)",
        corpus_validated, csv_by_name, appears_in, next_idx,
    )
    next_idx = _print_tier(
        "TIER 2 -- test-only (an official QA ruling covers this card's number; no replay evidence)",
        test_only, csv_by_name, appears_in, next_idx,
    )
    _print_tier(
        "TIER 3 -- unvalidated (built from effect_text alone; checked against nothing -- see module doc comment's optimizer-secret-deck note)",
        unvalidated, csv_by_name, appears_in, next_idx,
    )

    if noise:
        print()
        print(f"=== {len(noise)} unresolved 1-log parse artifacts (not real cards, not in any tier above) ===")
        for name, logs in sorted(noise.items(), key=lambda kv: -len(kv[1])):
            print(f"  {name!r} -- e.g. {sorted(logs)[0]}")

    ranked_corpus_validated = corpus_validated  # already sorted by -log count
    print()
    print(
        f"=== corpus REPLAY-VALIDATION depth curve: N cards (tier 1, in order) -> logs fully replayable with only those N + the "
        f"{len(implemented)} already implemented -- NOT the optimizer's legal-pool requirement, which is all {len(all_names)} regardless ==="
    )
    print(f"{'N':>4}{'logs covered':>15}  / {len(files)}")
    for n, covered in coverage_curve(names_by_log, ranked_corpus_validated):
        print(f"{n:>4}{covered:>15}  / {len(files)}")


if __name__ == "__main__":
    main()
