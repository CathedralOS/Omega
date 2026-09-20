#!/usr/bin/env python3
"""Comparison metrics record for the proof-checking route.

``wiki/drafts/matching_logic.md#possible-bounded-comparison`` requires that
any matching-logic comparison measure checker, translation, and theory size;
certificate size and checking time; and every imported rule, assumption, or
trusted bridge — over identical pinned positive and negative cases. This tool
measures the route that exists today (the bounded-entailment checker plus the
certificate route through the proof-admission kernel) and records the
matching-logic encoding column as pending; the compared encoding needs the
bounded slice under ``tools/matching-logic-slice``.

Axis → measured surface:

  checker      ``omega-rust/psi/semantics/proof-admission/src`` — the admission
               kernel that independently re-decides every certificate. This is
               the trusted computing base a different checker would replace.
  translation  ``omega-rust/psi/semantics/proof/src/checker/certificate*`` and
               ``proof/src/obligations*`` — the producer that encodes a checked
               obligation into ``ProofNode`` certificates and reconstructs the
               verifier's goal. A foreign encoding replaces exactly this layer.
  theory       the accepted rule inventory: ``ProofRule`` node rules,
               ``AcceptedProofRule`` families, ``PrimitiveJudgment`` values,
               ``ObligationClass`` obligations, ``ProofRuleFoundation``
               classes, and the ``integer_rules`` / ``mathematical_core``
               modules that carry the integer theory.
  trusted      the rest of ``omega-rust/psi/semantics/proof/src`` — the
               derivation that still decides every certificate-uncovered leg
               on its own say-so. That remainder is the trusted bridge.

Per-case rows run ``omega --check --offline --timings`` on each pinned case
and keep the median wall time plus the ``compile: sources -> requested
product`` phase, which contains all checking. Rejected cases receive only
the total-elapsed line — the CLI prints the per-phase table on success — so
``compile_ms`` is ``null`` there, not zero. Certificate node counts and
emitted-certificate bytes are captured automatically when a future
``OMEGA_PROOF_MEASUREMENTS`` stderr line appears (the PROOF-SEARCH-MEASUREMENT
surface); until then ``certificate`` reports ``unavailable`` rather than
omitting the axis.

Requires Python 3.9+. The ``measure`` command needs a built ``omega`` binary;
``--skip-cases`` runs the inventory axes alone. Usage from a checkout root:

    python3 tools/matching-logic-metrics/run_metrics.py measure \
        --omega target/debug/omega --out records/$(git rev-parse --short HEAD).json

    python3 tools/matching-logic-metrics/run_metrics.py validate \
        tools/matching-logic-metrics/records/*.json
"""

import argparse
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
import time


SCHEMA = "omega-matching-logic-comparison/1"
CASES_SCHEMA = "omega-matching-logic-pinned-cases/1"
TIMING_LINE = re.compile(r"^\s*([0-9]+(?:\.[0-9]+)?) ms  (.+)$")
COMPILE_PHASE = "compile: sources -> requested product"
MEASUREMENTS_LINE = "OMEGA_PROOF_MEASUREMENTS"

KERNEL_PATH = "omega-rust/psi/semantics/proof-admission/src"
TRANSLATION_PATHS = (
    "omega-rust/psi/semantics/proof/src/checker/certificate.rs",
    "omega-rust/psi/semantics/proof/src/checker/certificate",
    "omega-rust/psi/semantics/proof/src/obligations.rs",
    "omega-rust/psi/semantics/proof/src/obligations",
)
TRUSTED_PATH = "omega-rust/psi/semantics/proof/src"
PROOF_BUNDLE_PATH = (
    "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle"
)

# (file, enum name) inventories that together make up the accepted theory
# the comparison must reproduce: certificate node rules, the route classes
# the kernel reports, the obligations evidence binds to, and the classicality
# axes every imported rule carries.
RULE_INVENTORY = (
    (PROOF_BUNDLE_PATH + "/nodes.rs", "ProofRule"),
    (PROOF_BUNDLE_PATH + "/nodes.rs", "PrimitiveJudgment"),
    (PROOF_BUNDLE_PATH + "/admission.rs", "EvidenceRoute"),
    ("omega-rust/psi/semantics/proof-admission/src/proof.rs", "AcceptedProofRule"),
    ("omega-rust/psi/semantics/proof-admission/src/admission/evidence.rs", "ObligationClass"),
    ("omega-rust/psi/semantics/proof-admission/src/admission/evidence.rs", "AcceptedFactRoute"),
    ("omega-rust/psi/semantics/proof-admission/src/classicality.rs", "ProofRuleFoundation"),
)


