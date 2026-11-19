"""Run the bootstrap observations against an executable supplied by the caller."""
import argparse
from pathlib import Path
import struct
import subprocess
import sys

from vectors import GAMMA_CASES, INVALID_GAMMA, epsilon_observations


def gamma_observations():
    for name, text, stdin, expected in GAMMA_CASES:
        source = text.encode("ascii")
        yield name, struct.pack("<I", len(source)) + source + stdin, expected
    for index, source in enumerate(INVALID_GAMMA, 1):
        yield f"invalid Gamma {index}", struct.pack("<I", len(source)) + source, (1, b"")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_subparsers(dest="rung", required=True)
    gamma = modes.add_parser("gamma", help="evaluator reading length-framed Gamma source on stdin")
    gamma.add_argument("evaluator", type=Path)
    epsilon = modes.add_parser("epsilon", help="evaluator reading EEREQ on stdin")
    epsilon.add_argument("evaluator", type=Path)
    epsilon.add_argument("closure", type=Path, help="packed evaluator used to build the executable")
    omega = modes.add_parser("omega", help="command that compiles and runs an Omega source file")
    omega.add_argument("--timeout", type=int, default=1800, help="seconds for compilation and execution")
    omega.add_argument("command", nargs="+", help="runner command to which program.omg is appended")
    args = parser.parse_args()

    timeout = 60
    if args.rung == "omega":
        timeout = args.timeout
        if timeout <= 0:
            parser.error("--timeout must be positive")
        directory = Path(__file__).resolve().with_name("omega")
        command = [*args.command, directory / "program.omg"]
        cases = [("Omega program", b"", (int((directory / "expected.txt").read_text()), b""))]
    else:
        command = [args.evaluator.resolve()]
        cases = (gamma_observations() if args.rung == "gamma"
                 else epsilon_observations(args.closure.read_bytes()))

    passed = failed = 0
    for name, payload, expected in cases:
        try:
            result = subprocess.run(command, input=payload, capture_output=True, timeout=timeout)
        except subprocess.TimeoutExpired:
            print(f"{name}: timed out after {timeout}s", file=sys.stderr)
        else:
            if (result.returncode, result.stdout, result.stderr) == (*expected, b""):
                passed += 1
                continue
            print(f"{name}: expected exit {expected[0]} and stdout {expected[1]!r}, "
                  f"got {result.returncode}, {result.stdout!r}, stderr {result.stderr!r}", file=sys.stderr)
        failed += 1
    print(f"{args.rung}: {passed} passed, {failed} failed")
    return int(failed > 0)


if __name__ == "__main__":
    sys.exit(main())
