"""Parse two machine headers and check which one admits a crash."""

import argparse
import shutil
import tempfile
from pathlib import Path

from execution import check_silent_execution


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE.parent / "tests/fixtures/source_to_psi/parser/run/crash_clauses"
DIRECTORIES = (
    "pipeline/source_files_to_tokens",
    "pipeline/tokens_to_syntax_trees",
    "representations/source_files",
    "representations/tokens",
    "representations/syntax_trees",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-parser-crash-clauses-") as directory:
        root = Path(directory)
        for name in DIRECTORIES:
            shutil.copytree(PACKAGE / name, root / name)
        for name in ("build.omg", "main.omg"):
            shutil.copyfile(FIXTURE / name, root / name)
        check_silent_execution(cli, root)


if __name__ == "__main__":
    main()
