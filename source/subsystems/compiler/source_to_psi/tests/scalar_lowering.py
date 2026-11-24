"""Run the product scalar lowerer's operation and layout lookups in native code and Psi."""

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]
FIXTURES = PACKAGE / "tests/fixtures/lowering/run"


def use_fixture(project, name):
    for source in ("main.omg", "build.omg"):
        shutil.copyfile(FIXTURES / name / source, project / source)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-scalar-lowering-") as directory:
        project = Path(directory)
        layouts = Path("representations/lowered_program")
        shutil.copytree(PACKAGE / layouts, project / layouts)
        command = [cli, "run", "--both", "build.omg"]

        use_fixture(project, "layouts")
        subprocess.run(command, cwd=project, stdin=subprocess.DEVNULL, check=True)
        shutil.copyfile(FIXTURES / "layouts/outside/main.omg", project / "main.omg")
        outside = subprocess.run(
            command, cwd=project, stdin=subprocess.DEVNULL,
            text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        print(outside.stdout, end="")
        if "interp crash: Trap (host presentation agrees)" not in outside.stdout.splitlines():
            raise SystemExit("reading past layout_count must trap in native code and Psi")

        lowerer = Path("pipeline/checked_trees_to_scalar_operations/lowerer.omg")
        (project / lowerer.parent).mkdir(parents=True)
        shutil.copyfile(PACKAGE / lowerer, project / lowerer)
        use_fixture(project, "operation_capacity")
        subprocess.run(command, cwd=project, stdin=subprocess.DEVNULL, check=True)


if __name__ == "__main__":
    main()
