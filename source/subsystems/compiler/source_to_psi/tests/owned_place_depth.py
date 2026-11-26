"""Check ownership across field and index chains that exceed 64 projections."""

import argparse
from pathlib import Path
import re
import shutil
import tempfile

from execution import check_silent_execution


PACKAGE = Path(__file__).resolve().parents[1]
DECLARATIONS = (
    "data PlaceRelation",
    "machine path_root",
    "machine field_place",
    "machine index_steps",
    "machine place_depth",
    "machine place_ancestor",
    "machine same_field_path",
    "machine distinct_indexes",
    "machine skip_indexes",
    "machine selected_field",
    "machine path_relation",
    "machine through_write_only",
    "machine borrowed_place",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path)
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE.parent / "tests/fixtures/source_to_psi/ownership/run/deep_projections"
    places = (PACKAGE / "pipeline/typed_trees_to_checked_trees/owned_places.omg").read_text()
    declarations = []
    for name in DECLARATIONS:
        match = re.search(rf"^(?:public )?{name}\b.*?^}}", places, re.M | re.S)
        if not match:
            raise ValueError(f"missing product declaration {name}")
        declarations.append(match[0])
    with tempfile.TemporaryDirectory(prefix="omega-deep-projections-") as directory:
        root = Path(directory)
        shutil.copyfile(fixture / "build.omg", root / "build.omg")
        shutil.copyfile(fixture / "main.omg", root / "main.omg")
        # The fixture supplies the small tables that the product machines read.
        imports = [
            f"use main.omg as {alias};"
            for alias in ("integers", "symbol_resolved_trees", "syntax_trees", "typed_trees")
        ]
        (root / "owned_places.omg").write_text(
            "\n".join(imports) + "\n\n" + "\n\n".join(declarations) + "\n"
        )
        check_silent_execution(cli, root)


if __name__ == "__main__":
    main()
