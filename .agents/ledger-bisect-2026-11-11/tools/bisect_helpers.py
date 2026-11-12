#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

PIN_CORPUS = Path("/tmp/ledger_bisect_corpus_outcomes.txt")
EXPECTATIONS = Path("/tmp/ledger_bisect_expectations.json")
REPO = Path(os.environ.get("OMEGA_REPO", "/agent/repos/omega"))
def has_corpus_runner_at(rev: str) -> bool:
    try:
        manifest = subprocess.check_output(
            [
                "git",
                "-C",
                str(REPO),
                "show",
                f"{rev}:omega-rust/omega/compilation/compiler/Cargo.toml",
            ],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except subprocess.CalledProcessError:
        return False
    return 'name = "corpus_runner"' in manifest


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:200]


def parse_record(text: str, witness: str) -> dict | None:
    diagnostics: list[str] = []
    header: tuple[str, str] | None = None
    for line in text.splitlines():
        if line.startswith("\t") and header:
            diagnostics.append(line[1:].strip())
            continue
        if not line.strip():
            continue
        parts = line.split()
        if len(parts) < 2:
            continue
        if parts[0] != witness:
            continue
        header = (parts[0], parts[1])
        flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
        return {
            "fixture": parts[0],
            "status": parts[1],
            "tier": parts[0].split("/")[0],
            "expected_satisfied": "expected" in flags if "expected" in flags or "unexpected" in flags else None,
            "diagnostics": [],
            "_line": line,
        }
    if header is None:
        return None
    rec = {
        "fixture": header[0],
        "status": header[1],
        "tier": header[0].split("/")[0],
        "expected_satisfied": None,
        "diagnostics": diagnostics,
    }
    return rec


def parse_record_file(path: Path, witness: str) -> dict | None:
    text = path.read_text() if path.exists() else ""
    rec: dict | None = None
    current: str | None = None
    diags: list[str] = []
    for line in text.splitlines():
        if line.startswith("\t"):
            if current == witness:
                diags.append(line[1:].strip())
            continue
        parts = line.split()
        if len(parts) < 2:
            continue
        if current == witness and rec is not None:
            rec["diagnostics"] = diags
        if parts[0] == witness:
            flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
            exp = None
            if "expected" in flags:
                exp = True
            elif "unexpected" in flags:
                exp = False
            rec = {
                "fixture": witness,
                "status": parts[1],
                "tier": witness.split("/")[0],
                "expected_satisfied": exp,
                "diagnostics": [],
            }
            current = witness
            diags = []
        elif current == witness:
            pass
        else:
            current = None
    if rec is not None:
        rec["diagnostics"] = diags
    return rec


def load_expectations() -> dict[str, dict]:
    return json.loads(EXPECTATIONS.read_text())


def ledger_expected_status(witness: str) -> str | None:
    for line in PIN_CORPUS.read_text().splitlines():
        if line.startswith(witness + " "):
            return line.split()[1]
    return None


def classify_observation(witness: str, rec_path: Path) -> int:
    """0 good, 1 bad (PIN regression), 125 skip."""
    if not EXPECTATIONS.exists() or not PIN_CORPUS.exists():
        return 125
    exp_all = load_expectations()
    if witness not in exp_all:
        return 125
    exp = exp_all[witness]
    rec = parse_record_file(rec_path, witness)
    if rec is None:
        return 125
    bad_status = exp["bad_status"]
    bad_norm = exp.get("bad_diag_norm") or norm_diag(exp.get("bad_diag") or "")
    obs_norm = norm_diag((rec["diagnostics"] or [""])[0])
    if rec["status"] == bad_status and obs_norm == bad_norm:
        return 1
    if rec["status"] == bad_status and bad_norm and obs_norm.startswith(bad_norm[:60]):
        return 1
    expected = exp["expected_good_status"]
    if rec["status"] != expected:
        return 125
    if rec["tier"] == "fail" and exp.get("expected_satisfied") is not None:
        if rec.get("expected_satisfied") != exp["expected_satisfied"]:
            return 125
    return 0


def commit_only_ledger_or_docs(rev: str) -> bool:
    repo = str(REPO)
    try:
        files = subprocess.check_output(
            ["git", "-C", repo, "diff", "--name-only", f"{rev}^", rev],
            text=True,
            stderr=subprocess.DEVNULL,
        ).splitlines()
    except subprocess.CalledProcessError:
        return False
    if not files:
        return False
    for f in files:
        if f == "tests/omega/corpus_outcomes.txt":
            continue
        if f.endswith(".md"):
            continue
        return False
    return True


def setup_expectations_from_observed(witnesses: list[str], observed_path: Path, pin: str) -> None:
    corpus = subprocess.check_output(
        ["git", "-C", str(REPO), "show", f"{pin}:tests/omega/corpus_outcomes.txt"],
        text=True,
    )
    PIN_CORPUS.write_text(corpus)
    records: dict[str, dict] = {}
    for w in witnesses:
        rec = parse_record_file(observed_path, w)
        if rec is None:
            records[w] = {
                "bad_status": "rejected",
                "bad_diag": "",
                "bad_diag_norm": "",
            }
        else:
            first = (rec["diagnostics"] or [""])[0]
            records[w] = {
                "bad_status": rec["status"],
                "bad_diag": first,
                "bad_diag_norm": norm_diag(first),
            }
        st = None
        for line in corpus.splitlines():
            if line.startswith(w + " "):
                st = line.split()[1]
                flags = [p for p in line.split()[3:] if not p.startswith("@") and not p.endswith("ms")]
                if "expected" in flags:
                    records[w]["expected_satisfied"] = True
                elif "unexpected" in flags:
                    records[w]["expected_satisfied"] = False
                break
        records[w]["expected_good_status"] = st if st else "checked"
    EXPECTATIONS.write_text(json.dumps(records, indent=2))
