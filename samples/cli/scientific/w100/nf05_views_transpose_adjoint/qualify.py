"""Sequential, separately recorded compiler routes for the independent NF05 samples."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parent
repository = root.parents[4]
compiler = Path(sys.argv[1]).resolve()
evidence = root / "evidence"
evidence.mkdir(exist_ok=True)
records = []
for project in ("sensor_patch", "hermitian_energy", "invalid_readonly_write"):
    source = root / project / "main.omg"
    routes = (("check", ["--check"]),) if project.startswith("invalid") else (
        ("check", ["--check"]), ("native", ["run"]), ("terminal_comparison", ["run", "--both"]))
    for route, flags in routes:
        command = [str(compiler), *flags, str(source)]
        started = time.monotonic()
        try:
            result = subprocess.run(command, cwd=repository, capture_output=True,
                                    text=True, timeout=180)
            stdout, stderr, exit_code = result.stdout, result.stderr, result.returncode
        except subprocess.TimeoutExpired as error:
            stdout = error.stdout or b""
            stderr = error.stderr or b""
            if isinstance(stdout, bytes): stdout = stdout.decode(errors="replace")
            if isinstance(stderr, bytes): stderr = stderr.decode(errors="replace")
            stderr += "\nQUALIFICATION TIMEOUT after 180 seconds\n"
            exit_code = "timeout"
        prefix = f"{project}.{route}"
        (evidence / f"{prefix}.stdout.txt").write_text(stdout)
        (evidence / f"{prefix}.stderr.txt").write_text(stderr)
        record = {"project": project, "route": route, "command": command,
                  "cwd": str(repository), "exit": exit_code,
                  "elapsed_seconds": round(time.monotonic() - started, 3),
                  "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                  "stdout_log": f"{prefix}.stdout.txt", "stderr_log": f"{prefix}.stderr.txt"}
        if project.startswith("invalid"):
            record["intended_rejection"] = "write through immutable shared slice"
            record["requires_diagnostic_review"] = True
        elif route != "check":
            expected = (root / project / "expected_stdout.txt").read_text()
            record["expected_stdout_present"] = expected.strip() in stdout
            record["expected_exit_matches"] = exit_code == 0
            if route == "terminal_comparison":
                record["requires_agreement_review"] = True
        if exit_code != 0 and not project.startswith("invalid"):
            destination = evidence / "failures" / prefix
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination / "main.omg.txt")
            shutil.copy2(source.with_name("build.omg"), destination / "build.omg.txt")
            (destination / "command.json").write_text(json.dumps(command, indent=2) + "\n")
        records.append(record)
        (evidence / "routes.json").write_text(json.dumps(records, indent=2) + "\n")
        print(f"{project} {route}: exit {exit_code}; {record['elapsed_seconds']}s", flush=True)
