#!/usr/bin/env python3
"""Rank where the compiler has two models of one thing, from history and source.

Agents repair the case in front of them. 278 of 1,064 commits from 2026-10-25
to 2026-11-01 were "preserve/carry" repairs, and the two checkers that model
storage history separately (stage 04 FlowFacts and checking's program
validation) only showed up in the aggregate record, not in any one session.
This tool prints that aggregate in one report so design work starts from
measured hot spots instead of from whichever file a session happened to open.

Usage:

    python3 tools/architecture_signals.py                 # the default report
    python3 tools/architecture_signals.py --since 2026-10-25 --top 30
    python3 tools/architecture_signals.py --focus custody      # one family's clone pairs
    python3 tools/architecture_signals.py --json signals.json   # raw rows, keep out of git
    python3 tools/architecture_signals.py --coverage --witnesses tools/route_witnesses.txt \\
        --coverage-rev e5812a0365a

The default report reads git history, `cargo metadata` and the source tree
and finishes in about a minute. Granularity is crate plus top-level module
(`crate::module`); pipeline stage crates print as `psi:04`, `omega:03`, and
checking's package prints by its Cargo name, `validation`. Omega-written
product sources print as `src:psi/checking::<file>`.

Signals and what each points at:

1. Co-change. Pairs of modules in different crates changed by the same
   commit. Commits touching more than `--max-modules` modules (default 15)
   are mass migrations and are left out; the report says how many. Points at
   two models of one thing.
2. Repair hot spots. Per module, commits whose subject uses a preserve-type
   verb anywhere (preserve, retain, carry, keep, transport, propagate,
   thread, forward), their share of the module's commits and weekly rate.
   Points at a missing abstraction: each new production needs its own route.
3. Rust clones. 8-gram shingles over Rust tokens with local names and
   literals erased, 1-in-4 hash-sampled, counted per file pair across crates
   for shingles found in at most 4 files. Type names, callee names and field
   names are kept: with every name erased (the `check_rules.py` method for
   fact rows), only 15 function pairs in 18,410 reached Jaccard 0.5 and none
   was a custody home, so that variant failed calibration. Also lists
   same-named files in two or more crates that share shingles, which is how
   a mechanism rebuilt per stage usually looks. Points at one mechanism
   built more than once.
4. Crate map. Cargo metadata plus each `lib.rs` header line, checked against
   the "Connected program route" table in `omega-rust/pipeline.md`. Reports
   stages missing from either side, each pipeline crate's input and output
   representation, representation crates a stage reads besides its own
   input and output, pipeline-to-pipeline dependencies and Omega compilation
   crates that depend on source-side Psi crates. Points at firewall leaks and
   stages that consume the wrong representation.
5. Route coverage (`--coverage`, off by default because it builds and runs
   the compiler). In a scratch worktree at `--coverage-rev`, every `||` chain
   of proof routes in the named `--route-site FILE::FUNCTION` functions is
   rewritten to evaluate every operand and print one trace row, and a
   fallback route consulted only when the chain fails (`if !proved && ...
   let Some(..) = route(..)`) is evaluated too. The scratch worktree is never
   committed. Each witness runs `omega --check` with a fresh HOME, because the
   check cache replays earlier judgments without running the checker. Nested
   chains are flattened into one decision per fact. A route that never
   proves is dead; one that proves but is never the only proof is redundant
   on that witness set. Witness sets above 200 fixtures are refused.

Raw rows (`--json`) are measurement data: link them from a claim note or a
pinned commit, never commit them.
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import datetime as dt
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import zlib

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SINCE = "2026-10-18"
REPAIR_VERBS = ("preserve", "retain", "carry", "keep", "transport", "propagate",
                "thread", "forward")
REPAIR = re.compile(r"\b(" + "|".join(REPAIR_VERBS) + r")\w*\b", re.IGNORECASE)
STAGE = re.compile(r"(psi|omega)/compilation/pipeline/(\d+b?)_")
TRACE_PREFIX = "OMEGA_ROUTE_TRACE\t"
DEFAULT_ROUTE_SITES = (
    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/"
    "contracts/exits.rs::check_exit_ensures",
    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/"
    "contracts/exits.rs::live_contexts_prove_scalar_postcondition",
)
MAX_WITNESSES = 200


def git(*arguments, root=ROOT) -> str:
    return subprocess.run(["git", *arguments], cwd=root, check=True, text=True,
                          capture_output=True).stdout


# Module keys ------------------------------------------------------------------

def cargo_metadata(root: Path = ROOT) -> dict:
    output = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1",
                             "--offline"], cwd=root, check=True, text=True,
                            capture_output=True).stdout
    return json.loads(output)


def crate_label(name: str, directory: str) -> str:
    stage = STAGE.search(directory)
    return f"{stage.group(1)}:{stage.group(2)}" if stage else name


def relative_directory(manifest_path: str, root: Path):
    directory = Path(manifest_path).parent.resolve()
    try:
        return directory.relative_to(root.resolve()).as_posix()
    except ValueError:
        return None


def crate_directories(metadata: dict, root: Path = ROOT) -> dict:
    """Manifest directory, relative to the checkout, to its report label."""
    directories = {}
    for package in metadata["packages"]:
        relative = relative_directory(package["manifest_path"], root)
        if relative is not None:
            directories[relative] = crate_label(package["name"], relative)
    return directories


def module_key(path: str, crates: dict):
    """`crate::module` for a code path, or None for docs, fixtures and tests.

    A Rust file belongs to the crate whose manifest directory holds its `src/`
    and to the module named by the first component under `src/`. Paths of a
    crate that has since moved keep their directory as the label. Omega-written
    product and library sources use their package directory as the crate and
    the next component as the module.
    """
    if path.endswith(".rs") and "/src/" in path:
        directory, rest = path.split("/src/", 1)
        module = rest.split("/", 1)[0]
        module = module[:-3] if module.endswith(".rs") else module
        return f"{crates.get(directory, directory)}::{module}"
    if path.endswith(".omg") and path.startswith("source/"):
        parts = path.split("/")
        if len(parts) >= 4:
            crate, module = "/".join(parts[1:3]), parts[3]
        elif len(parts) == 3:
            crate, module = parts[1], parts[2]
        else:
            return None
        module = module[:-4] if module.endswith(".omg") else module
        return f"src:{crate}::{module}"
    return None


def crate_of(key: str) -> str:
    return key.split("::", 1)[0]


# History ----------------------------------------------------------------------

class Commit:
    __slots__ = ("sha", "date", "subject", "modules")

    def __init__(self, sha, date, subject, modules):
        self.sha, self.date, self.subject, self.modules = sha, date, subject, modules

    @property
    def repair(self) -> bool:
        return bool(REPAIR.search(self.subject))


def read_history(since: str, until, crates: dict, root: Path = ROOT) -> list:
    arguments = ["log", f"--since={since}", "--no-merges", "--name-only",
                 "--format=@@%H%x09%aI%x09%s"]
    if until:
        arguments.append(f"--until={until}")
    return parse_history(git(*arguments, root=root), crates)


def parse_history(text: str, crates: dict) -> list:
    commits, current = [], None
    for line in text.splitlines():
        if line.startswith("@@"):
            sha, date, subject = line[2:].split("\t", 2)
            current = Commit(sha, dt.datetime.fromisoformat(date.replace("Z", "+00:00")), subject,
                             set())
            commits.append(current)
        elif line and current is not None:
            key = module_key(line, crates)
            if key:
                current.modules.add(key)
    return commits


def co_change(commits: list, max_modules: int) -> tuple:
    """Cross-crate module pairs by shared commits, and the commits left out."""
    pairs, singles, skipped = collections.Counter(), collections.Counter(), 0
    for commit in commits:
        modules = sorted(commit.modules)
        if len(modules) > max_modules:
            skipped += 1
            continue
        singles.update(modules)
        for index, left in enumerate(modules):
            for right in modules[index + 1:]:
                if crate_of(left) != crate_of(right):
                    pairs[(left, right)] += 1
    return pairs, singles, skipped


def repair_hot_spots(commits: list) -> list:
    """Per module: (repairs, all commits, repairs per week, repairs in the
    last 7 days, module), most repairs first."""
    if not commits:
        return []
    newest = max(commit.date for commit in commits)
    oldest = min(commit.date for commit in commits)
    weeks = max(1.0, (newest - oldest).total_seconds() / (7 * 86400))
    recent_start = newest - dt.timedelta(days=7)
    total, repairs, recent = collections.Counter(), collections.Counter(), collections.Counter()
    for commit in commits:
        for module in commit.modules:
            total[module] += 1
            if commit.repair:
                repairs[module] += 1
                if commit.date > recent_start:
                    recent[module] += 1
    rows = [(repairs[module], total[module], repairs[module] / weeks, recent[module], module)
            for module in total if repairs[module]]
    rows.sort(key=lambda row: (-row[0], row[4]))
    return rows


# Rust clones --------------------------------------------------------------------

RUST_TOKEN = re.compile(
    r"//[^\n]*|/\*.*?\*/"
    r"|b?r(#*)\".*?\"\1"
    r"|b?\"(?:\\.|[^\"\\])*\""
    r"|b?'(?:\\.|[^'\\])'"
    r"|'[A-Za-z_]\w*"
    r"|r#[A-Za-z_]\w*|[A-Za-z_]\w*"
    r"|\d[\w.]*"
    r"|::|->|=>|==|!=|<=|>=|&&|\|\||\.\.=|\.\.\.|\.\.|<<=|>>=|[-+*/%^&|]="
    r"|\S", re.S)
RUST_KEYWORDS = frozenset(
    "as async await break const continue crate dyn else enum extern false fn for if impl "
    "in let loop match mod move mut pub ref return self Self static struct super trait "
    "true type unsafe use where while".split())
PRIMITIVE_TYPES = frozenset(
    "u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize f32 f64 bool char str".split())
SHINGLE_WIDTH = 8
SAMPLE = 4
MAX_SHARING_FILES = 4


def literal(token: str) -> bool:
    return token.startswith(('"', 'b"', "r\"", "r#\"", "br\"", "br#\"", "b'"))


def erased_tokens(text: str) -> list:
    """Rust tokens with comments dropped and local names and literals erased.

    Type names (capitalized or primitive), keywords, called names and names reached through
    `.` or `::` stay, so two implementations of one mechanism that call the
    same helpers and match the same variants share shingles even when their
    locals and literals differ."""
    raw = [token for token in (match.group(0) for match in RUST_TOKEN.finditer(text))
           if not token.startswith(("//", "/*"))]
    tokens = []
    for index, token in enumerate(raw):
        first = token[0]
        if literal(token):
            tokens.append("s")
        elif first == "'":
            tokens.append("'")
        elif first.isdigit():
            tokens.append("0")
        elif first.isalpha() or first == "_":
            name = token[2:] if token.startswith("r#") else token
            following = raw[index + 1] if index + 1 < len(raw) else ""
            preceding = raw[index - 1] if index else ""
            if (name in RUST_KEYWORDS or name in PRIMITIVE_TYPES or name[0].isupper() or following in ("(", "::", "!")
                    or preceding in (".", "::")):
                tokens.append(name)
            else:
                tokens.append("x")
        else:
            tokens.append(token)
    return tokens


def shingles(tokens: list, width: int = SHINGLE_WIDTH, sample: int = SAMPLE) -> frozenset:
    found = set()
    for index in range(len(tokens) - width + 1):
        value = zlib.crc32("\x1f".join(tokens[index:index + width]).encode())
        if value % sample == 0:
            found.add(value)
    return frozenset(found)


def rust_sources(root: Path, crates: dict):
    """(module key, path relative to src, text) for each non-test Rust source."""
    base = root / "omega-rust"
    for directory, subdirectories, files in os.walk(base):
        subdirectories[:] = [name for name in subdirectories
                             if name not in ("target", "tests", "benches", "examples")]
        for name in files:
            if not name.endswith(".rs") or name == "tests.rs" or name.endswith("_tests.rs"):
                continue
            path = (Path(directory) / name).relative_to(root).as_posix()
            if "/src/" not in path:
                continue
            key = module_key(path, crates)
            if key is None:
                continue
            yield key, path.split("/src/", 1)[1], (root / path).read_text(
                encoding="utf-8", errors="replace")


def clone_pairs(files: dict, max_sharing: int = MAX_SHARING_FILES) -> list:
    """File pairs in different crates by shingles shared only among few files.

    `files` maps (module key, file) to its shingle set. Returns rows of
    (shared, containment, left, right) where containment is shared divided by
    the smaller file's shingle count."""
    frequency = collections.Counter()
    for found in files.values():
        frequency.update(found)
    postings = collections.defaultdict(list)
    for name, found in files.items():
        for value in found:
            if 2 <= frequency[value] <= max_sharing:
                postings[value].append(name)
    shared = collections.Counter()
    for names in postings.values():
        for index, left in enumerate(names):
            for right in names[index + 1:]:
                if crate_of(left[0]) != crate_of(right[0]):
                    shared[tuple(sorted((left, right)))] += 1
    rows = []
    for (left, right), count in shared.items():
        smaller = max(1, min(len(files[left]), len(files[right])))
        rows.append((count, count / smaller, left, right))
    rows.sort(key=lambda row: (-row[0], row[2], row[3]))
    return rows