def strip_comments(text):
    """Remove ``//`` line comments and ``#[...]`` attribute lines.

    Enum bodies contain no string literals, so line-level comment removal
    cannot eat real code; doc comments and attribute lines would otherwise
    skew brace depth and variant counts.
    """
    kept = []
    for line in text.splitlines():
        code = line.split("//", 1)[0]
        if code.lstrip().startswith("#["):
            continue
        kept.append(code)
    return "\n".join(kept)


def enum_variant_count(path, name):
    """Count the variant heads of ``enum <name>`` defined in ``path``.

    Returns ``None`` when the enum is absent so a renamed inventory is a loud
    measurement failure rather than a silent zero.
    """
    try:
        text = strip_comments(path.read_text(encoding="utf-8"))
    except OSError:
        return None
    match = re.search(r"\benum\s+" + re.escape(name) + r"\b[^{]*\{", text)
    if not match:
        return None
    depth = 1
    index = match.end()
    start = index
    while index < len(text) and depth:
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        index += 1
    body = text[start:index - 1]
    count = 0
    for line in body.splitlines():
        head = line.strip()
        if re.match(r"[A-Z][A-Za-z0-9_]*\s*[,({=]|^[A-Z][A-Za-z0-9_]*$", head):
            count += 1
    return count


def is_test_source(path):
    parts = path.parts
    name = path.name
    return (
        "tests" in parts
        or name in ("tests.rs", "test_support.rs")
        or name.startswith("test_")
        or name.endswith("_tests.rs")
    )


def source_inventory(root):
    """Files, non-test/test lines and bytes for one source tree or file."""
    root = Path(root)
    if root.is_file():
        candidates = [root]
    elif root.is_dir():
        candidates = sorted(root.rglob("*.rs"))
    else:
        return None
    files = lines = bytes_total = 0
    test_files = test_lines = test_bytes = 0
    for path in candidates:
        data = path.read_bytes()
        line_count = data.count(b"\n") + (
            0 if data.endswith(b"\n") or not data else 1
        )
        if is_test_source(path.relative_to(root.parent if root.is_file() else root)):
            test_files += 1
            test_lines += line_count
            test_bytes += len(data)
        else:
            files += 1
            lines += line_count
            bytes_total += len(data)
    return {
        "files": files,
        "lines": lines,
        "bytes": bytes_total,
        "test_files": test_files,
        "test_lines": test_lines,
        "test_bytes": test_bytes,
    }


def sum_inventories(parts):
    """Fold several inventory dicts (or None) into one."""
    total = {
        "files": 0, "lines": 0, "bytes": 0,
        "test_files": 0, "test_lines": 0, "test_bytes": 0,
    }
    missing = []
    for name, inventory in parts.items():
        if inventory is None:
            missing.append(name)
            continue
        for key in total:
            total[key] += inventory[key]
    if missing:
        total["missing"] = missing
    return total


def measure_axis(repo, paths):
    return sum_inventories(
        {entry: source_inventory(repo / entry) for entry in paths}
    )


