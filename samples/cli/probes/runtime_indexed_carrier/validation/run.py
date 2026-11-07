"""Observe authored native/Terminal behavior without changing package decisions."""
import argparse
import csv
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

SAMPLE = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--omega", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--expected-native", type=Path)
args = parser.parse_args()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
command = [str(args.omega.resolve()), "run", "--both", "--keep",
           "--input", str(SAMPLE / "validation/authored.stdin"),
           "--outputs", str(output), str(SAMPLE / "main.omg")]
observed = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, timeout=300)
(output / "launcher.stdout").write_bytes(observed.stdout)
(output / "launcher.stderr").write_bytes(observed.stderr)
assert observed.returncode == 0, observed.stderr.decode(errors="replace")
with (output / "results.tsv").open() as stream:
    rows = list(csv.DictReader(stream, delimiter="\t"))
assert len(rows) == 1 and rows[0]["case"] == "authored", rows
row = rows[0]
assert row["interpreter"] == "agrees", row
for suffix in ("stdout", "stderr", "terminal.stdout", "terminal.stderr"):
    assert (output / ("authored." + suffix)).read_bytes() == b"", suffix
oracle = subprocess.run([sys.executable, str(SAMPLE / "validation/oracle.py"),
                         "--sample", str(SAMPLE), "--native-exit", row["native"],
                         "--terminal-exit", row["interpreter_exit"]],
                        stdin=subprocess.DEVNULL, capture_output=True, timeout=30)
(output / "oracle.stdout").write_bytes(oracle.stdout)
(output / "oracle.stderr").write_bytes(oracle.stderr)
assert oracle.returncode == 0, oracle.stderr.decode(errors="replace")
match = re.search(rb"^build dir kept: (.+)$", observed.stderr, re.M)
assert match, "native artifact must be retained"
build = Path(match[1].decode())
def executable_file(path):
    if not path.is_file():
        return False
    with path.open("rb") as stream:
        magic = stream.read(4)
    return magic in (b"\x7fELF", b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf") or magic[:2] == b"MZ"

executables = [path for path in build.iterdir() if executable_file(path)]
assert len(executables) == 1, executables
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
if args.expected_native:
    assert digest(executables[0]) == digest(args.expected_native), "native artifact differs from frozen producer output"
receipt = {"command": command, "launcher_exit": observed.returncode,
           "producer_sha256": digest(args.omega), "cases": rows,
           "native_artifact": str(executables[0]),
           "native_sha256": digest(executables[0]),
           "oracle": json.loads(oracle.stdout)}
(output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt, indent=2))
