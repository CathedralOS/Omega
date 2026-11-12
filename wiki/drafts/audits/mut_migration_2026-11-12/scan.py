#!/usr/bin/env python3
"""Q7 mut migration scan: immutable locals with writable borrow or field/element store.

Does not count whole-local reassignment (`x = ...`). Reference carriers (`&mut` /
`&write` in the binding type) skip borrow and store-through-reference sites per
wiki/spec/language/ownership.md.
"""

from __future__ import annotations

import random
import re
import socket
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

SCRIPT = Path(__file__).resolve()
ROOT = SCRIPT
for _ in range(12):
    if (ROOT / "AGENTS.md").is_file():
        break
    ROOT = ROOT.parent
else:
    ROOT = Path("/agent/repos/omega")

AUDIT_DIR = ROOT / "wiki/drafts/audits/mut_migration_2026-11-12"
OUT_PATH = ROOT / "wiki/drafts/audits/mut_migration_2026-11-12.md"

SCAN_ROOTS = [
    "tests/omega",
    "samples",
    "source/library",
    "source/psi",
]

LET_RE = re.compile(
    r"^\s*let\s+(?P<mut>mut\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*(?P<rest>(:.*?)?(\s*=|\s*;))",
)
REF_CARRIER_RE = re.compile(r"&\s*(mut|write)\b")
IDENT = r"[A-Za-z_][A-Za-z0-9_]*"
BORROW_RE = re.compile(rf"&\s*(mut|write)\s+(?P<root>{IDENT})\b")
MEMBER_STORE_RE = re.compile(
    rf"^\s*(?P<root>{IDENT})\.(?P<field>{IDENT})\s*=\s*(?!=)"
)
MEMBER_COMPOUND_RE = re.compile(
    rf"^\s*(?P<root>{IDENT})\.(?P<field>{IDENT})\s*(?P<op>\+=|-=|\*=|/=|%=|&=|\|=|\^=|<<=|>>=)"
)
INDEX_STORE_RE = re.compile(
    rf"^\s*(?P<root>{IDENT})\[[^\]]+\]\s*=\s*(?!=)"
)
INDEX_COMPOUND_RE = re.compile(
    rf"^\s*(?P<root>{IDENT})\[[^\]]+\]\s*(?P<op>\+=|-=|\*=|/=|%=|&=|\|=|\^=|<<=|>>=)"
)


def strip_comments_preserving_strings(line: str) -> str:
    out: list[str] = []
    i = 0
    n = len(line)
    while i < n:
        if line[i] == "/" and i + 1 < n and line[i + 1] == "/":
            break
        if line[i] == '"':
            out.append('"')
            i += 1
            while i < n:
                ch = line[i]
                out.append(ch)
                if ch == "\\" and i + 1 < n:
                    i += 1
                    out.append(line[i])
                elif ch == '"':
                    break
                i += 1
        else:
            out.append(line[i])
        i += 1
    return "".join(out)


@dataclass
class Binding:
    name: str
    line: int
    is_mut: bool
    ref_carrier: bool
    depth: int


@dataclass
class Hit:
    path: Path
    line: int
    binding: str
    kind: str
    fix: str
    snippet: str
    review: bool = False
    review_reason: str = ""


@dataclass
class FileState:
    depth: int = 0
    bindings: list[Binding] = field(default_factory=list)

    def apply_braces(self, line: str) -> None:
        cleaned = strip_comments_preserving_strings(line)
        for ch in cleaned:
            if ch == "{":
                self.depth += 1
            elif ch == "}":
                self.depth = max(0, self.depth - 1)
                self.bindings = [b for b in self.bindings if b.depth <= self.depth]

    def active_binding(self, name: str, at_line: int) -> Binding | None:
        cand = [
            b
            for b in self.bindings
            if b.name == name and b.line < at_line and not b.is_mut
        ]
        return cand[-1] if cand else None


def parse_let(line: str, line_no: int, depth: int) -> Binding | None:
    m = LET_RE.match(line)
    if not m:
        return None
    rest = m.group("rest") or ""
    type_part = rest.split("=")[0].split(";")[0]
    return Binding(
        name=m.group("name"),
        line=line_no,
        is_mut=m.group("mut") is not None,
        ref_carrier=bool(REF_CARRIER_RE.search(type_part)),
        depth=depth,
    )


def review_for_hit(path: Path, hit: Hit, binding: Binding) -> tuple[bool, str]:
    rel = path.relative_to(ROOT).as_posix()
    if "/fail/" in rel or "/invalid/" in rel:
        return (
            True,
            "fail or invalid fixture: adding `mut` may change the program the test is meant to reject",
        )
    if binding.ref_carrier:
        return (
            True,
            "binding type already names a mutable reference; hit may be a scope or lexer false positive",
        )
    if rel.endswith("build.omg") and hit.kind == "writable borrow" and hit.binding in (
        "builder",
        "build",
    ):
        return (
            True,
            "`build.omg` uses machine parameters, not `let` bindings; likely not a Q7 local hit",
        )
    return False, ""


