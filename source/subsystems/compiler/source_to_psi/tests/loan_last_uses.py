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
        (root / "main.omg").write_text("\n\n".join([*declarations, (fixture / "main.omg").read_text()]))
        subprocess.run([cli, "run", "--both", "build.omg"], cwd=root, check=True)


if __name__ == "__main__":
    main()
