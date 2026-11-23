"""Run the product resolver's declaration-path and name-role checks."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURES = PACKAGE / "tests/fixtures/resolver/run"
STAGE = "pipeline/syntax_trees_to_symbol_resolved_trees"
SHARED = (
    "representations/source_files/source.omg",
    "representations/syntax_trees/syntax_trees.omg",
    "representations/tokens/numeric.omg",
)
PROGRAMS = {
    "declaration_paths": (f"{STAGE}/declarations.omg",),
    "roles": (f"{STAGE}/resolution_state.omg", "representations/symbol_resolved_trees/symbol_resolved_trees.omg"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    for name, sources in PROGRAMS.items():
        with tempfile.TemporaryDirectory(prefix=f"omega-resolver-{name}-") as directory:
            project = Path(directory)
            for source in (*SHARED, *sources):
                destination = project / source
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(PACKAGE / source, destination)
            for filename in ("main.omg", "build.omg"):
                shutil.copyfile(FIXTURES / name / filename, project / filename)
            subprocess.run([cli, "run", "--both", "build.omg"], cwd=project, stdin=subprocess.DEVNULL, check=True)


if __name__ == "__main__":
    main()
