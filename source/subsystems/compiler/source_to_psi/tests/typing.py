"""Check the product compiler’s literal values and type lookups in both execution engines."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURES = PACKAGE / "tests/fixtures/typing/run"
STAGE = "pipeline/symbol_resolved_trees_to_typed_trees"
SHARED = (
    "representations/source_files/source.omg",
    "representations/symbol_resolved_trees/symbol_resolved_trees.omg",
    "representations/syntax_trees/syntax_trees.omg",
    "representations/tokens/numeric.omg",
)
PROGRAMS = {
    "join_types": (
        f"{STAGE}/joins.omg", f"{STAGE}/admission.omg", f"{STAGE}/landing.omg",
        f"{STAGE}/typing_state.omg", "foundation/integers.omg",
        "representations/typed_trees/typed_trees.omg",
    ),
    "literal_values": (f"{STAGE}/literals.omg", f"{STAGE}/quoted_literals.omg"),
    "typing_state": (
        f"{STAGE}/typing_state.omg",
        "foundation/integers.omg",
        "representations/typed_trees/typed_trees.omg",
    ),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    for name, sources in PROGRAMS.items():
        with tempfile.TemporaryDirectory(prefix=f"omega-{name}-") as directory:
            project = Path(directory)
            for source in (*SHARED, *sources):
                destination = project / source
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(PACKAGE / source, destination)
            for filename in ("main.omg", "build.omg"):
                shutil.copyfile(FIXTURES / name / filename, project / filename)
            subprocess.run(
                [cli, "run", "--both", "build.omg"],
                cwd=project, stdin=subprocess.DEVNULL, check=True,
            )


if __name__ == "__main__":
    main()
