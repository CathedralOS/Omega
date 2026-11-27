"""Send a valid construction request and a Build entry owned by the wrong package to the product reader."""

import argparse
import csv
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

APPLICATION = Path(__file__).resolve().parents[1]
ROOT = APPLICATION.parents[2]
sys.path.insert(0, str(APPLICATION))
import construct

sys.path.insert(0, str(ROOT / "source/subsystems/compiler/source_to_psi/tests"))
from execution import compile_fixture


def requests():
    packet, packages, edges, units = construct.encode(
        [("app", 0), ("other", 0)], [], [(0, b"machine entry() {}")], 0, b"entry",
        paths=["main.omg"], build_sources=[(0, "build.omg", b"a"), (1, "build.omg", b"b")],
        build_entries=[(0, 0), (1, 1)],
    )
    # A valid Build inventory reaches the documented stop before Build execution.
    yield "valid", packet, 246
    # The first Build entry follows the version-2 header and the Product rows.
    # Its second word selects a Build unit. Unit 1 belongs to the other package.
    first_entry = 10 + 3 * len(packages) + 4 * len(edges) + 5 * len(units)
    wrong = bytearray(packet)
    struct.pack_into("<I", wrong, (first_entry + 1) * 4, 1)
    yield "wrong-owner", wrong, 253


def check_interpreter(cli, project, inputs, outputs, expected):
    subprocess.run(
        [cli, "run", "--inputs", inputs, "--outputs", outputs, "build.omg"],
        cwd=project, stdin=subprocess.DEVNULL, check=True,
    )
    with (outputs / "results.tsv").open(newline="") as report:
        rows = list(csv.DictReader(report, delimiter="\t"))
    if [row["case"] for row in rows] != sorted(expected):
        raise SystemExit(f"interpreter returned an unexpected case roster: {rows}")

    errors = {}
    for row in rows:
        name = row["case"]
        status = str(expected[name])
        if (row["termination"], row["detail"]) != ("exit", status):
            raise SystemExit(f"expected Psi to exit {status} for {name}, observed {row}")
        if (outputs / f"{name}.stdout").read_bytes():
            raise SystemExit(f"interpreter wrote tape bytes for {name}")
        errors[name] = (outputs / f"{name}.stderr").read_bytes()
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="optimized omega executable")
    cli = parser.parse_args().omega.resolve()
    build = ROOT / "build"
    build.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=build, prefix="construction-input-") as directory:
        root = Path(directory)
        for name in ("input.omg", "input_reader.omg", "build_sources.omg"):
            shutil.copyfile(APPLICATION / name, root / name)
        fixture = APPLICATION / "tests/fixtures/build_input"
        shutil.copyfile(fixture / "main.omg.in", root / "main.omg")
        shutil.copyfile(fixture / "build.omg.in", root / "build.omg")
        with tempfile.TemporaryDirectory() as case_directory:
            inputs = Path(case_directory) / "inputs"
            inputs.mkdir()
            expected = {}
            for name, packet, exit_code in requests():
                (inputs / f"{name}.stdin").write_bytes(packet)
                expected[name] = exit_code
            executable, _ = compile_fixture(cli, root, "build.omg", inputs.parent)
            psi = check_interpreter(cli, root, inputs, inputs.parent / "psi", expected)
            for name, status in expected.items():
                native = subprocess.run(
                    [executable], cwd=root, input=(inputs / f"{name}.stdin").read_bytes(),
                    capture_output=True,
                )
                if native.returncode != status or native.stdout:
                    raise SystemExit(f"unexpected compiled program result for {name}: {native!r}")
                if native.stderr != psi[name]:
                    raise SystemExit(f"native and Psi emitted different diagnostics for {name}")


if __name__ == "__main__":
    main()