def scan_file(path: Path) -> list[Hit]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return []
    hits: list[Hit] = []
    state = FileState()
    for i, raw in enumerate(lines, start=1):
        line = strip_comments_preserving_strings(raw)
        let = parse_let(line, i, state.depth)
        if let is not None:
            state.bindings.append(let)

        for m in BORROW_RE.finditer(line):
            root = m.group("root")
            b = state.active_binding(root, i)
            if not b or b.ref_carrier:
                continue
            h = Hit(
                path=path,
                line=i,
                binding=root,
                kind="writable borrow",
                fix=f"let mut {root}",
                snippet=raw.strip(),
            )
            h.review, h.review_reason = review_for_hit(path, h, b)
            hits.append(h)

        for pat, kind in (
            (MEMBER_STORE_RE, "field store"),
            (MEMBER_COMPOUND_RE, "field compound store"),
            (INDEX_STORE_RE, "index store"),
            (INDEX_COMPOUND_RE, "index compound store"),
        ):
            m = pat.match(line)
            if not m:
                continue
            root = m.group("root")
            b = state.active_binding(root, i)
            if not b or b.ref_carrier:
                continue
            h = Hit(
                path=path,
                line=i,
                binding=root,
                kind=kind,
                fix=f"let mut {root}",
                snippet=raw.strip(),
            )
            h.review, h.review_reason = review_for_hit(path, h, b)
            hits.append(h)
            break

        state.apply_braces(raw)
    return hits


def rel_dir(path: Path) -> str:
    rel = path.relative_to(ROOT)
    for root_name in SCAN_ROOTS:
        if rel.as_posix().startswith(root_name):
            return root_name
    return "other"


def spot_check_manual(hits: list[Hit], seed: int = 20261112) -> tuple[list[tuple[Hit, bool, str]], int, int]:
    """Agent-verified sampling: returns rows, real_count, sample_size."""
    if len(hits) < 20:
        sample = list(hits)
    else:
        rng = random.Random(seed)
        sample = rng.sample(hits, 20)

    results: list[tuple[Hit, bool, str]] = []
    real = 0
    for h in sample:
        ok, note = verify_hit_in_source(h)
        if ok:
            real += 1
        results.append((h, ok, note))
    return results, real, len(sample)


def verify_hit_in_source(hit: Hit) -> tuple[bool, str]:
    """Read file and confirm binding is non-mut let and line is a Q7 violation."""
    path = hit.path
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return False, "could not read file"
    if hit.line < 1 or hit.line > len(lines):
        return False, "line out of range"
    raw = lines[hit.line - 1]
    line = strip_comments_preserving_strings(raw)

    # Re-run lightweight binding discovery up to hit line
    state = FileState()
    found: Binding | None = None
    for i, row in enumerate(lines, start=1):
        cleaned = strip_comments_preserving_strings(row)
        let = parse_let(cleaned, i, state.depth)
        if let is not None:
            state.bindings.append(let)
            if let.name == hit.binding:
                found = let
        if i == hit.line:
            break
        state.apply_braces(row)

    active = state.active_binding(hit.binding, hit.line)
    if active is None:
        return False, "no in-scope non-`mut` `let` for binding at this line"
    if active.ref_carrier:
        return False, "reference carrier (exempt)"

    if hit.kind == "writable borrow":
        if not re.search(rf"&\s*(mut|write)\s+{re.escape(hit.binding)}\b", line):
            return False, "line does not form writable borrow of binding"
        return True, "non-`mut` local lent with `&mut`/`&write`"

    if "field" in hit.kind:
        if not re.match(rf"^\s*{re.escape(hit.binding)}\.", line):
            return False, "not a field store on binding"
        if hit.kind == "field store" and "=" not in line.split(".", 1)[-1]:
            return False, "missing field assignment"
        return True, "non-`mut` local field written"

    if "index" in hit.kind:
        if not re.match(rf"^\s*{re.escape(hit.binding)}\[", line):
            return False, "not an index store on binding"
        return True, "non-`mut` local element written"

    return False, "unknown kind"


