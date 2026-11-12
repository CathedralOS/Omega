#!/usr/bin/env python3
"""
Cross-crate recursive-walker discovery and Omega source depth measurement.

Measured pin: 0835b3aaf803ed97f905be42d94727ff58459263
Output: recursive_walkers_2026-11-12_depth.json (depths), updates report via render.
"""

from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

REPO = Path(__file__).resolve().parents[3]

STAGE_CRATES: dict[str, list[Path]] = {
    "01": [REPO / "omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees"],
    "02": [REPO / "omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees"],
    "03": [REPO / "omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees"],
    "04": [REPO / "omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees"],
    "05": [
        REPO / "omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees",
        REPO / "omega-rust/psi/compilation/validation",
    ],
    "06": [REPO / "omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi"],
}

OMG_ROOTS = [
    REPO / "tests/omega",
    REPO / "samples",
    REPO / "source/library",
    REPO / "source/psi",
]

DEPTH_TEST_PATH_MARKERS = (
    "fail/parser/nesting_exceeds_max_depth",
    "pass/parser/deep_nesting_within_limit",
    "pass/expressions/deep_paren_nesting",
)

BIN_OPS = [
    "||",
    "&&",
    "==",
    "!=",
    "<=",
    ">=",
    "<<",
    ">>",
    "+",
    "-",
    "*",
    "/",
    "%",
    "&",
    "|",
    "^",
    "<",
    ">",
]

FN_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+|pub\(crate\)\s+|async\s+)*"
    r"fn\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)",
    re.MULTILINE,
)

USE_RE = re.compile(
    r"^\s*use\s+(?P<path>[\w:]+)(?:\s+as\s+(?P<alias>[a-zA-Z_][a-zA-Z0-9_]*))?\s*;",
    re.MULTILINE,
)

AST_TOUCH = re.compile(
    r"SyntaxTrees|SymbolResolved|TypedTrees|CheckedTrees|LoweredPsi|"
    r"ExpressionHandle|StatementHandle|PatternHandle|TypeReferenceHandle|"
    r"ExpressionNode|StatementNode|PatternNode|TransitionTarget|"
    r"Input<'tokens|deepen\(\)|expression_table|statement_table"
)

DEPTH_BOUND_DROP = re.compile(
    r"(?:depth|len|level)\s*>\s*(?:128|64|32|16|8)\b|"
    r"max_depth:\s*usize|projections\.len\(\)\s*>=\s*max_depth"
)


@dataclass
class Function:
    stage: str
    crate: str
    file: Path
    line: int
    name: str
    body: str
    imports: dict[str, str] = field(default_factory=dict)
    callees: set[str] = field(default_factory=set)

    @property
    def fid(self) -> str:
        rel = self.file.relative_to(REPO)
        return f"{self.stage}:{rel}:{self.name}"


def strip_comments_strings(text: str) -> str:
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        if text.startswith("//", i):
            while i < n and text[i] != "\n":
                i += 1
        elif text.startswith("/*", i):
            i += 2
            d = 1
            while i < n and d:
                if text.startswith("/*", i):
                    d += 1
                    i += 2
                elif text.startswith("*/", i):
                    d -= 1
                    i += 2
                else:
                    i += 1
        elif text[i] == '"':
            i += 1
            while i < n and text[i] != '"':
                if text[i] == "\\":
                    i += 2
                else:
                    i += 1
            if i < n:
                i += 1
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def lex_tokens(text: str) -> list[tuple[str, int]]:
    """Return (token, start_offset) excluding comments/strings."""
    bare = strip_comments_strings(text)
    tokens: list[tuple[str, int]] = []
    i = 0
    while i < len(bare):
        if bare[i].isspace():
            i += 1
            continue
        if bare.startswith("::", i):
            tokens.append(("::", i))
            i += 2
            continue
        for op in sorted(BIN_OPS, key=len, reverse=True):
            if bare.startswith(op, i):
                tokens.append((op, i))
                i += len(op)
                break
        else:
            if bare[i] in "()[]{}":
                tokens.append((bare[i], i))
                i += 1
                continue
            if bare[i].isalpha() or bare[i] == "_":
                j = i + 1
                while j < len(bare) and (bare[j].isalnum() or bare[j] == "_"):
                    j += 1
                tokens.append((bare[i:j], i))
                i = j
                continue
            i += 1
    return tokens


def max_paren_in_text(text: str) -> int:
    d = best = 0
    for ch in strip_comments_strings(text):
        if ch == "(":
            d += 1
            best = max(best, d)
        elif ch == ")":
            d = max(0, d - 1)
    return best


def max_angle_in_text(text: str) -> int:
    d = best = 0
    i = 0
    text = strip_comments_strings(text)
    while i < len(text):
        if text.startswith("<<", i) or text.startswith(">>", i):
            i += 2
            continue
        if text[i] == "<":
            d += 1
            best = max(best, d)
        elif text[i] == ">":
            d = max(0, d - 1)
        i += 1
    return best


