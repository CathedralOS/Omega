"""Exercise selected binary operators and named calls through the ownership checker."""

import argparse
from pathlib import Path
import re
import shutil
import tempfile

from execution import check_silent_execution


PACKAGE = Path(__file__).resolve().parents[1]
CHECKER = PACKAGE / "pipeline/typed_trees_to_checked_trees"
DECLARATIONS = {
    "holds.omg": ["Hold", "called_binary", "check_holds", "pair_hold", "retained_holds", "outlives_calls", "scan_accesses"],
    "ownership.omg": ["pass_binary_operand_roles", "set_adapted_role", "check_discarded_temporary"],
    "obligations.omg": ["selects_machine", "operand_count", "checker_operand"],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path)
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE / "tests/fixtures/ownership/run/operator_holds"
    declarations = []
    for filename, names in DECLARATIONS.items():
        source = (CHECKER / filename).read_text()
        for name in names:
            match = re.search(rf"^(?:public )?(?:machine|data) {name}\b.*?^}}", source, re.M | re.S)
            if not match:
                raise ValueError(f"missing {name} in {filename}")
            declarations.append(match[0])
    with tempfile.TemporaryDirectory(prefix="omega-operator-holds-") as directory:
        root = Path(directory)
        shutil.copyfile(fixture / "build.omg", root / "build.omg")
        # The fixture supplies the small tables and lookups reached by these
        # declarations. Keep each product reference qualified as in its source.
        aliases = (
            "checker", "checks", "holds", "loans", "obligations", "owned_places",
            "ownership", "symbol_resolved_trees", "syntax_trees", "typed_trees",
        )
        imports = "\n".join(f"use main.omg as {alias};" for alias in aliases)
        (root / "main.omg").write_text(
            "\n\n".join([imports, *declarations, (fixture / "main.omg").read_text()])
        )
        check_silent_execution(cli, root)


if __name__ == "__main__":
    main()
