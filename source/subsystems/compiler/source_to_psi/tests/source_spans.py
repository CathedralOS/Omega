"""Check span equality and spelling against the product source set."""

import argparse
from pathlib import Path
import shutil
import tempfile

from execution import check_silent_execution


PACKAGE = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path)
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE.parent / "tests/fixtures/source_to_psi/source_spans"
    with tempfile.TemporaryDirectory(prefix="omega-source-spans-") as directory:
        project = Path(directory)
        source = Path("representations/source_files/source.omg")
        (project / source.parent).mkdir(parents=True)
        shutil.copyfile(PACKAGE / source, project / source)
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(fixture / name, project / name)
        check_silent_execution(cli, project)


if __name__ == "__main__":
    main()
