"""Write and read the Omega compiler's check, name-resolution and type tables."""

import argparse
from pathlib import Path
import shutil
import tempfile

from execution import check_silent_execution


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE / "tests/fixtures/checking_tables"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-checking-tables-") as directory:
        project = Path(directory)
        for path in (
            "representations/checks/checks.omg",
            "representations/symbol_resolved_trees/symbol_resolved_trees.omg",
            "representations/typed_trees/typed_trees.omg",
            "representations/syntax_trees/syntax_trees.omg",
            "representations/source_files/source.omg",
            "representations/tokens/numeric.omg",
            "foundation/integers.omg",
        ):
            source = project / path
            source.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(PACKAGE / path, source)
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(FIXTURE / name, project / name)
        check_silent_execution(cli, project)


if __name__ == "__main__":
    main()
