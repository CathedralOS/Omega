"""Run the product compiler's scalar layout lookups with native code and Psi."""

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    fixture = PACKAGE / "tests/fixtures/lowering/run/layouts"
    with tempfile.TemporaryDirectory(prefix="omega-scalar-layouts-") as directory:
        project = Path(directory)
        layouts = Path("representations/lowered_program")
        shutil.copytree(PACKAGE / layouts, project / layouts)
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(fixture / name, project / name)
        command = [cli, "run", "--both", "build.omg"]
        subprocess.run(command, cwd=project, check=True)

        shutil.copyfile(fixture / "outside/main.omg", project / "main.omg")
        outside = subprocess.run(command, cwd=project, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        print(outside.stdout, end="")
        if "interp crash: Trap (host presentation agrees)" not in outside.stdout.splitlines():
            raise SystemExit("reading past layout_count must trap in native code and Psi")


if __name__ == "__main__":
    main()
