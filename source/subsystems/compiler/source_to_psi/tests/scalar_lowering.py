"""Run the product scalar lowerer's operation and layout lookups in native code and Psi."""

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURES = PACKAGE / "tests/fixtures/lowering/run"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-scalar-lowering-") as directory:
        project = Path(directory)
        for relative in ("foundation", "representations", "pipeline/checked_trees_to_scalar_operations"):
            shutil.copytree(PACKAGE / relative, project / relative)

        command = [cli, "run", "--both", "build.omg"]
        for fixture in ("operation_capacity", "layouts"):
            for name in ("main.omg", "build.omg"):
                shutil.copyfile(FIXTURES / fixture / name, project / name)
            subprocess.run(command, cwd=project, stdin=subprocess.DEVNULL, check=True)

        shutil.copyfile(FIXTURES / "layouts/outside/main.omg", project / "main.omg")
        outside = subprocess.run(
            command, cwd=project, stdin=subprocess.DEVNULL,
            text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        print(outside.stdout, end="")
        if "interp crash: Trap (host presentation agrees)" not in outside.stdout.splitlines():
            raise SystemExit("reading past layout_count must trap in native code and Psi")


if __name__ == "__main__":
    main()