def module_clone_mass(rows: list) -> list:
    mass, files = collections.Counter(), collections.Counter()
    for count, _, left, right in rows:
        pair = tuple(sorted((left[0], right[0])))
        mass[pair] += count
        files[pair] += 1
    return sorted(((count, files[pair], *pair) for pair, count in mass.items()),
                  key=lambda row: (-row[0], row[2], row[3]))


def same_name_families(rows: list, minimum: int = 3) -> list:
    """File stems found in two or more crates whose files share shingles."""
    families = collections.defaultdict(lambda: [0, set()])
    for count, _, left, right in rows:
        stem = Path(left[1]).stem
        if count < minimum or stem != Path(right[1]).stem or stem in ("mod", "lib"):
            continue
        family = families[stem]
        family[0] += count
        family[1].update((left, right))
    return sorted(((total, stem, sorted(members)) for stem, (total, members) in
                   families.items()), key=lambda row: (-row[0], row[1]))


def rust_clones(root: Path, crates: dict) -> tuple:
    files, lines = {}, collections.Counter()
    for key, relative, text in rust_sources(root, crates):
        lines[key] += text.count("\n")
        # A crate root mostly re-exports and declares modules.
        if not key.endswith(("::lib", "::main")):
            files[(key, relative)] = shingles(erased_tokens(text))
    rows = clone_pairs(files)
    return rows, module_clone_mass(rows), same_name_families(rows), lines


