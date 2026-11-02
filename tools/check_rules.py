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
- Stage 06 re-derivation. The 2026-10-04 owner ruling (TASKS.md, "Lowering,
  checker, data-flow, and PCC simplification") makes stage 06 a mechanical
  lowering of stage 05's plans, and omega-rust/pipeline.md forbids stages from
  reconstructing missing input facts. In the two weeks to 2026-11-01 stage 06
  took 124 repairs in 219 commits and changed with stage 05's execution and
  checks in 39 and 36 commits: each production stage 05 learned needed a
  matching shape gate or source re-read in stage 06. Two per-file counts may only fall (LOC-VALUE-CUSTODY
  retires them): shape gates (`unsupported(`, `Unsupported`, `unpinned(`) in
  `tools/stage06_shape_gates.txt`, and reads of source context (the
  `TypedTrees` table, `typed_trees::expression` nodes, and `.typed`, the typed
  trees that checked trees still carry) in `tools/stage06_source_context.txt`.
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
- Status in contracts. `omega-rust/pipeline.md` and `wiki/spec` are normative
  contracts, but by 2026-11-01 pipeline.md had gained 1,115 lines in two weeks,
  about half of them migration log ("now lives", "still open", "remains
  migration work"), and the spec held "Implementation status" blocks and test
  commands with "exits 70 on macOS ARM64". The rules that would have stopped
  per-case repairs sat after that narrative. Status belongs on the board; the
  per-file counts of status phrasing in `tools/contract_status_phrases.txt`
  may only fall.
- Dangling task IDs. AGENTS.md, the skills, pipeline.md and the specification
  route work by citing board items (`NESTED-MACHINE-UNIFICATION`). On
  2026-11-01 they cited 12 IDs no board held: closed items such as
  `EXPLICIT-ENTRY-STORAGE`, still described as open, and one moved item. Every
  upper-case hyphenated ID they cite must be an item on a board or an ID an
  item heading says it also covers. Settled decisions are named in lower case
  and are not task IDs. Fact READMEs held 121 such citations; those are listed
  in `tools/dangling_task_ids.txt` and may only be removed.

The diff checks compare `--base` (default: the merge base with origin/main)
with HEAD and read only Git and the tree. Nothing here builds or runs the
compiler; re-recording a golden row is the author's run.
"""

import argparse
from pathlib import Path
import re
import subprocess
import sys
from typing import Callable, NamedTuple


ROOT = Path(__file__).resolve().parents[1]
RUST_ROOT = "omega-rust"
READS_BASELINE = "tools/source_text_reads.txt"
STATUS_BASELINE = "tools/contract_status_phrases.txt"
STAGE06_ROOT = "omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi"
SHAPE_GATE_BASELINE = "tools/stage06_shape_gates.txt"
SOURCE_CONTEXT_BASELINE = "tools/stage06_source_context.txt"
CONTRACTS = ("omega-rust/pipeline.md", "wiki/spec")
DANGLING_BASELINE = "tools/dangling_task_ids.txt"
TASK_BOARDS = ("TASKS.md", "TASKS_OPTIMIZER.md", "TASKS_BOOTSTRAP.md", "OWNER_QUESTIONS.md")
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

# Wording that reports where the implementation stands rather than what the
# contract requires. Words may wrap across lines.
STATUS_PHRASE = re.compile(
    r"^\#{1,6}[ \t]+(?:implementation[ \t]+|acceptance[ \t]+)?status[ \t]*$"
    r"|\bnot\s+yet\b"
    r"|\bno\s+longer\b"
    r"|\bnow\s+(?:passes|clears|lives|owns|has|retains|admits|rejects|accepts|runs"
    r"|takes|belongs|projects|joins|closes|carries)\b"
    r"|\b(?:exits?|returns?)\s+\d+\s+(?:on|in)\b"
    r"|\bre-verified\b"
    r"|\b(?:enforced|claimed)\s+today\b"
    r"|\b(?:is|are)\s+still\s+open\b"
    r"|\bremains?\s+(?:open|unfinished|unimplemented)\b"
    r"|\bremain\s+migration\b"
    r"|\bmigration\s+work\b"
    r"|\bnot\s+a\s+claim\b"
    r"|\bdoes\s+not\s+claim\b"
    r"|\b(?:is|are)\s+implemented\b",
    re.IGNORECASE | re.MULTILINE,
)

# Stage 06 sites that refuse a production stage 05 already checked, and reads
# that reach past stage 05's plans into the typed source trees.
SHAPE_GATE = re.compile(r"unsupported\(|Unsupported|unpinned\(")
SOURCE_CONTEXT = re.compile(r"\b(?:Frozen)?TypedTrees\b|\btyped_trees::expression\b|\.typed\b")


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


def rust_files(root: Path = ROOT) -> list:
    return sorted(path for path in (root / RUST_ROOT).rglob("*.rs")
                  if "target" not in path.relative_to(root).parts)


def read_baseline(text: str) -> dict:
    baseline = {}
    for line in text.splitlines():
        if line.strip() and not line.startswith("#"):
            count, path = line.split(None, 1)
            baseline[path.strip()] = int(count)
    return baseline


# Status in contracts --------------------------------------------------------

def count_status_phrases(text: str) -> int:
    return len(STATUS_PHRASE.findall(text))


def contract_files(root: Path = ROOT) -> list:
    files = []
    for contract in CONTRACTS:
        path = root / contract
        files += [path] if path.is_file() else sorted(path.rglob("*.md"))
    return files


def stage06_files(root: Path = ROOT) -> list:
    return sorted(path for path in (root / STAGE06_ROOT).rglob("*.rs")
                  if "target" not in path.relative_to(root).parts)


# Count ratchets -------------------------------------------------------------

class Ratchet(NamedTuple):
    """A per-file count that may only fall. `baseline` holds `<count> <path>`
    rows; a rise fails with `advice`, and a fall fails until the author
    rewrites the baseline with `--<flag>`, so the next change cannot spend it."""

    baseline: str
    flag: str
    noun: str
    files: Callable
    count: Callable
    header: tuple
    advice: str


RATCHETS = (
    Ratchet(
        READS_BASELINE, "update-source-reads", "source-text reads", rust_files,
        count_source_reads,
        ("Source-text reads (include_str!/read_to_string of .rs or .toml) per file.",
         "Counts may only fall: express the rule as a dependency in",
         "tools/check_native_boundary.py, a type or visibility, or a test that runs code."),
        "Tests check behavior or structure, never source text (AGENTS.md): state the "
        "rule as a dependency, a type, or a test that runs the code."),
    Ratchet(
        STATUS_BASELINE, "update-status-phrases", "status phrases", contract_files,
        count_status_phrases,
        ("Status phrasing (\"not yet\", \"now passes\", \"exits N on <host>\", an",
         "\"Implementation status\" heading, ...) per contract file.",
         "Counts may only fall: put open work on its TASKS.md item, not in the contract."),
        "Contracts state rules; record where the implementation stands as a remaining "
        "bullet on the owning TASKS.md item, and phrase the rule without it."),
    Ratchet(
        SHAPE_GATE_BASELINE, "update-shape-gates", "stage-06 shape gates", stage06_files,
        lambda text: len(SHAPE_GATE.findall(text)),
        ("Stage 06 shape gates (unsupported(, Unsupported, unpinned() per file.",
         "Counts may only fall: stage 06 is a mechanical lowering of stage 05's plans",
         "(TASKS.md owner ruling 2026-10-04; LOC-VALUE-CUSTODY retires these gates)."),
        "Stage 06 lowers stage 05's plans without re-checking them (TASKS.md owner "
        "ruling 2026-10-04): record the missing decision on the stage 05 plan and "
        "lower it, or reject the program in checking with a reject twin."),
    Ratchet(
        SOURCE_CONTEXT_BASELINE, "update-source-context", "stage-06 source-context reads",
        stage06_files, lambda text: len(SOURCE_CONTEXT.findall(text)),
        ("Stage 06 reads of source context (TypedTrees, typed_trees::expression,",
         ".typed) per file. Counts may only fall: stages do not reconstruct missing",
         "input facts (omega-rust/pipeline.md \"Public contracts\")."),
        "Stages cannot reconstruct missing input facts from source context "
        "(omega-rust/pipeline.md \"Public contracts\"): read the fact from the stage 05 "
        "plan, and record it there when stage 05 does not yet publish it."),
)


def ratchet_counts(ratchet: Ratchet, root: Path = ROOT) -> dict:
    counts = {}
    for path in ratchet.files(root):
        count = ratchet.count(path.read_text(encoding="utf-8", errors="replace"))
        if count:
            counts[path.relative_to(root).as_posix()] = count
    return counts


def write_ratchet(ratchet: Ratchet, counts: dict) -> str:
    lines = [f"# {line}" for line in ratchet.header]
    lines.append(f"# Written by `python tools/check_rules.py --{ratchet.flag}`.")
    lines += [f"{count} {path}" for path, count in sorted(counts.items())]
    return "\n".join(lines) + "\n"


def ratchet_violations(ratchet: Ratchet, counts: dict, baseline: dict) -> list:
    violations = []
    for path in sorted(set(counts) | set(baseline)):
        now, allowed = counts.get(path, 0), baseline.get(path, 0)
        if now > allowed:
            violations.append(f"{path}: {now} {ratchet.noun}, baseline {allowed}. "
                              f"{ratchet.advice}")
        elif now < allowed:
            violations.append(f"{path}: {now} {ratchet.noun}, baseline {allowed}. Lower "
                              f"the baseline: python tools/check_rules.py --{ratchet.flag}")
    return violations


SOURCE_READS, STATUS_PHRASES, SHAPE_GATES, SOURCE_CONTEXT_READS = RATCHETS


# Dangling task IDs ----------------------------------------------------------

# A segment holds at least two letters, which leaves out SHA-256, UTF-8,
# IEEE-2019 and R1-R9; a following file extension leaves out REPORT.md.
TASK_SEGMENT = r"(?=[A-Z0-9]*[A-Z][A-Z0-9]*[A-Z])[A-Z0-9]+"
TASK_ID = re.compile(rf"(?<![\w/.-])({TASK_SEGMENT}(?:-{TASK_SEGMENT})+)(?![\w-]|\.[a-z])")
# `- **ITEM.** (also covers **OTHER** and **THIRD**.)` at the left margin.
BOARD_ITEM = re.compile(r"^- \*\*([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+)\.\*\*(\s*\([^)]*\))?", re.M)


def board_task_ids(boards: list) -> set:
    ids = set()
    for text in boards:
        for item in BOARD_ITEM.finditer(text):
            ids.add(item[1])
            ids.update(TASK_ID.findall(item[2] or ""))
    return ids


def citing_documents(root: Path = ROOT) -> dict:
    paths = [root / "AGENTS.md", root / "omega-rust/pipeline.md"]
    paths += sorted((root / ".agents/skills").rglob("*.md"))
    paths += sorted((root / "wiki/spec").rglob("*.md"))
    paths += sorted((root / FACTS_ROOT).glob("*/README.md"))
    return {path.relative_to(root).as_posix(): path.read_text(encoding="utf-8")
            for path in paths if path.is_file()}


def dangling_task_ids(documents: dict, known: set) -> set:
    return {(path, task) for path, text in documents.items()
            for task in TASK_ID.findall(text) if task not in known}


def read_dangling_baseline(text: str) -> set:
    return {tuple(line.split()) for line in text.splitlines()
            if line.strip() and not line.startswith("#")}


def write_dangling_baseline(dangling: set) -> str:
    lines = [
        "# Task IDs cited by samples/cli/facts READMEs that no board holds.",
        "# Written by `python tools/check_rules.py --update-dangling-task-ids`.",
        "# Entries may only be removed: repoint a citation to its current board",
        "# item, or delete it when the work is done.",
    ]
    lines += [f"{path} {task}" for path, task in sorted(dangling) if path.startswith(FACTS_ROOT)]
    return "\n".join(lines) + "\n"


def dangling_task_violations(dangling: set, baseline: set) -> list:
    violations = [
        f"{path}: cites {task}, which is on no board ({', '.join(TASK_BOARDS)}). "
        "Repoint it to the item that owns the work now (git log -S finds where it "
        "went), or delete it when the work is done"
        for path, task in sorted(dangling)
        if (path, task) not in baseline or not path.startswith(FACTS_ROOT)]
    resolved = sorted(baseline - dangling)
    if resolved:
        violations.append(
            f"{DANGLING_BASELINE}: {len(resolved)} entries no longer dangle "
            f"({resolved[0][0]} {resolved[0][1]} first). Lower the baseline: "
            "python tools/check_rules.py --update-dangling-task-ids")
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
    for ratchet in RATCHETS:
        parser.add_argument(f"--{ratchet.flag}", action="store_true",
                            help=f"rewrite {ratchet.baseline} from the tree")
    parser.add_argument("--update-dangling-task-ids", action="store_true",
                        help=f"rewrite {DANGLING_BASELINE} from the tree")
    options = parser.parse_args(arguments)
    counts = {ratchet: ratchet_counts(ratchet) for ratchet in RATCHETS}
    updates = [ratchet for ratchet in RATCHETS
               if getattr(options, ratchet.flag.replace("-", "_"))]
    for ratchet in updates:
        (ROOT / ratchet.baseline).write_text(write_ratchet(ratchet, counts[ratchet]),
                                             encoding="utf-8")
        print(f"check_rules: wrote {len(counts[ratchet])} files to {ratchet.baseline}")
    if updates:
        return 0
    dangling = dangling_task_ids(
        citing_documents(),
        board_task_ids([(ROOT / board).read_text(encoding="utf-8") for board in TASK_BOARDS]))
    if options.update_dangling_task_ids:
        text = write_dangling_baseline(dangling)
        (ROOT / DANGLING_BASELINE).write_text(text, encoding="utf-8")
        print(f"check_rules: wrote {len(read_dangling_baseline(text))} citations to {DANGLING_BASELINE}")
        return 0
    violations = []
    for ratchet in RATCHETS:
        violations += ratchet_violations(
            ratchet, counts[ratchet],
            read_baseline((ROOT / ratchet.baseline).read_text(encoding="utf-8")))
    violations += dangling_task_violations(
        dangling,
        read_dangling_baseline((ROOT / DANGLING_BASELINE).read_text(encoding="utf-8")))
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
