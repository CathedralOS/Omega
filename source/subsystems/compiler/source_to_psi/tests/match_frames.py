"""Run the product parser's match-frame transitions on a frame with known positions."""

import argparse
import shutil
import tempfile
from pathlib import Path

from execution import check_silent_execution


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE / "tests/fixtures/parser/run/match_frames"
SOURCES = (
    "pipeline/tokens_to_syntax_trees/expression_frames.omg",
    "representations/syntax_trees/syntax_trees.omg",
    "representations/source_files/source.omg",
    "representations/tokens/numeric.omg",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="omega-match-frames-") as directory:
        root = Path(directory)
        for name in SOURCES:
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(PACKAGE / name, target)
        for name in ("build.omg", "main.omg"):
            shutil.copyfile(FIXTURE / name, root / name)
        check_silent_execution(args.omega.resolve(), root)


if __name__ == "__main__":
    main()