# Crate map --------------------------------------------------------------------

REPRESENTATIONS = {
    "source-files": None,
    "tokens": "tokens",
    "syntax-trees": "syntax-trees",
    "symbol-resolved-trees": "symbol-resolved-trees",
    "typed-trees": "typed-trees",
    "checked-trees": "checked-trees",
    "lowered-psi": "lowered-psi",
    "terminal-psi": "terminal-psi",
    "abstract-operations": "abstract-operations",
    "target-operations": "target-operations",
    "selected-instructions": "selected-instructions",
    "register-homes": "register-homes",
    "post-allocation-machine": "physical-instructions",
    "selected-form-encoding": "machine-code",
    "resolved-layout": "machine-code",
}
# Families nested beneath a representation (pipeline.md "Program families").
REPRESENTATION_FAMILY = {
    "facts": "checked-trees",
    "terminal-codec": "terminal-psi",
    "terminal-semantics": "terminal-psi",
    "terminal-fuel": "terminal-psi",
    "optimization-unit": "abstract-operations",
    "optimization-unit-semantics": "abstract-operations",
    "legalized-operations": "target-operations",
}
ROUTE_ROW = re.compile(r"^\|\s*([^|]+?)\s*\|\s*\[([^\]]+)\]\(([^)]+)\)\s*\|\s*$")


def pipeline_route(text: str) -> list:
    """(input → output, owner crate name, owner path) rows of pipeline.md's
    Connected program route table."""
    rows, inside = [], False
    for line in text.splitlines():
        if line.startswith("## "):
            inside = line.strip() == "## Connected program route"
            continue
        match = ROUTE_ROW.match(line) if inside else None
        if match:
            rows.append((match.group(1), match.group(2), match.group(3)))
    return rows


def header_summary(path: Path) -> str:
    """The first sentence of a crate root's `//!` header."""
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""
    lines = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("//!"):
            content = stripped[3:].strip()
            if not content and lines:
                break
            if content:
                lines.append(content)
        elif stripped.startswith("#!") or not stripped:
            continue
        elif lines or not stripped.startswith("//"):
            break
    summary = " ".join(lines).replace("|", "/")
    match = re.match(r"(.+?[.!?])(\s|$)", summary)
    return (match.group(1) if match else summary)[:160]


def representation_of(crate_name: str, directory: str):
    if "/compilation/representations/" not in directory:
        return None
    return REPRESENTATION_FAMILY.get(crate_name, crate_name)


def stage_ends(crate_name: str):
    """(input, output) representation crate names from an `X-to-Y` name."""
    for source in sorted(REPRESENTATIONS, key=len, reverse=True):
        if crate_name.startswith(source + "-to-"):
            target = crate_name[len(source) + 4:]
            if target in REPRESENTATIONS:
                return REPRESENTATIONS[source], REPRESENTATIONS[target]
    return None


def zone(directory: str) -> str:
    if directory.startswith("omega-rust/psi/compilation/representations/terminal-psi"):
        return "terminal"
    if directory.startswith("omega-rust/psi/foundation"):
        return "psi-foundation"
    if directory.startswith("omega-rust/psi/"):
        return "psi"
    if directory.startswith(("omega-rust/omega/compilation/pipeline",
                             "omega-rust/omega/compilation/representations",
                             "omega-rust/omega/compilation/backend",
                             "omega-rust/omega/compilation/realization")):
        return "omega-compilation"
    return "other"


