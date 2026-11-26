"""Check attached domain indices and malformed lists in the Omega-written parser."""

import argparse
from pathlib import Path
import shutil
import tempfile

from execution import check_silent_execution


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE.parent / "tests/fixtures/source_to_psi/parser/run/indexed_membership/main.omg"


def main():
    arguments = argparse.ArgumentParser(description=__doc__)
    arguments.add_argument("omega", type=Path, help="Omega executable")
    cli = arguments.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-indexed-membership-parser-") as directory:
        root = Path(directory)
        for name in (
            "pipeline/source_files_to_tokens",
            "pipeline/tokens_to_syntax_trees",
            "representations/source_files",
            "representations/tokens",
            "representations/syntax_trees",
        ):
            shutil.copytree(PACKAGE / name, root / name)
        shutil.copyfile(FIXTURE, root / "main.omg")
        check_silent_execution(cli, root, entry="main.omg")


if __name__ == "__main__":
    main()
