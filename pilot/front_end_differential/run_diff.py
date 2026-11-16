#!/usr/bin/env python3
"""Front-end differential pilot: run each corpus program through the
Omega-written front end (tests/psi/checking harness, native) and the Rust
compiler (`omega --check`), and record both verdicts.

usage: run_diff.py <harness-program> <omega-cli> <corpus.jsonl> <out.jsonl> [jobs]
corpus rows: {"id":..., "origin":..., "mutation":..., "source": <str>}
"""
import json, os, struct, subprocess, sys, tempfile, time
from concurrent.futures import ThreadPoolExecutor

HARNESS, CLI, CORPUS, OUT = sys.argv[1:5]
JOBS = int(sys.argv[5]) if len(sys.argv) > 5 else 4
PHASE = {0: "accept", 247: "check", 248: "resolve", 249: "type", 250: "parse",
         251: "lex", 252: "capacity", 253: "framing"}


def frame(source: bytes) -> bytes:
    name = b"gate"
    p = struct.pack("<I", 1) + struct.pack("<II", 0, len(name)) + name
    p += struct.pack("<II", 0, 1)              # no dependency edges, one unit
    p += struct.pack("<II", 0, 0)              # package 0, root module path ""
    return p + struct.pack("<I", len(source)) + source


def omega_written(source: bytes):
    t = time.time()
    try:
        r = subprocess.run([HARNESS], input=frame(source), capture_output=True, timeout=60)
        code = r.returncode
        phase = PHASE.get(code, f"exit{code}")
    except subprocess.TimeoutExpired:
        phase = "timeout"
    return phase, time.time() - t


def rust(source: bytes):
    t = time.time()
    with tempfile.TemporaryDirectory(prefix="fediff-") as d:
        path = os.path.join(d, "main.omg")
        open(path, "wb").write(source)
        try:
            r = subprocess.run([CLI, "--check", path], stdin=subprocess.DEVNULL,
                               capture_output=True, timeout=120, cwd=d)
            text = (r.stdout + r.stderr).decode(errors="replace")
            verdict = "accept" if r.returncode == 0 else "refuse"
            first = next((l for l in text.splitlines() if "error" in l.lower()), text[:200])
        except subprocess.TimeoutExpired:
            verdict, first, text = "timeout", "", ""
    return verdict, first.strip()[:300], text[:4000], time.time() - t


def one(row):
    src = row["source"].encode()
    ow, owt = omega_written(src)
    rv, rfirst, rtext, rt = rust(src)
    return {**{k: row[k] for k in ("id", "origin", "mutation")},
            "omega_written": ow, "rust": rv, "rust_first": rfirst, "rust_text": rtext,
            "t_omega": round(owt, 3), "t_rust": round(rt, 3)}


rows = [json.loads(l) for l in open(CORPUS)]
start = time.time()
with open(OUT, "w") as out, ThreadPoolExecutor(JOBS) as pool:
    for i, res in enumerate(pool.map(one, rows)):
        out.write(json.dumps(res) + "\n")
        if i % 200 == 0:
            print(f"{i}/{len(rows)} {time.time()-start:.0f}s", flush=True)
print(f"done {len(rows)} in {time.time()-start:.0f}s", flush=True)