# Psi crates Omega realization may read: Terminal installation admission and
# the shared mathematical checker (check_native_boundary.py's permitted owners).
OMEGA_READABLE_PSI = ("omega-rust/psi/compilation/checking/terminal-installation",
                      "omega-rust/psi/compilation/proof/admission")


def normal_path_dependencies(package: dict, packages: dict) -> list:
    return [dependency["name"] for dependency in package["dependencies"]
            if dependency.get("path") and dependency["name"] in packages
            and dependency.get("kind") is None]


def crate_map(metadata: dict, pipeline_text: str, root: Path = ROOT) -> dict:
    packages = {}
    for package in metadata["packages"]:
        relative = relative_directory(package["manifest_path"], root)
        if relative is not None:
            packages[package["name"]] = (relative, package)
    route = pipeline_route(pipeline_text)
    route_names = [owner for _, owner, _ in route]
    pipeline_crates = sorted((name for name, (directory, _) in packages.items()
                              if "/compilation/pipeline/" in directory),
                             key=lambda name: packages[name][0])
    stages = []
    for name in pipeline_crates:
        directory, package = packages[name]
        ends = stage_ends(name)
        dependencies = normal_path_dependencies(package, packages)
        representations = sorted({representation_of(dependency, packages[dependency][0])
                                  for dependency in dependencies} - {None})
        own = set(ends or ())
        stages.append({
            "crate": name,
            "label": crate_label(name, directory),
            "input": ends[0] if ends else None,
            "output": ends[1] if ends else None,
            "other_representations": [rep for rep in representations if rep not in own],
            "pipeline_dependencies": [dependency for dependency in dependencies
                                      if "/compilation/pipeline/" in packages[dependency][0]],
            "header": header_summary(root / directory / "src" / "lib.rs"),
        })
    leaks = []
    for name, (directory, package) in sorted(packages.items()):
        if zone(directory) != "omega-compilation":
            continue
        for dependency in normal_path_dependencies(package, packages):
            target = packages[dependency][0]
            if zone(target) == "psi" and not target.startswith(OMEGA_READABLE_PSI):
                leaks.append((name, dependency))
    return {
        "route": route,
        "stages": stages,
        "missing_from_pipeline_md": [name for name in pipeline_crates if name not in route_names],
        "missing_from_workspace": [name for name in route_names if name not in packages
                                   and "-to-" in name],
        "firewall": leaks,
    }


# Route coverage: trace injection ------------------------------------------------

BRACKETS = {"(": ")", "[": "]", "{": "}"}
CLOSERS = {value: key for key, value in BRACKETS.items()}
OPERAND_END = re.compile(r"^(?:[A-Za-z_]\w*|\d[\w.]*|\"|b\"|b'|r#?\"|'.'|'\\|\)|\]|\}|\?)")
LEFT_BOUNDARY = {",", ";", "=", "=>", "|", "return", "if", "while", "match", "in",
                 "else", "break", "+=", "-=", "|=", "&="}
CALL = re.compile(r"((?:[A-Za-z_]\w*(?:::|\.))*[A-Za-z_]\w*!?)\s*\(")


def rust_token_spans(text: str) -> list:
    """(token, start, end) with comments dropped, for source rewriting."""
    return [(match.group(0), match.start(), match.end()) for match in RUST_TOKEN.finditer(text)
            if not match.group(0).startswith(("//", "/*"))]


def matching(tokens: list, start: int) -> int:
    depth = 0
    for position in range(start, len(tokens)):
        token = tokens[position][0]
        if token in BRACKETS:
            depth += 1
        elif token in CLOSERS:
            depth -= 1
            if depth == 0:
                return position
    raise ValueError("unbalanced brackets")


def function_body(tokens: list, name: str) -> tuple:
    """Token index range (open brace, close brace) of `fn name`'s body."""
    for index in range(len(tokens) - 1):
        if tokens[index][0] == "fn" and tokens[index + 1][0] == name:
            depth = 0
            for position in range(index + 2, len(tokens)):
                token = tokens[position][0]
                if token in ("(", "["):
                    depth += 1
                elif token in (")", "]"):
                    depth -= 1
                elif token == "{" and depth == 0:
                    return position, matching(tokens, position)
    raise ValueError(f"no function body for `{name}`")


def is_operand_end(token: str) -> bool:
    if token in RUST_KEYWORDS and token not in ("self", "Self", "true", "false"):
        return False
    return bool(OPERAND_END.match(token))


