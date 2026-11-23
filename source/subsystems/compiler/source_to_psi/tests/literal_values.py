"""Run the product typer on decoded integers and quoted byte escapes."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE / "tests/fixtures/typing/run/literal_values"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-literal-values-") as directory:
        project = Path(directory)
        for source in (
            "pipeline/symbol_resolved_trees_to_typed_trees/literals.omg",
            "representations/source_files/source.omg",
            "representations/symbol_resolved_trees/symbol_resolved_trees.omg",
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
