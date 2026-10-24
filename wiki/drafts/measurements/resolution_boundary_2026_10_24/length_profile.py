"""Compile length/local without its unsupported omega.lock."""

import hashlib
import json
import os
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
LOCAL = ROOT / "samples" / "cli" / "facts" / "length" / "local"
OMEGA = Path(os.environ["OMEGA_PROFILE_OMEGA"])
ENV = os.environ.copy()
ENV["OMEGA_RESOLUTION_MEASURE"] = "1"
ENV["OMEGA_PHASE_PROFILE"] = "1"


def parse(stderr):
    stages = []
    phases = []
    probes = []
    for line in stderr.splitlines():
        if line.startswith("RESMEASURE\t"):
            parts = line.split("\t")
            if len(parts) >= 5 and parts[2] in {"scope", "clone"}:
                row = {
                    "kind": parts[2],
                    "label": parts[3],
                    "ns": int(parts[4]),
                }
                if len(parts) >= 6 and parts[5].startswith("rows="):
                    row["rows"] = int(parts[5].split("=", 1)[1])
                probes.append(row)
            continue
        stripped = line.strip()
        if " ms  " not in stripped:
            continue
        left, name = stripped.split(" ms  ", 1)
        try:
            milliseconds = float(left)
        except ValueError:
            continue
        if " x  " in name:
            count, phase_name = name.split(" x  ", 1)
            phases.append(
                {"ms": milliseconds, "count": int(count), "name": phase_name}
            )
        else:
            stages.append({"ms": milliseconds, "name": name})
    return stages, phases, probes


def main():
    runs = []
    for index in range(5):
        role = "warmup" if index == 0 else "measured"
        with tempfile.TemporaryDirectory(prefix="omega-length-") as directory:
            project = Path(directory)
            shutil.copyfile(LOCAL / "main.omg", project / "main.omg")
            shutil.copyfile(LOCAL / "build.omg", project / "build.omg")
            started = time.perf_counter_ns()
            completed = subprocess.run(
                [
                    str(OMEGA),
                    "--timings",
                    "--target",
                    "windows_x86_64",
                    "--build-dir",
                    str(project / "out"),
                    str(project / "main.omg"),
                ],
                cwd=ROOT,
                env=ENV,
                text=True,
                encoding="utf-8",
                errors="replace",
                capture_output=True,
            )
            elapsed = time.perf_counter_ns() - started
            native_exit = None
            published = None
            for line in completed.stdout.splitlines():
                if line.startswith("published native output to "):
                    published = line.removeprefix("published native output to ").strip()
            if published and role == "measured" and completed.returncode == 0:
                native_exit = subprocess.run([published], cwd=ROOT).returncode
            stages, phases, probes = parse(completed.stderr)
            runs.append(
                {
                    "role": role,
                    "returncode": completed.returncode,
                    "elapsed_ns": elapsed,
                    "native_exit": native_exit,
                    "stages": stages,
                    "phase_rows": phases,
                    "probes": probes,
                    "stderr_tail": "\n".join(completed.stderr.splitlines()[-8:]),
                }
            )
            print(
                f"{role} exit={completed.returncode} wall_ms={elapsed / 1e6:.1f} native={native_exit}",
                flush=True,
            )
    measured = [run for run in runs if run["role"] == "measured" and run["returncode"] == 0]
    record = {
        "omega_sha256": hashlib.sha256(OMEGA.read_bytes()).hexdigest(),
        "runs": runs,
        "measured_wall_ms": [run["elapsed_ns"] / 1e6 for run in measured],
    }
    if measured:
        record["wall_median_ms"] = statistics.median(record["measured_wall_ms"])
    Path("length-observations.json").write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    main()
