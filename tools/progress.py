#!/usr/bin/env python3
"""Measure how much of Omega the Rust reference compiler can compile and run.

The question this answers is "how close is the compiler to producing real
programs?", as numbers rather than as the stall point of one large sample. A
large sample stops at its first unsupported construct and reports one bit; a
corpus of small fixtures, each pinning one rule, reports a coverage fraction
per tier and per language surface.

Four measurements, each independent of the others:

  corpus    every `tests/omega/{pass,fail,run}` fixture, which roster runs it
            and on which tier, and which fixtures no roster runs at all;
  spec      every `## ` section of `wiki/spec/language/*.md`, the corpus groups
            that exercise it (from `tools/progress/spec_groups.tsv`), and the
            sections nothing exercises;
  pairs     the language surfaces every maintained sample combines, and whether
            any pass fixture combines the same pair -- "combinations that
            matter" means pairs that at least `--pair-floor` samples use;
  outcomes  per-fixture pass/fail from the collect-all suites, when their logs
            are supplied with `--pass-log`, `--fail-log` and `--samples-log`.

Run from the repository root; standard library only:

    python tools/progress.py
    python tools/progress.py --pass-log build/pass.log --samples-log build/samples.log --json out.json

Outcome logs come from the umbrella tests, which collect every failure rather
than stopping at the first:

    cargo test -p compiler --test canary_suite pass_canaries_compile -- --nocapture --test-threads=1
    cargo test -p compiler --test canary_suite fail_canaries_reject_with_expected_diagnostic_fragment -- --nocapture --test-threads=1
    cargo test -p compiler --test samples_compile -- --nocapture --test-threads=1

The surfaces in `SURFACES` are regular expressions over authored source. They
are deliberately coarse -- a keyword's presence, not a parse -- so a pair
count is evidence that a fixture spells both constructs, not that it exercises
their interaction. A finer instrument needs the compiler's own checked trees.
"""

from __future__ import annotations

import argparse
import itertools
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "tests" / "omega"
SAMPLES = ROOT / "samples"
SPEC = ROOT / "wiki" / "spec" / "language"
COMPILER_TESTS = ROOT / "omega-rust" / "omega" / "compiler" / "compiler" / "tests"
# The umbrella's tiers are literal arrays split across the umbrella test file
# and its roster file; both are read.
UMBRELLA_ROSTERS = (
    COMPILER_TESTS / "canary_suite.rs",
    COMPILER_TESTS / "fixture_rosters" / "canary_suite.rs",
)
SPEC_GROUPS = ROOT / "tools" / "progress" / "spec_groups.tsv"

# One regular expression per language surface. Multi-line mode; presence only.
SURFACES = {
    "machine": r"\bmachine\b",
    "state": r"\bstate\b",
    "transition": r"\btransition\b",
    "data": r"\bdata\b",
    "domain": r"\bdomain\b",
    "requires": r"\brequires\b",
    "ensures": r"\bensures\b",
    "trait": r"\btrait\b",
    "satisfies": r"\bsatisfies\b",
    "generic": r"<\s*[A-Z]\w*\s*(?::|>)",
    "borrow_mut": r"&mut\b",
    "borrow": r"&(?!mut)\w",
    "match": r"\bmatch\b",
    "array": r"\[\w+;\s*\d+\]",
    "slice": r"&\[",
    "float": r"\bf32\b|\bf64\b",
    "string_lit": r'"[^"]*"',
    "reaches": r"\breaches\b",
    "boundary": r"\bboundary\b",
    "drop": r"\bdrop\b",
    "atomic": r"\bAtomic\b|\batomic\b",
    "task": r"\bTask\b",
    "asm": r"\basm\b",
    "in_domain": r"\bin\s+[A-Z]\w*",
    "terminates": r"\bterminates\b",
    "case": r"\bcase\b",
    "use": r"^\s*use\s",
    "pub": r"\bpub\b",
    "const": r"\bconst\b",
    "nested_call": r"\.\w+\(",
    "self_receiver": r"&mut self|&self",
}

# Surfaces nearly every program spells; a pair with one of these says nothing.
STRUCTURAL = {
    "machine", "data", "self_receiver", "nested_call", "use", "string_lit",
    "borrow_mut", "generic", "transition", "state", "array", "reaches",
}

# Umbrella tiers in the order the pass suite runs them. A fixture on several
# rosters is counted at its highest tier.
TIERS = [
    ("windows_host", "WINDOWS_HOST_PASS_CANARIES"),
    ("rooted_target", "ROOTED_TARGET_BACKEND_PASS_CANARIES"),
    ("cross_target", "CROSS_TARGET_PASS_CANARIES"),
    ("active", "ACTIVE_PASS_CANARIES"),
    ("checked_only", "CHECKED_ONLY_PASS_CANARIES"),
]