def measure_theory(repo):
    rules = {}
    missing = []
    for relative, name in RULE_INVENTORY:
        count = enum_variant_count(repo / relative, name)
        key = name[0].lower() + name[1:]
        if count is None:
            missing.append(f"{name} ({relative})")
        else:
            rules[key] = count
    integer_dir = source_inventory(
        repo / "omega-rust/psi/semantics/proof-admission/src/integer_rules"
    )
    core_dir = source_inventory(
        repo / "omega-rust/psi/semantics/proof-admission/src/mathematical_core"
    )
    core_root = source_inventory(
        repo / "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs"
    )
    theory = {"rule_inventory": rules}
    theory["integer_rules"] = sum_inventories({
        "integer_rules.rs": source_inventory(
            repo / "omega-rust/psi/semantics/proof-admission/src/integer_rules.rs"),
        "integer_rules/": integer_dir,
    })
    theory["mathematical_core"] = sum_inventories({
        "mathematical_core.rs": core_root,
        "mathematical_core/": core_dir,
    })
    theory["predicate_denotation"] = sum_inventories({
        "predicate_denotation.rs": source_inventory(
            repo / "omega-rust/psi/semantics/proof-admission/src/predicate_denotation.rs"),
        "predicate_denotation/": source_inventory(
            repo / "omega-rust/psi/semantics/proof-admission/src/predicate_denotation"),
    })
    theory["imported_rule_forms"] = {
        "SemanticAxiom": "ProofRule::SemanticAxiom cites one proposition from the "
                         "obligation's verifier-reconstructed axiom roster by index",
        "Assumption": "ProofRule::Assumption cites one supplied premise by index",
        "trusted_bridge": "obligations no certificate shape covers are decided by the "
                          "trusted derivation in proof/src/checker (see certificate.rs)",
    }
    if missing:
        theory["missing"] = missing
    return theory


def detect_omega(repo, supplied):
    if supplied:
        candidate = Path(supplied)
        return candidate if candidate.is_file() else None
    for flavor in ("debug", "release"):
        for name in ("omega", "omega.exe"):
            candidate = repo / "target" / flavor / name
            if candidate.is_file():
                return candidate
    return None


def parse_timings(output):
    stages = {}
    total = None
    measurements = []
    for line in output.splitlines():
        timing = TIMING_LINE.match(line)
        if timing:
            label = timing.group(2).strip()
            stages[label] = float(timing.group(1))
            if label == "total elapsed":
                total = float(timing.group(1))
        if MEASUREMENTS_LINE in line:
            measurements.append(line.strip())
    return stages, total, measurements


def run_check(omega, root):
    """One ``omega --check`` run: (exit code, wall ms, stage ms, output lines)."""
    argv = [str(omega), "--check", "--offline", "--timings", str(root)]
    started = time.monotonic()
    completed = subprocess.run(
        argv, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        check=False, text=True,
    )
    wall_ms = (time.monotonic() - started) * 1000.0
    stages, total, measurements = parse_timings(completed.stdout or "")
    return {
        "exit_code": completed.returncode,
        "wall_ms": wall_ms,
        "compile_ms": stages.get(COMPILE_PHASE),
        "total_elapsed_ms": total,
        "stages": stages,
        "proof_measurements": measurements,
    }


def case_source_bytes(case_dir):
    root = case_dir / "main.omg"
    try:
        return root.stat().st_size
    except OSError:
        return None


def measure_case(omega, repo, path, polarity, repetitions):
    case_dir = repo / path
    root = case_dir / "main.omg"
    row = {
        "path": path,
        "polarity": polarity,
        "source_bytes": case_source_bytes(case_dir),
    }
    if not root.is_file():
        row["status"] = "missing-case"
        return row, False
    runs = [run_check(omega, root) for _ in range(repetitions)]
    exit_codes = {run["exit_code"] for run in runs}
    accepted = all(code == 0 for code in exit_codes)
    expected_accept = polarity == "accept"
    row["runs"] = [
        {
            "exit_code": run["exit_code"],
            "total_elapsed_ms": run["total_elapsed_ms"],
            "compile_ms": run["compile_ms"],
            "wall_ms": run["wall_ms"],
        }
        for run in runs
    ]
    row["check_time_ms"] = {
        "total_elapsed": round(statistics.median(
            run["total_elapsed_ms"] for run in runs
            if run["total_elapsed_ms"] is not None
        ), 3) if any(run["total_elapsed_ms"] is not None for run in runs) else None,
        "compile": round(statistics.median(
            run["compile_ms"] for run in runs
            if run["compile_ms"] is not None
        ), 3) if any(run["compile_ms"] is not None for run in runs) else None,
        "wall": round(statistics.median(run["wall_ms"] for run in runs), 3),
    }
    row["proof_measurements"] = sorted({
        line for run in runs for line in run["proof_measurements"]
    })
    row["certificate"] = {
        "status": "unavailable",
        "reason": "per-obligation certificate nodes/bytes are not emitted on "
                  "this revision; the OMEGA_PROOF_MEASUREMENTS surface is "
                  "PROOF-SEARCH-MEASUREMENT's delivery and is captured "
                  "automatically once present",
    }
    row["outcome"] = "accept" if accepted else "reject"
    row["expected"] = polarity
    ok = accepted == expected_accept
    row["match"] = ok
    return row, ok


