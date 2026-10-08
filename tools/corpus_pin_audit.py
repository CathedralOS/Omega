#!/usr/bin/env python3
"""Advisory audit of corpus pin strength in tests/omega/corpus_outcomes.txt.

A fail fixture pins its rule only when expected.txt's fragments are specific
enough that matching them means the pinned rule fired — not merely that the
compiler rejected for some reason. Two reports surface the weak spots:

- weak pins: fragments that also appear in the diagnostics of many OTHER
  fail fixtures. A pin whose fragment is ubiquitous ("cannot prove",
  "requires") cannot distinguish "rejected for the pinned rule" from
  "rejected for any reason" at --record time.
- cross-rule hits: a fixture carrying a diagnostic that matches none of its
  own fragments but does match another fixture's pinned fragment — it trips
  a rule a sibling owns, violating the corpus's pin-one-rule convention.

Advisory only: prints both reports, never gates, and always exits 0.

    tools/corpus_pin_audit.py [--threshold 25] [--minimum-length 25]
"""
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_GOLDEN = ROOT / "tests" / "omega" / "corpus_outcomes.txt"


def golden_rows(text):
    """Parse the golden: `name -> (status, [diagnostics])`. Tab-indented
    lines are diagnostics of the preceding row; `#` lines are comments."""
    rows = {}
    current = None
    for line in text.splitlines():
        if line.startswith("#") or not line.strip():
            continue
        if line.startswith("\t"):
            if current is not None:
                rows[current][1].append(line.strip())
            continue
        fields = line.split()
        current = fields[0]
        rows[current] = (fields[1] if len(fields) > 1 else "", [])
    return rows


def expected_fragments(root, name):
    """The pinned fragments of tests/omega/<name>/expected.txt; empty list
    when the fixture carries no expectation file."""
    path = root / "tests" / "omega" / name / "expected.txt"
    if not path.is_file():
        return []
    return [line.strip() for line in path.read_text(
        encoding="utf-8").splitlines() if line.strip()]


def weak_pins(rows, fragments, threshold):
    """Fragments shared with many other fail fixtures: for each fail
    fixture and each of its fragments, count OTHER fail fixtures carrying
    at least one diagnostic that contains it. Yield (count, name,
    fragment) at or above threshold, sorted by count descending."""
    findings = []
    names = [name for name in rows if name.startswith("fail/")]
    for name in names:
        for fragment in fragments.get(name, []):
            count = sum(
                1 for other in names if other != name
                and any(fragment in diagnostic
                        for diagnostic in rows[other][1]))
            if count >= threshold:
                findings.append((count, name, fragment))
    findings.sort(key=lambda finding: -finding[0])
    return findings


def cross_rule_hits(rows, fragments, minimum_length=25):
    """Diagnostics tripping a sibling's pinned rule: for each fail fixture,
    each own diagnostic matching none of its own fragments but containing
    some other fail fixture's fragment of at least minimum_length.
    Yields (name, other_name, diagnostic); one hit per diagnostic, first
    matching other fixture by sorted name."""
    hits = []
    for name in sorted(rows):
        if not name.startswith("fail/"):
            continue
        own = fragments.get(name, [])
        candidates = [
            (other, fragment)
            for other in sorted(rows)
            if other != name and other.startswith("fail/")
            for fragment in fragments.get(other, [])
            if len(fragment) >= minimum_length]
        for diagnostic in rows[name][1]:
            if any(fragment in diagnostic for fragment in own):
                continue
            for other, fragment in candidates:
                if fragment in diagnostic:
                    hits.append((name, other, diagnostic))
                    break
    return hits


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--golden", type=Path, default=DEFAULT_GOLDEN,
                        help="corpus golden file (default %(default)s)")
    parser.add_argument("--threshold", type=int, default=25,
                        help="minimum other-fixture matches that make a "
                             "fragment weak (default %(default)s)")
    parser.add_argument("--minimum-length", type=int, default=25,
                        help="minimum fragment length for cross-rule "
                             "matching (default %(default)s)")
    arguments = parser.parse_args()
    if not arguments.golden.is_file():
        print(f"corpus_pin_audit: {arguments.golden} not found")
        return 0
    rows = golden_rows(arguments.golden.read_text(encoding="utf-8"))
    fragments = {name: expected_fragments(ROOT, name)
                 for name in rows if name.startswith("fail/")}
    weak = weak_pins(rows, fragments, arguments.threshold)
    print(f"weak pins (fragment matches >= {arguments.threshold} "
          f"other fixtures): {len(weak)}")
    for count, name, fragment in weak:
        print(f'  {count:4d} {name}  "{fragment}"')
    hits = cross_rule_hits(rows, fragments, arguments.minimum_length)
    fixtures = {name for name, _, _ in hits}
    print(f"\nfixtures tripping another fixture's pinned rule: "
          f"{len(fixtures)}")
    for name, other, diagnostic in hits:
        print(f"  {name}\n      trips {other}\n      {diagnostic[:140]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