FIXTURE = re.compile(r'"([a-z_0-9]+/[a-z_0-9]+)"')


# --------------------------------------------------------------------------
# corpus and rosters


def corpus_members(kind: str) -> list[str]:
    """`pass/` and `fail/` are `<group>/<fixture>/main.omg`; `run/` is one
    level flatter, `<fixture>/main.omg`, and is reported by fixture name."""
    root = CORPUS / kind
    if not root.is_dir():
        return []
    found = []
    for group in sorted(p for p in root.iterdir() if p.is_dir()):
        if (group / "main.omg").is_file():
            found.append(group.name)
            continue
        for fixture in sorted(p for p in group.iterdir() if p.is_dir()):
            if (fixture / "main.omg").is_file():
                found.append(f"{group.name}/{fixture.name}")
    return found


def roster_names(path: Path) -> set[str]:
    return set(FIXTURE.findall(path.read_text(encoding="utf-8")))


def const_block(text: str, name: str) -> str:
    """The source between `const NAME` and its closing `];`, or ''."""
    start = text.find(f"const {name}")
    if start < 0:
        return ""
    end = text.find("\n];", start)
    return text[start : end if end > 0 else len(text)]


def rostered_anywhere() -> set[str]:
    names: set[str] = set()
    for path in COMPILER_TESTS.rglob("*.rs"):
        names |= roster_names(path)
    return names


def umbrella_tiers() -> dict[str, str]:
    """fixture -> highest umbrella tier naming it. Each tier is one literal
    array; a fixture on several is counted once, at the first tier in TIERS."""
    texts = [p.read_text(encoding="utf-8") for p in UMBRELLA_ROSTERS if p.is_file()]
    tiers: dict[str, str] = {}
    for tier, const in TIERS:
        for text in texts:
            for member in FIXTURE.findall(const_block(text, const)):
                tiers.setdefault(member, tier)
    return tiers


def corpus_report() -> dict:
    pass_members = corpus_members("pass")
    fail_members = corpus_members("fail")
    run_members = corpus_members("run")
    anywhere = rostered_anywhere()
    tiers = umbrella_tiers()
    dark = [m for m in pass_members if m not in anywhere]
    by_tier: dict[str, int] = {}
    dedicated = 0
    for member in pass_members:
        if member in tiers:
            by_tier[tiers[member]] = by_tier.get(tiers[member], 0) + 1
        elif member in anywhere:
            dedicated += 1
    dark_by_group: dict[str, int] = {}
    for member in dark:
        group = member.split("/", 1)[0]
        dark_by_group[group] = dark_by_group.get(group, 0) + 1
    return {
        "pass": len(pass_members),
        "fail": len(fail_members),
        "run": len(run_members),
        "pass_groups": len({m.split("/", 1)[0] for m in pass_members}),
        "rostered": len(pass_members) - len(dark),
        "dark": len(dark),
        "dark_by_group": dict(sorted(dark_by_group.items(), key=lambda kv: -kv[1])),
        "dark_members": dark,
        "umbrella_tiers": by_tier,
        "dedicated_target_only": dedicated,
        "tier_of": tiers,
    }


# --------------------------------------------------------------------------
# surfaces, footprints, pairs


def footprint(directory: Path) -> set[str]:
    source = "".join(
        p.read_text(encoding="utf-8", errors="ignore")
        for p in sorted(directory.iterdir())
        if p.suffix == ".omg"
    )
    return {name for name, pattern in SURFACES.items() if re.search(pattern, source, re.M)}


def sample_dirs() -> list[Path]:
    found = []
    for base, dirs, files in os.walk(SAMPLES):
        if "main.omg" in files:
            found.append(Path(base))
            dirs[:] = []
            continue
        dirs[:] = [d for d in dirs if d != "build"]
    return sorted(found)


def pairs_report(pair_floor: int) -> dict:
    samples = [footprint(d) - STRUCTURAL for d in sample_dirs()]
    corpus = {
        member: footprint(CORPUS / "pass" / member) - STRUCTURAL
        for member in corpus_members("pass")
    }
    usage: dict[str, int] = {}
    for surfaces in samples:
        for name in surfaces:
            usage[name] = usage.get(name, 0) + 1
    counts: dict[tuple[str, str], int] = {}
    for surfaces in samples:
        for a, b in itertools.combinations(sorted(surfaces), 2):
            counts[(a, b)] = counts.get((a, b), 0) + 1
    rows = []
    for (a, b), n in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0])):
        covering = [m for m, s in corpus.items() if a in s and b in s]
        rows.append({
            "pair": f"{a}+{b}",
            "samples": n,
            "covering": len(covering),
            "example": covering[0] if covering else "",
        })
    matter = [r for r in rows if r["samples"] >= pair_floor]
    return {
        "samples": len(samples),
        "surface_usage": dict(sorted(usage.items(), key=lambda kv: -kv[1])),
        "pairs_total": len(rows),
        "pair_floor": pair_floor,
        "pairs_that_matter": len(matter),
        "matter_uncovered": [r for r in matter if r["covering"] == 0],
        "matter_thin": [r for r in matter if 0 < r["covering"] < 3],
        "rows": rows,
    }