def max_binary_chain_in_parens(text: str) -> int:
    """Parse-lexical: within each (...) region, count binary ops (longest sum of segments)."""
    tokens = lex_tokens(text)
    best = 0
    stack_paren_start: list[int] = []
    seg_ops = 0
    region_best: dict[int, int] = defaultdict(int)

    def flush(seg_start: int) -> None:
        nonlocal seg_ops, best
        if stack_paren_start:
            p = stack_paren_start[-1]
            region_best[p] = max(region_best[p], seg_ops)
        seg_ops = 0

    for tok, _ in tokens:
        if tok == "(":
            stack_paren_start.append(len(stack_paren_start))
            seg_ops = 0
        elif tok == ")":
            if stack_paren_start:
                p = stack_paren_start.pop()
                region_best[p] = max(region_best[p], seg_ops)
                seg_ops = 0
        elif tok in BIN_OPS and stack_paren_start:
            seg_ops += 1
        elif tok == "::":
            continue
        elif tok not in "()[]{}":
            flush(0)
    for v in region_best.values():
        best = max(best, v)
    return best


def max_prefix_chain(text: str) -> int:
    tokens = lex_tokens(text)
    best = cur = 0
  # scan inside paren groups only for prefix at operand start
    paren = 0
    at_start = True
    for tok, _ in tokens:
        if tok == "(":
            paren += 1
            at_start = True
            cur = 0
        elif tok == ")":
            paren = max(0, paren - 1)
            best = max(best, cur)
            cur = 0
            at_start = True
        elif paren > 0 and at_start and tok in ("~", "&", "*", "!"):
            cur += 1
            best = max(best, cur)
        elif tok not in BIN_OPS and tok not in "()":
            at_start = False
    return best


def is_depth_test_fixture(rel: str) -> bool:
    return any(m in rel for m in DEPTH_TEST_PATH_MARKERS)


def measure_omg_files() -> dict:
    keys = ("paren", "angle", "binary_chain", "prefix_chain")
    all_max = {k: 0 for k in keys}
    excl_max = {k: 0 for k in keys}
    all_file = {k: "" for k in keys}
    excl_file = {k: "" for k in keys}
    count = 0
    for root in OMG_ROOTS:
        if not root.is_dir():
            continue
        for path in root.rglob("*.omg"):
            count += 1
            text = path.read_text(encoding="utf-8", errors="replace")
            rel = str(path.relative_to(REPO))
            m = {
                "paren": max_paren_in_text(text),
                "angle": max_angle_in_text(text),
                "binary_chain": max_binary_chain_in_parens(text),
                "prefix_chain": max_prefix_chain(text),
            }
            for k in keys:
                if m[k] > all_max[k]:
                    all_max[k] = m[k]
                    all_file[k] = rel
                if not is_depth_test_fixture(rel) and m[k] > excl_max[k]:
                    excl_max[k] = m[k]
                    excl_file[k] = rel
    return {
        "file_count": count,
        "all": {"max": all_max, "file": all_file},
        "excluding_depth_fixtures": {"max": excl_max, "file": excl_file},
        "fixture_paths": list(DEPTH_TEST_PATH_MARKERS),
        "parser_test_pins": read_parser_test_pins(),
    }


def read_parser_test_pins() -> dict:
    tests = REPO / "omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/tests.rs"
    text = tests.read_text(encoding="utf-8")
    unary = None
    m = re.search(r'"~"\.repeat\((\d+)\)', text)
    if m:
        unary = int(m.group(1))
    binary_groups = None
    m = re.search(r'repeat_n\("a \| b \^ c & d << e \+ f \* g", (\d+)\)', text)
    if m:
        binary_groups = int(m.group(1))
    return {
        "unary_tilde_repeat": unary,
        "binary_seven_op_groups": binary_groups,
        "note": "Rust integration tests in stage-01 crate, not .omg corpus",
    }


