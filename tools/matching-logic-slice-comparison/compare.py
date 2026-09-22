#!/usr/bin/env python3
"""Bounded comparison record for a matching-logic slice versus the current
Omega proof route, per wiki/drafts/reference/matching_logic.md ("Possible bounded
comparison").

The comparison checklist asks for: checker size, translation size, theory
size, certificate size and checking time, and every imported rule,
assumption, or trusted bridge — measured on identical pinned positive and
negative cases, with logical fragment, rule/semantics versions, subject,
capsule, observation profile, bridge graph, and admissions retained.

This tool produces the record for the route that exists today: it measures
the current receiver-checking and theory surfaces, inventories the accepted
proof-rule foundations from proof-admission's enforced classifier, and runs
the pinned positive/negative corpus pairs through `omega --check`. The
candidate side's translation leg is measured: the typed-to-one-sorted
encoder under `tools/matching-logic-sort-encoding` is run over its pinned
case corpus and its emitted clause inventory — every clause an axiom
admission — lands under `candidate.sort_encoding`. The candidate checker,
theory, and certificate columns are filled from
MATCHING-LOGIC-BOUNDED-SLICE's landed slice: `tools/matching-logic-slice`
`slice_checker.py record` is re-run and its emitted record lands under
`candidate.slice`. MATCHING-LOGIC-COMPARISON-METRICS
(tools/matching-logic-metrics) remains the pending column owner.

Standard library only. Run from the repository root or the crate directory:

    python3 tools/matching-logic-slice-comparison/compare.py \
        --omega target/debug/omega \
        --record tools/matching-logic-slice-comparison/record.json \
        --report wiki/drafts/reference/matching_logic_slice_comparison.md

`--skip-run` measures only the static surfaces (no case execution).
`--native-sidecar <path>` records the size of a produced `.proof` or
`.psi.proof` certificate artifact when one is available from a native
compile (certificate bytes are otherwise recorded as pending: bare-file
`--check` runs deliberately emit no artifact).
"""

import argparse
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
PINNED_CASES = HERE / "pinned_cases.json"
CLASSICALITY = Path(
    "omega-rust/psi/semantics/proof-admission/src/classicality.rs"
)
SORT_ENCODING = Path("tools/matching-logic-sort-encoding")
SLICE = Path("tools/matching-logic-slice")
SLICE_RECORD_SCHEMA = "omega-matching-logic-slice-record/1"

# Receiver-side checking surfaces, by honest role. The comparison measures
# what a receiver or producer must trust/run, so components are broken out
# rather than summed into a single opaque number.
SURFACES = {
    # Terminal Psi artifact verification: the receiver's checker for a
    # compiled semantic product (terminal-verifier) plus the codec modules
    # that decode and verify the emitted proof structures (trust graph,
    # proof bundle, proof sidecar sections).
    "receiver_terminal_verifier": [
        "omega-rust/psi/semantics/terminal-verifier/src",
    ],
    "receiver_pcc_codec": [
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_sidecar.rs",
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle",
        "omega-rust/psi/semantics/terminal-codec/src/sections/trust_graph",
        "omega-rust/psi/semantics/terminal-codec/src/sections/trust_graph.rs",
        "omega-rust/omega/compiler/compilation-report/src/pcc.rs",
        "omega-rust/omega/compiler/compilation-report/src/pcc",
    ],
    # The admission kernel and the mathematical theory the kernel checks
    # against: sorts, terms, denotation, integer rule theory. On the
    # candidate side these are the "theory" the encoding must restate.
    "admission_kernel": [
        "omega-rust/psi/semantics/proof-admission/src/kernel.rs",
        "omega-rust/psi/semantics/proof-admission/src/kernel",
        "omega-rust/psi/semantics/proof-admission/src/admission.rs",
        "omega-rust/psi/semantics/proof-admission/src/admission",
        "omega-rust/psi/semantics/proof-admission/src/classicality.rs",
        "omega-rust/psi/semantics/proof-admission/src/proof.rs",
        "omega-rust/psi/semantics/proof-admission/src/proof",
    ],
    "theory_surface": [
        "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        "omega-rust/psi/semantics/proof-admission/src/mathematical_core",
        "omega-rust/psi/semantics/proof-admission/src/integer_rules.rs",
        "omega-rust/psi/semantics/proof-admission/src/integer_rules",
        "omega-rust/psi/semantics/proof-admission/src/predicate_denotation.rs",
        "omega-rust/psi/semantics/proof-admission/src/predicate_denotation",
    ],
    # Producer-side proof derivation: obligations, checker, derivation
    # store. Not trusted on the receiver side, but part of the complete
    # current route the comparison must count.
    "derivation_support": [
        "omega-rust/psi/semantics/proof/src",
    ],
}

