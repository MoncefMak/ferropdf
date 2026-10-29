"""Project-wide pytest configuration.

CLI flags:

* ``--regenerate-golden`` — refresh the structural fingerprints in
  ``tests/golden/expected/`` (used by tests/test_golden.py).
* ``--regenerate-visual`` — refresh the reference PNGs in
  ``tests/corpus/reference/`` (used by tests/test_visual.py).
"""

from __future__ import annotations

import pytest


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--regenerate-golden",
        action="store_true",
        default=False,
        help="rewrite tests/golden/expected/*.json from the current build",
    )
    parser.addoption(
        "--regenerate-visual",
        action="store_true",
        default=False,
        help="rewrite tests/corpus/reference/*.png from the current build",
    )


@pytest.fixture
def regenerate_golden(request: pytest.FixtureRequest) -> bool:
    return bool(request.config.getoption("--regenerate-golden"))


@pytest.fixture
def regenerate_visual(request: pytest.FixtureRequest) -> bool:
    return bool(request.config.getoption("--regenerate-visual"))