def parse_imports(source: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for m in USE_RE.finditer(source):
        path = m.group("path")
        alias = m.group("alias") or path.split("::")[-1]
        out[alias] = path
    return out


def extract_functions(stage: str, crate_dir: Path) -> list[Function]:
    crate = crate_dir.name
    out: list[Function] = []
    src = crate_dir / "src"
    if not src.is_dir():
        return out
    for path in sorted(src.rglob("*.rs")):
        if path.name == "tests.rs" or "/tests/" in str(path):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        imports = parse_imports(text)
        matches = list(FN_RE.finditer(text))
        for idx, m in enumerate(matches):
            start = m.end()
            end = matches[idx + 1].start() if idx + 1 < len(matches) else len(text)
            body = text[start:end]
            name = m.group("name")
            line = text[: m.start()].count("\n") + 1
            callees = set(re.findall(r"\b(?:Self::)?([a-zA-Z_][a-zA-Z0-9_]*)\s*\(", body))
            out.append(
                Function(
                    stage=stage,
                    crate=crate,
                    file=path,
                    line=line,
                    name=name,
                    body=body,
                    imports=imports,
                    callees=callees,
                )
            )
    return out


def build_stage_graph(stage: str, crates: list[Path]) -> tuple[dict[str, Function], dict[str, set[str]]]:
    functions: dict[str, Function] = {}
    by_name: dict[str, list[str]] = defaultdict(list)
    for crate_dir in crates:
        for fn in extract_functions(stage, crate_dir):
            functions[fn.fid] = fn
            by_name[fn.name].append(fn.fid)

    graph: dict[str, set[str]] = {fid: set() for fid in functions}
    for fid, fn in functions.items():
        if re.search(rf"\b{re.escape(fn.name)}\s*\(", fn.body):
            graph[fid].add(fid)
        for callee in fn.callees:
            targets = by_name.get(callee, [])
            if not targets and callee in fn.imports:
                leaf = fn.imports[callee].split("::")[-1]
                targets = by_name.get(leaf, [])
            for t in targets:
                if t != fid:
                    graph[fid].add(t)
    return functions, graph


def tarjan(graph: dict[str, set[str]]) -> list[list[str]]:
    index = 0
    stack: list[str] = []
    on: set[str] = set()
    idx: dict[str, int] = {}
    low: dict[str, int] = {}
    out: list[list[str]] = []

    def strong(v: str) -> None:
        nonlocal index
        idx[v] = low[v] = index
        index += 1
        stack.append(v)
        on.add(v)
        for w in graph.get(v, ()):
            if w not in idx:
                strong(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], idx[w])
        if low[v] == idx[v]:
            comp: list[str] = []
            while True:
                w = stack.pop()
                on.remove(w)
                comp.append(w)
                if w == v:
                    break
            out.append(comp)

    for v in graph:
        if v not in idx:
            strong(v)
    return out


def classify_scc(comp: list[str], functions: dict[str, Function]) -> str:
    reps = [functions[n] for n in comp]
    if not any(AST_TOUCH.search(f.body[:2500]) for f in reps):
        return "drop:no_ast"
    if all(DEPTH_BOUND_DROP.search(f.body[:2000]) and "ExpressionNode" not in f.body[:2000] for f in reps):
        return "drop:fixed_depth_cap"
    if any("Polynomial" in f.body or "proposition::" in f.body[:500] for f in reps):
        return "drop:proof_polynomial"
    if any("Token" in f.body[:800] and "SyntaxTrees" not in f.body[:800] for f in reps):
        return "drop:token_not_tree"
    return "keep:review"


def analyze_call_graph() -> dict:
    report: dict = {}
    for stage, crates in STAGE_CRATES.items():
        functions, graph = build_stage_graph(stage, crates)
        sccs = tarjan(graph)
        recursive = []
        for comp in sccs:
            if len(comp) == 1 and comp[0] not in graph.get(comp[0], set()):
                continue
            label = classify_scc(comp, functions)
            if not label.startswith("keep"):
                continue
            rep = min(comp, key=lambda n: functions[n].line)
            f = functions[rep]
            recursive.append(
                {
                    "representative": f.name,
                    "file": str(f.file.relative_to(REPO)),
                    "line": f.line,
                    "members": sorted(functions[n].name for n in comp),
                    "classifier": label,
                }
            )
        report[stage] = {
            "function_count": len(functions),
            "recursive_scc_candidates": len(recursive),
            "candidates": sorted(recursive, key=lambda x: (x["file"], x["line"])),
        }
    return report


def main() -> int:
    sys.setrecursionlimit(200_000)
    depths = measure_omg_files()
    graph = analyze_call_graph()
    out = {
        "commit": "0835b3aaf803ed97f905be42d94727ff58459263",
        "depths": depths,
        "call_graph": graph,
    }
    audit_dir = REPO / "wiki/drafts/audits"
    parser_path = audit_dir / "recursive_walkers_2026-11-12_parser_depths.json"
    if parser_path.is_file():
        parser = json.loads(parser_path.read_text(encoding="utf-8"))
        out["depths"]["parser"] = parser
        out["depths"]["parser_note"] = (
            "Psi-01 syntax parse via wiki/drafts/audits/measure_depths "
            "(run with `ulimit -s unlimited` on default Linux stack)."
        )
    (audit_dir / "recursive_walkers_2026-11-12_depth.json").write_text(
        json.dumps(out, indent=2), encoding="utf-8"
    )
    print(json.dumps({s: graph[s]["recursive_scc_candidates"] for s in graph}, indent=2))
    print("depths excluding fixtures:", depths["excluding_depth_fixtures"]["max"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
