"""Run the product scalar lowerer's operation and layout lookups in native code and Psi."""

import argparse
import shutil
import tempfile
from pathlib import Path

from execution import check_silent_execution


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

        use_fixture(project, "layouts")
        check_silent_execution(cli, project)
        shutil.copyfile(FIXTURES / "layouts/outside/main.omg", project / "main.omg")
        check_silent_execution(cli, project, expected="Trap")

        lowerer = Path("pipeline/checked_trees_to_scalar_operations/lowerer.omg")
        (project / lowerer.parent).mkdir(parents=True)
        shutil.copyfile(PACKAGE / lowerer, project / lowerer)
        use_fixture(project, "operation_capacity")
        check_silent_execution(cli, project)


if __name__ == "__main__":
    main()
