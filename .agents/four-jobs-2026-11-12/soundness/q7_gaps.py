"""Load Q7 mut-migration gap sites from PR #2638 audit and scan mutants."""

from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
_REF = "origin/pr-2638"
_AUDIT = "wiki/drafts/audits/mut_migration_2026-11-12.md"

_SITE = re.compile(
    r"`(tests/omega/[^`]+)`:(\d+)\s+[`—]?\s*`?([A-Za-z_][A-Za-z0-9_]*)`?"
)
_LET = re.compile(r"^\s*let\s+(?P<mut>mut\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b", re.MULTILINE)
_BORROW_MUT = re.compile(r"&\s*(mut|write)\s+(?P<root>[A-Za-z_][A-Za-z0-9_]*)\b")
_STORE_RE = re.compile(
    r"^\s*(?P<root>[A-Za-z_][A-Za-z0-9_]*)(?:\[[^\]]+\]|\.[A-Za-z_][A-Za-z0-9_]*)\s*="
)


@dataclass(frozen=True)
class Q7Site:
    path: str
    line: int
    name: str


@dataclass
class Q7Catalog:
    sites: set[Q7Site]
    by_path_name: set[tuple[str, str]]
    names_by_path: dict[str, set[str]]

    def audit_lists_mutant_hit(self, fixture_rel: str, mutant: str) -> bool:
        for hit in scan_q7_hits(mutant):
            if (fixture_rel, hit.line, hit.name) in {
                (s.path, s.line, s.name) for s in self.sites
            }:
                return True
            if (fixture_rel, hit.name) in self.by_path_name:
                return True
        return False


def _load_audit_text() -> str:
    try:
        return subprocess.check_output(
            ["git", "-C", str(REPO), "show", f"{_REF}:{_AUDIT}"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except subprocess.CalledProcessError:
        return ""


def load_q7_catalog() -> Q7Catalog:
    text = _load_audit_text()
    sites: set[Q7Site] = set()
    for line in text.splitlines():
        m = _SITE.search(line)
        if m:
            sites.add(Q7Site(m.group(1), int(m.group(2)), m.group(3)))
    by_path_name = {(s.path, s.name) for s in sites}
    names_by_path: dict[str, set[str]] = {}
    for p, n in by_path_name:
        names_by_path.setdefault(p, set()).add(n)
    return Q7Catalog(sites=sites, by_path_name=by_path_name, names_by_path=names_by_path)


def load_q7_gap_sites() -> set[tuple[str, str]]:
    """Backward-compatible (path, binding) set."""
    return load_q7_catalog().by_path_name


def strip_line_comment(line: str) -> str:
    if "//" in line:
        return line.split("//", 1)[0]
    return line


def scan_q7_hits(mutant: str) -> list[Q7Site]:
    """Lexical Q7 scan: non-mut let with writable borrow or field/index store."""
    # Approximate brace-depth scopes per line (good enough for corpus fixtures).
    scopes: list[set[str]] = [set()]
    hits: list[Q7Site] = []
    mut_bindings: set[str] = set()
    for i, raw in enumerate(mutant.splitlines(), 1):
        for ch in raw:
            if ch == "{":
                scopes.append(set())
            elif ch == "}" and len(scopes) > 1:
                scopes.pop()
        line = strip_line_comment(raw).strip()
        if not line:
            continue
        lm = _LET.search(raw)
        if lm:
            name = lm.group("name")
            if lm.group("mut"):
                mut_bindings.add(name)
            else:
                non_mut = name
                tail = mutant[mutant.find(raw) + len(raw) :]
                # Same-scope use only (heuristic: next 40 lines).
                window = "\n".join(mutant.splitlines()[i - 1 : i + 40])
                for bm in _BORROW_MUT.finditer(window):
                    if bm.group("root") == non_mut:
                        hits.append(Q7Site("", i, non_mut))
                        break
                else:
                    sm = _STORE_RE.match(line)
                    if not sm:
                        for j, raw2 in enumerate(mutant.splitlines()[i : i + 40], i + 1):
                            sm2 = _STORE_RE.match(strip_line_comment(raw2).strip())
                            if sm2 and sm2.group("root") == non_mut:
                                hits.append(Q7Site("", j, non_mut))
                                break
        bm = _BORROW_MUT.search(line)
        if bm:
            root = bm.group("root")
            if root not in mut_bindings and root not in {h.name for h in hits if h.line == i}:
                hits.append(Q7Site("", i, root))
    return hits


def q7_mut_enforcement_gap(kind: str, invalid_only_missing_mut: bool) -> bool:
    return kind in ("remove_mut", "mut_ref_instead_of_shared") and invalid_only_missing_mut


def classify_q7_mut_row(
    catalog: Q7Catalog,
    fixture_rel: str,
    kind: str,
    invalid_only_missing_mut: bool,
    mutant_text: str,
) -> tuple[bool, bool]:
    """
    Returns (is_known_gap_q7, audit_listed).
    Known gap: Q7 missing-mut enforcement class accepted by a checker.
    audit_listed: mutant scan hit matches a row in the PR #2638 audit for this fixture.
    """
    if not q7_mut_enforcement_gap(kind, invalid_only_missing_mut):
        return False, False
    listed = catalog.audit_lists_mutant_hit(fixture_rel, mutant_text)
    return True, listed
