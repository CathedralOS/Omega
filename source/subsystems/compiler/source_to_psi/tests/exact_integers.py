"""Run the product compiler's integer bounds, overflow and fraction reduction checks."""

import argparse
import re
import shutil
import tempfile
from pathlib import Path

from execution import check_silent_return


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE / "tests/fixtures/integers/run/bounds"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-exact-integers-") as directory:
        project = Path(directory)
        integers = Path("foundation/integers.omg")
        (project / integers.parent).mkdir()
        shutil.copyfile(PACKAGE / integers, project / integers)

        # BuiltinType is the only resolver declaration arithmetic uses.
        # Keep it at the package path that integers.omg imports.
        resolver = Path("representations/symbol_resolved_trees/symbol_resolved_trees.omg")
        builtin = re.search(r"^public data BuiltinType\b.*?^}", (PACKAGE / resolver).read_text(), re.M | re.S)
        if builtin is None:
            raise ValueError(f"missing BuiltinType in {resolver}")
        (project / resolver.parent).mkdir(parents=True)
        (project / resolver).write_text(builtin[0] + "\n")
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(FIXTURE / name, project / name)
        check_silent_return(cli, project)


if __name__ == "__main__":
    main()