def measure(args):
    repo = Path(
        subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], cwd=args.repo, text=True
        ).strip()
    ).resolve()
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=repo, text=True
    ).strip()
    dirty = bool(
        subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=repo, text=True
        ).strip()
    )
    cases_path = Path(args.cases)
    cases_doc = json.loads(cases_path.read_text())
    if cases_doc.get("schema") != CASES_SCHEMA:
        raise SystemExit(f"pinned-cases schema mismatch in {cases_path}")

    checker = source_inventory(repo / KERNEL_PATH)
    translation = measure_axis(repo, TRANSLATION_PATHS)
    whole_proof = source_inventory(repo / TRUSTED_PATH)
    covered_translation = translation.copy()
    trusted = (
        {key: whole_proof[key] - covered_translation.get(key, 0)
         for key in ("files", "lines", "bytes", "test_files", "test_lines", "test_bytes")}
        if whole_proof is not None and "missing" not in covered_translation
        else None
    )

    record = {
        "schema": SCHEMA,
        "generated_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "revision": revision,
        "dirty": dirty,
        "toolchain": rustc_version(),
        "host": platform_host(),
        "route": {
            "current": {
                "checker": checker,
                "translation": translation,
                "trusted_derivation": trusted,
                "theory": measure_theory(repo),
            },
            "matching_logic_encoding": {
                "status": "pending",
                "note": "the compared encoding needs the bounded slice under "
                        "tools/matching-logic-slice (MATCHING-LOGIC-BOUNDED-SLICE); "
                        "every current-route axis is measured so the comparison "
                        "has its baseline",
            },
        },
        "cases": [],
        "evidence": {
            "logical_fragment": "bounded entailment + certificate admission",
            "rule_semantics_version": revision,
            "observation_profile": "host wall-clock and source inventory under "
                                   "`omega --check --offline --timings`; no "
                                   "product publication",
            "target_capsule": "check-only; certificates are decided but no "
                              "artifact/.proof companion is requested",
            "bridge_graph": "proof-admission kernel verifies emitted "
                            "certificates; uncovered shapes keep the trusted "
                            "derivation",
            "admissions": "per-obligation semantic-axiom and assumption rosters "
                          "cited by ProofRule::SemanticAxiom/Assumption index",
        },
    }

    if args.skip_cases:
        record["case_status"] = "skipped"
    else:
        omega = detect_omega(repo, args.omega)
        if omega is None:
            raise SystemExit(
                "no omega binary; build `cargo build -p omega` or pass --omega")
        record["omega"] = str(omega)
        mismatches = 0
        for pair in cases_doc["pairs"]:
            rows = [(pair["positive"], "accept")]
            rows.extend((negative, "reject") for negative in pair["negative"])
            for path, polarity in rows:
                row, ok = measure_case(omega, repo, path, polarity,
                                       args.repetitions)
                row["pair"] = pair["id"]
                row["subject"] = pair["subject"]
                record["cases"].append(row)
                mismatches += 0 if ok else 1
                status = "ok" if ok else "MISMATCH"
                sys.stderr.write(f"{status:>8} {path}\n")
        record["case_status"] = "measured"
        record["case_mismatches"] = mismatches

    text = json.dumps(record, indent=2) + "\n"
    if args.out:
        out = Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
        sys.stderr.write(f"recorded {out}\n")
    else:
        sys.stdout.write(text)
    return 0 if record.get("case_mismatches", 0) == 0 else 1


def platform_host():
    import platform
    return f"{platform.system()}-{platform.machine()}".lower()


