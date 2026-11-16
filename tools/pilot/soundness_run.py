#!/usr/bin/env python3
"""PILOT ONLY: run mutants under the Psi interpreter with the soundness oracle.

Usage: soundness_run.py <omega-binary> <mutant-root> <results.jsonl> [--jobs N] [--both]

Each mutant directory holds main.omg/build.omg. A mutant the compiler refuses
is `refused`. An accepted mutant is a candidate finding when the oracle file
holds a VIOLATION line, or when the Psi run fails with an interpreter fault
rather than completing or reaching a declared crash.
"""
import concurrent.futures as cf
import json
import os
import subprocess
import sys
import tempfile
import time


def run_one(binary, mutant, both):
    oracle = tempfile.NamedTemporaryFile(prefix="oracle", suffix=".txt", delete=False).name
    env = dict(os.environ, OMEGA_SOUNDNESS_ORACLE=oracle)
    root = os.path.join(mutant, "build.omg")
    cmd = [binary, "run"] + (["--both"] if both else ["--engine", "psi"]) + [root]
    start = time.time()
    try:
        proc = subprocess.run(cmd, stdin=subprocess.DEVNULL, capture_output=True, text=True,
                              timeout=120, env=env, cwd=mutant)
        code, out, err = proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired:
        code, out, err = "timeout", "", ""
    seconds = time.time() - start
    lines = open(oracle).read().splitlines() if os.path.exists(oracle) else []
    os.unlink(oracle)
    meta = json.load(open(os.path.join(mutant, "mutation.json")))
    violations = [l for l in lines if " VIOLATION " in l]
    decided = sum(1 for l in lines if " ok " in l or " VIOLATION " in l)
    text = (out + "\n" + err)
    return {
        "mutant": mutant, **meta, "exit": code, "seconds": round(seconds, 2),
        "oracle_lines": len(lines), "oracle_decided": decided, "violations": violations[:5],
        "output": text[-1500:],
    }


def main():
    binary, root, results = sys.argv[1], sys.argv[2], sys.argv[3]
    jobs = int(sys.argv[sys.argv.index("--jobs") + 1]) if "--jobs" in sys.argv else 3
    both = "--both" in sys.argv
    mutants = sorted(
        os.path.join(dp) for dp, dn, fn in os.walk(root) if "mutation.json" in fn
    )
    with open(results, "w") as out, cf.ThreadPoolExecutor(jobs) as pool:
        for row in pool.map(lambda m: run_one(binary, m, both), mutants):
            out.write(json.dumps(row) + "\n")
            out.flush()


if __name__ == "__main__":
    main()
