#!/usr/bin/env python3
"""Fail a pull request at admission when it breaks a rule prose did not hold.

Each check replaces an AGENTS.md sentence that agents demonstrably broke after
it was written, so the rule stops the change instead of waiting for a later
cleanup:

- Source-text reads. Tests that `include_str!` a `.rs` or `.toml` file and
  assert on its spelling test nothing the program does. PR #1139 removed 165
  such asserts; within an hour of the AGENTS.md ban (#1159), three more PRs
  added 14 reads to product_boundary.rs. The per-file counts in
  `tools/source_text_reads.txt` may only fall.
- Stale goldens. A row in `tests/omega/corpus_outcomes.txt` states what a run
  observed. PR #958 rewrote 738 variant-payload fixtures and left their rows
  saying `checked` for programs that no longer checked; on 2026-11-01, 4,055
  rows had not been re-recorded since their fixture last changed. A change that
  edits or adds a recorded fixture must change that fixture's row, and a change
  that deletes one must delete it.
- Near-duplicate fact rows. 196 count and width variants of existing fact rows
  landed on 2026-10-19 and were deleted again two weeks later. A new folder
  under `samples/cli/facts` whose program closely matches an existing row's
  must name that row in its README, where the advance skill asks for the
  difference.
- Commit lanes. A squash merge takes the pull request title as its subject;
  178 of 1,017 subjects from 2026-10-25 to 2026-11-01 used no listed lane.
  Lanes are read from AGENTS.md's table, so the table stays the only list.
- Raw measurement dumps. `wiki/drafts/measurements` reached 5,828 raw files
  (1.1M lines, 125 MB) committed as evidence by performance PRs, beside 223
  summaries. A change may add summaries there, not data: only `.md` files of
  at most 300 lines. Raw data is linked by a pinned commit or kept in the claim
  note.

The diff checks compare `--base` (default: the merge base with origin/main)
with HEAD and read only Git and the tree. Nothing here builds or runs the
compiler; re-recording a golden row is the author's run.
"""

import argparse
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
RUST_ROOT = "omega-rust"
READS_BASELINE = "tools/source_text_reads.txt"
GOLDEN = "tests/omega/corpus_outcomes.txt"
FIXTURE_TIERS = ("tests/omega/pass/", "tests/omega/fail/", "tests/omega/run/")
FACTS_ROOT = "samples/cli/facts/"
MEASUREMENTS_ROOT = "wiki/drafts/measurements/"
MEASUREMENT_LINE_LIMIT = 300
NEAR_DUPLICATE = 0.8
SUBJECT_LIMIT = 85

# A read whose literal path names Rust or manifest source. `concat!` and `env!`
# arguments still end in the literal that names the file.
INCLUDE = re.compile(r"\binclude_(?:str|bytes)!\s*\(")
SOURCE_PATH = re.compile(r"\"[^\"\n]*\.(?:rs|toml)\"")
RUNTIME_READ = re.compile(
    r"\b(?:read_to_string|fs::read)\s*\([^;]{0,240}?\"[^\"]*\.(?:rs|toml)\"",
    re.DOTALL,
)


def git(*arguments, root=ROOT):
    result = subprocess.run(["git", "-C", str(root), *arguments],
                            capture_output=True, text=True, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"git {' '.join(arguments)}: {result.stderr.strip()}")
    return result.stdout


# Source-text reads ----------------------------------------------------------

def macro_arguments(text: str, start: int) -> str:
    depth, position = 1, start
    while depth and position < len(text):
        depth += {"(": 1, ")": -1}.get(text[position], 0)
        position += 1
    return text[start:position]


def count_source_reads(text: str) -> int:
    included = sum(1 for match in INCLUDE.finditer(text)
                   if SOURCE_PATH.search(macro_arguments(text, match.end())))
    return included + len(RUNTIME_READ.findall(text))


def source_read_counts(root: Path = ROOT) -> dict:
    counts = {}
    for path in sorted((root / RUST_ROOT).rglob("*.rs")):
        if "target" in path.relative_to(root).parts:
            continue
        count = count_source_reads(path.read_text(encoding="utf-8", errors="replace"))
        if count:
            counts[path.relative_to(root).as_posix()] = count
    return counts


def read_baseline(text: str) -> dict:
    baseline = {}
    for line in text.splitlines():
        if line.strip() and not line.startswith("#"):
            count, path = line.split(None, 1)
            baseline[path.strip()] = int(count)
    return baseline


def write_baseline(counts: dict) -> str:
    lines = [
        "# Source-text reads (include_str!/read_to_string of .rs or .toml) per file.",
        "# Written by `python tools/check_rules.py --update-source-reads`.",
        "# Counts may only fall: express the rule as a dependency in",
        "# tools/check_native_boundary.py, a type or visibility, or a test that runs code.",
    ]
    lines += [f"{count} {path}" for path, count in sorted(counts.items())]
    return "\n".join(lines) + "\n"


