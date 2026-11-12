#!/usr/bin/env python3
"""First first-parent commit on FROM..TO showing PIN status+diagnostic for witness."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

ROOT = Path("/agent/repos/omega")
LB = Path("/tmp/ledger-bisect-2026-11-11")
PIN_OBS = LB / "pin_full.txt"


def run_at_commit(commit: str, witness: str) -> dict | None:
    wt = Path(f"/tmp/ledger-bisect-wt-{commit[:12]}")
    tdir = ROOT / f"target/verify-{commit[:12]}"
    subprocess.run(["rm", "-rf", str(wt)], check=False)
    subprocess.run(
        ["git", "-C", str(ROOT), "worktree", "add", "-f", str(wt), commit],
        check=True,
        capture_output=True,
    )
    env = {**os.environ, "OMEGA_REPO": str(wt), "PIN_OBS": str(PIN_OBS)}
    try:
        subprocess.run(
            [
                "nice",
                "-n",
                "19",
                "bash",
                "-lc",
                f"CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR={tdir} mbx build -p omega < /dev/null",
            ],
            check=True,
            env=env,
            capture_output=True,
        )
        subprocess.run(
            [
                "nice",
                "-n",
                "19",
                "bash",
                "-lc",
                f"CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR={tdir} mbx test -p compiler corpus_runner --no-run < /dev/null",
            ],
            check=True,
            env=env,
            capture_output=True,
        )
        subprocess.run(
            ["python3", str(LB / "tools/prepare_bisect_pin.py"), witness],
            check=True,
            env=env,
            capture_output=True,
        )
        runner = sorted(
            (tdir / "debug/build/compiler").glob("*/out/corpus_runner-*"),
            key=lambda p: p.stat().st_mtime,
            reverse=True,
        )[0]
        rec = Path("/tmp/pin_class_one.txt")
        rec.unlink(missing_ok=True)
        subprocess.run(
            [
                "nice",
                "-n",
                "19",
                str(runner),
            ],
            check=False,
            env={
                **env,
                "OMEGA_CORPUS_ROOT": str(wt),
                "OMEGA_LIBRARY_ROOT": str(wt / "source/library"),
                "OMEGA_CORPUS_FIXTURE_FILTER": witness,
                "OMEGA_CORPUS_RECORDS": str(rec),
            },
            input=b"",
            capture_output=True,
        )
        if not rec.exists() or witness not in parse_records(rec.read_text()):
            return None
        return parse_records(rec.read_text())[witness]
    finally:
        subprocess.run(["git", "-C", str(ROOT), "worktree", "remove", "-f", str(wt)], check=False)


def main() -> int:
    witness = sys.argv[1]
    from_c = sys.argv[2]
    to_c = sys.argv[3]
    meta = json.loads(Path("/tmp/ledger-bisect-witness.json").read_text()) if Path(
        "/tmp/ledger-bisect-witness.json"
    ).exists() else None
    if not meta:
        subprocess.run(
            ["python3", str(LB / "tools/prepare_bisect_pin.py"), witness],
            check=True,
            env={"OMEGA_REPO": str(ROOT), "PIN_OBS": str(PIN_OBS)},
        )
        meta = json.loads(Path("/tmp/ledger-bisect-witness.json").read_text())
    pin_records = parse_records(PIN_OBS.read_text())
    pin_diag_norm = norm_diag((pin_records[witness]["diagnostics"] or [""])[0])
    pin_status = pin_records[witness]["status"]
    commits = subprocess.check_output(
        ["git", "-C", str(ROOT), "rev-list", "--first-parent", "--reverse", f"{from_c}..{to_c}"],
        text=True,
    ).splitlines()
    for c in commits:
        obs = run_at_commit(c, witness)
        if not obs:
            print(f"skip {c}", file=sys.stderr)
            continue
        dnorm = norm_diag((obs.get("diagnostics") or [""])[0])
        print(
            f"probe {c} status={obs['status']} diag={dnorm[:80]}",
            file=sys.stderr,
        )
        if obs["status"] == pin_status and dnorm == pin_diag_norm:
            print(c)
            return 0
    print("", end="")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
