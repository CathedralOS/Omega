"""Check the product resolver's spelling and visibility fields for declaration paths."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE / "tests/fixtures/resolver/run/declaration_paths"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-declaration-paths-") as directory:
        project = Path(directory)
        for source in (
            "pipeline/syntax_trees_to_symbol_resolved_trees/declarations.omg",
            "representations/source_files/source.omg",
            "representations/syntax_trees/syntax_trees.omg",
            "representations/tokens/numeric.omg",
        ):
            destination = project / source
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(PACKAGE / source, destination)
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(FIXTURE / name, project / name)
        subprocess.run([cli, "run", "--both", "build.omg"], cwd=project, stdin=subprocess.DEVNULL, check=True)


if __name__ == "__main__":
    main()