RULE_RE = re.compile(r"AcceptedProofRule::([A-Za-z0-9_]+)")
FOUNDATION_RE = re.compile(
    r"((?:\s*\|?\s*Self::[A-Za-z0-9_]+)+)\s*=>"
    r"\s*ProofRuleFoundation::([A-Za-z0-9_]+)"
)
ARM_NAME_RE = re.compile(r"Self::([A-Za-z0-9_]+)")


def repo_root() -> Path:
    """Resolve the repository root: works from a worktree or the checkout."""
    return Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )


def count_lines(path: Path, pattern: str = "*.rs") -> dict:
    """Physical line counts for a file or directory of matching sources."""
    files = sorted(path.rglob(pattern)) if path.is_dir() else [path]
    total = 0
    nonblank = 0
    missing = []
    for file in files:
        if not file.exists():
            missing.append(str(file))
            continue
        lines = file.read_text(encoding="utf-8").splitlines()
        total += len(lines)
        nonblank += sum(1 for line in lines if line.strip())
    return {
        "files": len([f for f in files if f.exists()]),
        "lines": total,
        "nonblank_lines": nonblank,
        "missing": missing,
    }


def measure_surfaces(root: Path) -> dict:
    measured = {}
    for name, paths in SURFACES.items():
        totals = {"files": 0, "lines": 0, "nonblank_lines": 0, "missing": []}
        for rel in paths:
            stats = count_lines(root / rel)
            totals["files"] += stats["files"]
            totals["lines"] += stats["lines"]
            totals["nonblank_lines"] += stats["nonblank_lines"]
            totals["missing"].extend(stats["missing"])
        measured[name] = totals
    measured["trusted_check_total"] = {
        "lines": measured["receiver_terminal_verifier"]["lines"]
        + measured["receiver_pcc_codec"]["lines"],
        "nonblank_lines": measured["receiver_terminal_verifier"]["nonblank_lines"]
        + measured["receiver_pcc_codec"]["nonblank_lines"],
        "note": "receiver-side verification surface only; producer derivation "
        "and admission machinery are reported separately",
    }
    return measured


def rule_inventory(root: Path) -> dict:
    """Read the enforced rule->foundation classification out of
    classicality.rs. The `foundation` match is exhaustive, so parsing it is
    parsing the audited inventory itself."""
    text = (root / CLASSICALITY).read_text(encoding="utf-8")
    inventory = {}
    for arm, foundation in FOUNDATION_RE.findall(text):
        for name in ARM_NAME_RE.findall(arm):
            inventory[name] = foundation
    all_block = re.search(
        r"const ALL: \[AcceptedProofRule; (\d+)\] = \[(.*?)\];",
        text,
        re.DOTALL,
    )
    declared = RULE_RE.findall(all_block.group(2)) if all_block else []
    counts = {}
    for foundation in inventory.values():
        counts[foundation] = counts.get(foundation, 0) + 1
    return {
        "rules": dict(sorted(inventory.items())),
        "counts_by_foundation": counts,
        "declared_rule_count": int(all_block.group(1)) if all_block else 0,
        "declared_rules": declared,
        "unclassified_declared": sorted(set(declared) - set(inventory)),
    }


def run_case(omega: Path, root: Path, rel: str, timeout: int) -> dict:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            [str(omega), "--check", str(root / rel)],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        output = proc.stdout + proc.stderr
        return {
            "exit_code": proc.returncode,
            "elapsed_ms": round((time.monotonic() - start) * 1000, 1),
            "output": output.strip(),
        }
    except subprocess.TimeoutExpired:
        return {
            "exit_code": None,
            "elapsed_ms": timeout * 1000,
            "timed_out": True,
            "output": "",
        }


