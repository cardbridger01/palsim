from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
LOG_DIR = REPO_ROOT / "data" / "replays"


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
