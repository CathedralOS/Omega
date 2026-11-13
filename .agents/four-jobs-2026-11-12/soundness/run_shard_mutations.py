#!/usr/bin/env python3
"""Job 2 soundness mutation runner (lead-review buckets, shard S of 8)."""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

from q7_gaps import Q7Catalog, classify_q7_mut_row, load_q7_catalog, q7_mut_enforcement_gap

PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(os.environ.get("SOUNDNESS_SHARD", "6"))
SHARD_COUNT = 8
REPO = Path(__file__).resolve().parents[3]
OUT_ROOT = REPO / ".agents" / "four-jobs-2026-11-12" / "soundness"
OMEGA = REPO / "target" / "debug" / "omega"
ROUTE = REPO / "target" / "debug" / "examples" / "route_one_comparison"
CHECK_TIMEOUT = 120
ENV = {
    **os.environ,
    "OMEGA_LIBRARY_ROOT": str(REPO / "source/library"),
    "OMEGA_CORPUS_ROOT": str(REPO),
}

COMPILE_TIME_SUFFIX = (
    " Checked compilation must establish this obligation before execution; "
    "a runtime trap or crash arm does not discharge it at check time."
)

SPEC_RULES: dict[str, tuple[str, str, str]] = {
    "remove_mut": (
        "wiki/spec/language/expressions.md",
        "Assignment and explicit `&mut` / `&write` loans",
        "Such a loan requires existing writable authority at the selected place; "
        "owned-local mutability requires a `mut` binding when that place is the local's owned storage."
        + COMPILE_TIME_SUFFIX,
    ),
    "widen_requires": (
        "wiki/spec/language/machines.md",
        "Call preconditions",
        "Every machine application establishes the selected contract's `requires` under "
        "the exact argument substitution." + COMPILE_TIME_SUFFIX,
    ),
    "drop_requires": (
        "wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md",
        "Nested machines and identity; Private entry inference",
        "A written entry contract remains complete even when all currently visible callers "
        "happen to establish stronger facts. An authored `requires` is complete, including "
        "on a private helper. Inference cannot silently conjoin further entry conditions to "
        "make the body check." + COMPILE_TIME_SUFFIX,
    ),
    "remove_guard_transition": (
        "wiki/spec/language/state_contracts.md",
        "Requirement clauses",
        "Entry obligations established at every invocation under exact actual-argument "
        "substitution must be dischargeable at every reachable call." + COMPILE_TIME_SUFFIX,
    ),
    "mut_ref_instead_of_shared": (
        "wiki/spec/language/expressions.md",
        "Assignment and explicit `&mut` / `&write` loans",
        "Such a loan requires existing writable authority at the selected place; "
        "forming `&mut` without writable authority violates exclusivity and mutability rules."
        + COMPILE_TIME_SUFFIX,
    ),
    "shared_instead_of_mut": (
        "wiki/spec/language/expressions.md",
        "Assignment and explicit `&mut` / `&write` loans",
        "Writable use through a shared `&` loan without the required `&mut` / `&write` authority "
        "violates exclusivity and mutability rules." + COMPILE_TIME_SUFFIX,
    ),
    "none": ("", "", ""),
}

KIND_ALIASES = {
    "weaken_requires": "widen_requires",
    "remove_guard": "remove_guard_transition",
    "amp_to_ampmut": "mut_ref_instead_of_shared",
}

ROUTE_OUTSIDE = frozenset(
    {
        "no-entry",
        "unreached",
        "outside",
        "incomplete",
        "defect",
        "error",
        "fallback",
    }
)


@dataclass
class SpecRead:
    invalid: bool
    obligation: str
    valid_reason: str
    invalid_only_missing_mut: bool
    rule: tuple[str, str, str] | None

LET_RE = re.compile(r"^\s*let\s+(?P<mut>mut\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b", re.MULTILINE)
BORROW_MUT_RE = re.compile(r"&\s*(mut|write)\s+(?P<root>[A-Za-z_][A-Za-z0-9_]*)\b")
STORE_RE = re.compile(
    r"^\s*(?P<root>[A-Za-z_][A-Za-z0-9_]*)(?:\[[^\]]+\]|\.[A-Za-z_][A-Za-z0-9_]*)\s*="
)


@dataclass
class Mutation:
    kind: str
    detail: str
    text: str
    meta: dict[str, str] = field(default_factory=dict)


@dataclass
class Row:
    fixture: str
    tier: str
    kind: str
    detail: str
    bucket: str
    current: str
    route: str
    route_detail: str
    discard_reason: str
    obligation: str
    rule_file: str
    rule_heading: str
    rule_quote: str
    accepted_by: str
    hole_subtype: str
    mutant_path: str
    current_s: float
    route_s: float
    q7_audit_listed: str = ""


@dataclass
class FixtureLedgerRow:
    fixture: str
    tier: str
    outcome: str
    diagnostic: str = ""


BUCKETS = (
    "confirmed-hole",
    "known-gap-q7-mut",
    "disagreement-valid",
    "valid-route-outside",
    "discarded-valid",
    "rejected-both",
    "rejected-current-route-outside",
    "timeout",
    "failed-run",
)


def list_shard_fixtures() -> list[Path]:
    listed = subprocess.check_output(
        [
            "git",
            "-C",
            str(REPO),
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
        ],
        text=True,
    )
    paths = sorted(p for p in listed.splitlines() if p.endswith("/main.omg"))
    out = [REPO / rel for i, rel in enumerate(paths) if i % SHARD_COUNT == SHARD]
    limit = os.environ.get("SOUNDNESS_FIXTURE_LIMIT")
    return out[: int(limit)] if limit else out