# --------------------------------------------------------------------------
# spec coverage


def spec_sections() -> list[tuple[str, str]]:
    found = []
    for path in sorted(SPEC.glob("*.md")):
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.startswith("## "):
                found.append((path.stem, line[3:].strip()))
    return found


def spec_report(corpus: dict) -> dict:
    sections = spec_sections()
    group_sizes: dict[str, int] = {}
    for member in corpus_members("pass"):
        group = member.split("/", 1)[0]
        group_sizes[group] = group_sizes.get(group, 0) + 1
    mapping: dict[tuple[str, str], dict] = {}
    if SPEC_GROUPS.is_file():
        for line in SPEC_GROUPS.read_text(encoding="utf-8").splitlines():
            if not line.strip() or line.startswith("#") or line.startswith("spec_file\t"):
                continue
            parts = line.split("\t")
            if len(parts) < 5:
                continue
            spec_file, heading, klass, pass_groups, fail_groups = parts[:5]
            groups = [g.strip() for g in pass_groups.split(",") if g.strip() and g.strip() != "NONE"]
            mapping[(spec_file, heading)] = {
                "class": klass.strip(),
                "groups": groups,
                "members": sum(group_sizes.get(g, 0) for g in groups),
            }
    rows = []
    for spec_file, heading in sections:
        row = mapping.get((spec_file, heading), {"class": "?", "groups": [], "members": 0})
        rows.append({"spec": spec_file, "heading": heading, **row})
    by_class: dict[str, dict[str, int]] = {}
    for row in rows:
        bucket = by_class.setdefault(row["class"], {"sections": 0, "covered": 0})
        bucket["sections"] += 1
        if row["groups"]:
            bucket["covered"] += 1
    return {
        "sections": len(sections),
        "mapped": len(mapping),
        "covered": sum(1 for r in rows if r["groups"]),
        "uncovered": [f'{r["spec"]}: {r["heading"]}' for r in rows if not r["groups"]],
        "by_class": by_class,
        "rows": rows,
    }


# --------------------------------------------------------------------------
# outcomes from collect-all logs


# A failure header is `<tier> <path>:` where the path is `Path::display()`
# output, so its separator is the host's; the fixture is the last two
# components under `pass`.
# The active tier's header carries no prefix; every other tier names itself.
PASS_FAILURE = re.compile(
    r"^(?:(checked-only|cross-target \S+|rooted target \S+|windows-host) )?"
    r"\S*?pass[/\\]([a-z_0-9]+)[/\\]([a-z_0-9]+):$",
    re.M,
)
SAMPLE_FAILURE = re.compile(r"^([a-z]+/[a-z_0-9/]+): ", re.M)


def normalize_diagnostic(line: str) -> str:
    """Collapse a diagnostic to its family: names, numbers and paths vary per
    fixture, the sentence around them is the gap."""
    line = re.sub(r"`[^`]*`", "`_`", line)
    line = re.sub(r"\S*[/\\]\S*", "<path>", line)
    line = re.sub(r"\d+", "N", line)
    return line.strip()[:120]


def failure_families(text: str) -> list[dict]:
    """Every failure block, keyed by the family of its first diagnostic line."""
    families: dict[str, dict] = {}
    for match in PASS_FAILURE.finditer(text):
        fixture = f"{match.group(2)}/{match.group(3)}"
        tier = match.group(1).split()[0] if match.group(1) else "active"
        rest = text[match.end():]
        body = rest[: rest.find("\n\n")] if "\n\n" in rest else rest
        first = next((l for l in body.splitlines() if l.strip()), "")
        key = normalize_diagnostic(first)
        family = families.setdefault(key, {"family": key, "count": 0, "groups": {}, "fixtures": [], "tiers": {}})
        family["count"] += 1
        group = fixture.split("/", 1)[0]
        family["groups"][group] = family["groups"].get(group, 0) + 1
        family["tiers"][tier] = family["tiers"].get(tier, 0) + 1
        family["fixtures"].append(fixture)
    return sorted(families.values(), key=lambda f: -f["count"])


