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
- Psi 06 re-derivation. The 2026-10-04 owner ruling (TASKS.md, "Lowering,
  checker, data-flow, and PCC simplification") makes Psi 06 a mechanical
  lowering of Psi 05's plans, and omega-rust/pipeline.md forbids stages from
  reconstructing missing input facts. In the two weeks to 2026-11-01 Psi 06
  took 124 repairs in 219 commits and changed with Psi 05's execution and
  checks in 39 and 36 commits: each production Psi 05 learned needed a
  matching shape gate or source re-read in Psi 06. Two per-file counts may only fall (LOC-VALUE-CUSTODY
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
- Dense-ID maps and caches. The compiler mints dense `semantic_id!` IDs and
  arena handles, then on 2026-11-02 stored them in 1,537 BTreeMap/HashMap/set
  types across 344 non-test files and layered 311 cache- or memo-shaped
  bindings over them, while the ITERATION-PERFORMANCE campaign replaced them
  with flat tables. Per-file counts in `tools/dense_id_maps.txt` and
  `tools/cache_state.txt` may only fall.
- Monotonic discrete task board (`TASKS.md`). Despite the board's own rule to
  update by subtraction (`Delta lines <= 0`) using discrete one-line bullets,
  50 commits added 378 lines while deleting 163 (+215 net), leaving 1,882
  wrapped continuation lines across 2,332 lines as PRs parked deferred sub-tasks
  and status prose on the board. `TASKS.md` must stay at or below 460 lines,
  use discrete single-line bullets without wrapped continuation lines or
  status-history prose, and never grow in line or sub-bullet count across a PR.
- Rewritten diagnostic pins. A fail fixture's `expected.txt` is the diagnostic
  the corpus requires. Rewriting that text makes a new refusal look pinned
  without the compiler learning the old one. An `expected.txt` that already
  existed on the base keeps its diagnostic fragments. A new fixture file may
  carry its own pin.
- Duplicate fact programs. A fact already pinned under
  `tests/omega/{pass,fail,run}/facts` does not need another program under
  `samples/cli/facts`. A new `.omg` there fails when a base fixture's directory
  name is that fact or starts with `<fact>_` and no longer fact folder owns it.

One check reports instead of failing:

- Capability evidence. A compiler repair should show what it closes. From
  2026-10-29 to 2026-11-05, 549 of the 621 commits that added compiler Rust
  under `omega-rust/psi/` or `omega-rust/omega/compilation/` turned no
  existing pass/run row green, and 245 of all 1,085 commits were per-case
  "keep/carry/preserve/retain" repairs, most of which added a new fixture row
  rather than closing a failing one. Refusing that shape outright would also
  refuse legitimate work (a new behavior pinned by its own fixture pair,
  performance work, restructuring), so this prints a notice that coordinator
  review must resolve. Evidence is a change to a row that existed at the base
  (a positive row turned green, a control that now rejects for its pinned
  reason, a control whose recorded diagnostic changed) or a deleted
  `TASKS.md` line. Without it, the notice distinguishes code pinned only on
  rows the change adds from code that changes no corpus outcome at all.

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
SEMANTIC_IDS = "omega-rust/psi/foundation/semantic-vocabulary/src/identity.rs"
DENSE_MAP_BASELINE = "tools/dense_id_maps.txt"
CACHE_STATE_BASELINE = "tools/cache_state.txt"
CONTRACTS = ("omega-rust/pipeline.md", "wiki/spec")
DANGLING_BASELINE = "tools/dangling_task_ids.txt"
TASK_BOARDS = ("TASKS.md", "TASKS_OPTIMIZER.md", "TASKS_BOOTSTRAP.md", "OWNER_QUESTIONS.md")
GOLDEN = "tests/omega/corpus_outcomes.txt"
FIXTURE_TIERS = ("tests/omega/pass/", "tests/omega/fail/", "tests/omega/run/")
FACTS_ROOT = "samples/cli/facts/"
MEASUREMENTS_ROOT = "wiki/drafts/measurements/"
MEASUREMENT_LINE_LIMIT = 300
MAX_NEW_OMG_FILES = 6
NEAR_DUPLICATE = 0.8
SUBJECT_LIMIT = 85
TASKS_BOARD = "TASKS.md"
TASKS_LINE_LIMIT = 210
TASKS_CHAR_LIMIT = 22500
TASKS_ITEM_LINE = re.compile(
    r"^(?:## \S|<a id=\"[a-z0-9-]+\"></a>$|- \*\*[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+\.\*\* \S|  - \S|  Acceptance: \S)"
)
TASKS_HISTORY_PHRASE = re.compile(
    r"\b(?:now\s+(?:passes|preserves|reserves|clears|lives|owns|has|retains|admits|rejects|accepts|runs"
    r"|takes|belongs|projects|joins|closes|carries)|(?:is|are)\s+still\s+open"
    r"|remains?\s+(?:open|unfinished|unimplemented)|not\s+yet)\b|^\s*Landed:",
    re.IGNORECASE | re.MULTILINE,
)

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

# Psi 06 sites that refuse a production Psi 05 already checked, and reads
# that reach past Psi 05's plans into the typed source trees.
SHAPE_GATE = re.compile(r"unsupported\(|Unsupported|unpinned\(")
SOURCE_CONTEXT = re.compile(r"\b(?:Frozen)?TypedTrees\b|\btyped_trees::expression\b|\.typed\b")

# Product Rust excludes test directories, test files and `#[cfg(test)]` /
# `#[cfg(all(test, …))]` modules. Cache state is a binding named *cache*/*memo*
# or typed as a *Cache*/*Memo* type, so renaming the binding does not hide it.
# Each branch consumes through its colon, so `let cache: LayoutCache` counts
# once; `memo` before `r` is `memory`, not a memo.
TEST_FILE = re.compile(r"(?:^|_)tests?\.rs$")
LINE_COMMENT = re.compile(r"//[^\n]*")
TEST_MODULE = re.compile(r"#\[cfg\((?:all\()?test\b[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*"
                         r"(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{")
CACHE_NAME = r"[a-z_][a-z0-9_]*(?:cache|memo(?!r))[a-z0-9_]*"
CACHE_TYPE = r"(?:\w+::)*\w*(?:Cache|Memo(?!r))\w*\b"
CACHE_STATE = re.compile(rf"\blet\s+(?:mut\s+)?{CACHE_NAME}\b|\b{CACHE_NAME}\s*:(?!:)"
                         rf"|(?<!:):(?!:)\s*&?\s*(?:mut\s+)?{CACHE_TYPE}"
                         r"|\b(?:OwnedIndex|KeyedRows)\b")


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


# Dense-ID maps and caches ---------------------------------------------------

def product_rust_files(root: Path = ROOT) -> list:
    return [path for path in rust_files(root)
            if not {"tests", "benches"} & set(path.relative_to(root).parts)
            and not TEST_FILE.search(path.name)]


def product_text(text: str) -> str:
    """Drop line comments and `#[cfg(test)]` modules, which ship nothing."""
    text = LINE_COMMENT.sub("", text)
    while match := TEST_MODULE.search(text):
        depth, position = 1, match.end()
        while depth and position < len(text):
            depth += {"{": 1, "}": -1}.get(text[position], 0)
            position += 1
        text = text[:match.start()] + text[position:]
    return text


def count_cache_state(text: str) -> int:
    # Substring tests skip the ~88% of files the regex would scan at every word.
    if not any(word in text for word in ("cache", "Cache", "memo", "Memo",
                                         "OwnedIndex", "KeyedRows")):
        return 0
    return len(CACHE_STATE.findall(product_text(text)))


# A map or set keyed by a `semantic_id!` type, an arena `Handle<…>` or a
# `*Handle` alias. The IDs are read from identity.rs, so a new ID is covered.
DENSE_KEYS = "|".join(re.findall(r"semantic_id!\(\s*(\w+)",
                                 (ROOT / SEMANTIC_IDS).read_text(encoding="utf-8"))
                      + [r"\w*Handle"])
DENSE_KEY_MAP = re.compile(
    rf"\b(?:Fx)?(?:BTree|Hash|Index)(?:Map|Set)\s*<\s*&?\s*(?:\w+::)*(?:{DENSE_KEYS})\b")


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
        SHAPE_GATE_BASELINE, "update-shape-gates", "Psi 06 shape gates", stage06_files,
        lambda text: len(SHAPE_GATE.findall(text)),
        ("Psi 06 shape gates (unsupported(, Unsupported, unpinned() per file.",
         "Counts may only fall: Psi 06 is a mechanical lowering of Psi 05's plans",
         "(TASKS.md owner ruling 2026-10-04; LOC-VALUE-CUSTODY retires these gates)."),
        "Psi 06 lowers Psi 05's plans without re-checking them (TASKS.md owner "
        "ruling 2026-10-04): record the missing decision on the Psi 05 plan and "
        "lower it, or reject the program in checking with a reject twin."),
    Ratchet(
        SOURCE_CONTEXT_BASELINE, "update-source-context", "Psi 06 source-context reads",
        stage06_files, lambda text: len(SOURCE_CONTEXT.findall(text)),
        ("Psi 06 reads of source context (TypedTrees, typed_trees::expression,",
         ".typed) per file. Counts may only fall: stages do not reconstruct missing",
         "input facts (omega-rust/pipeline.md \"Public contracts\")."),
        "Stages cannot reconstruct missing input facts from source context "
        "(omega-rust/pipeline.md \"Public contracts\"): read the fact from the Psi 05 "
        "plan, and record it there when Psi 05 does not yet publish it."),
    Ratchet(
        DENSE_MAP_BASELINE, "update-dense-id-maps", "maps keyed by dense IDs",
        product_rust_files, lambda text: len(DENSE_KEY_MAP.findall(product_text(text))),
        ("BTreeMap/HashMap/BTreeSet/HashSet (Fx, Index) keyed by a semantic_id! type or",
         "an arena Handle, per non-test file. Counts may only fall: dense IDs index",
         "flat tables (AGENTS.md conventions)."),
        "Dense IDs index flat tables, not maps (AGENTS.md): store the rows in an "
        "IdTable/IdSet, or a Vec indexed by the ID."),
    Ratchet(
        CACHE_STATE_BASELINE, "update-cache-state", "cache-shaped names",
        product_rust_files, count_cache_state,
        ("Fields, parameters and locals named *cache*/*memo* (not memory) or typed",
         "*Cache*/*Memo*, plus OwnedIndex/KeyedRows uses, per non-test file. Counts",
         "may only fall: derived data is computed once by its owner, not cached",
         "(AGENTS.md conventions)."),
        "Derived data is computed once by its owner, not cached (AGENTS.md): have the "
        "producing stage publish it as a table its consumers index."),
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


(SOURCE_READS, STATUS_PHRASES, SHAPE_GATES, SOURCE_CONTEXT_READS, DENSE_ID_MAPS,
 CACHE_STATE_NAMES) = RATCHETS


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


# Monotonic discrete task board ----------------------------------------------

def tasks_board_violations(head_text: str, base_text: str = "") -> list:
    """Enforce that TASKS.md stays a compact, subtract-only remaining-work board:
    1. Total lines stay at or below TASKS_LINE_LIMIT and never exceed base_text.
    2. Sub-bullet count (`  - `) never exceeds base_text (no deferral parking lot).
    3. Every item/bullet/Acceptance line is a single discrete unwrapped line.
    4. No status-history prose (`now passes`, `now preserves`, `remains open`, `Landed:`)."""
    if not head_text:
        return []
    violations = []
    head_lines = head_text.splitlines()
    if len(head_lines) > TASKS_LINE_LIMIT:
        violations.append(
            f"{TASKS_BOARD}: {len(head_lines)} lines (limit is {TASKS_LINE_LIMIT}); "
            "keep outcomes as discrete one-line bullets and delete completed work")
    if len(head_text) > TASKS_CHAR_LIMIT:
        violations.append(
            f"{TASKS_BOARD}: {len(head_text)} characters (limit is {TASKS_CHAR_LIMIT}); "
            "keep bullets concise rather than packing prose onto single lines")
    if base_text:
        base_lines = base_text.splitlines()
        if len(head_lines) > len(base_lines):
            violations.append(
                f"{TASKS_BOARD}: grew from {len(base_lines)} to {len(head_lines)} lines; "
                "update by subtraction (delete completed bullets or fold without growing "
                "the board, AGENTS.md 'Monotonic board closure')")
        if len(head_text) > len(base_text):
            violations.append(
                f"{TASKS_BOARD}: grew from {len(base_text)} to {len(head_text)} characters; "
                "update by subtraction rather than appending prose to existing lines")
        base_subbullets = sum(1 for line in base_lines if line.startswith("  - "))
        head_subbullets = sum(1 for line in head_lines if line.startswith("  - "))
        if head_subbullets > base_subbullets:
            violations.append(
                f"{TASKS_BOARD}: sub-bullets grew from {base_subbullets} to {head_subbullets}; "
                "fix discovered roadblocks in the same PR or fold into an existing bullet "
                "instead of adding deferred sub-bullets")
    in_items = False
    for index, line in enumerate(head_lines, 1):
        if line == "## Priority (human-set only)":
            in_items = True
        if not line.strip():
            continue
        if line[:1] in " \t" and not line.startswith(("  - ", "  Acceptance: ")):
            violations.append(
                f"{TASKS_BOARD}:{index}: wrapped continuation or over-indented line; "
                "write each bullet and Acceptance gate on a single discrete line")
        elif in_items and not TASKS_ITEM_LINE.match(line):
            violations.append(
                f"{TASKS_BOARD}:{index}: unexpected line in item section; "
                "use only `## Section`, `- **ITEM.** ...`, `  - ...`, or `  Acceptance: ...`")
    for match in TASKS_HISTORY_PHRASE.finditer(head_text):
        line_no = head_text[:match.start()].count("\n") + 1
        violations.append(
            f"{TASKS_BOARD}:{line_no}: status-history phrase {match.group(0).strip()!r}; "
            "delete completed clauses outright instead of recording status prose")
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


def golden_row_quality_violations(diff: str) -> list:
    """Reject newly recorded rows in corpus_outcomes.txt that carry a dirty
    worktree stamp (+wt), record a pass/run fixture as rejected or regressed-from,
    or record a fail fixture as unexpected or passing."""
    removed = {line[1:] for line in diff.splitlines()
               if line.startswith("-") and not line.startswith("---")}
    violations = []
    for line in diff.splitlines():
        if not line.startswith("+") or line.startswith("+++"):
            continue
        body = line[1:]
        if body in removed or not body.strip() or body[0] in "#\t ":
            continue
        tokens = body.split()
        if len(tokens) < 2:
            continue
        name, outcome = tokens[0], tokens[1]
        for token in tokens[2:]:
            if token.startswith("@") and token.endswith("+wt"):
                violations.append(
                    f"{GOLDEN}: {name} carries a dirty worktree stamp ({token}); "
                    "commit all changes before running corpus_gate.py --record")
        if name.startswith(("pass/", "run/")):
            if outcome == "rejected" or any(t.startswith("regressed-from:") for t in tokens[2:]):
                violations.append(
                    f"{GOLDEN}: {name} is a pass/run fixture recorded as {outcome}; "
                    "fix the fixture or compiler before recording")
        elif name.startswith("fail/"):
            if outcome != "rejected" or "unexpected" in tokens[2:]:
                violations.append(
                    f"{GOLDEN}: {name} is a fail fixture recorded as "
                    f"{'unexpected' if 'unexpected' in tokens[2:] else outcome}; "
                    "ensure it rejects with its expected obligation before recording")
    return violations


# Capability evidence (notice, not a violation) -------------------------------

COMPILER_ROOTS = ("omega-rust/psi/", "omega-rust/omega/compilation/")
GREEN_OUTCOMES = ("checked", "built")


class Row(NamedTuple):
    outcome: str
    flags: frozenset
    diagnostics: tuple


def golden_records(text: str) -> dict:
    """Each row's outcome, its flag tokens (`expected`, `unexpected`,
    `regressed-from:…`; timings and stamps dropped, since a re-record changes
    them without changing what was observed) and its diagnostic lines."""
    records, name = {}, None
    for line in text.splitlines():
        if line.startswith("\t") and name:
            records[name] = records[name]._replace(
                diagnostics=records[name].diagnostics + (line.strip(),))
            continue
        if not line.strip() or line[0] in "#\t ":
            continue
        tokens = line.split()
        if len(tokens) < 2:
            name = None
            continue
        name = tokens[0]
        flags = frozenset(token for token in tokens[2:]
                          if not token.startswith("@") and not re.fullmatch(r"\d+m?s", token))
        records[name] = Row(tokens[1], flags, ())
    return records


def is_green(row: Row) -> bool:
    return row.outcome in GREEN_OUTCOMES and not any(
        flag.startswith("regressed-from:") for flag in row.flags)


def is_pinned_rejection(row: Row) -> bool:
    return row.outcome == "rejected" and "expected" in row.flags and "unexpected" not in row.flags


class CorpusEvidence(NamedTuple):
    greened: list
    closed_controls: list
    diagnostic_changes: list
    added_rows: list


def corpus_evidence(base_text: str, head_text: str) -> CorpusEvidence:
    """What the golden says the change did to fixtures that existed at the
    base. Rows the change adds are listed separately: a new fixture shows the
    repair's own case, not a failure the compiler already had."""
    base, head = golden_records(base_text), golden_records(head_text)
    greened, closed, reworded = [], [], []
    for name, row in head.items():
        before = base.get(name)
        if before is None:
            continue
        if name.startswith(("pass/", "run/")) and is_green(row) and not is_green(before):
            greened.append(name)
        elif name.startswith("fail/") and is_pinned_rejection(row):
            # A control closes when the base accepted it or rejected it for
            # the wrong reason. An unpinned rejection that gains `expected`
            # gained its pin in this change; the compiler did not move.
            if before.outcome != "rejected" or "unexpected" in before.flags:
                closed.append(name)
            elif is_pinned_rejection(before) and row.diagnostics != before.diagnostics:
                reworded.append(name)
    return CorpusEvidence(sorted(greened), sorted(closed), sorted(reworded),
                          sorted(set(head) - set(base)))


def is_product_compiler_rust(path: str) -> bool:
    parts = path.split("/")
    return (path.startswith(COMPILER_ROOTS) and path.endswith(".rs")
            and not {"tests", "benches"} & set(parts)
            and not TEST_FILE.search(parts[-1]))


def added_compiler_lines(numstat: str) -> int:
    """Net lines added to product compiler Rust; binary entries count zero."""
    total = 0
    for line in numstat.splitlines():
        fields = line.split("\t")
        if len(fields) == 3 and fields[0] != "-" and is_product_compiler_rust(fields[2]):
            total += int(fields[0]) - int(fields[1])
    return total


def capability_notices(compiler_lines: int, evidence: CorpusEvidence, tasks_deleted: int) -> list:
    """No notice when the change closes something that already existed: a
    positive row turned green, a control that now rejects for its pinned
    reason, a control whose recorded diagnostic changed, or a board line.
    Otherwise the notice says which of the two remaining shapes it is."""
    if compiler_lines <= 0 or tasks_deleted or evidence.greened \
            or evidence.closed_controls or evidence.diagnostic_changes:
        return []
    if evidence.added_rows:
        shown = ", ".join(evidence.added_rows[:3]) + (", …" if len(evidence.added_rows) > 3 else "")
        return [f"this change adds {compiler_lines} lines of compiler Rust pinned only on "
                f"{len(evidence.added_rows)} row(s) it adds to {GOLDEN} ({shown}); no existing row "
                f"turns green or changes its rejection, and no {TASKS_BOARD} line is deleted. Name "
                "in the PR body the existing failing row, sample or acceptance it closes, or the "
                "behavior no existing fixture covered."]
    return [f"this change adds {compiler_lines} lines of compiler Rust and changes no corpus "
            f"outcome: no row in {GOLDEN} is added, turns green or changes its rejection, and no "
            f"{TASKS_BOARD} line is deleted. Performance or restructuring work says so in the PR "
            "body with its measurement; any other repair with no observable outcome is per-case glue."]


def capability_evidence(base: str, root: Path = ROOT) -> list:
    numstat = git("diff", "--numstat", "--no-renames", base, "HEAD", "--",
                  *[prefix.rstrip("/") for prefix in COMPILER_ROOTS], root=root)
    tasks = git("diff", "--numstat", base, "HEAD", "--", TASKS_BOARD, root=root).split()
    return capability_notices(
        added_compiler_lines(numstat),
        corpus_evidence(show(base, GOLDEN, root), show("HEAD", GOLDEN, root)),
        int(tasks[1]) if len(tasks) >= 2 and tasks[1] != "-" else 0)


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
    all_rows = base_rows | head_rows
    for _, path in changes:
        if not path.endswith("README.md"):
            touched.add(fixture_of(path, all_rows, fixture_dirs))
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


def diagnostic_fragments(text: str) -> frozenset:
    """Non-empty trimmed lines of expected.txt. The corpus requires each one
    to occur in the diagnostic, so the set is the pin."""
    return frozenset(line.strip() for line in text.splitlines() if line.strip())


def _is_omega_expected(path: str) -> bool:
    return path.startswith("tests/omega/") and path.endswith("/expected.txt")


def _quote_fragments(fragments: list) -> str:
    shown = ", ".join(repr(fragment) for fragment in fragments[:3])
    if len(fragments) > 3:
        shown += f" (+{len(fragments) - 3} more)"
    return shown


def expected_pin_violations(changes: list, before, after) -> list:
    """Refuse a rewrite of pinned diagnostic text.

    `before(path)` and `after(path)` return the file at the base and at HEAD.
    A newly added expected.txt may pin a new fixture. Changing or deleting the
    fragments of an expected.txt that already existed on the base may not.
    """
    violations = []
    for status, path in changes:
        if status == "A" or not _is_omega_expected(path):
            continue
        old = diagnostic_fragments(before(path))
        new = frozenset() if status == "D" else diagnostic_fragments(after(path))
        if old == new:
            continue
        if status == "D":
            detail = "removed the file and its pinned diagnostic"
        else:
            parts = []
            removed = sorted(old - new)
            added = sorted(new - old)
            if removed:
                parts.append("dropped " + _quote_fragments(removed))
            if added:
                parts.append("added " + _quote_fragments(added))
            detail = "; ".join(parts)
        violations.append(
            f"{path}: broke existing-diagnostic-pin: {detail}. "
            "An expected.txt that already existed on the base keeps its pinned "
            "diagnostic; add a new fixture file to pin a different diagnostic.")
    return violations


def sample_fact_name(path: str) -> str:
    """The fact folder of a samples/cli/facts path, or "" for a root file."""
    if not path.startswith(FACTS_ROOT):
        return ""
    fact, separator, _rest = path[len(FACTS_ROOT):].partition("/")
    return fact if separator and fact else ""


def omega_fact_fixtures(paths) -> set:
    """`tier/facts/<directory>` for files under tests/omega fact fixtures."""
    found = set()
    for path in paths:
        parts = path.split("/")
        if (len(parts) >= 5 and parts[0] == "tests" and parts[1] == "omega"
                and parts[3] == "facts" and parts[4]):
            found.add(f"{parts[2]}/facts/{parts[4]}")
    return found


def fixture_owner(directory: str, fact_names: set) -> str:
    """The longest fact folder that `directory` names, at a '_' boundary."""
    matches = [fact for fact in fact_names
               if directory == fact or directory.startswith(fact + "_")]
    if not matches:
        return ""
    return max(matches, key=lambda fact: (len(fact), fact))


def pinned_fixture_for_fact(fact: str, fixtures: set, fact_names: set) -> str:
    names = set(fact_names)
    names.add(fact)
    hits = [fixture for fixture in fixtures
            if fixture_owner(fixture.rsplit("/", 1)[-1], names) == fact]
    return min(hits) if hits else ""


def already_pinned_fact_violations(changes: list, fixtures: set, fact_names: set) -> list:
    """Refuse a new samples/cli/facts program for a fact the corpus already pins.

    `fixtures` holds `tier/facts/<directory>` names present on the base.
    `fact_names` holds samples/cli/facts folder names on the base and HEAD, so
    a longer fact such as may_write_codec_argument keeps its own fixtures.
    """
    names = set(fact_names)
    added = []
    for status, path in changes:
        if status != "A" or not path.startswith(FACTS_ROOT) or not path.endswith(".omg"):
            continue
        fact = sample_fact_name(path)
        if not fact:
            continue
        names.add(fact)
        added.append((path, fact))
    violations = []
    for path, fact in added:
        witness = pinned_fixture_for_fact(fact, fixtures, names)
        if not witness:
            continue
        violations.append(
            f"{path}: broke already-pinned-fact: new .omg under {FACTS_ROOT}{fact} "
            f"while tests/omega already has facts fixture `{witness}` for that fact. "
            "Adding a samples/cli/facts program for a fact already pinned under "
            "tests/omega/{pass,fail,run}/facts is refused.")
    return violations


def list_tree(revision: str, *paths, root: Path = ROOT) -> list:
    if not paths:
        return []
    return [line for line in git("ls-tree", "-r", "--name-only", revision, *paths, root=root).splitlines()
            if line]


def diff_hygiene_violations(changes: list) -> list:
    """Enforce repository organization and anti-bloat guardrails on added files:
    1. No new .omg files under samples/cli/facts/ or tests/omega/{pass,fail,run}/facts/.
    2. At most MAX_NEW_OMG_FILES (6) new .omg files in a single PR across the tree.
    3. No new tests/omega/corpus_*.txt files.
    4. No new tools/swarm/waves/ files committed to git (use build/swarm/<wave>/)."""
    violations = []
    added_omg = []
    for status, path in changes:
        if status != "A":
            continue
        if path.endswith(".omg"):
            added_omg.append(path)
            if path.startswith(FACTS_ROOT) or path.startswith(
                ("tests/omega/pass/facts/", "tests/omega/fail/facts/", "tests/omega/run/facts/")
            ):
                violations.append(
                    f"{path}: adding new .omg files under facts/ is frozen to prevent corpus bloat; "
                    f"pin the repair by flipping existing regressions in {GOLDEN} or adding a "
                    "focused fixture pair under tests/omega/{pass,fail,run}/<behavior>/")
        elif path.startswith("tests/omega/corpus_") and path.endswith(".txt"):
            violations.append(
                f"{path}: do not add new corpus_*.txt files; record outcomes in {GOLDEN}")
        elif path.startswith("tools/swarm/waves/"):
            violations.append(
                f"{path}: swarm wave state belongs in ignored build/swarm/<wave>/, not tools/swarm/waves/")
    if len(added_omg) > MAX_NEW_OMG_FILES:
        violations.append(
            f"PR adds {len(added_omg)} .omg files (limit is {MAX_NEW_OMG_FILES}); "
            "pin repairs with at most a focused positive/negative fixture pair, never a combinatorial matrix")
    return violations


def diff_checks(base: str, root: Path = ROOT) -> list:
    changes = []
    for line in git("diff", "--name-status", "--no-renames", base, "HEAD", root=root).splitlines():
        status, path = line.split("\t", 1)
        changes.append((status[0], path))
    golden_diff = git("diff", "-U0", base, "HEAD", "--", GOLDEN, root=root)
    violations = golden_violations(
        changes,
        golden_rows(show(base, GOLDEN, root)),
        golden_rows(show("HEAD", GOLDEN, root)),
        changed_golden_rows(golden_diff),
        fixture_directories(root),
    )
    violations += golden_row_quality_violations(golden_diff)
    violations += measurement_violations(
        changes, lambda path: len(show("HEAD", path, root).splitlines()))
    violations += diff_hygiene_violations(changes)
    tasks_path = root / TASKS_BOARD
    head_tasks = tasks_path.read_text(encoding="utf-8") if tasks_path.is_file() else show("HEAD", TASKS_BOARD, root)
    violations += tasks_board_violations(head_tasks, show(base, TASKS_BOARD, root))
    base_folders = {path.split("/")[3] for path in
                    git("ls-tree", "-r", "--name-only", base, FACTS_ROOT, root=root).split()
                    if path.count("/") >= 4}
    facts = root / FACTS_ROOT
    head_folders = {entry.name for entry in facts.iterdir() if entry.is_dir()} if facts.is_dir() else set()
    new_folders = sorted(head_folders - base_folders)
    for name in new_folders:
        violations.append(
            f"{FACTS_ROOT}{name}: new fact folders are frozen to prevent corpus bloat; "
            f"pin the repair by flipping existing regressions in {GOLDEN} or adding a "
            "focused fixture pair under tests/omega/{pass,fail,run}/<behavior>/")
    violations += expected_pin_violations(
        changes,
        lambda path: show(base, path, root),
        lambda path: show("HEAD", path, root))
    fixture_roots = ("tests/omega/pass/facts", "tests/omega/fail/facts", "tests/omega/run/facts")
    fact_names = set()
    for revision in (base, "HEAD"):
        for path in list_tree(revision, FACTS_ROOT, root=root):
            fact = sample_fact_name(path)
            if fact:
                fact_names.add(fact)
    violations += already_pinned_fact_violations(
        changes, omega_fact_fixtures(list_tree(base, *fixture_roots, root=root)), fact_names)
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
    for notice in capability_evidence(base):
        print(f"check_rules: notice: {notice}", file=sys.stderr)
    for violation in violations:
        print(f"check_rules: {violation}", file=sys.stderr)
    if not violations:
        print(f"check_rules: clean against {base[:12]}")
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