def run_pinned_cases(
    omega: Path, root: Path, timeout: int, include_heavy: bool
) -> list:
    manifest = json.loads(PINNED_CASES.read_text(encoding="utf-8"))
    results = []
    for pair in manifest["pairs"]:
        # `heavy` pairs depend on source/library/std: each --check re-checks
        # the full package closure (minutes). They stay pinned but opt-in.
        if pair.get("heavy") and not include_heavy:
            results.append(
                {
                    "subject": pair["subject"],
                    "positive": {"path": pair["positive"], "skipped": "heavy"},
                    "negative": {"path": pair["negative"], "skipped": "heavy"},
                }
            )
            continue
        positive = run_case(omega, root, pair["positive"], timeout)
        negative = run_case(omega, root, pair["negative"], timeout)
        # The fail corpus's own expected.txt is the expected-diagnostic
        # contract; match every nonblank line, not a duplicated copy.
        expected_file = (root / pair["negative"]).parent / "expected.txt"
        fragments = [
            line.strip()
            for line in expected_file.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ] if expected_file.exists() else [pair["expected_fragment"]]
        neg_output = negative["output"]
        results.append(
            {
                "subject": pair["subject"],
                "positive": {
                    "path": pair["positive"],
                    **positive,
                    "accepted": positive["exit_code"] == 0,
                },
                "negative": {
                    "path": pair["negative"],
                    **negative,
                    "rejected": (
                        negative["exit_code"] not in (0, None)
                    ),
                    "expected_fragments": fragments,
                    "fragment_observed": all(
                        fragment in neg_output for fragment in fragments
                    ),
                },
            }
        )
    return results


def sort_encoding_cases(root: Path, timeout: int = 120) -> dict:
    """Run the typed-to-one-sorted encoder over its pinned case corpus.

    The encoder is the candidate route's translation leg: each case is a
    typed subject schema, and `check` emits the clause inventory — every
    clause is an axiom admission — plus the consistency diagnostics the
    comparison checklist requires. Per-case rows retain the checklist
    provenance (fragment, versions, subject, capsule, bridge graph,
    admissions) verbatim from the encoder's emitted record."""
    tool = root / SORT_ENCODING / "sort_encoding.py"
    cases_dir = root / SORT_ENCODING / "cases"
    if not tool.exists() or not cases_dir.is_dir():
        return {
            "status": "absent",
            "tool": str(SORT_ENCODING / "sort_encoding.py"),
            "note": "encoding tool not in this checkout; "
            "candidate translation column unmeasured",
        }
    results = []
    for case_path in sorted(cases_dir.glob("*.json")):
        start = time.monotonic()
        try:
            proc = subprocess.run(
                [sys.executable, str(tool), "check", str(case_path)],
                capture_output=True,
                text=True,
                timeout=timeout,
            )
        except subprocess.TimeoutExpired:
            results.append(
                {
                    "case": case_path.name,
                    "timed_out": True,
                    "exit_code": None,
                    "elapsed_ms": timeout * 1000,
                }
            )
            continue
        try:
            emitted = json.loads(proc.stdout)
        except json.JSONDecodeError:
            emitted = {}
        row = {
            "case": case_path.name,
            "exit_code": proc.returncode,
            "elapsed_ms": round((time.monotonic() - start) * 1000, 1),
            "check_clean": proc.returncode == 0,
            "diagnostics": emitted.get("diagnostics", []),
            "admissions": emitted.get("admissions", []),
            "declared_memberships": len(
                emitted.get("declared_type_memberships", [])
            ),
        }
        for key in (
            "schema",
            "fragment",
            "rule_version",
            "semantics_version",
            "subject",
            "target_capsule",
            "observation_profile",
            "bridge_graph",
        ):
            row[key] = emitted.get(key)
        results.append(row)
    clean = sum(1 for r in results if r.get("check_clean"))
    pinning = sum(1 for r in results if r.get("exit_code") not in (0, None))
    first = results[0] if results else {}
    return {
        "status": "measured",
        "tool": str(SORT_ENCODING / "sort_encoding.py"),
        "schema": first.get("schema"),
        "fragment": first.get("fragment"),
        "rule_version": first.get("rule_version"),
        "translation_surface": count_lines(root / SORT_ENCODING, "*.py"),
        "cases": results,
        "clean_cases": clean,
        "violation_pinning_cases": pinning,
    }