def read_source(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def strip_line_comment(line: str) -> str:
    if "//" in line:
        return line.split("//", 1)[0]
    return line


def count_let_bindings(text: str, name: str) -> int:
    return len(re.findall(rf"\blet\s+(?:mut\s+)?{re.escape(name)}\b", text))


def synthesize_violating_call(mutant: str, removed: str) -> str | None:
    m = re.search(r"machine\s+(\w+)\s*\(", mutant)
    if not m:
        return None
    name = m.group(1)
    if name == "main":
        m = re.search(r"machine\s+(\w+)\s*\(", mutant[m.end() :])
        if not m:
            return None
        name = m.group(1)
    sig = re.search(rf"machine\s+{re.escape(name)}\s*\(([^)]*)\)", mutant)
    if not sig:
        return None
    params: list[str] = []
    for part in sig.group(1).split(","):
        part = part.strip()
        if not part:
            continue
        params.append(part.split(":")[0].strip())
    args: list[str] = []
    for param in params:
        ge = re.search(rf"\b{re.escape(param)}\s*>=\s*(-?\d+)", removed)
        if ge:
            args.append(str(int(ge.group(1)) - 1))
            continue
        le = re.search(rf"\b{re.escape(param)}\s*<=\s*(-?\d+)", removed)
        if le:
            args.append(str(int(le.group(1)) + 1))
            continue
        args.append("-1" if "i32" in sig.group(1) or "i64" in sig.group(1) else "0")
    if not args:
        return None
    return f"{name}({', '.join(args)})"


def inject_violating_probe(mutant: str, call: str) -> str:
    probe = f"\n    // soundness-probe violating entry\n    let _probe = {call};\n"
    main = re.search(r"(machine\s+main\s*\(\)\s*\{)", mutant)
    if main:
        insert_at = main.end()
        return mutant[:insert_at] + probe + mutant[insert_at:]
    return mutant + f"\nmachine __soundness_probe() {{ {call}; }}\n"


MODULAR_ENTRY_CALLER = (
    "A written entry contract remains complete even when all currently visible callers "
    "happen to establish stronger facts."
)


def machine_internal_restates_removed(mutant: str, removed: str) -> str | None:
    """Transition arms or `when` guards inside the machine, not external callers."""
    removed_compact = re.sub(r"\s+", "", removed)
    for pat, label in (
        (r"transition\s+([^{;\n]+)", "transition"),
        (r"^\s*when\s+(.+)$", "when guard"),
    ):
        for m in re.finditer(pat, mutant, re.MULTILINE):
            arm = m.group(1).strip()
            arm_compact = re.sub(r"\s+", "", arm)
            if len(removed_compact) >= 8 and removed_compact in arm_compact:
                return f"{label} `{arm}` restates removed requires `{removed}`"
            for ge in re.finditer(r"(\w+)\s*>=\s*(-?\d+)", removed):
                param, bound = ge.group(1), ge.group(2)
                if re.search(rf"\b{re.escape(param)}\s*>=\s*{re.escape(bound)}\b", arm):
                    return f"{label} `{arm}` restates `{param} >= {bound}` from `{removed}`"
    return None


def probe_drop_requires_subtype(mutant_text: str, removed: str, mutant_main: Path) -> str:
    call = synthesize_violating_call(mutant_text, removed)
    if not call:
        return "probe-not-built"
    probed = inject_violating_probe(mutant_text, call)
    saved = mutant_main.read_text(encoding="utf-8")
    try:
        mutant_main.write_text(probed, encoding="utf-8")
        verdict, _, _, _ = run_omega(mutant_main)
    finally:
        mutant_main.write_text(saved, encoding="utf-8")
    return "unchecked" if accepts(verdict) else "inferred-entry"


def find_remove_mut_target(source: str) -> tuple[str, str] | None:
    for m in LET_RE.finditer(source):
        if not m.group("mut"):
            continue
        name = m.group("name")
        tail = source[m.end() :]
        if re.search(
            rf"(?:^|\n)[^\n]*\b{re.escape(name)}\s*[\[.]"
            rf"|&\s*(?:mut|write)\s+{re.escape(name)}\b",
            tail,
            re.MULTILINE,
        ):
            return name, m.group(0).strip()
    return None


def apply_remove_mut(source: str, name: str) -> str:
    return re.sub(rf"\blet\s+mut\s+{re.escape(name)}\b", f"let {name}", source, count=1)


def find_remove_mut_violation(mutant: str, name: str) -> str | None:
    for i, raw in enumerate(mutant.splitlines(), 1):
        line = strip_line_comment(raw).strip()
        if not line or line.startswith("//"):
            continue
        bm = BORROW_MUT_RE.search(line)
        if bm and bm.group("root") == name:
            return f"{i}: {raw.rstrip()}"
        sm = STORE_RE.match(line)
        if sm and sm.group("root") == name:
            return f"{i}: {raw.rstrip()}"
        if re.match(rf"^{re.escape(name)}\s*=", line):
            return f"{i}: {raw.rstrip()}"
    return None


def first_requires_clause(text: str) -> str | None:
    m = re.search(r"requires\s+([^\n{]+)", text)
    return m.group(1).strip() if m else None


def first_machine_header_requires(text: str) -> tuple[str, str] | None:
    """`machine Name(...) requires ... {` on one line."""
    m = re.search(r"(machine\s+\w+[^{]*?)requires\s+([^;\n{]+)", text)
    if not m:
        return None
    return m.group(0), m.group(2).strip()


def drop_machine_header_requires(text: str) -> tuple[str, str] | None:
    hdr = first_machine_header_requires(text)
    if not hdr:
        return None
    full, clause = hdr
    new = re.sub(
        r"(machine\s+\w+[^{]*?)requires\s+[^;\n{]+",
        r"\1",
        text,
        count=1,
    )
    if new == text:
        return None
    return new, clause


def drop_requires_dependent_obligation(
    original: str, mutant: str, removed: str
) -> str | None:
    names = set(re.findall(r"\b[a-z][a-z0-9_]*\b", removed))
    names -= {"requires", "ensures", "and", "or", "true", "false", "machine", "transition"}
    for i, raw in enumerate(mutant.splitlines(), 1):
        line = strip_line_comment(raw)
        for n in names:
            if re.search(rf"\b{re.escape(n)}\s*\[", line):
                return f"{i}: {raw.rstrip()} (index uses `{n}` formerly guarded by `{removed}`)"
            if re.search(rf"/\s*{re.escape(n)}\b|\b{re.escape(n)}\s*/", line):
                return f"{i}: {raw.rstrip()} (division uses `{n}` formerly guarded by `{removed}`)"
            if re.search(rf"\b{re.escape(n)}\s*[-+*/%]", line) and any(
                op in removed for op in (">=", ">", "<=", "<", "!=", "==")
            ):
                return f"{i}: {raw.rstrip()} (arithmetic on `{n}` formerly guarded by `{removed}`)"
    return None


def weaken_requires_breach(original: str, mutant: str, detail: str) -> str | None:
    removed = first_requires_clause(original)
    weakened = first_requires_clause(mutant)
    if not removed or not weakened or removed == weakened:
        return None
    return drop_requires_dependent_obligation(original, mutant, removed)


def flip_to_mut_line(mutant: str) -> str | None:
    for i, raw in enumerate(mutant.splitlines(), 1):
        if re.search(r"&mut\b", raw):
            return f"{i}: {raw.rstrip()}"
    return None


def remove_guard_obligation(original: str, mutant: str) -> str | None:
    orig_when = re.search(r"^\s*when\s+(.+)$", original, re.MULTILINE)
    if not orig_when:
        return None
    guard = orig_when.group(1).strip()
    if re.search(rf"^\s*when\s+{re.escape(guard)}", mutant, re.MULTILINE):
        return None
    for i, raw in enumerate(mutant.splitlines(), 1):
        if "transition" in raw and "when" not in original[max(0, original.find(raw) - 50) :]:
            return f"{i}: {raw.rstrip()} (transition arm formerly gated by `when {guard}`)"
    return f"guard removed: when {guard}"


def find_shared_instead_of_mut_target(source: str) -> str | None:
    """First `&mut` applied through a place that could be flipped to shared `&`."""
    for i, raw in enumerate(source.splitlines(), 1):
        if re.search(r"&\s*(mut|write)\s+", raw):
            # Flip only a direct `&mut name` to `& name` on same line (single-site mutation).
            if re.search(r"&\s*mut\s+[A-Za-z_]", raw):
                return f"line {i}"
    return None


def apply_shared_instead_of_mut(source: str) -> str | None:
    new = re.sub(r"&\s*mut(\s+)", r"&\1", source, count=1)
    return new if new != source else None


def generate_mutations(source: str) -> list[Mutation]:
    muts: list[Mutation] = []
    target = find_remove_mut_target(source)
    if target:
        name, _decl = target
        text = apply_remove_mut(source, name)
        muts.append(
            Mutation(
                "remove_mut",
                f"remove mut from `{name}`",
                text,
                {"binding": name},
            )
        )
    hdr_drop = drop_machine_header_requires(source)
    if hdr_drop:
        new, clause = hdr_drop
        muts.append(
            Mutation(
                "drop_requires",
                "delete machine-header requires",
                new,
                {"removed": clause, "site": "header"},
            )
        )
    if "requires" in source:
        clause = first_requires_clause(source)
        new = re.sub(r"^\s*requires\s+[^\n{]+\n", "", source, count=1, flags=re.MULTILINE)
        if new != source and clause and not hdr_drop:
            muts.append(
                Mutation(
                    "drop_requires",
                    "delete first body requires line",
                    new,
                    {"removed": clause, "site": "body"},
                )
            )
        for pat, repl, label in (
            (r"(requires[^\n]*?)(>=)", r"\1>", "requires >= to >"),
            (r"(requires[^\n]*?)(<=)", r"\1<", "requires <= to <"),
        ):
            new2 = re.sub(pat, repl, source, count=1)
            if new2 != source:
                muts.append(Mutation("widen_requires", label, new2, {}))
    if re.search(r"^\s*when\s+", source, re.MULTILINE):
        new = re.sub(r"^\s*when\s+[^\n]+\n", "", source, count=1, flags=re.MULTILINE)
        if new != source:
            muts.append(
                Mutation("remove_guard_transition", "delete first when line", new, {})
            )
    if re.search(r"&(?!\s*(?:mut|write)\b)", source):
        new = re.sub(r"&(?!mut|write)(\s*)", r"&mut\1", source, count=1)
        if new != source:
            muts.append(
                Mutation("mut_ref_instead_of_shared", "first shared & -> &mut", new, {})
            )
    if find_shared_instead_of_mut_target(source):
        new = apply_shared_instead_of_mut(source)
        if new:
            muts.append(
                Mutation("shared_instead_of_mut", "first &mut -> shared &", new, {})
            )
    return muts


def parse_route_output(stdout: str) -> tuple[str, str]:
    route_verdict = "error"
    detail = ""
    for line in stdout.splitlines():
        if line.startswith("program\t"):
            for token in line.split("\t")[2:]:
                if token.startswith("route="):
                    route_verdict = token.split("=", 1)[1]
        elif line.startswith("diagnostic\t") and not detail:
            parts = line.split("\t")
            if len(parts) >= 2:
                detail = "diagnostic: " + one_line_join(parts[4:])
        elif line.startswith("fallback\t") and not detail:
            parts = line.split("\t")
            detail = "fallback: " + one_line_join(parts[2:])
    if not detail:
        detail = f"route={route_verdict}"
    return route_verdict, detail


def one_line_join(parts: list[str]) -> str:
    return " ".join(parts).replace("\t", " ")[:400]


def check_root_for_fixture(fixture_dir: Path) -> Path:
    """Match corpus admission: check the package root (build.omg when present)."""
    build = fixture_dir / "build.omg"
    return build if build.is_file() else fixture_dir / "main.omg"


def diagnose_check(path: Path) -> str:
    _, _, out, err = run_omega(path)
    text = (err + out).strip()
    for line in text.splitlines():
        if "error:" in line:
            return line.strip()[:400]
    return text[:400] if text else "(empty)"


def run_omega(path: Path) -> tuple[str, float, str, str]:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(path)],
            cwd=REPO,
            env=ENV,
            capture_output=True,
            text=True,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - start, "", ""
    elapsed = time.monotonic() - start
    verdict = "accepted" if proc.returncode == 0 else "rejected"
    return verdict, elapsed, proc.stdout[:500], proc.stderr[:500]


