"""Run the product parser's match-frame transitions on a frame with known positions."""

import argparse
import re
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE / "tests/fixtures/parser/run/match_frames"
DECLARATIONS = {
    "representations/syntax_trees/syntax_trees.omg": ["ArmPatternKind"],
    "pipeline/tokens_to_syntax_trees/expression.omg": [
        "ExpressionFrameKind", "ExpressionFrame", "started_frame", "next_match_frame",
        "match_frame", "match_arm_frame", "match_value_frame",
    ],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    args = parser.parse_args()
    units = []
    for filename, names in DECLARATIONS.items():
        source = (PACKAGE / filename).read_text()
        for name in names:
            declaration = re.search(rf"^(?:public )?(?:data|machine) {name}\b.*?^}}", source, re.M | re.S)
            if declaration is None:
                raise ValueError(f"missing declaration {name} in {filename}")
            units.append(declaration[0])

    with tempfile.TemporaryDirectory(prefix="omega-match-frames-") as directory:
        root = Path(directory)
        (root / "build.omg").write_bytes((FIXTURE / "build.omg").read_bytes())
        units.append((FIXTURE / "main.omg").read_text())
        (root / "main.omg").write_text("\n\n".join(units))
        result = subprocess.run(
            [args.omega.resolve(), "run", "--both", "build.omg"], cwd=root,
            text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        print(result.stdout, end="")
        if result.returncode or not {"native exit 0", "interp exit: 0"}.issubset(result.stdout.splitlines()):
            raise SystemExit(1)


if __name__ == "__main__":
    main()
