"""Exercise exact file imports through the Omega-written parser and resolver."""

import argparse
from pathlib import Path
import shutil
import tempfile

from execution import check_silent_execution

PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE.parent / "tests/fixtures/source_to_psi/resolver/run/file_imports/main.omg"


def main():
    arguments = argparse.ArgumentParser(description=__doc__)
    arguments.add_argument("omega", type=Path, help="Omega executable")
    cli = arguments.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-source-file-imports-") as directory:
        project = Path(directory)
        for name in (
            "pipeline/source_files_to_tokens",
            "pipeline/tokens_to_syntax_trees",
            "pipeline/syntax_trees_to_symbol_resolved_trees",
            "representations/source_files",
            "representations/tokens",
            "representations/syntax_trees",
            "representations/symbol_resolved_trees",
        ):
            shutil.copytree(PACKAGE / name, project / name)
        shutil.copyfile(FIXTURE, project / "main.omg")
        check_silent_execution(cli, project, entry="main.omg")


if __name__ == "__main__":
    main()
