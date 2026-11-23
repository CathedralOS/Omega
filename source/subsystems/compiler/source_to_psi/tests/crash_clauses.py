"""Parse two machine headers and check which one admits a crash."""

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURE = PACKAGE / "tests/fixtures/parser/run/crash_clauses"
DIRECTORIES = (
    "pipeline/source_files_to_tokens",
    "pipeline/tokens_to_syntax_trees",
    "representations/source_files",
    "representations/tokens",
    "representations/syntax_trees",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-parser-crash-clauses-") as directory:
        root = Path(directory)
        for name in DIRECTORIES:
            shutil.copytree(PACKAGE / name, root / name)
        for name in ("build.omg", "main.omg"):
            shutil.copyfile(FIXTURE / name, root / name)
        result = subprocess.run(
            [cli, "run", "--both", "build.omg"],
            cwd=root,
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