def parse_pass_log(path: Path, tier_of: dict[str, str]) -> dict:
    text = path.read_text(encoding="utf-8", errors="ignore")
    failed = {}
    for tier, group, fixture in PASS_FAILURE.findall(text):
        failed[f"{group}/{fixture}"] = tier.split()[0] if tier else "active"
    families = failure_families(text)
    summary = re.search(r"(\d+) pass canary\(ies\) failed to compile", text)
    # Under OMEGA_PASS_CANARY_REPORT_COUNTS=1 the umbrella prints how many
    # selected fixtures it elided because a dedicated exact-native owner
    # judges them instead. Those never compile here, so they are neither
    # passes nor failures of this run.
    coverage = {}
    line = re.search(r"pass-canary coverage: (.*)", text)
    if line:
        for key, value in re.findall(r"([a-z-]+)=(\d+)", line.group(1)):
            coverage[key] = int(value)
    per_tier: dict[str, dict[str, int]] = {}
    for member, tier in tier_of.items():
        bucket = per_tier.setdefault(tier, {"members": 0, "failed": 0})
        bucket["members"] += 1
        if member in failed:
            bucket["failed"] += 1
    return {
        "reported_failures": int(summary.group(1)) if summary else None,
        "failed_members": failed,
        "per_tier": per_tier,
        "families": families,
        "ran": "test result:" in text,
        "coverage": coverage,
    }


FAIL_TIERS = [
    ("cross_target", "CROSS_TARGET_FAIL_CANARIES"),
    ("active", "ACTIVE_FAIL_CANARIES"),
    ("checked_only", "CHECKED_ONLY_FAIL_CANARIES"),
]
FAIL_FAILURE = re.compile(r"^\S*?fail[/\\]([a-z_0-9]+)[/\\]([a-z_0-9]+):$", re.M)


def fail_tier_sizes() -> dict[str, int]:
    texts = [p.read_text(encoding="utf-8") for p in UMBRELLA_ROSTERS if p.is_file()]
    sizes = {}
    for tier, const in FAIL_TIERS:
        members: set[str] = set()
        for text in texts:
            members |= set(FIXTURE.findall(const_block(text, const)))
        sizes[tier] = len(members)
    return sizes


SUITE_VERDICT = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)\b", re.M)


