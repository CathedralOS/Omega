"""Check ownership across field and index chains that exceed 64 projections."""

import argparse
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().parents[1]
MACHINES = (
    "path_root", "field_place", "index_steps", "place_depth", "place_ancestor",
    "same_field_path", "distinct_indexes", "skip_indexes", "selected_field",
    "path_relation", "through_write_only", "borrowed_place",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path)
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE / "tests/fixtures/ownership/run/deep_projections"
    places = (PACKAGE / "pipeline/typed_trees_to_checked_trees/owned_places.omg").read_text()
    declarations = []
    for name in MACHINES:
        match = re.search(rf"^(?:public )?machine {name}\b.*?^}}", places, re.M | re.S)
        if not match:
            raise ValueError(f"missing product declaration {name}")
        declarations.append(match[0])
    with tempfile.TemporaryDirectory(prefix="omega-deep-projections-") as directory:
        root = Path(directory)
        shutil.copyfile(fixture / "build.omg", root / "build.omg")
        (root / "main.omg").write_text("\n\n".join([*declarations, (fixture / "main.omg").read_text()]))
        subprocess.run([cli, "run", "--both", "build.omg"], cwd=root, check=True)


if __name__ == "__main__":
    main()
