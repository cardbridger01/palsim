from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
LOG_DIR = REPO_ROOT / "data" / "replays"

# The bracketed `[Tn phase]` fixtures replay_parser.py was built against
# -- the single canonical list, enumerated by hand rather than globbed.
# test_corpus.py claims exactly these; test_format_a_corpus.py claims
# every OTHER .log/.txt Format A file; test_corpus_partition.py asserts
# every top-level .log in data/replays/ is claimed by exactly one of the
# two. A fourth bracketed log must be added here by hand, or it falls
# into test_format_a_corpus.py's glob and fails there for a reason that
# carries no information (wrong grammar, not a real parsing gap).
BRACKETED_FIXTURE_NAMES = [
    "battle1_bg-chillet_vs_blue-purple_WIN.log",
    "battle2_bg-chillet_vs_blue-purple_LOSS.log",
    "battle3_bg-chillet_vs_blue-red_WIN.log",
]


@pytest.fixture
def log_dir() -> Path:
    return LOG_DIR


@pytest.fixture
def battle1_path() -> Path:
    return LOG_DIR / "battle1_bg-chillet_vs_blue-purple_WIN.log"


@pytest.fixture
def battle2_path() -> Path:
    return LOG_DIR / "battle2_bg-chillet_vs_blue-purple_LOSS.log"


@pytest.fixture
def battle3_path() -> Path:
    return LOG_DIR / "battle3_bg-chillet_vs_blue-red_WIN.log"


@pytest.fixture
def pingu01_path() -> Path:
    return LOG_DIR / "top_players_all20" / "pingu_01_ca8ae191.txt"


@pytest.fixture
def pingu03_path() -> Path:
    return LOG_DIR / "top_players_all20" / "pingu_03_edc15d3a.txt"


@pytest.fixture
def pingu02_path() -> Path:
    return LOG_DIR / "top_players_all20" / "pingu_02_3e67de53.txt"
