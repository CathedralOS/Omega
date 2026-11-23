"""Check span equality and spelling against the product source set."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path)
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE / "tests/fixtures/source_spans"
    with tempfile.TemporaryDirectory(prefix="omega-source-spans-") as directory:
        project = Path(directory)
        source = Path("representations/source_files/source.omg")
        (project / source.parent).mkdir(parents=True)
        shutil.copyfile(PACKAGE / source, project / source)
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(fixture / name, project / name)
        subprocess.run([cli, "run", "--both", "build.omg"], cwd=project, check=True)


if __name__ == "__main__":
    main()
