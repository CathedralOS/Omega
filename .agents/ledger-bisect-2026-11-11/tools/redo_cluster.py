#!/usr/bin/env python3
"""Re-bisect part-7 clusters.

Good commit: exponential first-parent walk from the PIN (distances 1, 2, 4, ...)
until the witness exits 0. Then `git bisect --first-parent` from that commit to
the PIN. Exit 1 is only the PIN status plus the PIN diagnostic text.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
BRANCH = "work/ledger-bisect-part-7"
TOOL = Path("/tmp/ledger-bisect-tools-7")
LEDGER = Path("/tmp/ledger-bisect-pin/corpus_outcomes.txt")
STATE = Path("/tmp/ledger-bisect-pin/witness_state.json")
LOG = Path("/tmp/redo-part7.log")

CLUSTERS = [
    {
        "id": "countdown",
        "witness": "pass/termination/measure_field_computed_limit",
        "members": [
            "pass/termination/measure_computed_endpoint",
            "pass/termination/measure_field_computed_limit",
        ],
        "fixture_add": "95171450b1171650307c17c3987b1f20b09eebff",
        "keywords": ["rank range", "ranking_range", "countdown", "cannot prove rank"],
    },
    {
        "id": "family",
        "witness": "pass/proofs/family_carrier_relation_laws",
        "members": ["pass/proofs/family_carrier_relation_laws"],
        "fixture_add": "82646dcce47ba69e002f90b5e6ecaf642e9cf4ea",
        "keywords": ["proof-static", "binder argument", "proposition", "chained"],
    },
    {
        "id": "kernel",
        "witness": "pass/proofs/kernel_integer_subtract_order",
        "members": ["pass/proofs/kernel_integer_subtract_order"],
        "fixture_add": "6ede3322eb881daaa996fb898d4a04732d309d56",
        "keywords": ["rank range", "ranking_range", "divisor", "cannot prove rank"],
    },
]


def log(msg: str) -> None:
    line = f"{time.strftime('%H:%M:%S')} {msg}"
    print(line, flush=True)
    with LOG.open("a") as fh:
        fh.write(line + "\n")


def git(*args: str, check: bool = True) -> str:
    proc = subprocess.run(["git", *args], text=True, capture_output=True)
    if check and proc.returncode != 0:
        raise RuntimeError(f"git {args[0]} failed: {proc.stderr.strip()}")
    return proc.stdout.strip()


def root() -> Path:
    return Path(git("rev-parse", "--show-toplevel"))


def checkout(sha: str) -> None:
    git("checkout", "-qf", sha)


def is_ancestor(older: str, newer: str) -> bool:
    proc = subprocess.run(["git", "merge-base", "--is-ancestor", older, newer])
    return proc.returncode == 0


def rev(spec: str) -> str | None:
    proc = subprocess.run(["git", "rev-parse", "--verify", spec], text=True, capture_output=True)
    if proc.returncode != 0:
        return None
    return proc.stdout.strip()


def prepare_pin() -> None:
    here = root()
    records = (here / ".agents/ledger-bisect-2026-11-11/pin_records.txt").read_text()
    ledger = subprocess.check_output(["git", "show", f"{PIN}:tests/omega/corpus_outcomes.txt"], text=True)
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    LEDGER.write_text(ledger)
    fixtures: dict[str, dict] = {}
    current = None
    status = None
    for line in records.splitlines():
        if line.startswith("\t") and current and status and "pin_diag" not in fixtures[current]:
            fixtures[current]["pin_diag"] = line[1:].strip()
            continue
        if not line.strip() or line.startswith("#") or line.startswith("\t"):
            continue
        parts = line.split()
        if len(parts) < 2:
            continue
        current = parts[0]
        status = parts[1]
        fixtures[current] = {"pin_status": status}
    for info in fixtures.values():
        info.setdefault("pin_diag", "")
    STATE.write_text(json.dumps({"pin": PIN, "fixtures": fixtures}, indent=2))


def probe(sha: str, witness: str, mode: str = "pin", force: bool = False, keep: bool = False) -> int:
    checkout(sha)
    env = os.environ.copy()
    env.update(
        {
            "BISECT_WITNESS": witness,
            "BISECT_MODE": mode,
            "BISECT_PIN_STATE": str(STATE),
            "BISECT_PIN_LEDGER": str(LEDGER),
            "BISECT_FORCE_RERUN": "1" if force else "0",
            "BISECT_KEEP_TARGET": "1" if keep else "0",
        }
    )
    log(f"probe mode={mode} force={int(force)} {sha[:12]} {witness}")
    proc = subprocess.run(["bash", str(TOOL / "bisect_witness.sh")], env=env)
    rc = proc.returncode
    obs_path = Path(f"/tmp/bisect-obs/{witness.replace('/', '_')}/{sha}.json")
    diag = ""
    if obs_path.exists():
        obs = json.loads(obs_path.read_text())
        diag = (obs.get("diag") or obs.get("error") or ("ledger-only" if obs.get("ledger_only") else ""))[:120]
    log(f"  rc={rc} {diag}")
    return rc


def drop_target(sha: str) -> None:
    shutil.rmtree(f"/tmp/bisect-target-{sha}", ignore_errors=True)


def repeat(sha: str, witness: str, mode: str = "pin") -> list[int]:
    codes = []
    for i in range(3):
        codes.append(probe(sha, witness, mode=mode, force=True, keep=True))
        log(f"  triple {i + 1}/3 -> {codes[-1]}")
    return codes


def exponential_good(witness: str, fixture_add: str, bucket: dict) -> str | None:
    dist = 1
    probes = bucket.setdefault("walk", [])
    while dist < 2_000_000:
        spec = f"{PIN}~{dist}"
        sha = rev(spec)
        if sha is None:
            log(f"walk ended at distance {dist}: no such parent")
            break
        if not is_ancestor(fixture_add, sha):
            log(f"walk distance {dist} {sha} is older than fixture-add; stop")
            break
        rc = probe(sha, witness)
        probes.append({"distance": dist, "sha": sha, "rc": rc})
        save_partial(bucket)
        if rc == 0:
            log(f"good at distance {dist}: {sha}")
            return sha
        dist *= 2
    rc = probe(fixture_add, witness)
    probes.append({"distance": "fixture-add", "sha": fixture_add, "rc": rc})
    save_partial(bucket)
    if rc == 0:
        log(f"good is fixture-add {fixture_add}")
        return fixture_add
    log(f"never-good since {fixture_add}")
    return None


RESULTS: dict = {}
PARTIAL_CLUSTER: dict | None = None


def save_partial(bucket: dict) -> None:
    global PARTIAL_CLUSTER
    PARTIAL_CLUSTER = bucket
    path = Path("/tmp/redo-results-partial.json")
    path.write_text(json.dumps(RESULTS | {"_partial": bucket}, indent=2))


def bisect(witness: str, good: str, mode: str) -> tuple[str | None, str]:
    git("bisect", "reset", check=False)
    env = os.environ.copy()
    env.update(
        {
            "BISECT_WITNESS": witness,
            "BISECT_MODE": mode,
            "BISECT_PIN_STATE": str(STATE),
            "BISECT_PIN_LEDGER": str(LEDGER),
            "BISECT_FORCE_RERUN": "0",
            "BISECT_KEEP_TARGET": "0",
        }
    )
    start = subprocess.run(
        ["git", "bisect", "start", "--first-parent", PIN, good],
        text=True,
        capture_output=True,
    )
    if start.returncode != 0:
        log(f"bisect start failed: {start.stderr.strip()}")
        git("bisect", "reset", check=False)
        subprocess.run(["git", "bisect", "start", "--first-parent"], check=True)
        subprocess.run(["git", "bisect", "bad", PIN], check=True)
        subprocess.run(["git", "bisect", "good", good], check=True)
    log(f"bisect --first-parent mode={mode} good={good[:12]}")
    subprocess.run(["git", "bisect", "run", "bash", str(TOOL / "bisect_witness.sh")], env=env)
    blog = git("bisect", "log", check=False)
    match = re.search(r"# first bad commit: \[([0-9a-f]{40})\]", blog)
    if not match:
        match = re.search(r"^([0-9a-f]{40}) is the first bad commit", blog, re.M)
    first = match.group(1) if match else None
    git("bisect", "reset", check=False)
    checkout(BRANCH)
    log(f"bisect first-bad={first}")
    return first, blog


def pr_number(sha: str) -> str:
    text = git("log", "-1", "--format=%s%n%b", sha)
    match = re.search(r"#(\d+)", text)
    if match:
        return match.group(1)
    try:
        out = subprocess.check_output(
            ["gh", "api", f"repos/CathedralOS/Omega/commits/{sha}/pulls", "--jq", ".[0].number"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except subprocess.CalledProcessError:
        return ""
    return "" if out in ("", "null") else out


def mechanism_hunk(sha: str, keywords: list[str]) -> tuple[str, str]:
    parent = rev(f"{sha}^")
    if parent is None:
        return "", "mechanism not established"
    names = git("diff", "--name-only", parent, sha).splitlines()
    rust = [n for n in names if n.endswith(".rs") and n.startswith("omega-rust/")]
    keys = [k.lower() for k in keywords]
    for path in rust:
        diff = git("diff", "-U12", parent, sha, "--", path, check=False)
        low = diff.lower()
        if not any(k in low for k in keys):
            continue
        lines = diff.splitlines()
        for i, line in enumerate(lines):
            if line.startswith("@@") and any(k in "\n".join(lines[i : i + 40]).lower() for k in keys):
                body = "\n".join(lines[max(0, i - 2) : i + 45])
                return path, body
        return path, "\n".join(lines[:60])
    return "", "mechanism not established"


def load_results(here: Path) -> dict:
    path = here / ".agents/ledger-bisect-2026-11-11/redo_results.json"
    if path.exists():
        return json.loads(path.read_text())
    partial = Path("/tmp/redo-results-partial.json")
    if partial.exists():
        data = json.loads(partial.read_text())
        data.pop("_partial", None)
        return data
    return {}


def publish(here: Path) -> None:
    checkout(BRANCH)
    dest = here / ".agents/ledger-bisect-2026-11-11"
    (dest / "redo_results.json").write_text(json.dumps(RESULTS, indent=2) + "\n")
    subprocess.check_call([sys.executable, str(dest / "write_part7_report.py")], cwd=here)
    rels = [
        ".agents/ledger-bisect-2026-11-11/part-7.md",
        ".agents/ledger-bisect-2026-11-11/part-7.tsv",
        ".agents/ledger-bisect-2026-11-11/redo_results.json",
    ]
    subprocess.check_call(["git", "add", *rels], cwd=here)
    cid = RESULTS.get("_last", "cluster")
    msg = f"repo: part 7 re-bisect {cid} from exponential first-parent walk"
    proc = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=here)
    if proc.returncode == 0:
        log("nothing new to commit")
    else:
        subprocess.check_call(["git", "commit", "-m", msg], cwd=here)
        log(f"committed {msg}")
    for delay in (0, 4, 8, 16):
        if delay:
            time.sleep(delay)
        push = subprocess.run(["git", "push", "origin", BRANCH], cwd=here)
        if push.returncode == 0:
            log("pushed")
            return
    raise RuntimeError("git push failed")


def run_cluster(spec: dict) -> None:
    witness = spec["witness"]
    bucket = {
        "id": spec["id"],
        "witness": witness,
        "members": spec["members"],
        "fixture_add": spec["fixture_add"],
        "pin_diag": json.loads(STATE.read_text())["fixtures"][witness]["pin_diag"],
        "walk": [],
    }
    RESULTS[spec["id"]] = bucket
    RESULTS["_last"] = spec["id"]
    log(f"=== cluster {spec['id']} witness {witness}")
    pin_rc = probe(PIN, witness)
    bucket["pin_rc"] = pin_rc
    if pin_rc != 1:
        bucket["status"] = "harness-failed"
        bucket["note"] = f"PIN probe returned {pin_rc}, expected 1"
        save_partial(bucket)
        publish(root())
        raise SystemExit(f"PIN probe for {witness} returned {pin_rc}")
    good = exponential_good(witness, spec["fixture_add"], bucket)
    bucket["good"] = good
    if not good:
        bucket["status"] = "never-good"
        bucket["note"] = f"never-good since {spec['fixture_add']}"
        save_partial(bucket)
        publish(root())
        return
    first, blog = bisect(witness, good, "pin")
    bucket["bisect_log"] = blog
    bucket["first_bad"] = first
    if not first:
        bucket["status"] = "bisect-inconclusive"
        save_partial(bucket)
        publish(root())
        return
    parent = rev(f"{first}^")
    bucket["first_bad_parent"] = parent
    bucket["pr"] = pr_number(first)
    bucket["triple_bad"] = repeat(first, witness)
    bucket["triple_parent"] = repeat(parent, witness) if parent else []
    members = {}
    for member in spec["members"]:
        at_good = probe(parent, member, force=True, keep=True) if parent else None
        at_bad = probe(first, member, force=True, keep=True)
        members[member] = {"at_good": at_good, "at_bad": at_bad}
        obs = Path(f"/tmp/bisect-obs/{member.replace('/', '_')}/{first}.json")
        if obs.exists():
            members[member]["diag_at_bad"] = json.loads(obs.read_text()).get("diag", "")
    bucket["member_results"] = members
    splits = []
    for member, row in members.items():
        if member == witness:
            continue
        if row["at_good"] == 0 and row["at_bad"] == 1:
            continue
        log(f"member {member} did not flip with witness; bisecting it alone")
        sub_good = exponential_good(member, spec["fixture_add"], {"walk": []})
        sub = {"witness": member, "good": sub_good}
        if sub_good:
            sub_first, sub_log = bisect(member, sub_good, "pin")
            sub["first_bad"] = sub_first
            sub["bisect_log"] = sub_log
            if sub_first:
                sub_parent = rev(f"{sub_first}^")
                sub["first_bad_parent"] = sub_parent
                sub["pr"] = pr_number(sub_first)
                sub["triple_bad"] = repeat(sub_first, member)
                sub["triple_parent"] = repeat(sub_parent, member) if sub_parent else []
                path, hunk = mechanism_hunk(sub_first, spec["keywords"])
                sub["hunk_file"] = path
                sub["hunk"] = hunk
        splits.append(sub)
    bucket["splits"] = splits
    drop_target(first)
    if parent:
        drop_target(parent)
    path, hunk = mechanism_hunk(first, spec["keywords"])
    bucket["hunk_file"] = path
    bucket["hunk"] = hunk
    parent_ok = bucket["triple_parent"] == [0, 0, 0]
    bad_ok = bucket["triple_bad"] == [1, 1, 1]
    if parent_ok and bad_ok:
        bucket["stops_matching"] = first
        bucket["status"] = "bisected"
    else:
        log("parent/bad triple is not 0,0,0 / 1,1,1; searching where the row stops matching")
        stop, stop_log = bisect(witness, good, "stops")
        bucket["stops_matching"] = stop
        bucket["stops_log"] = stop_log
        bucket["status"] = "split-boundary" if stop and stop != first else "verification-failed"
        if stop and parent_ok is False:
            stop_parent = rev(f"{stop}^")
            bucket["stops_parent"] = stop_parent
            bucket["stops_triple_bad"] = repeat(stop, witness, mode="stops")
            bucket["stops_triple_parent"] = repeat(stop_parent, witness, mode="stops") if stop_parent else []
    save_partial(bucket)
    publish(root())


def main() -> int:
    which = sys.argv[1:] or ["all"]
    here = root()
    checkout(BRANCH)
    prepare_pin()
    global RESULTS
    RESULTS = load_results(here)
    selected = CLUSTERS if which == ["all"] else [c for c in CLUSTERS if c["id"] in which]
    if not selected:
        raise SystemExit(f"unknown cluster {which}")
    for spec in selected:
        if RESULTS.get(spec["id"], {}).get("status") in ("bisected", "never-good", "split-boundary"):
            log(f"skip finished cluster {spec['id']}")
            continue
        run_cluster(spec)
    log("all requested clusters finished")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