def run_route(path: Path) -> tuple[str, float, str]:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            ["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(path)],
            cwd=REPO,
            env=ENV,
            capture_output=True,
            text=True,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - start, ""
    elapsed = time.monotonic() - start
    route, _ = parse_route_output(proc.stdout)
    return route, elapsed, proc.stdout


def accepts(verdict: str) -> bool:
    return verdict == "accepted"


def route_is_outside(route: str) -> bool:
    return route in ROUTE_OUTSIDE or route not in ("accepted", "rejected")


def route_rejects(route: str) -> bool:
    return route == "rejected"


def checker_accepted_by(current: str, route: str) -> str:
    ac = accepts(current)
    ar = route == "accepted"
    if ac and ar:
        return "both"
    if ac:
        return "current"
    if ar:
        return "route"
    return "neither"


def either_checker_accepts(current: str, route: str) -> bool:
    return accepts(current) or route == "accepted"


def spec_allows_runtime_trap(kind: str, mutant: str) -> bool:
    """True when the mutant still routes violations to an explicit crash arm."""
    if not re.search(r"\bcrash\s+[A-Za-z_]", mutant):
        return False
    return kind in ("remove_guard_transition", "drop_requires", "widen_requires")


def minimal_diff_lines(original: str, mutant: str) -> list[str]:
    out: list[str] = []
    for a, b in zip(original.splitlines(), mutant.splitlines()):
        if a != b:
            out.append(f"- {a}")
            out.append(f"+ {b}")
    o_extra = original.splitlines()[len(mutant.splitlines()) :]
    m_extra = mutant.splitlines()[len(original.splitlines()) :]
    for a in o_extra:
        out.append(f"- {a}")
    for b in m_extra:
        out.append(f"+ {b}")
    return out


def read_spec_verdict(
    kind: str,
    original: str,
    mutant: str,
    mutation: Mutation,
) -> SpecRead:
    """Invalidity from reading the mutant only (route messages are not used)."""
    if kind == "remove_mut":
        name = mutation.meta.get("binding", "")
        if count_let_bindings(mutant, name) > 1:
            return SpecRead(
                False,
                "",
                f"inner `let {name}` shadows the demoted binding",
                False,
                None,
            )
        obligation = find_remove_mut_violation(mutant, name) or ""
        if not obligation:
            return SpecRead(
                False,
                "",
                "demoted binding has no write, reassign, or &mut/&write use",
                False,
                None,
            )
        return SpecRead(
            True,
            obligation,
            "",
            True,
            SPEC_RULES["remove_mut"],
        )
    if kind == "drop_requires":
        removed = mutation.meta.get("removed") or first_requires_clause(original) or ""
        obligation = drop_requires_dependent_obligation(original, mutant, removed) or ""
        if obligation:
            return SpecRead(True, obligation, "", False, SPEC_RULES["drop_requires"])
        internal = machine_internal_restates_removed(mutant, removed)
        if internal:
            return SpecRead(
                False,
                internal,
                f"Inside the machine, {internal}. Outside callers cannot substitute: "
                f"{MODULAR_ENTRY_CALLER}",
                False,
                None,
            )
        return SpecRead(
            False,
            "",
            "no body/ensures line needs the removed fact; no internal transition/guard "
            f"restatement ({MODULAR_ENTRY_CALLER})",
            False,
            None,
        )
    if kind == "widen_requires":
        obligation = weaken_requires_breach(original, mutant, mutation.detail) or ""
        if obligation:
            return SpecRead(True, obligation, "", False, SPEC_RULES["widen_requires"])
        return SpecRead(False, "", "widened requires still satisfied at uses in this file", False, None)
    if kind == "remove_guard_transition":
        obligation = remove_guard_obligation(original, mutant) or ""
        if obligation:
            return SpecRead(True, obligation, "", False, SPEC_RULES["remove_guard_transition"])
        return SpecRead(False, "", "removed guard does not leave an unchecked branch", False, None)
    if kind == "mut_ref_instead_of_shared":
        obligation = flip_to_mut_line(mutant) or ""
        if obligation:
            return SpecRead(
                True,
                obligation,
                "",
                True,
                SPEC_RULES["mut_ref_instead_of_shared"],
            )
        return SpecRead(False, "", "no &mut loan site in mutant", False, None)
    if kind == "shared_instead_of_mut":
        for i, raw in enumerate(mutant.splitlines(), 1):
            if re.search(r"&(?!\s*(?:mut|write)\b)", raw) and re.search(
                r"&\s*(mut|write)\s+", original
            ):
                return SpecRead(
                    True,
                    f"{i}: {raw.rstrip()} (shared borrow where mutant removed &mut)",
                    "",
                    False,
                    SPEC_RULES["shared_instead_of_mut"],
                )
        return SpecRead(False, "", "no shared-borrow downgrade site", False, None)
    return SpecRead(False, "", f"no spec read for kind {kind}", False, None)


def assign_bucket(
    spec: SpecRead,
    kind: str,
    fixture_rel: str,
    mutation: Mutation,
    mutant_text: str,
    current: str,
    route: str,
    catalog: Q7Catalog,
) -> tuple[str, str, str, str, str, str, str]:
    """Return bucket, obligation, rule fields, accepted_by, discard note, q7_audit_listed."""
    if current == "timeout" or route == "timeout":
        return "timeout", "", "", "", "", "neither", "checker timeout", ""
    if current == "error" or route == "error":
        return "failed-run", "", "", "", "", "neither", "checker error", ""

    accepted = checker_accepted_by(current, route)
    either_accepts = either_checker_accepts(current, route)

    if not spec.invalid:
        if accepts(current) and route == "accepted":
            return (
                "discarded-valid",
                spec.obligation,
                "",
                "",
                "",
                accepted,
                spec.valid_reason,
                "",
            )
        if accepts(current) and route_is_outside(route):
            return (
                "valid-route-outside",
                spec.obligation,
                "",
                "",
                "",
                "current",
                f"valid mutant; route did not judge ({route})",
                "",
            )
        if (accepts(current) and route_rejects(route)) or (
            route == "accepted" and not accepts(current)
        ):
            return (
                "disagreement-valid",
                spec.obligation,
                "",
                "",
                "",
                accepted,
                spec.valid_reason or "valid under spec read; one checker rejects",
                "",
            )
        if not accepts(current) and route_is_outside(route):
            return (
                "rejected-current-route-outside",
                "",
                "",
                "",
                "",
                accepted,
                spec.valid_reason,
                "",
            )
        return (
            "rejected-both",
            "",
            "",
            "",
            "",
            accepted,
            spec.valid_reason,
            "",
        )

    rule = spec.rule or SPEC_RULES.get(kind)
    rfile, rhead, rquote = rule if rule else ("", "", "")
    is_q7, audit_listed = classify_q7_mut_row(
        catalog, fixture_rel, kind, spec.invalid_only_missing_mut, mutant_text
    )
    q7_flag = "yes" if audit_listed else ("no" if is_q7 else "")
    if is_q7 and either_accepts:
        return (
            "known-gap-q7-mut",
            spec.obligation,
            rfile,
            rhead,
            rquote,
            accepted,
            "",
            q7_flag,
        )
    if either_accepts:
        return (
            "confirmed-hole",
            spec.obligation,
            rfile,
            rhead,
            rquote,
            accepted,
            "",
            q7_flag,
        )
    if not accepts(current) and route_is_outside(route):
        return (
            "rejected-current-route-outside",
            spec.obligation,
            "",
            "",
            "",
            accepted,
            "",
            q7_flag,
        )
    return ("rejected-both", spec.obligation, "", "", "", accepted, "", q7_flag)


def mutant_id(fixture: Path, kind: str, detail: str) -> str:
    h = hashlib.sha256(f"{fixture}:{kind}:{detail}".encode()).hexdigest()[:16]
    safe = fixture.relative_to(REPO).as_posix().replace("/", "__")
    return f"{safe}__{kind}__{h}"


def stage_mutant(fixture: Path, text: str, mid: str) -> Path:
    dest = OUT_ROOT / f"shard-{SHARD}" / "mutants" / mid
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(fixture.parent, dest)
    (dest / "main.omg").write_text(text, encoding="utf-8")
    return dest / "main.omg"


def bucket_counts(rows: list[Row]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for r in rows:
        counts[r.bucket] = counts.get(r.bucket, 0) + 1
    return counts


def regen_mutant_text(fixture_rel: str, kind: str, detail: str) -> tuple[str, str] | None:
    fixture = REPO / fixture_rel
    original = read_source(fixture)
    kinds = [kind, KIND_ALIASES.get(kind, "")]
    details = [detail]
    if kind == "drop_requires" and "body" not in detail:
        details.append("delete first body requires line")
        details.append("delete first requires line")
    if kind == "mut_ref_instead_of_shared":
        details.append("first shared & -> &mut")
    for m in generate_mutations(original):
        if m.kind in kinds and m.detail in details:
            return original, m.text
    for m in generate_mutations(original):
        normalized = KIND_ALIASES.get(m.kind, m.kind)
        if normalized == kind and m.detail in details:
            return original, m.text
    return None


def hand_verify_row(row: Row) -> list[str]:
    fixture = REPO / row.fixture
    orig = read_source(fixture)
    mut: str
    mutant: Path
    if row.mutant_path and (REPO / row.mutant_path).is_file():
        mutant = REPO / row.mutant_path
        mut = read_source(mutant)
    else:
        regen = regen_mutant_text(row.fixture, row.kind, row.detail)
        if not regen:
            return [f"#### Hand verification: {row.fixture} — could not regen mutant"]
        orig, mut = regen
        mid = mutant_id(fixture, row.kind, row.detail)
        mutant = stage_mutant(fixture, mut, mid)
    cur_v, _, cur_out, cur_err = run_omega(mutant)
    route_v, _, route_out = run_route(mutant)
    _, route_detail = parse_route_output(route_out)
    lines = [
        f"#### Hand verification: {row.fixture} / {row.kind}",
        "",
        f"- Bucket: `{row.bucket}`",
        f"- Obligation: `{row.obligation}`",
        "",
        "**Mutant diff (excerpt)**",
        "```diff",
    ]
    for line in minimal_diff_lines(orig, mut):
        lines.append(line)
    lines.extend(
        [
            "```",
            "",
            f"**`omega --check`** exit={'0' if cur_v == 'accepted' else 'nonzero'} verdict=`{cur_v}`",
            "```",
            (cur_out or cur_err or "(empty)")[:800],
            "```",
            "",
            f"**`route_one_comparison`** route=`{route_v}`",
            f"- Route detail: {route_detail}",
            "```",
            route_out[:1200] or "(empty)",
            "```",
            "",
        ]
    )
    if not row.mutant_path and mutant.parent.name.startswith("tests__"):
        shutil.rmtree(mutant.parent, ignore_errors=True)
    return lines


def write_md(
    path: Path,
    fixtures_total: int,
    fixture_ledger: list[FixtureLedgerRow],
    fixtures_zero_mutations: int,
    mutants_attempted: int,
    rows: list[Row],
    catalog: Q7Catalog,
    finished: bool,
) -> None:
    n_run = len(rows)
    buckets = bucket_counts(rows)
    by_kind: dict[str, int] = {}
    by_accepted: dict[str, int] = {}
    drop_header = drop_body = 0
    kinds_per_fixture: dict[str, set[str]] = {}
    q7_in_list = q7_not_list = 0
    for r in rows:
        kinds_per_fixture.setdefault(r.fixture, set()).add(r.kind)
        if r.bucket == "known-gap-q7-mut":
            if r.q7_audit_listed == "yes":
                q7_in_list += 1
            else:
                q7_not_list += 1
        if r.kind == "drop_requires":
            if "header" in r.detail:
                drop_header += 1
            elif "body" in r.detail:
                drop_body += 1
        if r.bucket in ("confirmed-hole", "known-gap-q7-mut"):
            by_kind[r.kind] = by_kind.get(r.kind, 0) + 1
            by_accepted[r.accepted_by] = by_accepted.get(r.accepted_by, 0) + 1

    mutated_fixtures = {r.fixture for r in rows}
    no_mutation = sum(1 for f in fixture_ledger if f.outcome == "no-mutation")
    orig_not = sum(1 for f in fixture_ledger if f.outcome == "original-not-accepted")
    failed_run_fix = sum(1 for f in fixture_ledger if f.outcome == "failed-run")
    mean_kinds = (
        sum(len(v) for v in kinds_per_fixture.values()) / max(1, len(kinds_per_fixture))
    )

    lines = [
        f"# Soundness mutations shard {SHARD} (PIN {PIN})",
        "",
        f"Status: {'complete' if finished else 'in progress'}",
        "",
        "Lead review buckets (every mutant row belongs to exactly one bucket below).",
        "",
        "## Commands",
        "",
        "```bash",
        f"git checkout {PIN}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega --bin omega",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison",
        "export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD",
        "python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
        "```",
        "",
        "## Fixture coverage",
        "",
        f"| Metric | Count |",
        f"| --- | ---: |",
        f"| Fixtures in shard | {fixtures_total} |",
        f"| original-not-accepted | {orig_not} |",
        f"| failed-run (original check) | {failed_run_fix} |",
        f"| no-mutation | {no_mutation} |",
        f"| mutated fixtures | {len(mutated_fixtures)} |",
        f"| Mutant rows | {n_run} |",
        f"| Mean mutation kinds per mutated fixture | {mean_kinds:.2f} |",
        "",
        "Reconcile: `fixtures in shard` = original-not-accepted + failed-run + no-mutation + mutated; "
        "mutant row buckets sum to `mutant rows`.",
        "",
        "## Original-not-accepted diagnostics (top)",
        "",
    ]
    from collections import Counter

    diag_counts = Counter(
        f.diagnostic for f in fixture_ledger if f.outcome == "original-not-accepted"
    )
    if diag_counts:
        lines.append("| Diagnostic (prefix) | Count |")
        lines.append("| --- | ---: |")
        for msg, c in diag_counts.most_common(15):
            lines.append(f"| {msg[:120].replace('|', '/')} | {c} |")
    else:
        lines.append("(none)")

    lines.extend(
        [
            "",
            "## Mutation kinds (all rows)",
            "",
            "| Kind | Count |",
            "| --- | ---: |",
        ]
    )
    all_kinds: dict[str, int] = {}
    for r in rows:
        all_kinds[r.kind] = all_kinds.get(r.kind, 0) + 1
    for k, c in sorted(all_kinds.items()):
        lines.append(f"| `{k}` | {c} |")
    lines.extend(
        [
            "",
            f"drop_requires header vs body: header={drop_header}, body={drop_body}",
            "",
            "## Q7 mut split (known-gap-q7-mut rows)",
            "",
            f"- In PR #2638 audit list: **{q7_in_list}**",
            f"- Same Q7 enforcement class, not audit-listed: **{q7_not_list}**",
            f"- Audit sites loaded: {len(catalog.by_path_name)} distinct (path, binding)",
            "",
            "## Mutant buckets (sum = mutants attempted)",
            "",
            "| Bucket | Count |",
            "| --- | ---: |",
        ]
    )
    bucket_order = list(BUCKETS)
    total = 0
    for b in bucket_order:
        c = buckets.get(b, 0)
        total += c
        lines.append(f"| `{b}` | {c} |")
    for b, c in sorted(buckets.items()):
        if b not in bucket_order:
            total += c
            lines.append(f"| `{b}` | {c} |")
    lines.append(f"| **Total** | **{total}** |")
    if finished and total != mutants_attempted:
        lines.append(
            f"\n**Bucket mismatch:** total {total} vs attempted {mutants_attempted} (investigate)."
        )

    bad_rej_both = [
        r for r in rows if r.bucket == "rejected-both" and (r.current == "rejected" and r.route == "rejected") is False
    ]
    bad_route_outside = [
        r
        for r in rows
        if r.bucket == "valid-route-outside" and not (r.current == "accepted" and route_is_outside(r.route))
    ]
    missing_route = [r for r in rows if not r.route]
    lines.extend(
        [
            "",
            "## Bucket integrity",
            "",
            f"- rejected-both rows with current=rejected AND route=rejected: "
            f"{sum(1 for r in rows if r.bucket == 'rejected-both' and r.current == 'rejected' and r.route == 'rejected')}"
            f" / {buckets.get('rejected-both', 0)}",
            f"- valid-route-outside anomalies: {len(bad_route_outside)}",
            f"- rejected-both anomalies (not both rejected): {len(bad_rej_both)}",
            f"- mutants missing route verdict: {len(missing_route)}",
        ]
    )

    multi: dict[str, int] = {}
    for r in rows:
        multi[r.fixture] = multi.get(r.fixture, 0) + 1
    multi_list = sorted((f, c) for f, c in multi.items() if c > 1)
    lines.extend(["", "## Fixtures with more than one mutant row", ""])
    if not multi_list:
        lines.append("(none)")
    else:
        lines.append(f"Count: {len(multi_list)} fixtures.")
        for f, c in multi_list[:40]:
            lines.append(f"- `{f}`: {c} rows")
        if len(multi_list) > 40:
            lines.append(f"- … and {len(multi_list) - 40} more (see TSV)")

    lines.extend(
        [
            "",
            "## Confirmed holes by mutation kind",
            "",
            "| Kind | Count |",
            "| --- | ---: |",
        ]
    )
    for k, c in sorted(by_kind.items()):
        lines.append(f"| `{k}` | {c} |")
    if not by_kind:
        lines.append("| (none) | 0 |")
    lines.extend(
        [
            "",
            "## Confirmed holes by accepted_by",
            "",
            "| accepted_by | Count |",
            "| --- | ---: |",
        ]
    )
    for k, c in sorted(by_accepted.items()):
        lines.append(f"| `{k}` | {c} |")
    if not by_accepted:
        lines.append("| (none) | 0 |")

    kept = [r for r in rows if r.bucket in ("confirmed-hole", "known-gap-q7-mut", "disagreement-valid")]
    lines.extend(["", "## Kept rows (sample)", ""])
    for r in kept[:40]:
        lines.extend(
            [
                f"### {r.fixture} — {r.kind}",
                "",
                f"- Bucket: `{r.bucket}`",
                f"- Mutation: {r.detail}",
                f"- Mutant: `{r.mutant_path}`",
                f"- Current: `{r.current}`; route: `{r.route}`; accepted_by: `{r.accepted_by}`",
                f"- Route detail (hint only): {r.route_detail[:300] if r.route_detail else '(n/a)'}",
                f"- Broken obligation: `{r.obligation}`",
                f"- (a) Compile-time spec (`{r.rule_file}` § {r.rule_heading}):",
                "",
                f"> {r.rule_quote}",
                f"- Subtype: `{r.hole_subtype or '(n/a)'}`",
                f"- (b) Check-time vs runtime: accepting checker verdict `{r.current}` "
                f"(route `{r.route}`); obligation must be proved at check, not deferred to crash.",
                f"- (c) Minimal diff: see hand-verification section below.",
                "",
            ]
        )

    def pick_diverse(bucket: str, n: int, kinds: list[str] | None = None) -> list[Row]:
        pool = [r for r in rows if r.bucket == bucket]
        if not pool:
            return []
        chosen: list[Row] = []
        seen_kinds: set[str] = set()
        if kinds:
            for want in kinds:
                for r in pool:
                    if r.kind == want and r not in chosen:
                        chosen.append(r)
                        seen_kinds.add(want)
                        break
        for r in pool:
            if len(chosen) >= n:
                break
            if r not in chosen:
                chosen.append(r)
        return chosen[:n]

    hole_kinds = [
        k
        for k in (
            "remove_mut",
            "mut_ref_instead_of_shared",
            "drop_requires",
            "widen_requires",
            "remove_guard_transition",
        )
        if any(r.kind == k and r.bucket == "confirmed-hole" for r in rows)
    ]
    if os.environ.get("SOUNDNESS_SKIP_HAND") == "1":
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return

    samples = pick_diverse("confirmed-hole", 5, hole_kinds)
    if samples:
        lines.extend(["", "## Hand verification (5 confirmed-hole)", ""])
        for r in samples:
            lines.extend(hand_verify_row(r))
            if r.rule_quote:
                lines.extend(["", f"> Spec ({r.rule_file}): {r.rule_quote}", ""])
    discarded = pick_diverse("discarded-valid", 5)
    if discarded:
        lines.extend(["", "## Hand verification (5 discarded-valid)", ""])
        for r in discarded:
            lines.extend(hand_verify_row(r))
    q7_samples = pick_diverse("known-gap-q7-mut", 5)
    if q7_samples:
        lines.extend(["", "## Hand verification (5 known-gap-q7-mut)", ""])
        for r in q7_samples:
            lines.extend(hand_verify_row(r))
            if r.rule_quote:
                lines.extend(["", f"> Spec ({r.rule_file}): {r.rule_quote}", ""])
    vro = pick_diverse("valid-route-outside", 5)
    if vro:
        lines.extend(["", "## Hand verification (5 valid-route-outside)", ""])
        for r in vro:
            lines.extend(hand_verify_row(r))
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def maybe_git_push() -> None:
    if os.environ.get("SOUNDNESS_NO_PUSH") == "1":
        return
    try:
        paths = [
            ".agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
            ".agents/four-jobs-2026-11-12/soundness/q7_gaps.py",
            f".agents/four-jobs-2026-11-12/soundness/shard-{SHARD}.tsv",
            f".agents/four-jobs-2026-11-12/soundness/shard-{SHARD}.md",
        ]
        for rel in list((OUT_ROOT / f"shard-{SHARD}" / "mutants").rglob("main.omg")):
            paths.append(rel.relative_to(REPO).as_posix())
        subprocess.run(
            ["git", "add", *paths],
            cwd=REPO,
            check=False,
            capture_output=True,
        )
        subprocess.run(
            ["git", "commit", "-m", f"docs: soundness shard {SHARD} progress", "--allow-empty"],
            cwd=REPO,
            check=False,
            capture_output=True,
        )
        subprocess.run(
            ["git", "push", "origin", f"work/soundness-mutations-shard-{SHARD}"],
            cwd=REPO,
            check=False,
            capture_output=True,
            timeout=120,
        )
    except (subprocess.TimeoutExpired, OSError):
        pass


def write_fixture_ledger(path: Path, ledger: list[FixtureLedgerRow]) -> None:
    with path.open("w", encoding="utf-8") as f:
        f.write("fixture\ttier\toutcome\tdiagnostic\n")
        for row in ledger:
            f.write(
                f"{row.fixture}\t{row.tier}\t{row.outcome}\t"
                f"{row.diagnostic.replace(chr(9), ' ')[:500]}\n"
            )


def main() -> int:
    catalog = load_q7_catalog()
    OUT_ROOT.mkdir(parents=True, exist_ok=True)
    shard_dir = OUT_ROOT / f"shard-{SHARD}"
    shard_dir.mkdir(parents=True, exist_ok=True)
    tsv_path = shard_dir.with_suffix(".tsv")
    md_path = shard_dir.with_suffix(".md")
    ledger_path = shard_dir.with_suffix(".fixtures.tsv")
    progress_path = shard_dir / "progress.txt"

    fixtures = list_shard_fixtures()
    start = int(progress_path.read_text().strip() or "0") if progress_path.exists() else 0
    rows: list[Row] = []
    fixture_ledger: list[FixtureLedgerRow] = []
    if os.environ.get("SOUNDNESS_FRESH") == "1":
        shutil.rmtree(shard_dir / "mutants", ignore_errors=True)
        progress_path.unlink(missing_ok=True)
        start = 0
    if start == 0:
        tsv_path.write_text(
            "fixture\ttier\tmutation_kind\tdetail\tbucket\tcurrent_verdict\troute_verdict\t"
            "route_detail\tdiscard_reason\tobligation\trule_file\trule_heading\trule_quote\t"
            "accepted_by\thole_subtype\tq7_audit_listed\tmutant_path\tcurrent_seconds\troute_seconds\n",
            encoding="utf-8",
        )
        write_fixture_ledger(ledger_path, [])

    fixtures_zero_mutations = 0
    mutants_attempted = 0
    for idx in range(start, len(fixtures)):
        fixture = fixtures[idx]
        rel = fixture.relative_to(REPO).as_posix()
        tier = "pass" if "/pass/" in rel else "run"
        check_root = check_root_for_fixture(fixture.parent)
        cur0, _, _, _ = run_omega(check_root)
        if cur0 == "timeout":
            fixture_ledger.append(
                FixtureLedgerRow(rel, tier, "failed-run", diagnose_check(check_root))
            )
            progress_path.write_text(str(idx + 1), encoding="utf-8")
            continue
        if cur0 == "error":
            fixture_ledger.append(
                FixtureLedgerRow(rel, tier, "failed-run", diagnose_check(check_root))
            )
            progress_path.write_text(str(idx + 1), encoding="utf-8")
            continue
        if not accepts(cur0):
            fixture_ledger.append(
                FixtureLedgerRow(rel, tier, "original-not-accepted", diagnose_check(check_root))
            )
            progress_path.write_text(str(idx + 1), encoding="utf-8")
            continue
        original = read_source(fixture)
        mutations = generate_mutations(original)
        if not mutations:
            fixtures_zero_mutations += 1
            fixture_ledger.append(FixtureLedgerRow(rel, tier, "no-mutation", ""))
        for m in mutations:
            mutants_attempted += 1
            mid = mutant_id(fixture, m.kind, m.detail)
            try:
                mutant_main = stage_mutant(fixture, m.text, mid)
            except OSError:
                row = Row(
                    fixture=rel,
                    tier=tier,
                    kind=m.kind,
                    detail=m.detail,
                    bucket="failed-run",
                    current="error",
                    route="error",
                    route_detail="",
                    discard_reason="staging copy failed",
                    obligation="",
                    rule_file="",
                    rule_heading="",
                    rule_quote="",
                    accepted_by="neither",
                    hole_subtype="",
                    mutant_path="",
                    current_s=0,
                    route_s=0,
                )
                rows.append(row)
                continue
            current_v, current_s, _, _ = run_omega(mutant_main)
            route_v, route_s, route_out = run_route(mutant_main)
            _, route_detail = parse_route_output(route_out)
            spec = read_spec_verdict(m.kind, original, m.text, m)
            bucket, obligation, rfile, rhead, rquote, accepted, discard_reason, q7_listed = assign_bucket(
                spec, m.kind, rel, m, m.text, current_v, route_v, catalog
            )
            if bucket == "confirmed-hole" and spec_allows_runtime_trap(m.kind, m.text):
                bucket = "discarded-valid"
                discard_reason = (
                    "invalid under read but explicit crash arm may trap at runtime; "
                    "not a compile-time hole"
                )
                obligation = spec.obligation
                rfile = rhead = rquote = ""
            keep = bucket in ("confirmed-hole", "known-gap-q7-mut", "disagreement-valid")
            hole_subtype = ""
            if bucket == "confirmed-hole" and m.kind == "drop_requires" and keep:
                hole_subtype = probe_drop_requires_subtype(
                    m.text, m.meta.get("removed", ""), mutant_main
                )
            elif bucket == "confirmed-hole":
                hole_subtype = "n/a"
            rel_mutant = mutant_main.relative_to(REPO).as_posix()
            if not keep:
                shutil.rmtree(mutant_main.parent, ignore_errors=True)
                rel_mutant = mid
            row = Row(
                fixture=rel,
                tier=tier,
                kind=m.kind,
                detail=m.detail,
                bucket=bucket,
                current=current_v,
                route=route_v,
                route_detail=route_detail.replace("\t", " ")[:500],
                discard_reason=discard_reason.replace("\t", " ")[:500],
                obligation=obligation.replace("\t", " ")[:500],
                rule_file=rfile,
                rule_heading=rhead,
                rule_quote=rquote,
                accepted_by=accepted,
                hole_subtype=hole_subtype,
                q7_audit_listed=q7_listed,
                mutant_path=rel_mutant if keep else "",
                current_s=current_s,
                route_s=route_s,
            )
            rows.append(row)
            with tsv_path.open("a", encoding="utf-8") as f:
                f.write(
                    f"{rel}\t{tier}\t{m.kind}\t{m.detail}\t{bucket}\t{current_v}\t{route_v}\t"
                    f"{row.route_detail}\t{row.discard_reason}\t{row.obligation}\t{rfile}\t{rhead}\t"
                    f"{rquote.replace(chr(9), ' ')[:300]}\t{accepted}\t{hole_subtype}\t{q7_listed}\t"
                    f"{rel_mutant if keep else ''}\t{current_s:.3f}\t{route_s:.3f}\n"
                )
        if mutations:
            fixture_ledger.append(FixtureLedgerRow(rel, tier, "mutated", ""))
        write_fixture_ledger(ledger_path, fixture_ledger)
        progress_path.write_text(str(idx + 1), encoding="utf-8")
        if (idx + 1) % 25 == 0:
            write_md(
                md_path,
                len(fixtures),
                fixture_ledger,
                fixtures_zero_mutations,
                mutants_attempted,
                rows,
                catalog,
                finished=False,
            )
            maybe_git_push()

    write_md(
        md_path,
        len(fixtures),
        fixture_ledger,
        fixtures_zero_mutations,
        mutants_attempted,
        rows,
        catalog,
        finished=True,
    )
    write_fixture_ledger(ledger_path, fixture_ledger)
    maybe_git_push()
    return 0


if __name__ == "__main__":
    sys.exit(main())
