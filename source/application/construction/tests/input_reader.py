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
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(fixture / name, root / name)
        with tempfile.TemporaryDirectory() as case_directory:
            inputs = Path(case_directory) / "inputs"
            outputs = Path(case_directory) / "outputs"
            inputs.mkdir()
            expected = {}
            for name, packet, exit_code in requests():
                (inputs / f"{name}.stdin").write_bytes(packet)
                expected[name] = (str(exit_code), "agrees", str(exit_code))
            subprocess.run(
                [cli, "run", "--both", "--inputs", inputs, "--outputs", outputs, "build.omg"],
                cwd=root, check=True,
            )
            with (outputs / "results.tsv").open(newline="") as report:
                observed = {
                    row["case"]: (row["native"], row["interpreter"], row["interpreter_exit"])
                    for row in csv.DictReader(report, delimiter="\t")
                }
            if observed != expected:
                raise SystemExit(f"expected {expected}, observed {observed}")
            for name in expected:
                for stream in (f"{name}.stdout", f"{name}.terminal.stdout"):
                    if (outputs / stream).stat().st_size:
                        raise SystemExit(f"{stream} contains tape bytes")


if __name__ == "__main__":
    main()
