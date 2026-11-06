"""Temporary Linux stack observations for a selected stalled compiler check."""

import json
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import time


def main():
    root = pathlib.Path("build/compiler-probe/stack-samples")
    root.mkdir(parents=True, exist_ok=True)
    debugger = next((selected for name in ("lldb", "lldb-18", "lldb-19", "lldb-20")
                     if (selected := shutil.which(name))), None)
    if len(sys.argv) < 2:
        raise ValueError("provide the exact observed command")
    if not debugger:
        raise RuntimeError("LLDB is unavailable; no compiler observation started")
    process = subprocess.Popen(sys.argv[1:], stdin=subprocess.DEVNULL, start_new_session=True)
    observed = set()
    started = time.monotonic()
    outer_stopped = False
    try:
        while process.poll() is None:
            time.sleep(1)
            if time.monotonic() - started < 45:
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
            if time.monotonic() - started >= 180:
                outer_stopped = True
                (root / "outer-stop.json").write_text(json.dumps(dict(
                    seconds=time.monotonic() - started,
                    observed_command=sys.argv[1:],
                    ordinary_source_limit_changed=False,
                    result="Incomplete diagnostic: the outer observer stopped the process tree."
                ), indent=2) + "\n")
                # The source harness may own additional process groups. Stop
                # every current descendant as well as the observer's group.
                for shutdown_signal in (signal.SIGTERM, signal.SIGKILL):
                    for pid in sorted(descendants, reverse=True):
                        try:
                            os.kill(pid, shutdown_signal)
                        except ProcessLookupError:
                            pass
                    try:
                        os.killpg(process.pid, shutdown_signal)
                    except ProcessLookupError:
                        pass
                    if shutdown_signal == signal.SIGTERM:
                        time.sleep(2)
                process.wait(timeout=5)
                return 124
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
                        arguments = ["--batch", "-p", str(pid),
                                     "-o", "thread backtrace all -c 48",
                                     "-o", "process detach"]
                        result = subprocess.run(
                            ["sudo", "-n", debugger, *arguments],
                            stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT, timeout=30)
                        output.write("\nDebugger exit: " + str(result.returncode) + "\n")
                    except subprocess.TimeoutExpired:
                        output.write("\nDebugger timed out; this is an incomplete observation.\n")
        return process.returncode
    finally:
        (root / "request.json").write_text(json.dumps(dict(
            debugger=debugger, debugger_available=bool(debugger), observed_pids=sorted(observed),
            outer_stopped=outer_stopped, outer_diagnostic_limit_seconds=180,
            scope="Diagnostic observations only; original compiler/test limits and verdicts apply."), indent=2) + "\n")
        if process.poll() is None:
            process.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
