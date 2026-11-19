"""Compose exact whole sources and compare authored diagnostic bytes."""

import sys
from pathlib import Path

from fixtures import cases

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from derivation_gate import main_entries  # noqa: E402


def main():
    main_entries('Derivation comparison', 'comparison',
                 'no proof verdicts', cases,
                 Path(__file__).resolve().parent)


if __name__ == "__main__":
    main()