def source_read_violations(counts: dict, baseline: dict) -> list:
    violations = []
    for path in sorted(set(counts) | set(baseline)):
        now, allowed = counts.get(path, 0), baseline.get(path, 0)
        if now > allowed:
            violations.append(
                f"{path}: {now} source-text reads, baseline {allowed}. Tests check "
                "behavior or structure, never source text (AGENTS.md): state the "
                "rule as a dependency, a type, or a test that runs the code.")
        elif now < allowed:
            violations.append(
                f"{path}: {now} source-text reads, baseline {allowed}. Lower the "
                "baseline: python tools/check_rules.py --update-source-reads")
    return violations


# Golden freshness -----------------------------------------------------------

def golden_rows(text: str) -> set:
    return {line.split(None, 1)[0] for line in text.splitlines()
            if line.strip() and line[0] not in "#\t "}


def changed_golden_rows(diff: str) -> set:
    names = set()
    for line in diff.splitlines():
        if line[:1] in "+-" and not line.startswith(("+++", "---")):
            body = line[1:]
            if body.strip() and body[0] not in "#\t ":
                names.add(body.split(None, 1)[0])
    return names


def fixture_of(path: str, rows: set, fixture_dirs: set) -> str:
    """The fixture that owns `path`: the deepest recorded row whose directory
    holds it, else the shallowest directory that holds a main.omg."""
    if not path.startswith(FIXTURE_TIERS):
        return ""
    parts = path[len("tests/omega/"):].split("/")[:-1]
    prefixes = ["/".join(parts[:end]) for end in range(len(parts), 1, -1)]
    for prefix in prefixes:
        if prefix in rows:
            return prefix
    for prefix in reversed(prefixes):
        if prefix in fixture_dirs:
            return prefix
    return ""


def golden_violations(changes: list, base_rows: set, head_rows: set,
                      rewritten: set, fixture_dirs: set) -> list:
    """`changes` holds (status, path) pairs; `fixture_dirs` names the fixtures
    (tier/group/.../name) that hold a main.omg at HEAD; `rewritten` names the
    rows the change added, removed or rewrote."""
    touched = set()
    for _, path in changes:
        if not path.endswith("README.md"):
            touched.add(fixture_of(path, base_rows | head_rows, fixture_dirs))
    touched.discard("")
    stale = sorted(f for f in touched if f in fixture_dirs and f not in rewritten)
    orphaned = sorted(f for f in touched if f not in fixture_dirs and f in head_rows)
    violations = []
    if stale:
        violations.append(
            f"{len(stale)} fixture(s) changed without re-recording their row in "
            f"{GOLDEN}, so the row describes source that no longer exists. Run:\n"
            f"  python tools/corpus_gate.py --record --filter {','.join(stale)}")
    if orphaned:
        violations.append(
            f"{len(orphaned)} deleted fixture(s) keep a row in {GOLDEN}: "
            + ", ".join(orphaned))
    return violations


def fixture_directories(root: Path = ROOT) -> set:
    directories = set()
    for tier in FIXTURE_TIERS:
        base = root / tier
        if base.is_dir():
            for main in base.rglob("main.omg"):
                directories.add(main.parent.relative_to(root / "tests/omega").as_posix())
    return directories


# Near-duplicate fact rows ---------------------------------------------------

TOKEN = re.compile(r"[A-Za-z_][A-Za-z0-9_]*|\d[\d_]*[A-Za-z0-9]*|\S")
KEYWORDS = frozenset(
    "machine data case match let if else return transition requires ensures crash "
    "use const mut as true false loop for in impl trait dyn u8 u16 u32 u64 i8 i16 "
    "i32 i64 f32 f64 bool".split())


def program_shingles(text: str, width: int = 8) -> frozenset:
    """Token 8-grams with comments dropped and names and numbers erased, so two
    rows that differ only by a count, a width constant or their names match."""
    tokens = []
    for token in TOKEN.findall(re.sub(r"//[^\n]*", "", text)):
        if token[0].isdigit():
            tokens.append("0")
        elif token[0].isalpha() or token[0] == "_":
            tokens.append(token if token in KEYWORDS else "x")
        else:
            tokens.append(token)
    return frozenset(tuple(tokens[i:i + width])
                     for i in range(max(1, len(tokens) - width + 1)))


def similarity(left: frozenset, right: frozenset) -> float:
    return len(left & right) / max(1, len(left | right))


def representative_program(folder: Path):
    for candidate in (folder / "local" / "main.omg", folder / "main.omg"):
        if candidate.is_file():
            return candidate
    return None


def folder_shingles(folder: Path) -> frozenset:
    program = representative_program(folder)
    if program is None:
        return frozenset()
    return program_shingles(program.read_text(encoding="utf-8", errors="replace"))


def nearest_rows(new: dict, existing: dict) -> dict:
    """For each new folder's shingles, the most similar existing row."""
    nearest = {}
    for name, mine in new.items():
        if mine:
            nearest[name] = max(((similarity(mine, theirs), other)
                                 for other, theirs in existing.items() if theirs),
                                default=(0.0, ""))
    return nearest


