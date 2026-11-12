#!/usr/bin/env python3
"""Review mutants per lead soundness_fix rules; write shard-4.md + bucketed TSV."""

from __future__ import annotations

import csv
import difflib
import json
import os
import re
import shutil
import subprocess
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 4
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
CHECK_TIMEOUT = 90

JUDICIAL_ROUTE = frozenset({"accepted", "rejected", "incomplete", "fallback", "defect"})

# Specific wiki/spec quotes (heading context in file).
SPEC_RULES: dict[str, tuple[str, str, str]] = {
    "widen_requires_lt": (
        "wiki/spec/language/state_contracts.md",
        "Requirement clauses",
        "Machine entry obligations established at every invocation under exact actual-argument substitution.",
    ),
    "remove_transition_guard": (
        "wiki/spec/language/state_contracts.md",
        "Live facts",
        "Caller-visible mutation summaries include operand evaluation and callee writes.",
    ),
    "strip_and_guard": (
        "wiki/spec/language/state_contracts.md",
        "Live facts",
        "Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.",
    ),
    "ref_to_mut_param": (
        "wiki/spec/language/ownership.md",
        "Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    ),
    "delete_divisor_guard": (
        "wiki/spec/language/numeric_values.md",
        "Mathematical division",
        "The divisor must be nonzero before formation, including for symbolic operands.",
    ),
    "drop_mut_let": (
        "wiki/spec/language/ownership.md",
        "Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    ),
}

BUCKETS = (
    "confirmed-hole",
    "known-gap-q7-mut",
    "disagreement-valid",
    "valid-route-outside",
    "discarded-valid",
    "rejected-both",
    "timeout",
    "failed-run",
    "no-mutation",
)

ROUTE_NON_VERDICT = frozenset({"outside", "no-entry", "unreached"})

FIX5_MACHINES_QUOTE = (
    "wiki/spec/language/machines.md",
    "Nested machines and identity",
    "A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.",
)
FIX5_INFERENCE_QUOTE = (
    "wiki/spec/language/state_contracts.md",
    "Private entry inference",
    "An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.",
)


def route_is_outside(route: str) -> bool:
    return route in ROUTE_NON_VERDICT


def accepted_by_column(cur: str, route: str) -> str:
    ca, ra = cur == "accepted", route == "accepted"
    if ca and ra:
        return "both"
    if ca:
        return "current"
    if ra:
        return "route"
    return ""


def checkers_disagree(cur: str, route: str) -> bool:
    """Fix4: disagreement only when route has a real accept/reject verdict."""
    if cur in ("timeout", "error") or route in ("timeout", "error"):
        return False
    if route_is_outside(route) or route not in JUDICIAL_ROUTE | {"accepted", "rejected"}:
        return False
    return (cur == "accepted") != (route == "accepted")


def fix4_check_time_hole(kind: str, orig: str, mut: str) -> tuple[bool, str]:
    """Fix4: confirmed holes need check-time proof obligations, not runtime traps only."""
    if kind in ("drop_requires", "drop_mut_let", "widen_requires_lt", "ref_to_mut_param"):
        return True, "spec quote targets proof-time establishment (requires/ownership/contracts)"
    if kind == "delete_divisor_guard":
        return True, "nonzero divisor must be proved before formation (numeric_values.md)"
    if kind in ("remove_transition_guard", "strip_and_guard"):
        if re.search(r"\bincorrect\s*\(|\bcrashes\b", mut):
            return (
                False,
                "mutant retains incorrect()/crash routing; violation may be trapped at run time",
            )
        return True, "transition guard is a proof obligation at check time"
    return True, ""


def probe_modular_entry(
    fixture_rel: str, mutant_rel: str, removed: list[str]
) -> tuple[str, str]:
    """fix5: external violating caller; returns (hole_subtype, note)."""
    if not removed:
        return "probe-not-built", "no removed requires line"
    req = removed[0]
    m = re.search(r"requires\s+(\w+)\s*>\s*(\d+)", req)
    if not m:
        return "probe-not-built", f"no automated probe for: {req[:80]}"
    param, bound = m.group(1), m.group(2)
    try:
        mut_dir = (REPO / mutant_rel).parent
        probe_dir = mut_dir / "entry_probe"
        if probe_dir.exists():
            shutil.rmtree(probe_dir)
        shutil.copytree(mut_dir, probe_dir)
        main = (probe_dir / "main.omg").read_text()
        violator = min(int(bound), 0)
        snippet = (
            f"\n// fix5 entry probe\nmachine __soundness_entry_probe() {{\n"
            f"    let bad = {violator};\n"
            f"    // callee with removed requires `{req.strip()}` should reject this call\n"
            f"}}\n"
        )
        (probe_dir / "main.omg").write_text(main + snippet)
        env = os.environ.copy()
        env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
        mutant_ok = subprocess.run(
            ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(mut_dir / "main.omg")],
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
        probe = subprocess.run(
            ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(probe_dir / "main.omg")],
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
        if mutant_ok.returncode != 0:
            return "probe-not-built", "mutant no longer accepts under re-check"
        if probe.returncode == 0:
            return (
                "violating-caller-accepted",
                f"mutant accepts; external probe caller with {param}={violator} also accepted",
            )
        return (
            "inferred-entry",
            f"mutant accepts but external probe caller with {param}={violator} rejected",
        )
    except OSError as exc:
        return "probe-not-built", str(exc)


def minimal_diff(orig: str, mut: str) -> str:
    return "\n".join(
        list(
            difflib.unified_diff(
                orig.splitlines(),
                mut.splitlines(),
                fromfile="original",
                tofile="mutant",
                lineterm="",
                n=3,
            )
        )[:120]
    )


def requires_restated_in_transition(
    removed: list[str], mut: str
) -> tuple[bool, str, str]:
    transitions = [
        ln.strip()
        for ln in mut.splitlines()
        if re.search(r"\btransition\b", ln) and not ln.strip().startswith("//")
    ]
    trans_blob = " ".join(transitions)
    for req in removed:
        pred = re.sub(r"^.*\brequires\b", "", req).strip().rstrip(";")
        pred = re.sub(r"\s+", " ", pred)
        if len(pred) >= 6 and pred in trans_blob:
            hit = next((t for t in transitions if pred in t), transitions[0] if transitions else "")
            return True, req, hit
    return False, "", ""


def git_head() -> str:
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()


def load_rows() -> list[dict]:
    if not TSV.exists():
        return []
    with TSV.open() as f:
        return list(csv.DictReader(f, delimiter="\t"))


def read_pair(fixture_rel: str, mutant_rel: str) -> tuple[str, str]:
    orig = (REPO / fixture_rel).read_text()
    if not mutant_rel:
        return orig, orig
    mut_path = REPO / mutant_rel
    if mut_path.is_dir():
        mut_path = mut_path / "main.omg"
    mut = mut_path.read_text()
    return orig, mut


def removed_requires(orig: str, mut: str) -> list[str]:
    def reqs(text: str) -> list[str]:
        return [
            ln.strip()
            for ln in text.splitlines()
            if re.search(r"\brequires\b", ln) and not ln.strip().startswith("//")
        ]

    return [ln for ln in reqs(orig) if ln not in set(reqs(mut))]


def drop_mut_binding(mut: str) -> str | None:
    m = re.search(r"\blet\s+(\w+)\s*[:=]", mut)
    if not m:
        return None
    # Runner drops first let mut; binding is first let after mutation
    return m.group(1)


def demoted_mut_binding(orig: str, mut: str) -> str | None:
    mo = re.search(r"\blet\s+mut\s+(\w+)", orig)
    mm = re.search(r"\blet\s+(\w+)", mut)
    if not mo or not mm or mo.group(1) != mm.group(1):
        return None
    if "let mut " not in orig[: mo.start() + 20]:
        return None
    return mo.group(1)


def write_sites(mut: str, name: str) -> list[str]:
    sites: list[str] = []
    for i, ln in enumerate(mut.splitlines(), 1):
        stripped = ln.strip()
        if stripped.startswith(f"let {name}") or stripped.startswith(f"let mut {name}"):
            continue
        # Shadowing
        if re.search(rf"\blet\s+(?:mut\s+)?{re.escape(name)}\b", ln):
            break
        if re.search(rf"\b{re.escape(name)}\s*=", ln) and ": &" not in ln:
            sites.append(f"{i}: {stripped}")
        if re.search(rf"&mut\s+{re.escape(name)}\b", ln) or re.search(
            rf"&write\s+{re.escape(name)}\b", ln
        ):
            sites.append(f"{i}: {stripped}")
        if re.search(rf"\b{re.escape(name)}\.", ln) and "=" in ln.split(name, 1)[-1]:
            sites.append(f"{i}: {stripped}")
    return sites


def obligation_drop_requires(removed: list[str], mut: str) -> str | None:
    if not removed:
        return None
    body_lines = mut.splitlines()
    req_text = " ".join(removed)
    for i, ln in enumerate(body_lines, 1):
        s = ln.strip()
        if s.startswith("//") or not s:
            continue
        if re.search(r"\s/\s", ln) and re.search(r"zero|!=\s*0|>\s*0", req_text):
            return f"{i}: {s}"
        if "[" in ln and re.search(r"requires.*<|requires.*len|requires.*bound", req_text, re.I):
            return f"{i}: {s}"
        if "ensures" in req_text.lower() and "ensures" in ln:
            return f"{i}: {s}"
    # requires carried a bound name referenced in body comparisons
    for tok in re.findall(r"\b([a-zA-Z_]\w*)\s*[<>=]", req_text):
        for i, ln in enumerate(body_lines, 1):
            if re.search(rf"\b{re.escape(tok)}\b", ln) and re.search(
                r"\[|/\s*|\.len", ln
            ):
                return f"{i}: {ln.strip()}"
    return None


def assess_validity_from_mutant(kind: str, orig: str, mut: str) -> dict:
    """Invalidity from reading the mutant only (fix3-B). Route messages are ignored."""
    out = {
        "invalid": False,
        "obligation_line": "",
        "spec_rule_file": "",
        "spec_quote": "",
        "valid_reason": "",
        "q7_only": False,
    }
    if kind == "drop_requires":
        rem = removed_requires(orig, mut)
        restated, req_q, trans_q = requires_restated_in_transition(rem, mut)
        if restated:
            out["valid_reason"] = (
                "fix5: inside-machine restatement still enforces the fact | "
                f"removed requires: `{req_q}` | transition/guard: `{trans_q}`"
            )
            return out
        obl = obligation_drop_requires(rem, mut)
        if obl:
            out["invalid"] = True
            out["obligation_line"] = obl
            mfile, mhead, mquote = FIX5_MACHINES_QUOTE
            cfile, chead, cquote = FIX5_INFERENCE_QUOTE
            out["spec_rule_file"] = f"{mfile}; {cfile}"
            out["spec_quote"] = (
                f"({mhead}) {mquote} | ({chead}) {cquote}"
            )
        else:
            out["valid_reason"] = (
                "no body/ensures line needs the removed requires; "
                "no inside-machine transition restates it (external caller facts do not excuse removal)"
            )
        return out
    if kind == "drop_mut_let":
        name = demoted_mut_binding(orig, mut)
        sites = write_sites(mut, name) if name else []
        if not sites:
            out["valid_reason"] = f"demoted binding `{name or '?'}` has no write/&mut/&write on same binding"
            return out
        out["invalid"] = True
        out["q7_only"] = True
        out["obligation_line"] = "; ".join(sites[:3])
        rule = SPEC_RULES["drop_mut_let"]
        out["spec_rule_file"] = rule[0]
        out["spec_quote"] = f"({rule[1]}) {rule[2]}"
        return out
    if kind == "ref_to_mut_param":
        if "&mut " not in orig or "&mut " in mut:
            out["valid_reason"] = "mutation did not apply"
            return out
        for i, ln in enumerate(mut.splitlines(), 1):
            if re.search(r"\b\w+\.\w+\s*=", ln) or re.search(r"&mut\s+", ln):
                out["invalid"] = True
                out["obligation_line"] = f"{i}: {ln.strip()}"
                rule = SPEC_RULES["ref_to_mut_param"]
                out["spec_rule_file"] = rule[0]
                out["spec_quote"] = f"({rule[1]}) {rule[2]}"
                return out
        out["valid_reason"] = "shared receiver parameter not written through in mutant"
        return out
    if kind in SPEC_RULES:
        if mutant_still_valid_heuristic(kind, orig, mut, ""):
            out["valid_reason"] = f"other guards or facts still establish safety after `{kind}`"
            return out
        rule = SPEC_RULES[kind]
        out["invalid"] = True
        if kind == "delete_divisor_guard":
            hit = next(
                (
                    f"{i}: {ln.strip()}"
                    for i, ln in enumerate(mut.splitlines(), 1)
                    if re.search(r"\s/\s", ln)
                ),
                "",
            )
            out["obligation_line"] = hit
        elif kind in ("remove_transition_guard", "strip_and_guard"):
            hit = next(
                (
                    f"{i}: {ln.strip()}"
                    for i, ln in enumerate(mut.splitlines(), 1)
                    if ln.strip().startswith("transition")
                ),
                "",
            )
            out["obligation_line"] = hit
        out["spec_rule_file"] = rule[0]
        out["spec_quote"] = f"({rule[1]}) {rule[2]}"
        return out
    out["valid_reason"] = f"unknown mutation kind `{kind}`"
    return out


def mutant_still_valid_heuristic(kind: str, orig: str, mut: str, fixture: str) -> bool:
    if kind == "remove_transition_guard" or kind == "strip_and_guard":
        # Other guards may still protect (e.g. mixed_len_compare).
        if "guarded(" in mut and kind == "remove_transition_guard":
            return True
        if kind == "strip_and_guard":
            # If other conjuncts remain on same transition line
            if re.search(r"transition\s+[^;{]+&&", mut):
                return True
    if kind == "delete_divisor_guard":
        if not re.search(r"\s/\s", mut):
            return True
        if "!= 0" in mut or "> 0" in mut:
            return True
    if kind == "widen_requires_lt":
        return False
    return False


def classify_row(r: dict) -> dict:
    kind = r.get("mutation_kind", "")
    cur = r.get("current_verdict", "")
    route = r.get("route_verdict", "") or ""
    fixture = r.get("fixture", "")
    mutant_path = r.get("mutant_path", "")

    out = dict(r)
    out.setdefault("bucket", "")
    out.setdefault("accepted_by", "")
    out.setdefault("obligation_line", "")
    out.setdefault("spec_rule_file", "")
    out.setdefault("spec_quote", "")
    out.setdefault("validity_reason", "")
    out.setdefault("hand_check", "")

    if cur == "timeout" or route == "timeout":
        out["bucket"] = "timeout"
        out["hand_check"] = out["bucket"]
        return out

    if not mutant_path:
        out["bucket"] = "failed-run"
        out["hand_check"] = out["bucket"]
        return out

    try:
        orig, mut = read_pair(fixture, mutant_path)
    except OSError:
        out["bucket"] = "failed-run"
        out["hand_check"] = out["bucket"]
        return out

    assessment = assess_validity_from_mutant(kind, orig, mut)
    out["obligation_line"] = assessment.get("obligation_line", "")
    out["spec_rule_file"] = assessment.get("spec_rule_file", "")
    out["spec_quote"] = assessment.get("spec_quote", "")
    out["validity_reason"] = assessment.get("valid_reason", "")
    ac_by = accepted_by_column(cur, route)

    if assessment["invalid"]:
        if assessment.get("q7_only") and ac_by:
            out["bucket"] = "known-gap-q7-mut"
            out["accepted_by"] = ""
        elif ac_by:
            ok, why = fix4_check_time_hole(kind, orig, mut)
            if ok:
                out["bucket"] = "confirmed-hole"
                out["accepted_by"] = ac_by
                out["check_time_note"] = why
            else:
                out["bucket"] = "discarded-valid"
                out["validity_reason"] = why
        else:
            out["bucket"] = "rejected-both"
    elif route_is_outside(route):
        out["bucket"] = "valid-route-outside"
        out["validity_reason"] = (
            out["validity_reason"] or f"valid mutant; route non-verdict `{route}`"
        )
    elif checkers_disagree(cur, route):
        out["bucket"] = "disagreement-valid"
        out["validity_reason"] = out["validity_reason"] or "mutant read as valid; checkers disagree"
    elif cur == "accepted" and route == "accepted":
        out["bucket"] = "discarded-valid"
        out["validity_reason"] = out["validity_reason"] or "mutant valid; both checkers accept"
    elif cur == "rejected" and route == "rejected":
        out["bucket"] = "rejected-both"
    else:
        out["bucket"] = "discarded-valid"
        if not out["validity_reason"]:
            out["validity_reason"] = "mutant valid under manual read"

    out["hand_check"] = out["bucket"]
    return out


def rerun_checker(main_omg: Path) -> tuple[int, str, str]:
    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    cur = subprocess.run(
        ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(main_omg)],
        cwd=REPO,
        env=env,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )
    route = subprocess.run(
        ["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(main_omg)],
        cwd=REPO,
        env=env,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )
    cur_out = (cur.stdout + cur.stderr)[:2000]
    route_out = (route.stdout + route.stderr)[:2000]
    return cur.returncode, cur_out, route_out


def hand_verify_block(title: str, rows: list[dict], limit: int = 5) -> list[str]:
    lines = [f"## {title}", ""]
    for r in rows[:limit]:
        mp = REPO / r["mutant_path"]
        if mp.is_dir():
            mp = mp / "main.omg"
        orig = REPO / r["fixture"]
        orig_t, mut_t = read_pair(r["fixture"], r["mutant_path"])
        diff = "\n".join(
            list(
                difflib.unified_diff(
                    orig_t.splitlines(),
                    mut_t.splitlines(),
                    fromfile="original",
                    tofile="mutant",
                    lineterm="",
                    n=2,
                )
            )[:80]
        )
        code, cur_out, route_out = rerun_checker(mp)
        lines.extend(
            [
                f"### `{r['fixture']}` / {r['mutation_kind']} → {r.get('bucket')}",
                "",
                f"Route reason: `{r.get('route_rejection_reason', '')}`",
                f"Discard reason: `{r.get('discard_reason', '')}`",
                f"Obligation: `{r.get('obligation_line', '')}`",
                "",
                "```diff",
                diff,
                "```",
                "",
                f"Current checker exit={code}:",
                "",
                "```",
                cur_out[:1200],
                "```",
                "",
                "Route (first program line + diagnostics):",
                "",
                "```",
                "\n".join(
                    ln
                    for ln in route_out.splitlines()
                    if ln.startswith("program\t") or ln.startswith("diagnostic\t")
                )[:1200]
                or route_out[:1200],
                "```",
                "",
            ]
        )
    if not rows[:limit]:
        lines.append("_None in this category._")
    return lines


def hand_verify_sections(rows: list[dict]) -> list[str]:
    kept = [r for r in rows if r.get("bucket") == "confirmed-hole"]
    drop_req = [
        r for r in rows if r.get("mutation_kind") == "drop_requires" and r.get("bucket") == "discarded-valid"
    ]
    q7 = [r for r in rows if r.get("bucket") == "known-gap-q7-mut"]
    lines: list[str] = []
    lines.extend(hand_verify_block("Hand verification — confirmed holes (fix4)", kept, 5))
    lines.extend(hand_verify_block("Hand verification — discarded drop_requires (sample)", drop_req, 5))
    lines.extend(hand_verify_block("Hand verification — known-gap-q7-mut (sample)", q7, 5))
    return lines


def main() -> None:
    rows = load_rows()
    originals = sum(1 for r in rows if r.get("stage") == "original")
    mutants = [r for r in rows if r.get("stage") == "mutant"]
    mutants_run = len(mutants)
    stats_path = OUT / f"shard-{SHARD}.stats.json"
    stats_note = ""
    if stats_path.exists():
        try:
            stats = json.loads(stats_path.read_text())
            stats_note = (
                f"Runner stats: generated={stats.get('mutants_generated', '?')}, "
                f"run={stats.get('mutants_run', '?')}, "
                f"logged={stats.get('mutants_logged_tsv', '?')}, "
                f"current_rejected={stats.get('mutants_current_rejected', '?')}, "
                f"not_applicable={stats.get('mutants_not_applicable', '?')}, "
                f"failed_run={stats.get('mutants_failed_run', '?')}. "
                "Delta between generated and logged = early rejects + unlogged kinds before drop_requires rule."
            )
        except json.JSONDecodeError:
            stats_note = "Stats file present but unreadable."

    finalized_mutants: list[dict] = []
    for r in mutants:
        row = classify_row(r)
        if row.get("bucket") == "confirmed-hole" and row.get("mutation_kind") == "drop_requires":
            orig, mut = read_pair(row["fixture"], row["mutant_path"])
            rem = removed_requires(orig, mut)
            sub, note = probe_modular_entry(row["fixture"], row["mutant_path"], rem)
            row["hole_subtype"] = sub
            row["probe_note"] = note
        finalized_mutants.append(row)

    bucket_counts: Counter[str] = Counter()
    for r in finalized_mutants:
        b = r.get("bucket") or "discarded-valid"
        bucket_counts[b] += 1

    assert sum(bucket_counts.values()) == mutants_run, (
        f"buckets {sum(bucket_counts.values())} != mutants {mutants_run}"
    )

    by_kind: Counter[str] = Counter()
    by_accepted: Counter[str] = Counter()
    for r in finalized_mutants:
        if r.get("bucket") == "confirmed-hole":
            by_kind[r["mutation_kind"]] += 1
            by_accepted[r.get("accepted_by", "") or "?"] += 1

    # Rewrite TSV: originals unchanged, replace mutant rows
    orig_rows = [r for r in rows if r.get("stage") != "mutant"]
    out_rows = orig_rows + finalized_mutants
    fieldnames = [
        "fixture",
        "tier",
        "stage",
        "mutation_kind",
        "mutant_path",
        "current_verdict",
        "route_verdict",
        "route_rejection_reason",
        "seconds",
        "hand_check",
        "spec_rule_file",
        "spec_quote",
        "bucket",
        "obligation_line",
        "accepted_by",
        "validity_reason",
        "check_time_note",
        "hole_subtype",
        "probe_note",
    ]
    with TSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(out_rows)

    kept = [r for r in finalized_mutants if r.get("bucket") == "confirmed-hole"]

    fixture_rows: Counter[str] = Counter()
    for r in finalized_mutants:
        fixture_rows[r["fixture"]] += 1
    mutated_fixtures = set(fixture_rows)
    orig_fixture_set = {r["fixture"] for r in orig_rows}
    no_mutation_fixtures = sorted(orig_fixture_set - mutated_fixtures)
    multi_row = sorted((fx, n) for fx, n in fixture_rows.items() if n > 1)

    harness = git_head()
    now = datetime.now(UTC).strftime("%Y-%m-%d %H:%M UTC")
    md: list[str] = [
        f"# Soundness mutations shard {SHARD}",
        "",
        f"PIN: `{PIN}` (measured `{harness}`). Host: linux cloud agent. Generated {now}.",
        "",
        "## Bucket table (mutants only)",
        "",
        f"Total mutants run: **{mutants_run}** (must equal sum of buckets).",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for b in BUCKETS:
        if bucket_counts[b]:
            md.append(f"| `{b}` | {bucket_counts[b]} |")
    md.append(f"| **Sum** | **{sum(bucket_counts.values())}** |")
    if stats_note:
        md.extend(["", f"- {stats_note}", ""])
    md.extend(
        [
            "",
            "## Fixture / mutant reconcile (fix4)",
            "",
            f"- Fixtures in shard: **{originals}**",
            f"- Fixtures with no logged mutant row: **{len(no_mutation_fixtures)}**",
            f"- Fixtures with ≥1 mutant row: **{len(mutated_fixtures)}**",
            f"- Mutant rows in TSV: **{mutants_run}**",
            f"- Fixtures with >1 mutant row: **{len(multi_row)}**",
        ]
    )
    if multi_row[:20]:
        md.append("")
        md.append("| Fixture | Mutant rows |")
        md.append("| --- | ---: |")
        for fx, n in multi_row[:20]:
            md.append(f"| `{fx}` | {n} |")
        if len(multi_row) > 20:
            md.append(f"| … | ({len(multi_row) - 20} more) |")
    md.extend(
        [
            "",
            f"- Confirmed holes by mutation kind: "
            + (", ".join(f"{k}={v}" for k, v in sorted(by_kind.items())) or "none"),
            f"- Confirmed holes by accepted_by: "
            + (", ".join(f"{k}={v}" for k, v in sorted(by_accepted.items())) or "none"),
            "",
            "## Commands",
            "",
            "```bash",
            "bash tools/agent_setup.sh",
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison < /dev/null",
            "export OMEGA_LIBRARY_ROOT=$PWD/source/library",
            "python3 .agents/four-jobs-2026-11-12/soundness/shard4_runner.py",
            "python3 .agents/four-jobs-2026-11-12/soundness/shard4_finalize.py",
            "```",
            "",
        ]
    )
    md.extend(hand_verify_sections(finalized_mutants))
    md.extend(["", "## Confirmed holes and disagreements", ""])
    if not kept:
        md.append("_No confirmed-hole rows after fix3/fix4 review._")
    else:
        for i, r in enumerate(kept, 1):
            orig_t, mut_t = read_pair(r["fixture"], r["mutant_path"])
            diff = minimal_diff(orig_t, mut_t)
            code, cur_out, route_out = rerun_checker(REPO / r["mutant_path"])
            md.extend(
                [
                    f"### {i}. `{r['fixture']}` ({r['mutation_kind']})",
                    "",
                    f"- Bucket: `{r['bucket']}`; accepted_by: **{r.get('accepted_by', '')}**",
                    f"- Mutant: `{r['mutant_path']}`",
                    f"- Current: **{r['current_verdict']}**; route: **{r['route_verdict']}**",
                    f"- Route hint (not validity): `{r.get('route_rejection_reason', '')}`",
                    f"- hole_subtype (fix5): **{r.get('hole_subtype', '')}** — {r.get('probe_note', '')}",
                    f"- Check-time note (fix4a): {r.get('check_time_note', '')}",
                    f"- Obligation line (fix4c): `{r.get('obligation_line', '')}`",
                    f"- Spec file/heading/quote (fix4a): `{r.get('spec_rule_file', '')}` — {r.get('spec_quote', '')}",
                    f"- Check-only evidence (fix4b): `omega --check` exit **{code}** on mutant; no runtime witness run in this shard",
                    "",
                    "```diff",
                    diff,
                    "```",
                    "",
                    "```",
                    cur_out[:800],
                    "```",
                    "",
                ]
            )

    MD.write_text("\n".join(md) + "\n")
    print(f"mutants={mutants_run} buckets_sum={sum(bucket_counts.values())}")
    print(f"confirmed-hole={bucket_counts.get('confirmed-hole', 0)}")
    print(f"accepted_by={dict(by_accepted)}")
    print(f"by_kind={dict(by_kind)}")


if __name__ == "__main__":
    main()