def candidate_slice(root: Path, timeout: int = 120) -> dict:
    """Run the bounded matching-logic slice checker over its pinned corpus.

    `slice_checker.py record` re-derives every pinned case, emits the
    `omega-matching-logic-slice-record/1` record on stdout, and exits nonzero
    on any verdict divergence. Its emitted columns map onto the comparison
    checklist: checker size and rule inventory (`checker`), theory
    (`theory` — a per-case axiom inventory), certificate bytes and checking
    time (`cases[].certificate_bytes` / `elapsed_ms`), and the imported
    rule/assumption inventory (`cases[].admissions` — every clause an axiom
    admission; the slice has no trusted bridge)."""
    tool = root / SLICE / "slice_checker.py"
    if not tool.exists():
        return {
            "status": "absent",
            "tool": str(SLICE / "slice_checker.py"),
            "note": "slice checker not in this checkout; "
            "candidate column unmeasured",
        }

    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "slice-record.json"
        try:
            proc = subprocess.run(
                [sys.executable, str(tool), "record", "--out", str(out)],
                cwd=root,
                capture_output=True,
                text=True,
                timeout=timeout,
            )
        except subprocess.TimeoutExpired:
            return {
                "status": "failed",
                "tool": str(SLICE / "slice_checker.py"),
                "note": f"slice record timed out after {timeout}s",
            }
    try:
        emitted = json.loads(proc.stdout)
    except json.JSONDecodeError:
        emitted = {}
    if emitted.get("schema") != SLICE_RECORD_SCHEMA:
        return {
            "status": "failed",
            "tool": str(SLICE / "slice_checker.py"),
            "exit_code": proc.returncode,
            "note": "slice record missing or schema mismatch "
            f"(expected {SLICE_RECORD_SCHEMA})",
        }
    checker = emitted.get("checker", {})
    cases = emitted.get("cases", [])
    admissions = sorted(
        {a for case in cases for a in case.get("admissions", [])}
    )
    divergences = emitted.get("summary", {}).get("divergences", [])
    return {
        "status": "measured",
        "tool": str(SLICE / "slice_checker.py"),
        "schema": emitted.get("schema"),
        "fragment": emitted.get("fragment"),
        "semantics_doc": emitted.get("semantics_doc"),
        "recorded_utc": emitted.get("recorded_utc"),
        "checker": {
            "path": checker.get("path"),
            "physical_lines": checker.get("physical_lines"),
            "sha256_prefix": (checker.get("sha256") or "")[:16],
            "rules": checker.get("rules", []),
        },
        "theory": emitted.get("theory"),
        "cases": [
            {
                "case": case.get("case"),
                "expect": case.get("expect"),
                "verdict": case.get("verdict"),
                "elapsed_ms": case.get("elapsed_ms"),
                "certificate_bytes": case.get("certificate_bytes"),
                "admissions": case.get("admissions", []),
                "diagnostic": case.get("diagnostic"),
            }
            for case in cases
        ],
        "imported_admissions": admissions,
        "trusted_bridge": "none — every consumed clause is an axiom admission",
        "summary": emitted.get("summary", {}),
        "divergence_free": proc.returncode == 0 and not divergences,
    }


