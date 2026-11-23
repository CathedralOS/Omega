"""Run the product lexer on a valid hex escape and one with an invalid digit."""

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]
SOURCES = (
    "pipeline/source_files_to_tokens/lexer.omg",
    "pipeline/source_files_to_tokens/digits.omg",
    "representations/source_files/source.omg",
    "representations/tokens/numeric.omg",
    "representations/tokens/tokens.omg",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="path to an optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE / "tests/fixtures/lexer/run/hex_escape"
    with tempfile.TemporaryDirectory(prefix="omega-product-lexer-") as directory:
        project = Path(directory)
        for name in SOURCES:
            destination = project / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(PACKAGE / name, destination)
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(fixture / name, project / name)
        subprocess.run([cli, "run", "--both", "build.omg"], cwd=project, check=True)


if __name__ == "__main__":
    main()