def fact_row_violations(nearest: dict, readmes: dict) -> list:
    violations = []
    for name, (score, row) in sorted(nearest.items()):
        if score >= NEAR_DUPLICATE and row not in readmes.get(name, ""):
            violations.append(
                f"{FACTS_ROOT}{name}: its program matches {row} at {score:.2f}. Name "
                f"{row} in {name}/README.md and show a twin obligation its twins lack "
                "or an emitted form that differs (.agents/skills/advance/SKILL.md); "
                "otherwise it is a production of that row.")
    return violations


# Raw measurement dumps ------------------------------------------------------

def measurement_violations(changes: list, count_lines) -> list:
    """`changes` holds (status, path) pairs; `count_lines(path)` returns the
    line count of a file as it stands at HEAD. Only added files count: the
    summaries already in the folder may be edited."""
    violations = []
    for status, path in changes:
        if status != "A" or not path.startswith(MEASUREMENTS_ROOT):
            continue
        if not path.endswith(".md"):
            violations.append(
                f"{path}: raw run data stays out of git. Commit a .md summary "
                "and link the data by a pinned commit, or keep it in the claim note")
            continue
        lines = count_lines(path)
        if lines > MEASUREMENT_LINE_LIMIT:
            violations.append(
                f"{path}: {lines} lines; a measurement summary is at most "
                f"{MEASUREMENT_LINE_LIMIT}. Keep the numbers that decide something and "
                "link the raw data by a pinned commit")
    return violations


# Commit lanes ---------------------------------------------------------------

def lanes(agents_text: str) -> set:
    section = agents_text.split("## Commit naming", 1)[-1]
    found = set()
    for line in section.splitlines():
        if line.startswith("| `"):
            found.update(re.findall(r"`([a-z0-9-]+)`", line.split("|")[1]))
    return found


def subject_violations(subject: str, allowed: set) -> list:
    match = re.match(r"([a-z0-9-]+): \S", subject)
    if not match or match[1] not in allowed:
        return [f"title {subject!r} does not start with a lane from AGENTS.md "
                f"'Commit naming' ({', '.join(sorted(allowed))})"]
    if len(subject) > SUBJECT_LIMIT:
        return [f"title is {len(subject)} characters; the limit is {SUBJECT_LIMIT}"]
    return []


# Driver ---------------------------------------------------------------------

def show(revision: str, path: str, root: Path = ROOT) -> str:
    try:
        return git("show", f"{revision}:{path}", root=root)
    except RuntimeError:
        return ""


def diff_checks(base: str, root: Path = ROOT) -> list:
    changes = []
    for line in git("diff", "--name-status", "--no-renames", base, "HEAD", root=root).splitlines():
        status, path = line.split("\t", 1)
        changes.append((status[0], path))
    violations = golden_violations(
        changes,
        golden_rows(show(base, GOLDEN, root)),
        golden_rows(show("HEAD", GOLDEN, root)),
        changed_golden_rows(git("diff", "-U0", base, "HEAD", "--", GOLDEN, root=root)),
        fixture_directories(root),
    )
    violations += measurement_violations(
        changes, lambda path: len(show("HEAD", path, root).splitlines()))
    base_folders = {path.split("/")[3] for path in
                    git("ls-tree", "-r", "--name-only", base, FACTS_ROOT, root=root).split()
                    if path.count("/") >= 4}
    facts = root / FACTS_ROOT
    head_folders = {entry.name for entry in facts.iterdir() if entry.is_dir()} if facts.is_dir() else set()
    new_folders = sorted(head_folders - base_folders)
    if new_folders:
        readmes = {name: (facts / name / "README.md").read_text(encoding="utf-8")
                   if (facts / name / "README.md").is_file() else ""
                   for name in new_folders}
        nearest = nearest_rows({name: folder_shingles(facts / name) for name in new_folders},
                               {name: folder_shingles(facts / name)
                                for name in sorted(head_folders & base_folders)})
        violations += fact_row_violations(nearest, readmes)
    return violations


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", help="compare HEAD with this revision "
                        "(default: merge base with origin/main)")
    parser.add_argument("--title", help="pull request title to check as a commit subject")
    parser.add_argument("--update-source-reads", action="store_true",
                        help=f"rewrite {READS_BASELINE} from the tree")
    options = parser.parse_args(arguments)
    counts = source_read_counts()
    if options.update_source_reads:
        (ROOT / READS_BASELINE).write_text(write_baseline(counts), encoding="utf-8")
        print(f"check_rules: wrote {len(counts)} files to {READS_BASELINE}")
        return 0
    violations = source_read_violations(
        counts, read_baseline((ROOT / READS_BASELINE).read_text(encoding="utf-8")))
    base = options.base or git("merge-base", "origin/main", "HEAD").strip()
    violations += diff_checks(base)
    if options.title is not None:
        violations += subject_violations(
            options.title, lanes((ROOT / "AGENTS.md").read_text(encoding="utf-8")))
    for violation in violations:
        print(f"check_rules: {violation}", file=sys.stderr)
    if not violations:
        print(f"check_rules: clean against {base[:12]}")
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