def version_context(root: Path, omega: Path) -> dict:
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    toolchain = (root / "rust-toolchain.toml").read_text(encoding="utf-8")
    channel = re.search(r'channel\s*=\s*"([^"]+)"', toolchain)
    return {
        "repository_revision": revision,
        "toolchain_channel": channel.group(1) if channel else "unknown",
        "omega_binary_sha256_prefix": None,
        "host": {
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "omega_binary": str(omega),
        "omega_binary_bytes": omega.stat().st_size if omega.exists() else None,
    }


def render_report(record: dict) -> str:
    surfaces = record["omega_route"]["surface_lines"]
    cases = record["omega_route"]["pinned_cases"]
    rules = record["omega_route"]["rule_inventory"]
    ran = [c for c in cases if "accepted" in c["positive"]]
    ok = all(
        c["positive"]["accepted"] and c["negative"]["rejected"]
        for c in ran
    )
    frag_ok = all(c["negative"]["fragment_observed"] for c in ran)
    skipped = [c["subject"] for c in cases if "skipped" in c["positive"]]
    lines = [
        "# Matching-logic slice comparison — bounded record",
        "",
        "Status: "
        + (
            "both columns measured; the metrics aggregation axis stays "
            "pending on `MATCHING-LOGIC-COMPARISON-METRICS` "
            "(tools/matching-logic-metrics)"
            if record["candidate"].get("status") == "measured"
            else "measured Omega-side column; candidate-side columns are "
            "pending landings of `MATCHING-LOGIC-BOUNDED-SLICE` "
            "(tools/matching-logic-slice) and `MATCHING-LOGIC-COMPARISON-"
            "METRICS` (tools/matching-logic-metrics)"
        )
        + ".",
        "Method and checklist: [matching_logic.md](matching_logic.md). "
        "Regenerate: `python3 tools/matching-logic-slice-comparison/compare.py`.",
        "",
        "## Provenance",
        "",
        f"- revision: `{record['versions']['repository_revision']}`",
        f"- toolchain: `{record['versions']['toolchain_channel']}`",
        f"- host: {record['versions']['host']['system']} "
        f"{record['versions']['host']['machine']}",
        f"- checker binary: `{record['versions']['omega_binary']}`",
        "",
        "## Route sizes (physical lines of Rust)",
        "",
        "| component | role | files | lines |",
        "| --- | --- | --- | --- |",
    ]
    roles = {
        "receiver_terminal_verifier": "receiver: Terminal Psi artifact verification",
        "receiver_pcc_codec": "receiver: proof sidecar / bundle / trust-graph checking",
        "admission_kernel": "admission kernel + enforced rule classifier",
        "theory_surface": "mathematical theory the kernel checks against",
        "derivation_support": "producer: obligations, checker, derivation store",
        "trusted_check_total": "receiver-side total (what a receiver must run)",
    }
    for name, stats in surfaces.items():
        files = stats.get("files", "-")
        lines.append(
            f"| `{name}` | {roles.get(name, '')} | {files} | {stats['lines']} |"
        )
    lines += [
        "",
        "Translation surface on the current route: `0` — semantic products are "
        "checked natively; there is no theory translation layer to measure.",
        "",
        "## Accepted proof-rule inventory (enforced classifier)",
        "",
        "Source: `proof-admission/src/classicality.rs` — the `foundation` match "
        "is exhaustive over `AcceptedProofRule`, so this table is the audited "
        "inventory, not a sampled one.",
        "",
        "| foundation | rules |",
        "| --- | --- |",
    ]
    for foundation, count in sorted(rules["counts_by_foundation"].items()):
        names = ", ".join(
            f"`{n}`" for n, f in rules["rules"].items() if f == foundation
        )
        lines.append(f"| {foundation} | {count} ({names}) |")
    lines += [
        "",
        f"Declared rule variants: {rules['declared_rule_count']}; unclassified: "
        f"{rules['unclassified_declared'] or 'none'}.",
        "",
        "## Identical pinned positive/negative cases",
        "",
        "Every negative declares its positive twin in its header comment; both "
        "sides exercise the same proposition. Contract: positive checks "
        "(exit 0), negative rejects emitting the corpus `expected.txt` fragment.",
        "",
        "| subject | positive ms | negative ms | fragment observed |",
        "| --- | --- | --- | --- |",
    ]
    for case in cases:
        if "skipped" in case["positive"]:
            lines.append(
                f"| `{case['subject']}` | skipped | skipped "
                f"| heavy pair, `--include-heavy` |"
            )
            continue
        lines.append(
            f"| `{case['subject']}` | {case['positive']['elapsed_ms']} "
            f"| {case['negative']['elapsed_ms']} "
            f"| {'yes' if case['negative']['fragment_observed'] else 'NO'} |"
        )
    divergent = [
        c
        for c in ran
        if not (
            c["positive"]["accepted"]
            and c["negative"]["rejected"]
            and c["negative"]["fragment_observed"]
        )
    ]
    lines += [
        "",
        f"All {len(ran)} run positives accepted and negatives rejected: "
        f"{'yes' if ok else 'NO'}; expected fragments observed: "
        f"{'yes' if frag_ok else 'NO'}. Skipped heavy pairs: "
        f"{skipped or 'none'}.",
    ]
    if divergent:
        lines += [
            "",
            "### Divergences observed on the current route",
            "",
        ]
        for case in divergent:
            which = (
                "positive rejected"
                if not case["positive"]["accepted"]
                else "negative discipline violation"
            )
            detail = (
                case["positive"].get("output")
                or case["negative"].get("output")
                or ""
            )
            lines += [
                f"- `{case['subject']}` — {which}:",
                f"  `{detail.splitlines()[0] if detail else ''}`",
            ]
    lines += [
        "",
        "## Certificate artifact",
        "",
        "Bare-file `--check` runs emit no artifact (`wrote_output=false`), so "
        "certificate bytes are measured from a produced `.proof` / "
        "`.psi.proof` sidecar via `--native-sidecar` and currently recorded "
        f"as: `{record['omega_route']['certificate']}`.",
        "",
        "## Candidate side",
        "",
    ]
    slice_column = record["candidate"].get("slice", {})
    if slice_column.get("status") == "measured":
        checker = slice_column["checker"]
        summary = slice_column.get("summary", {})
        lines += [
            "The bounded slice checker is landed and measured — "
            f"`{checker['path']}` ({checker['physical_lines']} physical lines, "
            f"sha256 `{checker['sha256_prefix']}`, "
            f"{len(checker['rules'])} rules). Theory: "
            f"{slice_column['theory']}. Fragment: `{slice_column['fragment']}`; "
            f"record schema `{slice_column['schema']}`. Trusted bridge: "
            f"{slice_column['trusted_bridge']}.",
            "",
            f"{summary.get('total', 0)} pinned case(s): "
            f"{summary.get('positive', 0)} positive, "
            f"{summary.get('negative', 0)} negative; divergences: "
            f"{summary.get('divergences') or 'none'}.",
            "",
            "| case | expect | verdict | check ms | certificate bytes | admissions |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
        for case in slice_column.get("cases", []):
            certificate_bytes = case.get("certificate_bytes")
            lines.append(
                f"| `{case['case']}` | {case['expect']} | {case['verdict']} "
                f"| {case['elapsed_ms']} "
                f"| {certificate_bytes if certificate_bytes is not None else '—'} "
                f"| {len(case.get('admissions', []))} |"
            )
        lines += [
            "",
            "Imported admission inventory (union over cases): "
            + ", ".join(
                f"`{a}`" for a in slice_column.get("imported_admissions", [])
            )
            + ".",
            "",
        ]
    else:
        lines += [
            "The matching-logic slice checker column is unmeasured: "
            f"{slice_column.get('note', 'tools/matching-logic-slice not landed')}.",
            "",
        ]
    lines += [
        "The comparison-metrics aggregation column stays pending on "
        "tools/matching-logic-metrics landing.",
        "",
    ]
    encoding = record["candidate"].get("sort_encoding", {})
    if encoding.get("status") == "measured":
        surface = encoding["translation_surface"]
        lines += [
            "### Sort encoding (measured translation leg)",
            "",
            f"`{encoding['tool']}` — {surface['lines']} physical lines "
            f"({surface['nonblank_lines']} nonblank) across {surface['files']} "
            "file(s). Every emitted clause is an axiom admission on the "
            f"candidate side. Fragment: `{encoding['fragment']}`; record "
            f"schema `{encoding['schema']}` at semantics version "
            f"`{(encoding['cases'][0] if encoding['cases'] else {}).get('semantics_version')}`.",
            "",
            f"{encoding['clean_cases']} case(s) check clean; "
            f"{encoding['violation_pinning_cases']} pin an expected "
            "violation.",
            "",
            "| case | exit | admissions | diagnostics |",
            "| --- | --- | --- | --- |",
        ]
        for case in encoding["cases"]:
            if case.get("timed_out"):
                lines.append(
                    f"| `{case['case']}` | timed out | — | — |"
                )
                continue
            rules_seen = ", ".join(
                f"`{row.get('rule')}`" for row in case.get("diagnostics", [])
            ) or "—"
            lines.append(
                f"| `{case['case']}` | {case['exit_code']} "
                f"| {len(case.get('admissions', []))} | {rules_seen} |"
            )
    else:
        lines += [
            "### Sort encoding (absent)",
            "",
            f"`{encoding.get('tool')}` is not present in this checkout; the "
            "candidate translation column is unmeasured.",
        ]
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--omega",
        type=Path,
        default=Path("target/debug/omega"),
        help="path to the omega binary used for the pinned-case runs",
    )
    parser.add_argument(
        "--record",
        type=Path,
        default=HERE / "record.json",
        help="output path for the machine-readable comparison record",
    )
    parser.add_argument(
        "--report",
        type=Path,
        default=None,
        help="output path for the rendered markdown report",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=300,
        help="per-case omega --check timeout in seconds",
    )
    parser.add_argument(
        "--include-heavy",
        action="store_true",
        help="also run pinned pairs whose case dirs carry a build.omg "
        "(each checks the full std package closure; minutes per case)",
    )
    parser.add_argument(
        "--skip-run",
        action="store_true",
        help="measure static surfaces only; do not execute the pinned cases",
    )
    parser.add_argument(
        "--native-sidecar",
        type=Path,
        default=None,
        help="path to a produced .proof/.psi.proof artifact to record its size",
    )
    options = parser.parse_args()

    root = repo_root()
    surfaces = measure_surfaces(root)
    inventory = rule_inventory(root)
    versions = version_context(root, options.omega)

    certificate = "pending: no --native-sidecar supplied"
    if options.native_sidecar:
        sidecar = options.native_sidecar
        certificate = {
            "path": str(sidecar),
            "bytes": sidecar.stat().st_size,
            "note": "produced artifact; see compile_report companions for "
            "`.psi.proof` (semantic) and `.proof` (native) sidecars",
        }

    cases = [] if options.skip_run else run_pinned_cases(
        options.omega, root, options.timeout, options.include_heavy
    )
    encoding = sort_encoding_cases(root)
    slice_column = candidate_slice(root)

    record = {
        "record": "matching-logic-slice-comparison/v1",
        "generated_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "checklist_source": "wiki/drafts/reference/matching_logic.md#possible-bounded-comparison",
        "versions": versions,
        "omega_route": {
            "surface_lines": surfaces,
            "rule_inventory": inventory,
            "translation_lines": 0,
            "certificate": certificate,
            "pinned_cases": cases,
        },
        "candidate": {
            "status": slice_column["status"],
            "slice": slice_column,
            "sort_encoding": encoding,
            "blocked_on": [
                "MATCHING-LOGIC-COMPARISON-METRICS (tools/matching-logic-metrics)",
            ]
            if slice_column["status"] == "measured"
            else [
                "MATCHING-LOGIC-BOUNDED-SLICE (tools/matching-logic-slice)",
                "MATCHING-LOGIC-COMPARISON-METRICS (tools/matching-logic-metrics)",
            ],
            "note": "the bounded slice's checker/theory/certificate columns "
            "and pinned-case verdicts are measured under `slice`; the "
            "typed-to-one-sorted translation leg is measured under "
            "sort_encoding — translation surface lines plus per-case clause "
            "admissions, diagnostics, and checklist provenance",
        },
    }

    options.record.parent.mkdir(parents=True, exist_ok=True)
    options.record.write_text(json.dumps(record, indent=2) + "\n")
    if options.report:
        options.report.parent.mkdir(parents=True, exist_ok=True)
        options.report.write_text(render_report(record))

    if cases:
        failed = [
            c["subject"]
            for c in cases
            if "accepted" in c["positive"]
            and not (
                c["positive"]["accepted"]
                and c["negative"]["rejected"]
                and c["negative"]["fragment_observed"]
            )
        ]
        if failed:
            print(f"pinned-case discipline failures: {failed}", file=sys.stderr)
            return 1
    print(f"record: {options.record}")
    if options.report:
        print(f"report: {options.report}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