def rust_string(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def operand_label(text: str) -> str:
    stripped = text.strip()
    call = CALL.search(stripped)
    if stripped.startswith("{"):
        return f"{{{call.group(1)}...}}" if call else "{...}"
    label = call.group(1) if call else stripped.split()[0]
    while label.startswith(("super::", "crate::", "self::")):
        label = label.split("::", 1)[1]
    return label.replace("|", "/").replace("\t", " ")


class Chain:
    def __init__(self, start, end, operands):
        self.start, self.end, self.operands = start, end, operands  # character offsets


def route_chains(text: str, function: str) -> list:
    """`||` chains in `function` with at least two operands that call something.

    Chains inside macro arguments and closure bodies (`|x| a || b`) stay
    untouched: rewriting a macro pattern breaks it, and a closure-argument
    chain is a predicate on one element, not a choice among proof routes."""
    tokens = rust_token_spans(text)
    open_brace, close_brace = function_body(tokens, function)
    partner, stack = {}, []
    for index in range(open_brace, close_brace + 1):
        token = tokens[index][0]
        if token in BRACKETS:
            stack.append(index)
        elif token in CLOSERS and stack:
            partner[stack.pop()] = index
    partner.update({closing: opening for opening, closing in list(partner.items())})

    def left_boundary(position):
        index = position - 1
        while index > open_brace:
            token = tokens[index][0]
            if token in CLOSERS:
                index = partner[index] - 1
                continue
            if token in BRACKETS or token in LEFT_BOUNDARY:
                return index
            index -= 1
        return open_brace

    def right_boundary(position):
        index = position + 1
        while index < close_brace:
            token = tokens[index][0]
            if token == "{" and is_operand_end(tokens[index - 1][0]):
                return index
            if token in BRACKETS:
                index = partner[index] + 1
                continue
            if token in CLOSERS or token in (",", ";", "=>", "else"):
                return index
            index += 1
        return close_brace

    def inside_macro(position):
        # Any enclosing bracket opened by `name!(`.
        index = position
        while index > open_brace:
            index -= 1
            token = tokens[index][0]
            if token in CLOSERS:
                index = partner[index]
                continue
            if token in BRACKETS and index > 0 and tokens[index - 1][0] == "!":
                return True
        return False

    groups = collections.defaultdict(list)
    for index in range(open_brace + 1, close_brace):
        if tokens[index][0] == "||" and is_operand_end(tokens[index - 1][0]):
            groups[left_boundary(index)].append(index)
    chains = []
    for left, operators in sorted(groups.items()):
        if tokens[left][0] == "|" or inside_macro(operators[0]):
            continue
        right = right_boundary(operators[-1])
        cuts = [left, *operators, right]
        operands = [(tokens[begin + 1][1], tokens[finish - 1][2])
                    for begin, finish in zip(cuts, cuts[1:])]
        calls = sum(1 for start, end in operands if CALL.search(text[start:end]))
        if calls >= 2:
            chains.append(Chain(operands[0][0], operands[-1][1], operands))
    return chains


LET_SOME = re.compile(r"let\s+Some\s*\((?:[^()]|\([^()]*\))*\)\s*=\s*(.*)\Z", re.S)


def fallback_routes(text: str, function: str, chains: list) -> list:
    """`if !proved && ... let Some(..) = route(..)` after `let proved = <chain>`.

    Returns (if offset, label, condition) where the condition, with `!proved`
    dropped and the `let Some` binding turned into `.is_some()`, is the
    fallback route, evaluated unconditionally just before the `if`."""
    tokens = rust_token_spans(text)
    open_brace, close_brace = function_body(tokens, function)
    decided = set()
    for index in range(open_brace, close_brace - 2):
        if tokens[index][0] == "let" and tokens[index + 2][0] == "=":
            start, end = tokens[index][1], text.find(";", tokens[index + 2][2])
            if any(start <= chain.start and chain.end <= end for chain in chains):
                decided.add(tokens[index + 1][0])
    found = []
    for index in range(open_brace, close_brace - 2):
        if not (tokens[index][0] == "if" and tokens[index + 1][0] == "!"
                and tokens[index + 2][0] in decided):
            continue
        depth, position, conjuncts, current = 0, index + 3, [], []
        while position < close_brace:
            token = tokens[position][0]
            if token == "{" and depth == 0 and is_operand_end(tokens[position - 1][0]):
                break
            if token in BRACKETS:
                depth += 1
            elif token in CLOSERS:
                depth -= 1
            if token == "&&" and depth == 0:
                if current:
                    conjuncts.append(current)
                current = []
            else:
                current.append(position)
            position += 1
        if current:
            conjuncts.append(current)
        pieces, label = [], None
        for conjunct in conjuncts:
            piece = text[tokens[conjunct[0]][1]:tokens[conjunct[-1]][2]]
            binding = LET_SOME.match(piece)
            if binding:
                label = operand_label(binding.group(1))
                piece = f"{binding.group(1)}.is_some()"
            pieces.append(f"({piece})")
        if label:
            found.append((tokens[index][1], label, " && ".join(pieces)))
    return found


TRACE_MODULE = r'''

#[doc(hidden)]
#[allow(dead_code, clippy::all)]
pub(crate) mod __route_trace {
    // Injected by tools/architecture_signals.py --coverage; never committed.
    use std::cell::{Cell, RefCell};
    thread_local! {
        static FRAMES: RefCell<Vec<(u64, u32)>> = const { RefCell::new(Vec::new()) };
        static NEXT: Cell<u64> = const { Cell::new(1) };
        static LAST_ROOT: Cell<u64> = const { Cell::new(0) };
    }
    fn thread() -> String {
        format!("{:?}", std::thread::current().id())
    }
    pub(crate) fn begin() -> (u64, u64, u32) {
        let sequence = NEXT.with(|next| {
            let value = next.get();
            next.set(value + 1);
            value
        });
        let parent = FRAMES.with(|frames| frames.borrow().last().copied().unwrap_or((0, 0)));
        FRAMES.with(|frames| frames.borrow_mut().push((sequence, 0)));
        (sequence, parent.0, parent.1)
    }
    pub(crate) fn operand(index: u32) {
        FRAMES.with(|frames| {
            if let Some(top) = frames.borrow_mut().last_mut() {
                top.1 = index;
            }
        });
    }
    pub(crate) fn end(frame: (u64, u64, u32), site: &str, chain: u32, labels: &str,
                      bits: &[bool]) -> bool {
        FRAMES.with(|frames| {
            frames.borrow_mut().pop();
        });
        if frame.1 == 0 {
            LAST_ROOT.with(|root| root.set(frame.0));
        }
        let text: String = bits.iter().map(|bit| if *bit { '1' } else { '0' }).collect();
        eprintln!("OMEGA_ROUTE_TRACE\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", thread(), frame.0,
                  frame.1, frame.2, site, chain, labels, text);
        bits.iter().any(|bit| *bit)
    }
    pub(crate) fn last_root() -> u64 {
        LAST_ROOT.with(|root| root.get())
    }
    pub(crate) fn fallback(site: &str, label: &str, root: u64, proved: bool) {
        eprintln!("OMEGA_ROUTE_TRACE\t{}\t0\t{}\tF\t{}\tfallback\t{}\t{}", thread(), root, site,
                  label, if proved { '1' } else { '0' });
    }
}
'''


def instrument(text: str, function: str, site: str) -> tuple:
    """Source with `function`'s route chains evaluating every operand and
    tracing, plus the number of chains and fallbacks rewritten."""
    chains = route_chains(text, function)
    fallbacks = fallback_routes(text, function, chains)
    numbers = {id(chain): number for number, chain in enumerate(chains)}
    marks = []  # (offset, inserted text) for fallbacks, applied while rendering

    for start, label, condition in fallbacks:
        marks.append((start, f"crate::__route_trace::fallback({rust_string(site)}, "
                             f"{rust_string(label)}, crate::__route_trace::last_root(), "
                             f"{condition});\n            "))

    def render(low: int, high: int) -> str:
        inside = [chain for chain in chains if low <= chain.start and chain.end <= high]
        top = [chain for chain in inside if not any(
            other is not chain and other.start <= chain.start and chain.end <= other.end
            for other in inside)]
        events = sorted([(chain.start, 1, chain) for chain in top]
                        + [(offset, 0, inserted) for offset, inserted in marks
                           if low <= offset < high
                           and not any(chain.start <= offset < chain.end for chain in top)],
                        key=lambda event: (event[0], event[1]))
        pieces, cursor = [], low
        for offset, kind, item in events:
            pieces.append(text[cursor:offset])
            cursor = offset
            if kind == 0:
                pieces.append(item)
                continue
            chain = item
            operands = [render(start, end) for start, end in chain.operands]
            labels = "|".join(operand_label(text[start:end]) for start, end in chain.operands)
            body = ", ".join(f"{{ crate::__route_trace::operand({index}); ({operand}) }}"
                             for index, operand in enumerate(operands))
            pieces.append(
                f"{{ let __route_frame = crate::__route_trace::begin(); "
                f"let __route_bits: [bool; {len(operands)}] = [{body}]; "
                f"crate::__route_trace::end(__route_frame, {rust_string(site)}, "
                f"{numbers[id(chain)]}, {rust_string(labels)}, &__route_bits) }}")
            cursor = chain.end
        pieces.append(text[cursor:high])
        return "".join(pieces)

    return render(0, len(text)), len(chains), len(fallbacks)


def crate_root_of(path: Path) -> Path:
    for parent in path.parents:
        if (parent / "Cargo.toml").is_file():
            return parent / "src" / "lib.rs"
    raise ValueError(f"no crate for {path}")


def apply_instrumentation(worktree: Path, sites: list) -> list:
    """Rewrite each `FILE::FUNCTION` site in `worktree`; returns per-site counts."""
    by_file = collections.defaultdict(list)
    for site in sites:
        file, function = site.rsplit("::", 1)
        by_file[file].append(function)
    results, roots = [], set()
    for file, functions in by_file.items():
        path = worktree / file
        text = path.read_text(encoding="utf-8")
        for function in functions:
            text, chains, fallbacks = instrument(text, function, function)
            results.append((file, function, chains, fallbacks))
        path.write_text(text, encoding="utf-8")
        roots.add(crate_root_of(path))
    for root in roots:
        text = root.read_text(encoding="utf-8")
        if "mod __route_trace" not in text:
            root.write_text(text + TRACE_MODULE, encoding="utf-8")
    return results


# Route coverage: analysis --------------------------------------------------------

class Record:
    __slots__ = ("thread", "sequence", "parent", "operand", "site", "chain", "labels", "bits")

    def __init__(self, fields):
        (self.thread, sequence, parent, self.operand, self.site, self.chain, labels,
         bits) = fields
        self.sequence, self.parent = int(sequence), int(parent)
        self.labels = labels.split("|")
        self.bits = [bit == "1" for bit in bits]


def parse_trace(text: str) -> list:
    records = []
    for line in text.splitlines():
        if line.startswith(TRACE_PREFIX):
            fields = line[len(TRACE_PREFIX):].split("\t")
            if len(fields) == 8:
                records.append(Record(fields))
    return records


def decisions(records: list) -> list:
    """(root record, {route label: proved}) per root chain evaluation.

    An operand whose evaluation ran a nested instrumented chain is replaced by
    that chain's operands when its value equals the nested chain's result (the
    operand is the chain, possibly behind a guard that held). A fallback row
    joins the root evaluation that preceded it on its thread."""
    children = collections.defaultdict(list)
    roots, fallbacks = [], []
    for record in records:
        if record.chain == "fallback":
            fallbacks.append(record)
        elif record.parent == 0:
            roots.append(record)
        else:
            children[(record.thread, record.parent, int(record.operand))].append(record)

    def leaves(record):
        found = {}
        for index, (label, bit) in enumerate(zip(record.labels, record.bits)):
            nested = [child for child in children.get((record.thread, record.sequence, index), [])
                      if any(child.bits) == bit]
            if nested:
                for child in nested:
                    for name, value in leaves(child).items():
                        found[name] = found.get(name, False) or value
            else:
                found[label] = found.get(label, False) or bit
        return found

    result = {(root.thread, root.sequence): (root, leaves(root)) for root in roots}
    for record in fallbacks:
        entry = result.get((record.thread, record.parent))
        if entry is not None:
            entry[1][record.labels[-1]] = entry[1].get(record.labels[-1], False) or record.bits[-1]
    return list(result.values())


def route_table(fixture_records: dict) -> list:
    """Per (site, route): decisions, proofs, sole proofs and verdict."""
    stats = collections.defaultdict(lambda: {"evaluated": 0, "proves": 0, "sole": 0,
                                             "fixtures": set(), "sole_fixtures": set()})
    for fixture, records in fixture_records.items():
        for root, routes in decisions(records):
            proved = [name for name, bit in routes.items() if bit]
            for name, bit in routes.items():
                row = stats[(root.site, name)]
                row["evaluated"] += 1
                row["fixtures"].add(fixture)
                if bit:
                    row["proves"] += 1
                    if len(proved) == 1:
                        row["sole"] += 1
                        row["sole_fixtures"].add(fixture)
    rows = []
    for (site, name), row in sorted(stats.items()):
        verdict = ("dead" if row["proves"] == 0 else
                   "redundant" if row["sole"] == 0 else "needed")
        rows.append({"site": site, "route": name, "evaluated": row["evaluated"],
                     "proves": row["proves"], "sole": row["sole"],
                     "fixtures": len(row["fixtures"]),
                     "sole_fixtures": len(row["sole_fixtures"]), "verdict": verdict})
    return rows


# Route coverage: running ---------------------------------------------------------

def read_witnesses(path: Path) -> list:
    witnesses = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            witnesses.append(line)
    return witnesses


def default_coverage_worktree() -> Path:
    common = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir").strip())
    return common.parent / ".agents" / "worktree" / "signals-coverage"


def prepare_worktree(worktree: Path, revision: str) -> str:
    revision = git("rev-parse", "--verify", f"{revision}^{{commit}}").strip()
    if not (worktree / ".git").exists():
        subprocess.run(["git", "worktree", "add", "--detach", str(worktree), revision],
                       cwd=ROOT, check=True, capture_output=True)
    git("checkout", "--quiet", "--", ".", root=worktree)
    git("checkout", "--quiet", "--detach", revision, root=worktree)
    return git("rev-parse", "HEAD", root=worktree).strip()


def build_compiler(worktree: Path) -> Path:
    builder = "mbx" if shutil.which("mbx") else "cargo"
    command = [sys.executable, str(worktree / "tools" / "build_slot.py"), "--timeout", "3600",
               "--", builder, "build", "-p", "omega", "--bin", "omega"]
    subprocess.run(command, cwd=worktree, check=True, stdout=sys.stderr)
    binary = worktree / "target" / "debug" / ("omega.exe" if os.name == "nt" else "omega")
    if not binary.is_file():
        raise SystemExit(f"architecture_signals: no compiler at {binary}")
    return binary


def run_witness(binary: Path, corpus: Path, fixture: str, timeout: int) -> tuple:
    with tempfile.TemporaryDirectory(prefix="signals-home-") as home:
        environment = dict(os.environ, HOME=home)
        started = time.monotonic()
        try:
            completed = subprocess.run([str(binary), "--check", "main.omg"],
                                       cwd=corpus / fixture, env=environment,
                                       capture_output=True, text=True, timeout=timeout)
            code, output = completed.returncode, completed.stderr + completed.stdout
        except subprocess.TimeoutExpired as error:
            code, output = "timeout", error.stderr or ""
            if isinstance(output, bytes):
                output = output.decode("utf-8", "replace")
        return fixture, code, parse_trace(output), time.monotonic() - started


def route_coverage(arguments) -> dict:
    witnesses = read_witnesses(Path(arguments.witnesses))
    print(f"route coverage: {len(witnesses)} witness fixtures", file=sys.stderr)
    if len(witnesses) > MAX_WITNESSES:
        raise SystemExit(f"architecture_signals: {len(witnesses)} witnesses is a sweep; "
                         f"narrow the set to {MAX_WITNESSES} or fewer")
    worktree = Path(arguments.coverage_worktree or default_coverage_worktree()).resolve()
    started = time.monotonic()
    revision = prepare_worktree(worktree, arguments.coverage_rev)
    sites = arguments.route_site or list(DEFAULT_ROUTE_SITES)
    instrumented = apply_instrumentation(worktree, sites)
    for _, function, chains, fallbacks in instrumented:
        print(f"route coverage: {function}: {chains} chains, {fallbacks} fallbacks",
              file=sys.stderr)
    binary = build_compiler(worktree)
    built = time.monotonic()
    corpus = worktree / "tests" / "omega"
    missing = [fixture for fixture in witnesses if not (corpus / fixture / "main.omg").is_file()]
    present = [fixture for fixture in witnesses if fixture not in missing]
    records, outcomes = {}, {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=arguments.jobs) as pool:
        for fixture, code, trace, seconds in pool.map(
                lambda fixture: run_witness(binary, corpus, fixture, arguments.timeout),
                present):
            records[fixture] = trace
            outcomes[fixture] = (code, len(trace), round(seconds, 1))
    finished = time.monotonic()
    git("checkout", "--quiet", "--", ".", root=worktree)
    return {
        "revision": revision,
        "sites": sites,
        "instrumented": instrumented,
        "witnesses": len(witnesses),
        "missing": missing,
        "reached": sum(1 for trace in records.values() if trace),
        "accepted": sum(1 for fixture, trace in records.items()
                        if trace and outcomes[fixture][0] == 0),
        "outcomes": outcomes,
        # A witness the compiler refuses decides no accepted fact, so only
        # accepted witnesses count, as in the 2,742-fixture reverse sweep.
        "routes": route_table({fixture: trace for fixture, trace in records.items()
                               if outcomes[fixture][0] == 0}),
        "build_seconds": round(built - started),
        "run_seconds": round(finished - built),
    }


# Report -----------------------------------------------------------------------

def table(headers: list, rows: list) -> str:
    lines = ["| " + " | ".join(headers) + " |", "|" + "---|" * len(headers)]
    lines += ["| " + " | ".join(str(cell) for cell in row) + " |" for row in rows]
    return "\n".join(lines)


def report(arguments) -> dict:
    timings, data = {}, {}
    started = time.monotonic()
    metadata = cargo_metadata()
    crates = crate_directories(metadata)
    commits = read_history(arguments.since, arguments.until, crates)
    timings["history"] = time.monotonic() - started
    pairs, singles, skipped = co_change(commits, arguments.max_modules)
    hot = repair_hot_spots(commits)
    top = arguments.top
    print(f"# Architecture signals at {git('rev-parse', '--short', 'HEAD').strip()}\n")
    print(f"{len(commits)} commits since {arguments.since}, "
          f"{sum(1 for commit in commits if commit.repair)} with a preserve-type verb.\n")

    print("## Co-change: module pairs in different crates changed together\n")
    print(f"{skipped} commits touching more than {arguments.max_modules} modules left out.\n")
    rows = [(count, left, right, f"{count / min(singles[left], singles[right]):.2f}")
            for (left, right), count in pairs.most_common(top)]
    print(table(["commits", "module", "module", "share of smaller"], rows), "\n")
    data["co_change"] = [{"commits": count, "left": left, "right": right}
                         for (left, right), count in pairs.most_common()]

    started = time.monotonic()
    clone_rows, mass, families, lines = rust_clones(ROOT, crates)
    timings["clones"] = time.monotonic() - started

    print("## Repair hot spots: commits with a preserve-type verb\n")
    rows = [(module, lines.get(module, ""), f"{repairs} / {total}", f"{weekly:.1f}", recent)
            for repairs, total, weekly, recent, module in hot[:top]]
    print(table(["module", "Rust lines", "repair / all commits", "repairs per week",
                 "last 7 days"], rows), "\n")
    data["repair_hot_spots"] = [
        {"module": module, "repairs": repairs, "commits": total, "per_week": weekly,
         "last_7_days": recent} for repairs, total, weekly, recent, module in hot]

    print("## Rust clones\n")
    print("Module pairs in different crates by shared rare shingles (clone mass):\n")
    print(table(["mass", "file pairs", "module", "module"], mass[:top]), "\n")
    print("File pairs:\n")
    rows = [(count, f"{containment:.2f}", f"{left[0]} {left[1]}", f"{right[0]} {right[1]}")
            for count, containment, left, right in clone_rows[:top]]
    print(table(["shared", "containment", "file", "file"], rows), "\n")
    if arguments.focus:
        focus = re.compile(arguments.focus)
        print(f"File pairs whose path matches `{arguments.focus}`:\n")
        rows = [(count, f"{containment:.2f}", f"{left[0]} {left[1]}", f"{right[0]} {right[1]}")
                for count, containment, left, right in clone_rows
                if focus.search(left[1]) or focus.search(right[1])][:top]
        print(table(["shared", "containment", "file", "file"], rows), "\n")
    print("Same-named files in several crates that share shingles:\n")
    rows = [(total, stem, "; ".join(f"{key} {path}" for key, path in members))
            for total, stem, members in families[:top]]
    print(table(["shared", "file stem", "homes"], rows), "\n")
    data["clones"] = {
        "file_pairs": [{"shared": count, "containment": containment, "left": list(left),
                        "right": list(right)} for count, containment, left, right in clone_rows],
        "module_mass": [list(row) for row in mass],
        "families": [{"shared": total, "stem": stem,
                      "homes": [list(member) for member in members]}
                     for total, stem, members in families],
    }

    started = time.monotonic()
    pipeline_text = (ROOT / "omega-rust" / "pipeline.md").read_text(encoding="utf-8")
    mapping = crate_map(metadata, pipeline_text)
    timings["crate_map"] = time.monotonic() - started
    print("## Crate map\n")
    print(f"pipeline.md Connected program route: {len(mapping['route'])} rows. "
          f"Pipeline crates missing from it: {mapping['missing_from_pipeline_md'] or 'none'}. "
          f"Stage rows naming no workspace crate: "
          f"{mapping['missing_from_workspace'] or 'none'}.\n")
    rows = [(stage["label"], stage["input"] or "?", stage["output"] or "?",
             ", ".join(stage["other_representations"]),
             ", ".join(stage["pipeline_dependencies"]), stage["header"])
            for stage in mapping["stages"]]
    print(table(["stage", "input", "output", "also reads", "depends on stages", "lib.rs header"],
                rows), "\n")
    print("Omega compilation crates depending on source-side Psi crates:\n")
    print((table(["crate", "depends on"], mapping["firewall"]) if mapping["firewall"]
           else "none"), "\n")
    data["crate_map"] = mapping

    if arguments.coverage:
        started = time.monotonic()
        coverage = route_coverage(arguments)
        timings["coverage"] = time.monotonic() - started
        print("## Route coverage\n")
        print(f"Revision {coverage['revision'][:11]}; {coverage['witnesses']} witnesses, "
              f"{len(coverage['missing'])} missing at that revision, {coverage['reached']} "
              f"reached an instrumented site, {coverage['accepted']} of them checked with "
              f"exit 0 and are counted. Build {coverage['build_seconds']} s, runs "
              f"{coverage['run_seconds']} s.\n")
        rows = [(row["site"], row["route"], row["evaluated"], row["proves"], row["sole"],
                 row["sole_fixtures"], row["verdict"]) for row in coverage["routes"]]
        print(table(["site", "route", "decisions", "proves", "sole proof", "sole fixtures",
                     "verdict"], rows), "\n")
        data["coverage"] = coverage

    print("Timings: " + ", ".join(f"{name} {seconds:.0f} s" for name, seconds in
                                  timings.items()))
    data["timings"] = timings
    return data


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    parser.add_argument("--since", default=DEFAULT_SINCE)
    parser.add_argument("--until")
    parser.add_argument("--top", type=int, default=25)
    parser.add_argument("--max-modules", type=int, default=15,
                        help="leave out commits touching more modules (mass migrations)")
    parser.add_argument("--focus", help="also list clone file pairs whose path matches this "
                                        "regular expression, such as `custody`")
    parser.add_argument("--json", help="write raw rows here (measurement data, not for git)")
    parser.add_argument("--coverage", action="store_true",
                        help="build an instrumented compiler and trace proof routes")
    parser.add_argument("--witnesses", help="fixture list, relative to tests/omega")
    parser.add_argument("--coverage-rev", default="HEAD")
    parser.add_argument("--coverage-worktree",
                        help="scratch worktree (default <checkout>/.agents/worktree/"
                             "signals-coverage)")
    parser.add_argument("--route-site", action="append",
                        help="FILE::FUNCTION whose `||` route chains are traced")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=300)
    arguments = parser.parse_args(arguments)
    if arguments.coverage and not arguments.witnesses:
        parser.error("--coverage needs --witnesses")
    data = report(arguments)
    if arguments.json:
        Path(arguments.json).write_text(json.dumps(data, indent=1, default=list),
                                        encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
