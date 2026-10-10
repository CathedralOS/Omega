"""Run each NF14 route sequentially and preserve commands and full diagnostics.

Activate the repository host environment before invocation. This script does
not change trust policy, approve reviews, or reinterpret unrelated rejection
as acceptance. It is a scoped qualification record, not a compiler repair.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument("--omega", type=Path, required=True)
arguments = parser.parse_args()
root = Path(__file__).resolve().parent
repository = root.parents[4]
evidence = root / "evidence"
evidence.mkdir(exist_ok=True)
compiler = arguments.omega.resolve()
records = []
revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repository, text=True).strip()
commands = [("library-check", ["--check", "source/library/numerics/linalg/cholesky_spd_solve.omg"])]
for project in ("diagonal_readonly", "correlated_copied"):
    source = str((root / project / "main.omg").relative_to(repository))
    commands.extend([(project + "-check", ["--check", source]),
                     (project + "-native", ["run", source]),
                     (project + "-terminal-comparison", ["run", "--both", source])])
source = str((root / "invalid_readonly_write" / "main.omg").relative_to(repository))
commands.append(("readonly-rejection", ["--check", source]))
for route, suffix in commands:
    command = [str(compiler)] + suffix
    started = time.monotonic()
    try:
        result = subprocess.run(command, cwd=repository, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                timeout=180)
        exit_status = result.returncode
        stdout, stderr = result.stdout, result.stderr
    except subprocess.TimeoutExpired as error:
        exit_status = None
        stdout, stderr = error.stdout or b"", error.stderr or b""
    (evidence / (route + ".stdout.txt")).write_bytes(stdout)
    (evidence / (route + ".stderr.txt")).write_bytes(stderr)
    diagnostic = stderr.decode("utf-8", errors="replace")
    custody_failure = "cache" in diagnostic.lower() and ("custody" in diagnostic.lower() or "owned" in diagnostic.lower())
    record = {"route": route, "command": command, "cwd": str(repository),
              "revision": revision, "exit_status": exit_status,
              "elapsed_seconds": round(time.monotonic() - started, 3),
              "environmental_cache_custody_failure": custody_failure}
    records.append(record)
    print(json.dumps(record), flush=True)
(evidence / "routes.json").write_text(json.dumps({
    "revision": revision,
    "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
    "routes": records
}, indent=2) + "\n")
