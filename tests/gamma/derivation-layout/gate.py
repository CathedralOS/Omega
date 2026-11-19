"""Frame exact Gamma inputs and compare source-owned layout diagnostics."""

import sys
from pathlib import Path

from fixtures import cases

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from derivation_gate import main as gate_main  # noqa: E402


def main():
    gate_main('Derivation layout', 'layout', 'no proof verdicts', cases, Path(__file__).resolve().parent)


if __name__ == "__main__":
    main()
