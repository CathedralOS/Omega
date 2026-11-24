"""Check that the Omega-written checker's `proves` refuses an expression whose goals overflow its stack.

The test runs the product goal stack and `proves` unchanged on a computed
expression table, so a goal the stack drops can never be reported proved.
"""

import argparse
import re
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]
CHECKER = "pipeline/typed_trees_to_checked_trees/"
DECLARATIONS = {
    "representations/syntax_trees/syntax_trees.omg": ["ExpressionKind", "OperatorKind"],
    CHECKER + "facts.omg": ["push_assumption", "checker_polarity", "pop_assumption", "proves"],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="path to an optimized omega executable")
    args = parser.parse_args()
    fixture = PACKAGE / "tests/fixtures/facts/run/goal_stack"
    units = []
    facts = []
    for filename, names in DECLARATIONS.items():
        source = (PACKAGE / filename).read_text()
        for name in names:
            # A top-level declaration closes at the left margin. Nested machines
            # stay with their owner, exactly as compiled in the product package.
            match = re.search(rf"^(?:public )?(?:data|machine) {name}\b.*?^}}", source, re.M | re.S)
            if not match:
                raise ValueError(f"missing declaration {name} in {filename}")
            if filename.startswith(CHECKER):
                facts.append(match[0])
            else:
                units.append(match[0])

    with tempfile.TemporaryDirectory(prefix="omega-goal-stack-") as directory:
        root = Path(directory)
        (root / "build.omg").write_bytes((fixture / "build.omg").read_bytes())
        inputs = (fixture / "facts.omg").read_text()
        (root / "facts.omg").write_text("\n\n".join([inputs, *facts]) + "\n")
        program = root / "main.omg"
        program.write_text("\n\n".join([*units, (fixture / "main.omg").read_text()]))
        result = subprocess.run(
            [args.omega.resolve(), "run", "--both", program],
            stdin=subprocess.DEVNULL,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        print(result.stdout, end="")
        expected = {"native exit 0", "interp exit: 0"}
        if result.returncode or not expected.issubset(result.stdout.splitlines()):
            raise SystemExit(1)


if __name__ == "__main__":
    main()