def parse_owner_index(path: Path) -> dict[str, list[dict]]:
    """The umbrella's own owner index, written under
    OMEGA_PASS_CANARY_OWNER_INDEX: one row per (kind, canary, target, owner
    test, expected status). A fixture with a unique host-executed owner is
    elided from the umbrella and judged by that owner's test."""
    owners: dict[str, list[dict]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        kind, canary, target, test, status = (line.split("\t") + [""] * 5)[:5]
        owners.setdefault(canary, []).append(
            {"kind": kind, "target": target, "test": test, "expected_status": status}
        )
    return owners


def parse_suite_log(path: Path) -> dict[str, str]:
    """Per-test verdicts of one `cargo test -p compiler --test canary_suite`
    run, keyed by the module-qualified test name the log prints."""
    text = path.read_text(encoding="utf-8", errors="ignore")
    return {name: verdict for name, verdict in SUITE_VERDICT.findall(text)}


SUITE_FAILURE_BLOCK = re.compile(r"^---- (\S+) stdout ----\n(.*?)(?=\n\n|\n---- |\Z)", re.M | re.S)


def normalize_family(text: str) -> str:
    return re.sub(r"\d+", "N", re.sub(r"`[^`]*`", "`_`", text))[:90]


def parse_suite_failures(path: Path) -> dict[str, dict]:
    """Per failing test: the stage its assertion names ('compile' for a
    'should compile' assertion carrying diagnostics, 'check' for 'should
    check', 'run' for an exit-status or output mismatch) and the family of
    its first diagnostic, normalized like the umbrella's families."""
    text = path.read_text(encoding="utf-8", errors="ignore")
    failures = {}
    for name, body in SUITE_FAILURE_BLOCK.findall(text):
        line = next((l for l in body.splitlines() if l.strip() and not l.startswith(("thread '", "note:"))), "")
        if "should compile" in line or "Diagnostic {" in line:
            stage = "compile"
        elif "should check" in line:
            stage = "check"
        elif re.search(r"exit|status|stdout|output", line):
            stage = "run"
        else:
            stage = "other"
        message = re.search(r'message: "([^"]*)', line)
        failures[name] = {"stage": stage, "family": normalize_family(message.group(1) if message else line)}
    return failures


def owner_outcomes(owners: dict[str, list[dict]], verdicts: dict[str, str], failures: dict[str, dict] | None = None) -> dict:
    """Join each fixture's host-executed owners (rooted or direct) to the
    suite log. A fixture passes when every such owner test passed, fails when
    any failed, and has no verdict when none of its owners ran in the log.
    Target-only owners compile for another target and are not judged here.
    With the log's failure blocks, each failing fixture also carries the
    stage its owner failed at and the family of the first diagnostic."""
    failures = failures or {}
    passing, failing, no_verdict = [], [], []
    stage_of, family_of = {}, {}
    for canary, rows in sorted(owners.items()):
        tests = [row["test"] for row in rows if row["kind"] in ("rooted", "direct")]
        judged = [(test, verdicts.get(test)) for test in tests if verdicts.get(test) is not None]
        if not judged:
            no_verdict.append(canary)
        elif all(v == "ok" for _, v in judged):
            passing.append(canary)
        else:
            failing.append(canary)
            failed = next(test for test, v in judged if v != "ok")
            detail = failures.get(failed, {"stage": "other", "family": ""})
            stage_of[canary] = detail["stage"]
            family_of[canary] = detail["family"]
    families: dict[str, int] = {}
    for family in family_of.values():
        families[family] = families.get(family, 0) + 1
    return {
        "passing": passing,
        "failing": failing,
        "no_verdict": no_verdict,
        "stage_of": stage_of,
        "families": sorted(families.items(), key=lambda kv: -kv[1]),
        "elided": sorted(canary for canary, rows in owners.items()
                         if sum(1 for row in rows if row["kind"] in ("rooted", "direct")) == 1),
    }


def merge_owner_verdicts(pass_outcome: dict, owner_outcome: dict, tier_of: dict[str, str]) -> None:
    """Count a fixture whose dedicated owner failed as a failure of its tier;
    the umbrella never compiled it, so its own log could not."""
    for member in owner_outcome["failing"]:
        if member in pass_outcome["failed_members"]:
            continue
        pass_outcome["failed_members"][member] = "dedicated-owner"
        tier = tier_of.get(member)
        if tier in pass_outcome["per_tier"]:
            pass_outcome["per_tier"][tier]["failed"] += 1
    pass_outcome["owners"] = {
        "passing": len(owner_outcome["passing"]),
        "failing": len(owner_outcome["failing"]),
        "no_verdict": len(owner_outcome["no_verdict"]),
        "elided": len(owner_outcome.get("elided", [])),
        "runs": set(owner_outcome["passing"]),
        "stage_of": owner_outcome.get("stage_of", {}),
        "families": owner_outcome.get("families", []),
        "unjudged": set(owner_outcome["no_verdict"]) & set(owner_outcome.get("elided", [])),
    }


def parse_fail_log(path: Path) -> dict:
    """The fail suite passes when every rostered fail fixture rejects with its
    expected diagnostic fragment; a failure names the fixture that did not."""
    text = path.read_text(encoding="utf-8", errors="ignore")
    failed = [f"{g}/{f}" for g, f in FAIL_FAILURE.findall(text)]
    return {
        "tier_sizes": fail_tier_sizes(),
        "failed_members": failed,
        "green": bool(re.search(r"test result: ok\.", text)),
        "ran": "test result:" in text,
    }


def parse_samples_log(path: Path) -> dict:
    text = path.read_text(encoding="utf-8", errors="ignore")
    checked = re.search(r"(\d+) of (\d+) samples failed to reach checked trees", text)
    results = re.findall(r"^test (\S+) \.\.\. (ok|FAILED)$", text, re.M)
    return {
        "checked_failed": int(checked.group(1)) if checked else 0,
        "checked_total": int(checked.group(2)) if checked else None,
        "tests": {name: ok for name, ok in results},
        "cohort_tests_failed": [n for n, ok in results if ok == "FAILED"],
        "ran": "test result:" in text,
    }


# --------------------------------------------------------------------------
# report


def print_report(report: dict) -> None:
    c = report["corpus"]
    print("== corpus ==")
    print(f"pass {c['pass']} in {c['pass_groups']} groups, fail {c['fail']}, run {c['run']}")
    print(f"rostered {c['rostered']}, dark {c['dark']}  (dark = no roster runs it)")
    if c["dark_by_group"]:
        print("  dark by group: " + ", ".join(f"{g} {n}" for g, n in c["dark_by_group"].items()))
    print("  umbrella tiers: " + ", ".join(f"{t} {n}" for t, n in c["umbrella_tiers"].items()) + f", dedicated-target {c['dedicated_target_only']}")

    s = report["spec"]
    print("\n== spec ==")
    print(f"{s['sections']} sections; {s['mapped']} mapped; {s['covered']} exercised by some pass group; {len(s['uncovered'])} exercised by none")
    for klass, b in sorted(s["by_class"].items()):
        print(f"  {klass:9s} {b['covered']}/{b['sections']}")
    for row in s["uncovered"]:
        print(f"  GAP  {row}")

    p = report["pairs"]
    print("\n== pairs ==")
    print(f"{p['samples']} samples; {p['pairs_total']} distinct surface pairs; {p['pairs_that_matter']} used by >= {p['pair_floor']} samples")
    print(f"  uncovered by any pass fixture: {len(p['matter_uncovered'])}; thin (1-2 fixtures): {len(p['matter_thin'])}")
    for row in p["matter_uncovered"]:
        print(f"  UNCOVERED  {row['pair']}  ({row['samples']} samples)")

    o = report.get("outcomes")
    if o:
        print("\n== outcomes ==")
        if "pass" in o:
            po = o["pass"]
            print(f"pass suite: {po['reported_failures']} failures reported")
            for tier, b in po["per_tier"].items():
                ok = b["members"] - b["failed"]
                print(f"  {tier:14s} {ok}/{b['members']} pass")
            print("  failure families (first diagnostic, count, groups):")
            for fam in po["families"][:20]:
                groups = ", ".join(f"{g} {n}" for g, n in sorted(fam["groups"].items(), key=lambda kv: -kv[1])[:4])
                print(f"    {fam['count']:4d}  {fam['family']}")
                print(f"          {groups}")
        if "fail" in o:
            fo = o["fail"]
            total = sum(fo["tier_sizes"].values())
            print(f"fail suite: {total - len(fo['failed_members'])}/{total} reject with their expected diagnostic"
                  + ("" if fo["green"] else f"; {len(fo['failed_members'])} did not"))
        if "samples" in o:
            so = o["samples"]
            if so["checked_total"]:
                print(f"samples: {so['checked_total'] - so['checked_failed']}/{so['checked_total']} reach checked trees")
            if so["cohort_tests_failed"]:
                print(f"  cohort tests failed: {', '.join(so['cohort_tests_failed'])}")

    if "levels" in report:
        print("\n== spec sections by the strongest verified level ==")
        print("  class      runs  compiles  checks  none  unexercised")
        for klass, tl in sorted(report["levels"]["tally"].items()):
            print(f"  {klass:9s} {tl.get('runs', 0):5d} {tl.get('compiles', 0):9d} {tl.get('checks', 0):7d} {tl.get('none', 0):5d} {tl.get('unexercised', 0):12d}")
        families = (report.get("outcomes", {}).get("pass", {}).get("owners") or {}).get("families") or []
        if families:
            print("  owner failures by family (first diagnostic, count):")
            for family, count in families[:8]:
                print(f"    {count:4d}  {family}")
    sec = report.get("sections")
    if sec:
        print("\n== spec sections by outcome ==")
        print("  class      native  checked  gap  unexercised")
        for klass in ("core", "typical", "advanced", "meta"):
            t = sec["tally"].get(klass, {})
            print(f"  {klass:9s} {t.get('native', 0):6d} {t.get('checked', 0):8d} {t.get('gap', 0):4d} {t.get('unexercised', 0):12d}")
        for row in sec["rows"]:
            if row["status"] in ("gap", "checked") and row["class"] in ("core", "typical"):
                print(f"  {row['status'].upper():7s} {row['class']:8s} {row['spec']}: {row['heading']}  ({row['native_passing']}/{row['native_members']} native)")

    h = report.get("headline")
    if h:
        print("\n== headline ==")
        for key, value in h.items():
            print(f"  {key}: {value}")


NATIVE_TIERS = {"active", "rooted_target", "cross_target", "windows_host"}


LEVELS = ("runs", "compiles", "checks", "fails", "unmeasured")


def fixture_levels(report: dict) -> dict[str, str]:
    """The strongest verified predicate per rostered pass fixture. `runs`: a
    dedicated owner compiled it on the rooted native route, executed it on
    the host and saw the expected exit. `compiles`: the umbrella produced a
    native artifact for it, never executed. `checks`: it passes checked
    semantics only. `fails`: some run refused it. `unmeasured`: elided by the
    umbrella with no owner verdict in the suite log, or unrostered."""
    tier_of = report["corpus"]["tier_of"]
    pass_outcome = report.get("outcomes", {}).get("pass", {})
    failed = pass_outcome.get("failed_members", {})
    owners = pass_outcome.get("owners") or {}
    runs = owners.get("runs", set())
    unjudged = owners.get("unjudged", set())
    levels = {}
    for member in corpus_members("pass"):
        tier = tier_of.get(member)
        if member in runs:
            levels[member] = "runs"
        elif member in failed:
            levels[member] = "fails"
        elif tier is None or member in unjudged:
            levels[member] = "unmeasured"
        elif tier in NATIVE_TIERS:
            levels[member] = "compiles"
        else:
            levels[member] = "checks"
    return levels


def section_levels(report: dict, levels: dict[str, str]) -> dict:
    """Per spec section, the strongest level any fixture in its groups
    reached, and how many fixtures sit at each level."""
    by_group: dict[str, list[str]] = {}
    for member in corpus_members("pass"):
        by_group.setdefault(member.split("/", 1)[0], []).append(member)
    rows = []
    for row in report["spec"]["rows"]:
        counts = {level: 0 for level in LEVELS}
        for group in row["groups"]:
            for member in by_group.get(group, []):
                counts[levels[member]] += 1
        best = next((level for level in ("runs", "compiles", "checks") if counts[level]), "none" if row["groups"] else "unexercised")
        rows.append({**{k: v for k, v in row.items() if k != "groups"}, "best": best, **counts})
    tally: dict[str, dict[str, int]] = {}
    for row in rows:
        bucket = tally.setdefault(row["class"], {})
        bucket[row["best"]] = bucket.get(row["best"], 0) + 1
    return {"rows": rows, "tally": tally}


def section_outcomes(report: dict) -> dict:
    """Per spec section: how many of its groups' fixtures sit on a native tier
    and how many of those pass. A section is 'native' when at least one does,
    'checked' when its fixtures only ever check, and 'gap' when none pass."""
    tier_of = report["corpus"]["tier_of"]
    failed = report.get("outcomes", {}).get("pass", {}).get("failed_members", {})
    by_group: dict[str, list[str]] = {}
    for member in corpus_members("pass"):
        by_group.setdefault(member.split("/", 1)[0], []).append(member)
    rows = []
    for row in report["spec"]["rows"]:
        native = passing = checked_ok = 0
        for group in row["groups"]:
            for member in by_group.get(group, []):
                tier = tier_of.get(member)
                if tier in NATIVE_TIERS:
                    native += 1
                    if member not in failed:
                        passing += 1
                elif tier == "checked_only" and member not in failed:
                    checked_ok += 1
        status = "native" if passing else ("checked" if checked_ok else ("unexercised" if not row["groups"] else "gap"))
        rows.append({**row, "native_members": native, "native_passing": passing, "status": status})
    tally: dict[str, dict[str, int]] = {}
    for row in rows:
        bucket = tally.setdefault(row["class"], {})
        bucket[row["status"]] = bucket.get(row["status"], 0) + 1
    return {"rows": rows, "tally": tally}


def headline(report: dict) -> dict:
    """The numbers to quote. Each is a plain fraction with its denominator
    named, so a reader can disagree with the denominator rather than the
    arithmetic."""
    c, s, p = report["corpus"], report["spec"], report["pairs"]
    out = {}
    if "levels" in report:
        tally = report["levels"]["tally"]
        core_t = s["by_class"].get("core", {}).get("sections", 0)
        typ_t = s["by_class"].get("typical", {}).get("sections", 0)
        def at(best, classes):
            return sum(tally.get(k, {}).get(best, 0) for k in classes)
        ct = ("core", "typical")
        out["core+typical sections with a fixture that runs natively"] = f"{at('runs', ct)}/{core_t + typ_t}"
        out["core+typical sections whose best fixture only compiles"] = f"{at('compiles', ct)}/{core_t + typ_t}"
        out["core+typical sections whose best fixture only checks"] = f"{at('checks', ct)}/{core_t + typ_t}"
        out["core+typical sections with no verified fixture"] = f"{at('none', ct)}/{core_t + typ_t}"
        every = tuple(tally)
        out["all sections: runs / compiles / checks / none"] = f"{at('runs', every)} / {at('compiles', every)} / {at('checks', every)} / {at('none', every) + at('unexercised', every)} of {s['sections']}"
        # Depth under core+typical groups: distinct fixtures per level. A
        # fixture in a group mapped to several sections is counted once.
        groups = set()
        for row in report["sections"]["rows"]:
            if row["class"] in ("core", "typical"):
                groups |= set(row["groups"])
        levels = report["fixture_levels"]
        under = [m for m in corpus_members("pass") if m.split("/", 1)[0] in groups]
        counts = {level: sum(1 for m in under if levels[m] == level) for level in LEVELS}
        out["core+typical fixtures: runs / compiles / checks / fails / unmeasured"] = " / ".join(str(counts[l]) for l in LEVELS) + f" of {len(under)}"
        every_counts = {level: sum(1 for m in levels.values() if m == level) for level in LEVELS}
        out["all pass fixtures: runs / compiles / checks / fails / unmeasured"] = " / ".join(str(every_counts[l]) for l in LEVELS) + f" of {len(levels)}"
    out["spec sections exercised"] = f"{s['covered']}/{s['sections']}"
    core = s["by_class"].get("core", {"covered": 0, "sections": 0})
    typical = s["by_class"].get("typical", {"covered": 0, "sections": 0})
    out["core+typical sections exercised"] = f"{core['covered'] + typical['covered']}/{core['sections'] + typical['sections']}"
    out["pairs that matter, covered"] = f"{p['pairs_that_matter'] - len(p['matter_uncovered'])}/{p['pairs_that_matter']}"
    out["pass fixtures some roster runs"] = f"{c['rostered']}/{c['pass']}"
    o = report.get("outcomes", {})
    if "pass" in o and o["pass"]["reported_failures"] is not None:
        measured = sum(b["members"] for b in o["pass"]["per_tier"].values())
        failed = sum(b["failed"] for b in o["pass"]["per_tier"].values())
        out["rostered pass fixtures that pass their tier"] = f"{measured - failed}/{measured}"
        coverage = o["pass"].get("coverage") or {}
        elided = sum(v for k, v in coverage.items() if k.endswith("-elided"))
        owners = o["pass"].get("owners")
        if coverage and not owners:
            out["fixtures the umbrella elided for a dedicated owner (not read here)"] = str(elided)
            out["umbrella-compiled fixtures that pass"] = f"{measured - elided - failed}/{measured - elided}"
        if owners:
            judged = owners["passing"] + owners["failing"]
            out["elided fixtures judged by their dedicated owner: runs"] = f"{owners['passing']}/{judged}"
            out["elided fixtures with no owner verdict in the suite log"] = str(len(owners.get("unjudged", ())))
            stages = {}
            for stage in owners.get("stage_of", {}).values():
                stages[stage] = stages.get(stage, 0) + 1
            if stages:
                out["owner failures by stage"] = ", ".join(f"{k} {v}" for k, v in sorted(stages.items(), key=lambda kv: -kv[1]))
        native = {t: b for t, b in o["pass"]["per_tier"].items() if t != "checked_only"}
        nm = sum(b["members"] for b in native.values())
        nf = sum(b["failed"] for b in native.values())
        out["fixtures that compile to a native artifact"] = f"{nm - nf}/{nm}"
    if "fail" in o:
        fo = o["fail"]
        total = sum(fo["tier_sizes"].values())
        out["fail fixtures rejecting as expected"] = f"{total - len(fo['failed_members'])}/{total}"
    if "samples" in o and o["samples"]["checked_total"]:
        so = o["samples"]
        out["samples reaching checked trees"] = f"{so['checked_total'] - so['checked_failed']}/{so['checked_total']}"
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--pass-log", type=Path, help="output of the collect-all pass canary suite")
    parser.add_argument("--fail-log", type=Path, help="output of the collect-all fail canary suite")
    parser.add_argument("--samples-log", type=Path, help="output of the samples harness")
    parser.add_argument("--owner-index", type=Path, help="owner index the umbrella writes under OMEGA_PASS_CANARY_OWNER_INDEX")
    parser.add_argument("--suite-log", type=Path, help="output of a full `cargo test -p compiler --test canary_suite` run, for the owners' verdicts")
    parser.add_argument("--pair-floor", type=int, default=5, help="samples a pair needs to 'matter' (default 5)")
    parser.add_argument("--json", type=Path, help="also write the full report as JSON")
    args = parser.parse_args()

    report = {"corpus": corpus_report()}
    report["spec"] = spec_report(report["corpus"])
    report["pairs"] = pairs_report(args.pair_floor)
    outcomes = {}
    if args.pass_log and args.pass_log.is_file():
        outcomes["pass"] = parse_pass_log(args.pass_log, report["corpus"]["tier_of"])
    if "pass" in outcomes and args.owner_index and args.owner_index.is_file() and args.suite_log and args.suite_log.is_file():
        verdicts = owner_outcomes(
            parse_owner_index(args.owner_index),
            parse_suite_log(args.suite_log),
            parse_suite_failures(args.suite_log),
        )
        merge_owner_verdicts(outcomes["pass"], verdicts, report["corpus"]["tier_of"])
    if args.fail_log and args.fail_log.is_file():
        outcomes["fail"] = parse_fail_log(args.fail_log)
    if args.samples_log and args.samples_log.is_file():
        outcomes["samples"] = parse_samples_log(args.samples_log)
    if outcomes:
        report["outcomes"] = outcomes
        if "pass" in outcomes:
            report["sections"] = section_outcomes(report)
            report["fixture_levels"] = fixture_levels(report)
            report["levels"] = section_levels(report, report["fixture_levels"])
    report["headline"] = headline(report)
    print_report(report)
    if args.json:
        slim = dict(report)
        slim["corpus"] = {k: v for k, v in report["corpus"].items() if k != "tier_of"}
        if "outcomes" in slim and "owners" in slim["outcomes"].get("pass", {}):
            owners = dict(slim["outcomes"]["pass"]["owners"])
            owners["runs"] = sorted(owners["runs"])
            owners["unjudged"] = sorted(owners["unjudged"])
            slim["outcomes"] = dict(slim["outcomes"])
            slim["outcomes"]["pass"] = {**slim["outcomes"]["pass"], "owners": owners}
        args.json.write_text(json.dumps(slim, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