def rustc_version():
    try:
        return subprocess.check_output(["rustc", "--version"], text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unavailable"


AXIS_KEYS = ("files", "lines", "bytes", "test_files", "test_lines", "test_bytes")
CASE_KEYS = ("path", "polarity", "outcome", "expected", "match", "runs",
             "check_time_ms", "pair", "subject")


def validate_inventory(inventory, name, errors):
    if inventory is None:
        errors.append(f"{name} inventory missing")
        return
    for key in AXIS_KEYS:
        if not isinstance(inventory.get(key), int) or inventory[key] < 0:
            errors.append(f"{name}.{key} must be a nonnegative integer")


def validate_record(path, errors):
    try:
        record = json.loads(Path(path).read_text())
    except (OSError, ValueError) as error:
        errors.append(f"{path}: unreadable record: {error}")
        return
    if record.get("schema") != SCHEMA:
        errors.append(f"{path}: schema is not {SCHEMA}")
        return
    route = record.get("route", {}).get("current", {})
    for axis in ("checker", "translation", "trusted_derivation"):
        validate_inventory(route.get(axis), axis, errors)
    rules = route.get("theory", {}).get("rule_inventory", {})
    for _, name in RULE_INVENTORY:
        key = name[0].lower() + name[1:]
        if not isinstance(rules.get(key), int) or rules[key] <= 0:
            errors.append(f"{path}: theory.rule_inventory.{key} must be positive")
    for case in record.get("cases", []):
        for key in CASE_KEYS:
            if key not in case:
                errors.append(f"{path}: case {case.get('path', '?')} missing {key}")
        if case.get("polarity") not in ("accept", "reject"):
            errors.append(f"{path}: case {case.get('path', '?')} polarity invalid")
    if record.get("case_status") == "measured" and not record.get("cases"):
        errors.append(f"{path}: measured record carries no cases")


def validate(args):
    errors = []
    for path in args.records:
        validate_record(path, errors)
    for error in errors:
        sys.stderr.write(error + "\n")
    if not errors:
        sys.stderr.write(f"{len(args.records)} record(s) conform to {SCHEMA}\n")
    return 1 if errors else 0


def report(args):
    records = [json.loads(Path(path).read_text()) for path in args.records]
    rows = []
    for record in records:
        route = record["route"]["current"]
        theory = route["theory"]["rule_inventory"]
        rows.append({
            "rev": record["revision"][:10],
            "utc": record["generated_utc"],
            "checker_lines": route["checker"]["lines"],
            "translation_lines": route["translation"]["lines"],
            "trusted_lines": (route["trusted_derivation"] or {}).get("lines"),
            "rules": sum(theory.values()),
            "cases": len(record.get("cases", [])),
            "mismatches": record.get("case_mismatches"),
        })
    print("| revision | measured_utc | checker_lines | translation_lines | "
          "trusted_lines | theory_rules | cases | mismatches |")
    print("|---|---|---|---|---|---|---|---|")
    for row in rows:
        print("| {rev} | {utc} | {checker_lines} | {translation_lines} | "
              "{trusted_lines} | {rules} | {cases} | {mismatches} |".format(**row))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    measure_cmd = sub.add_parser("measure", help="record the comparison metrics")
    measure_cmd.add_argument("--repo", default=".",
                             help="repository checkout (default: cwd)")
    measure_cmd.add_argument("--omega", default=None,
                             help="built omega binary (default: target/{debug,release})")
    measure_cmd.add_argument("--cases", default=str(
        Path(__file__).resolve().parent / "pinned_cases.json"))
    measure_cmd.add_argument("--repetitions", type=int, default=3)
    measure_cmd.add_argument("--skip-cases", action="store_true",
                             help="inventory only; no omega binary needed")
    measure_cmd.add_argument("--out", default=None,
                             help="record path (default: stdout)")
    measure_cmd.set_defaults(func=measure)
    validate_cmd = sub.add_parser("validate", help="check committed records")
    validate_cmd.add_argument("records", nargs="+")
    validate_cmd.set_defaults(func=validate)
    report_cmd = sub.add_parser("report", help="markdown table across records")
    report_cmd.add_argument("records", nargs="+")
    report_cmd.set_defaults(func=report)
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