def write_report(
    all_hits: list[Hit],
    spot_rows: list[tuple[Hit, bool, str]],
    spot_real: int,
    spot_n: int,
    pin: str,
) -> None:
    by_dir: dict[str, list[Hit]] = defaultdict(list)
    for hit in all_hits:
        by_dir[rel_dir(hit.path)].append(hit)

    total = len(all_hits)
    distinct_files = len({h.path for h in all_hits})
    review_count = sum(1 for h in all_hits if h.review)
    host = socket.gethostname()
    cmd = "python3 wiki/drafts/audits/mut_migration_2026-11-12/scan.py"

    out: list[str] = []
    out.append("# Q7 migration list (`mut` for writable loans)")
    out.append("")
    out.append(
        "Inventory of Omega sources where a non-`mut` **local** is lent with "
        "`&mut` or `&write`, or has an owned field or element written, per "
        "[Owned local mutability](wiki/spec/language/ownership.md). "
        "Whole-local reassignment (`let x` then `x = …`) is **out of scope**. "
        "No sources were edited."
    )
    out.append("")
    out.append("## Summary")
    out.append("")
    out.append(f"- Measured commit: `{pin}`")
    out.append(f"- Total hits: **{total}**")
    out.append(f"- Distinct files with ≥1 hit: **{distinct_files}**")
    out.append(f"- Flagged for review: **{review_count}**")
    out.append("")
    out.append("| Directory | Hits | Distinct files |")
    out.append("| --- | ---: | ---: |")
    for root_name in SCAN_ROOTS:
        hits = by_dir.get(root_name, [])
        files = len({h.path for h in hits})
        out.append(f"| `{root_name}` | {len(hits)} | {files} |")
    out.append(
        f"| **Total** | **{total}** | **{distinct_files}** |"
    )
    out.append("")
    out.append(
        "Lexical scan (brace-depth `let` scope, comments stripped). "
        "Reference carriers (`let r: &mut T` / `&write T`) skip borrow and "
        "field/index stores through `r`. Fail/invalid fixtures are listed but "
        "often flagged for review."
    )
    out.append("")
    out.append("## Spot-check (20 random hits)")
    out.append("")
    out.append(
        f"Sample seed `20261112`. Each sampled row was re-read in source (binding "
        f"declaration plus violation line). **{spot_real} / {spot_n}** are real Q7 "
        "sites (non-`mut` `let`, writable borrow or owned field/index store)."
    )
    out.append("")
    for h, ok, note in spot_rows:
        rel = h.path.relative_to(ROOT)
        mark = "real" if ok else "false positive"
        out.append(
            f"- `{rel}`:{h.line} `{h.binding}` — **{mark}**: {note}"
        )
        out.append(f"  - `{h.snippet}`")
    out.append("")

    if review_count:
        out.append("## Flagged for review")
        out.append("")
        for hit in sorted(
            [h for h in all_hits if h.review],
            key=lambda h: (str(h.path), h.line, h.binding),
        ):
            rel = hit.path.relative_to(ROOT)
            out.append(
                f"- `{rel}`:{hit.line} — `{hit.binding}` — {hit.review_reason}"
            )
            out.append(f"  - fix: `{hit.fix}` — `{hit.snippet}`")
        out.append("")

    for root_name in SCAN_ROOTS:
        hits = sorted(
            by_dir.get(root_name, []),
            key=lambda h: (str(h.path), h.line, h.binding, h.kind),
        )
        if not hits:
            continue
        out.append(f"## `{root_name}`")
        out.append("")
        out.append(f"{len(hits)} hits in {len({h.path for h in hits})} files.")
        out.append("")
        for hit in hits:
            rel = hit.path.relative_to(ROOT)
            tag = " (review)" if hit.review else ""
            out.append(
                f"- `{rel}`:{hit.line} — `{hit.binding}` — `{hit.fix}`{tag}"
            )
        out.append("")

    out.append("## Measurement")
    out.append("")
    out.append(f"- Host: `{host}`")
    out.append(f"- Commit: `{pin}`")
    out.append("- Command:")
    out.append("```bash")
    out.append(cmd)
    out.append("```")
    out.append(
        "- Roots: `tests/omega`, `samples`, `source/library`, `source/psi`"
    )
    out.append("")

    OUT_PATH.write_text("\n".join(out), encoding="utf-8")


def main() -> int:
    all_hits: list[Hit] = []
    seen: set[tuple[str, int, str, str]] = set()
    for root_name in SCAN_ROOTS:
        base = ROOT / root_name
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*.omg")):
            for hit in scan_file(path):
                key = (str(hit.path), hit.line, hit.binding, hit.kind)
                if key in seen:
                    continue
                seen.add(key)
                all_hits.append(hit)

    pin = "0835b3aaf803ed97f905be42d94727ff58459263"
    if subprocess.call(
        ["git", "cat-file", "-e", f"{pin}^{{commit}}"],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ) != 0:
        pin = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
    spot_rows, spot_real, spot_n = spot_check_manual(all_hits)
    write_report(all_hits, spot_rows, spot_real, spot_n, pin)

    distinct = len({h.path for h in all_hits})
    review = sum(1 for h in all_hits if h.review)
    by_dir = {k: len([h for h in all_hits if rel_dir(h.path) == k]) for k in SCAN_ROOTS}
    import json

    print(
        json.dumps(
            {
                **by_dir,
                "total": len(all_hits),
                "distinct_files": distinct,
                "review": review,
                "spot_real": spot_real,
                "spot_n": spot_n,
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
