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
  outcomes  per-fixture verdicts from corpus outcome records (the golden
            `tests/omega/corpus_outcomes.json` by default, or a baseline file
            written by `tools/corpus_gate.py --baseline --record`).

Run from the repository root; standard library only:

    python tools/progress.py
    python tools/progress.py --outcomes build/corpus_baselines/<head>.json --json out.json

The corpus runner compiles every fixture through the check route, so a pass
fixture's strongest verified level is `checks`; native builds and runs are not
measured until the runner grows a native leg.

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
SPEC_GROUPS = ROOT / "tools" / "progress" / "spec_groups.tsv"
CORPUS_OUTCOMES = CORPUS / "corpus_outcomes.json"

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

# --------------------------------------------------------------------------
# corpus


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


def corpus_report() -> dict:
    """Every fixture is run by the corpus runner; nothing is unrostered."""
    pass_members = corpus_members("pass")
    return {
        "pass": len(pass_members),
        "fail": len(corpus_members("fail")),
        "run": len(corpus_members("run")),
        "pass_groups": len({m.split("/", 1)[0] for m in pass_members}),
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
# outcomes from corpus records


def normalize_diagnostic(line: str) -> str:
    """Collapse a diagnostic to its family: names, numbers and paths vary per
    fixture, the sentence around them is the gap."""
    line = re.sub(r"`[^`]*`", "`_`", line)
    line = re.sub(r"\S*[/\\]\S*", "<path>", line)
    line = re.sub(r"\d+", "N", line)
    return line.strip()[:120]


def parse_corpus_outcomes(path: Path) -> dict:
    """Per-tier verdicts from corpus runner records. A pass or run fixture
    passes when it checked; a fail fixture passes when it rejected and did not
    reject for the wrong reason (`expected_satisfied` is not false). Fixture
    names drop the tier prefix: `pass/group/name` becomes `group/name`."""
    records = json.loads(path.read_text(encoding="utf-8"))
    pass_failed: dict[str, str] = {}
    pass_members = 0
    fail_failed: list[str] = []
    fail_members = 0
    families: dict[str, dict] = {}
    for record in records:
        tier, _, member = record["fixture"].partition("/")
        if tier in ("pass", "run"):
            pass_members += tier == "pass"
            if tier == "pass" and record["status"] != "checked":
                first = (record.get("diagnostics") or [record["status"]])[0]
                key = normalize_diagnostic(first)
                pass_failed[member] = key
                family = families.setdefault(key, {"family": key, "count": 0, "groups": {}, "fixtures": []})
                family["count"] += 1
                group = member.split("/", 1)[0]
                family["groups"][group] = family["groups"].get(group, 0) + 1
                family["fixtures"].append(member)
        elif tier == "fail":
            fail_members += 1
            if record["status"] != "rejected" or record.get("expected_satisfied") is False:
                fail_failed.append(member)
    return {
        "pass": {
            "members": pass_members,
            "failed_members": pass_failed,
            "families": sorted(families.values(), key=lambda f: -f["count"]),
        },
        "fail": {"members": fail_members, "failed_members": fail_failed},
    }


# --------------------------------------------------------------------------
# report


def print_report(report: dict) -> None:
    c = report["corpus"]
    print("== corpus ==")
    print(f"pass {c['pass']} in {c['pass_groups']} groups, fail {c['fail']}, run {c['run']}")

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
        po, fo = o["pass"], o["fail"]
        print(f"pass: {po['members'] - len(po['failed_members'])}/{po['members']} check")
        print("  failure families (first diagnostic, count, groups):")
        for fam in po["families"][:20]:
            groups = ", ".join(f"{g} {n}" for g, n in sorted(fam["groups"].items(), key=lambda kv: -kv[1])[:4])
            print(f"    {fam['count']:4d}  {fam['family']}")
            print(f"          {groups}")
        print(f"fail: {fo['members'] - len(fo['failed_members'])}/{fo['members']} reject as expected")

    sec = report.get("sections")
    if sec:
        print("\n== spec sections by outcome ==")
        print("  class      checked  gap  unexercised")
        for klass in ("core", "typical", "advanced", "meta"):
            t = sec["tally"].get(klass, {})
            print(f"  {klass:9s} {t.get('checked', 0):8d} {t.get('gap', 0):4d} {t.get('unexercised', 0):12d}")
        for row in sec["rows"]:
            if row["status"] == "gap" and row["class"] in ("core", "typical"):
                print(f"  GAP     {row['class']:8s} {row['spec']}: {row['heading']}")

    h = report.get("headline")
    if h:
        print("\n== headline ==")
        for key, value in h.items():
            print(f"  {key}: {value}")


def section_outcomes(report: dict) -> dict:
    """Per spec section: 'checked' when at least one fixture in its groups
    checks, 'gap' when none does, 'unexercised' when it names no group. The
    corpus runner does not build natively, so no section is reported as
    native."""
    failed = report["outcomes"]["pass"]["failed_members"]
    by_group: dict[str, list[str]] = {}
    for member in corpus_members("pass"):
        by_group.setdefault(member.split("/", 1)[0], []).append(member)
    rows = []
    for row in report["spec"]["rows"]:
        members = [m for g in row["groups"] for m in by_group.get(g, [])]
        passing = sum(1 for m in members if m not in failed)
        status = "unexercised" if not row["groups"] else ("checked" if passing else "gap")
        rows.append({**row, "members": len(members), "passing": passing, "status": status})
    tally: dict[str, dict[str, int]] = {}
    for row in rows:
        bucket = tally.setdefault(row["class"], {})
        bucket[row["status"]] = bucket.get(row["status"], 0) + 1
    return {"rows": rows, "tally": tally}


def headline(report: dict) -> dict:
    """The numbers to quote. Each is a plain fraction with its denominator
    named, so a reader can disagree with the denominator rather than the
    arithmetic."""
    s, p = report["spec"], report["pairs"]
    out = {}
    core = s["by_class"].get("core", {"covered": 0, "sections": 0})
    typical = s["by_class"].get("typical", {"covered": 0, "sections": 0})
    ct_total = core["sections"] + typical["sections"]
    out["spec sections exercised"] = f"{s['covered']}/{s['sections']}"
    out["core+typical sections exercised"] = f"{core['covered'] + typical['covered']}/{ct_total}"
    out["pairs that matter, covered"] = f"{p['pairs_that_matter'] - len(p['matter_uncovered'])}/{p['pairs_that_matter']}"
    if "sections" in report:
        tally = report["sections"]["tally"]
        checked = sum(tally.get(k, {}).get("checked", 0) for k in ("core", "typical"))
        out["core+typical sections with a fixture that checks"] = f"{checked}/{ct_total}"
    o = report.get("outcomes")
    if o:
        po, fo = o["pass"], o["fail"]
        out["pass fixtures that check"] = f"{po['members'] - len(po['failed_members'])}/{po['members']}"
        out["fail fixtures rejecting as expected"] = f"{fo['members'] - len(fo['failed_members'])}/{fo['members']}"
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--outcomes", type=Path, default=CORPUS_OUTCOMES,
                        help="corpus outcome records (default: the checked-in golden)")
    parser.add_argument("--pair-floor", type=int, default=5, help="samples a pair needs to 'matter' (default 5)")
    parser.add_argument("--json", type=Path, help="also write the full report as JSON")
    args = parser.parse_args()

    report = {"corpus": corpus_report()}
    report["spec"] = spec_report(report["corpus"])
    report["pairs"] = pairs_report(args.pair_floor)
    if args.outcomes and args.outcomes.is_file():
        report["outcomes"] = parse_corpus_outcomes(args.outcomes)
        report["sections"] = section_outcomes(report)
    report["headline"] = headline(report)
    print_report(report)
    if args.json:
        args.json.write_text(json.dumps(report, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
