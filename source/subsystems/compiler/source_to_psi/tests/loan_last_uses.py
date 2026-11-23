"""Run the ownership checker's last-use scan at and beyond its table capacity."""

import argparse
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path)
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE / "tests/fixtures/loans/run/last_uses"
    loans = (PACKAGE / "pipeline/typed_trees_to_checked_trees/loans.omg").read_text()
    declarations = []
    for name in ("record_tree_uses", "use_local", "use_slot"):
        match = re.search(rf"^(?:public )?machine {name}\b.*?^}}", loans, re.M | re.S)
        if not match:
            raise ValueError(f"missing product declaration {name}")
        declarations.append(match[0])
    with tempfile.TemporaryDirectory(prefix="omega-loan-last-uses-") as directory:
        root = Path(directory)
        shutil.copyfile(fixture / "build.omg", root / "build.omg")
        # Each imported file in the product has a small table or lookup in
        # the fixture. Keep the product machines and their qualifiers intact.
        imports = [f"use main.omg as {alias};" for alias in ("checker", "checks", "ownership", "symbol_resolved_trees", "syntax_trees")]
        (root / "loans.omg").write_text("\n".join(imports) + "\n\n" + "\n\n".join(declarations) + "\n")
        shutil.copyfile(fixture / "main.omg", root / "main.omg")
        subprocess.run([cli, "run", "--both", "build.omg"], cwd=root, check=True)


if __name__ == "__main__":
    main()
