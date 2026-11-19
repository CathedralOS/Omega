"""Invoke the selected Gamma program and compare complete diagnostic bytes."""

import struct
import subprocess
import sys
from pathlib import Path

from fixtures import cases

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import derivation_gate  # noqa: E402


def main():
    temporary = Path(sys.argv[1])
    gate = Path(__file__).resolve().parent
    source = (temporary / "diagnostic.gamma").read_bytes()
    derivation_gate.read_source_identity(
        "Derivation admission", source, gate)

    fixtures = 0
    for name, request, output in cases():
        # This is framing custody, not a second semantic or resource model.
        if 4 + len(source) + len(request) > 137363456:
            raise SystemExit(f"Derivation admission {name}: outside selected evaluator request")
        framed_input = struct.pack("<I", len(source)) + source + request
        try:
            result = subprocess.run(
                [str(temporary / "evaluator")], input=framed_input,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=600,
            )
        except subprocess.TimeoutExpired:
            raise SystemExit(f"Derivation admission {name}: host timeout; no admission result")
        if (result.returncode, result.stdout, result.stderr) != (0, output, b""):
            raise SystemExit(
                f"Derivation admission {name}: expected diagnostic "
                f"0/{output.hex()}, got {result.returncode}/{result.stdout.hex()}, "
                f"stderr={result.stderr!r}"
            )
        fixtures += 1
    print(f"Derivation admission: {fixtures} exact diagnostic vectors passed; no proof verdicts")


if __name__ == "__main__":
    main()
