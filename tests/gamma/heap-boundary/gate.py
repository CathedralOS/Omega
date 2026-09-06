"""Compare full authored heap canaries on the selected Gamma evaluator."""

import argparse
import csv
import hashlib
import os
import struct
import subprocess
import time
from pathlib import Path


def identity(data):
    return len(data), hashlib.sha256(data).hexdigest()


def rows(path, header):
    with path.open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != header:
            raise SystemExit(f"Gamma heap boundary: invalid {path.name} header")
        return list(reader)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evaluator", type=Path)
    parser.add_argument("source", type=Path)
    parser.add_argument("tape", type=Path)
    parser.add_argument("--case", help="exact fixture filename, including .gamma")
    options = parser.parse_args()
    gate = Path(__file__).resolve().parent
    try:
        timeout = int(os.environ.get("OMEGA_GAMMA_HEAP_SECONDS", "1200"))
    except ValueError:
        raise SystemExit("OMEGA_GAMMA_HEAP_SECONDS must be a positive integer")
    if timeout <= 0:
        raise SystemExit("OMEGA_GAMMA_HEAP_SECONDS must be a positive integer")

    artifacts = rows(gate / "evaluator.tsv", ["artifact", "bytes", "sha256"])
    if [row["artifact"] for row in artifacts] != ["source", "tape"]:
        raise SystemExit("Gamma heap boundary: invalid evaluator identity inventory")
    for row in artifacts:
        data = getattr(options, row["artifact"]).read_bytes()
        if identity(data) != (int(row["bytes"]), row["sha256"]):
            raise SystemExit(f"Gamma heap boundary: {row['artifact']} identity changed")

    controls = {}
    for row in rows(gate / "fixtures.tsv",
                    ["fixture", "bytes", "sha256", "output_hex", "status"]):
        name = row["fixture"]
        if Path(name).name != name or not name.endswith(".gamma") or name in controls:
            raise SystemExit("Gamma heap boundary: invalid fixture identity")
        source = (gate / name).read_bytes()
        if identity(source) != (int(row["bytes"]), row["sha256"]):
            raise SystemExit(f"Gamma heap boundary: {name} identity changed")
        controls[name] = source, int(row["status"]), bytes.fromhex(row["output_hex"])
    if not controls or set(controls) != {path.name for path in gate.glob("*.gamma")}:
        raise SystemExit("Gamma heap boundary: incomplete fixture inventory")
    if options.case is not None:
        if options.case not in controls:
            raise SystemExit(f"Gamma heap boundary: unknown fixture {options.case!r}")
        controls = {options.case: controls[options.case]}

    for name, (source, status, output) in controls.items():
        print(f"Gamma heap boundary {name}: started; watchdog {timeout}s", flush=True)
        started = time.monotonic()
        try:
            result = subprocess.run(
                [str(options.evaluator)],
                input=struct.pack("<I", len(source)) + source,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
            )
        except subprocess.TimeoutExpired:
            raise SystemExit(f"Gamma heap boundary {name}: timed out; no language judgment")
        elapsed = time.monotonic() - started
        if (result.returncode, result.stdout, result.stderr) != (status, output, b""):
            raise SystemExit(
                f"Gamma heap boundary {name}: {elapsed:.3f}s; expected "
                f"{status}/{output.hex()}, got {result.returncode}/{result.stdout.hex()}, "
                f"stderr={result.stderr!r}"
            )
        print(f"Gamma heap boundary {name}: exact {status}/{output.hex()}, "
              f"empty stderr, {elapsed:.3f}s", flush=True)
    print(f"Gamma heap boundary: {len(controls)} full authored controls passed", flush=True)


if __name__ == "__main__":
    main()
