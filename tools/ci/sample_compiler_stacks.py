"""Temporary Linux stack observations for a selected stalled compiler check."""

import json
import pathlib
import shutil
import subprocess
import sys
import time


def main():
    root = pathlib.Path("build/compiler-pilot/stack-samples")
    root.mkdir(parents=True, exist_ok=True)
    debugger = shutil.which("gdb")
    process = subprocess.Popen([sys.executable, "tools/ci/compiler_pilot.py", "--phase", "consume"])
    observed = set()
    started = time.monotonic()
    try:
        while process.poll() is None:
            time.sleep(1)
            if not debugger or time.monotonic() - started < 45:
                continue
            listing = subprocess.check_output(
                ["ps", "-eo", "pid=,ppid=,pcpu=,stat=,comm="], text=True)
            rows = [line.split(None, 4) for line in listing.splitlines()]
            descendants = {process.pid}
            while True:
                expanded = descendants | {int(row[0]) for row in rows if int(row[1]) in descendants}
                if expanded == descendants:
                    break
                descendants = expanded
            for row in rows:
                pid = int(row[0])
                if pid not in descendants or pid in observed or not row[4].startswith(("corpus_runner", "omega")):
                    continue
                observed.add(pid)
                destination = root / (str(pid) + ".txt")
                with destination.open("w") as output:
                    output.write("Process observation: " + " ".join(row) + "\n")
                    output.flush()
                    try:
                        result = subprocess.run(
                            ["sudo", "-n", debugger, "--batch", "-nx", "-p", str(pid),
                             "-ex", "set pagination off", "-ex", "thread apply all bt 24", "-ex", "detach"],
                            stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT, timeout=12)
                        output.write("\nDebugger exit: " + str(result.returncode) + "\n")
                    except subprocess.TimeoutExpired:
                        output.write("\nDebugger timed out; this is an incomplete observation.\n")
        return process.returncode
    finally:
        (root / "request.json").write_text(json.dumps(dict(
            debugger_available=bool(debugger), observed_pids=sorted(observed),
            scope="Diagnostic observations only; original compiler/test limits and verdicts apply."), indent=2) + "\n")
        if process.poll() is None:
            process.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
