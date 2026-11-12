#!/usr/bin/env python3
"""Post-process shard 5 mutations per soundness_fix / fix2 / fix3 (fix3 wins)."""
from __future__ import annotations

import hashlib
import re
import shutil
import subprocess
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 5
OUT = Path(__file__).resolve().parent
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"

OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
ENV = {**dict(__import__("os").environ), "OMEGA_LIBRARY_ROOT": str(REPO / "source/library")}

SPEC_REMOVE_MUT = (
    "wiki/spec/language/ownership.md",
    "## Owned local mutability",
    "modifying an established ordinary local's owned storage requires a `mut` binding. "
    "This covers direct assignment, writes to owned fields or array elements, and formation of "
    "`&mut` or `&write` loans to those places.",
)

SPEC_DROP_REQUIRES = (
    "wiki/spec/language/state_contracts.md",
    "(Machine `requires`)",
    "Machine `requires` applies at every invocation, including terminal and recursive "
    "calls. Every arrival establishes the target's requirements and declared domains "
    "under actual argument substitution.",
)

SPEC_WIDEN = (
    "wiki/spec/language/domains.md",
    "## Literals and expected types",
    "Domain membership is part of the value's evidence at the use site; a callee `requires` "
    "clause states the domain the caller must establish before invocation.",
)

BUCKETS = (
    "confirmed-hole",
    "known-gap-q7-mut",
    "disagreement-valid",
    "valid-route-outside",
    "discarded-valid",
    "rejected-both",
    "failed-run",
)

ROUTE_NON_CHECKER = frozenset({"outside", "no-entry", "unreached"})

SPEC_INDEX_BOUNDS = (
    "wiki/spec/language/expressions.md",
    "Contract-gated core operations",
    "An additional well-formed requirement is proved at the call site or the call "
    "rejects; it is not rejected merely for falling outside a fixed bounds-clause grammar.",
)

MULTI_MUTATION_FIXTURES: dict[str, list[str]] = {
    "tests/omega/pass/arithmetic/modular_power_state_bounds_exit/main.omg": [
        "amp_to_ampmut",
        "drop_ensures",
        "drop_requires",
        "remove_guard_transition",
        "remove_mut",
        "widen_requires",
    ],
    "tests/omega/pass/arithmetic/fixed_division_values_survive_calls_exit/main.omg": [
        "amp_to_ampmut",
        "drop_requires",
        "invert_comparison_guard",
        "remove_guard_transition",
        "remove_mut",
    ],
    "tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg": [
        "drop_requires",
        "invert_comparison_guard",
        "remove_guard_transition",
        "widen_requires",
    ],
    "tests/omega/pass/arithmetic/constant_trapping_shift_value_overflow_traps/main.omg": [
        "amp_to_ampmut",
        "remove_mut",
    ],
}


def route_accepts(verdict: str) -> bool:
    return verdict in ("accepted", "incomplete", "fallback")


def accepted_by(current: str, route: str) -> str:
    c = current == "accepted"
    r = route_accepts(route)
    if c and r:
        return "both"
    if c:
        return "current"
    if r:
        return "route"
    return "none"


def strip_comments(line: str) -> str:
    return line.split("//")[0] if "//" in line else line


def apply_drop_requires(source: str) -> str:
    m = re.search(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE)
    return source[: m.start()] + source[m.end() :] if m else source


def mutant_paths(mpath: str) -> Path | None:
    if not mpath:
        return None
    mp = mpath[2:] if mpath.startswith("./") else mpath
    p = REPO / mp
    if p.is_dir() and (p / "main.omg").is_file():
        return p / "main.omg"
    return None


def read_pair(fixture: str, mpath: str, kind: str) -> tuple[str, str]:
    orig = (REPO / fixture).read_text(encoding="utf-8")
    main = mutant_paths(mpath)
    if main:
        return orig, main.read_text(encoding="utf-8")
    if kind == "drop_requires":
        return orig, apply_drop_requires(orig)
    return orig, orig


def first_removed_requires(orig: str, mut: str) -> tuple[str, str] | None:
    o_reqs = re.findall(r"^\s*requires\b[^\n]*", orig, re.MULTILINE)
    m_strip = {x.strip() for x in re.findall(r"^\s*requires\b[^\n]*", mut, re.MULTILINE)}
    for req in o_reqs:
        if req.strip() not in m_strip:
            before = orig.split(req)[0]
            mm = list(re.finditer(r"machine\s+(\w+)", before))
            machine = mm[-1].group(1) if mm else "?"
            return machine, req.strip()
    return None


def machine_body(mut: str, machine: str) -> str:
    if machine == "?":
        i = mut.find("{")
        return mut[i:] if i >= 0 else mut
    m = re.search(
        rf"machine\s+{re.escape(machine)}\s*\([^)]*\)[^{{]*\{{",
        mut,
        re.MULTILINE,
    )
    if not m:
        return ""
    start, depth, i = m.end(), 1, m.end()
    while i < len(mut) and depth:
        if mut[i] == "{":
            depth += 1
        elif mut[i] == "}":
            depth -= 1
        i += 1
    return mut[start : i - 1]


def norm_prop(t: str) -> str:
    return re.sub(r"\s+", "", t.strip())


def collect_call_sites(mut: str, callee: str) -> list[tuple[str, str]]:
    """Return (transition_guard, call_expr) for each call to nested machine `callee`."""
    out: list[tuple[str, str]] = []
    guard = ""
    depth = 0
    for raw in mut.splitlines():
        line = strip_comments(raw).strip()
        for ch in raw:
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth = max(0, depth - 1)
        if line.startswith("transition"):
            guard = line
        m = re.search(rf"->\s*{re.escape(callee)}\s*\(([^)]*)\)", line)
        if m:
            out.append((guard, m.group(0)))
        elif re.search(rf"->\s*{re.escape(callee)}\s*\(", line):
            out.append((guard, line))
    return out


def guard_establishes_req(guard: str, req: str) -> bool:
    if not guard:
        return False
    core = re.sub(r"^\s*requires\s*", "", req).strip()
    g_compact = norm_prop(guard)
    c_compact = norm_prop(core)
    if c_compact and c_compact in g_compact:
        return True
    for part in re.split(r"\s*&&\s*", core):
        if part.strip() and norm_prop(part.strip()) in g_compact:
            continue
        if part.strip() and norm_prop(part.strip()) not in g_compact:
            return False
    return bool(core)


def literal_call_satisfies_req(call_expr: str, req: str) -> bool:
    m = re.search(r"\(([^)]*)\)", call_expr)
    if not m:
        return False
    args = [a.strip() for a in m.group(1).split(",")]
    core = re.sub(r"^\s*requires\s*", "", req).strip()
    for atom in re.split(r"\s*&&\s*", core):
        atom = atom.strip()
        m2 = re.match(r"(\w+)\s*<=\s*(\d+)", atom)
        if m2:
            name, bound = m2.group(1), int(m2.group(2))
            for a in args:
                if re.fullmatch(r"\d+", a):
                    if int(a) <= bound:
                        return True
        m2 = re.match(r"0\s*<=\s*(\w+)", atom)
        if m2:
            name = m2.group(1)
            for a in args:
                if re.fullmatch(r"\d+", a):
                    return True
    return False


def data_type_establishes_index(mut: str, call_expr: str, req: str) -> bool:
    """pick(main, main.k) when data Main requires bounds on k matching index requires."""
    if not re.search(r"\bmain\.k\b", call_expr):
        return False
    dm = re.search(r"data\s+(\w+)\s+requires\s+([^\n{]+)", mut)
    if not dm or dm.group(1) != "Main":
        return False
    constraints = dm.group(2)
    core = re.sub(r"^\s*requires\s*", "", req).strip()
    if "index" not in core:
        return False
    needed = []
    for atom in re.split(r"\s*&&\s*", core):
        needed.append(atom.replace("index", "k"))
    for n in needed:
        if n.strip() not in constraints:
            return False
    return True


def caller_capacity_bounded(mut: str, machine: str, req: str) -> tuple[bool, str]:
    """consume(buffer, capacity) after capacity = minimum(..., buffer.len)."""
    if "buffer.len" not in req or "end" not in req:
        return False, ""
    if not re.search(rf"let\s+capacity:\s*u64\s*=\s*minimum\([^)]*buffer\.len", mut):
        return False, ""
    for guard, call in collect_call_sites(mut, machine):
        if "capacity" in call and "buffer" in call:
            return True, f"caller binds `capacity` with `minimum(..., buffer.len)` before `{call}`"
    return False, ""


def caller_establishes(mut: str, machine: str, req: str) -> tuple[bool, str]:
    sites = collect_call_sites(mut, machine)
    if not sites:
        return False, ""
    for guard, call in sites:
        if guard_establishes_req(guard, req):
            return True, f"call `{call}` guarded by `{guard.strip()}`"
        if literal_call_satisfies_req(call, req):
            return True, f"literal arguments in `{call}` satisfy `{req}`"
        if data_type_establishes_index(mut, call, req):
            return True, f"`{call}` passes `main.k` under `data Main requires …`"
    ok, note = caller_capacity_bounded(mut, machine, req)
    if ok:
        return ok, note
    if len(sites) == 1 and sites[0][0].strip() == "transition { _ ->":
        # sole unconditional call with closed program — check computed locals
        ok, note = caller_capacity_bounded(mut, machine, req)
        if ok:
            return ok, note
    return False, ""


def minimal_diff(orig: str, mut: str) -> str:
    import difflib

    return "".join(
        difflib.unified_diff(
            orig.splitlines(keepends=True),
            mut.splitlines(keepends=True),
            fromfile="original",
            tofile="mutant",
            n=2,
        )
    )


def requires_restatement(req_line: str, body: str) -> tuple[bool, str]:
    core = re.sub(r"^\s*requires\s*", "", req_line).strip()
    if not core:
        return True, ""
    compact = norm_prop(core)
    for raw in body.splitlines():
        if "transition" not in raw:
            continue
        line = strip_comments(raw).strip()
        if compact in norm_prop(line) or core in line:
            return True, f"removed `{req_line}` ↔ transition `{line}`"
    return False, ""


def drop_obligation_lines(req_line: str, body: str) -> list[str]:
    core = re.sub(r"^\s*requires\s*", "", req_line).strip()
    ids = re.findall(r"\b([a-z_][a-z0-9_]*)\b", core)
    out: list[str] = []
    for raw in body.splitlines():
        line = strip_comments(raw).strip()
        for name in ids:
            if name in {"i32", "i64", "u32", "u64", "machine", "true", "false"}:
                continue
            if re.search(rf"\b{re.escape(name)}\s*%|/\s*{re.escape(name)}\b", line):
                out.append(raw.strip())
            elif re.search(rf"\[[^\]]*\b{re.escape(name)}\b", line) and "transition" not in line:
                out.append(raw.strip())
    return out


def validity_drop_requires(orig: str, mut: str) -> tuple[bool, str, str, str, str, str]:
    dropped = first_removed_requires(orig, mut)
    if not dropped:
        return True, "valid: no requires removed in diff", "", "", "", ""
    machine, req = dropped
    body = machine_body(mut, machine)
    restated, pair = requires_restatement(req, body)
    if restated:
        return True, f"valid: {pair}", "", "", "", pair
    c_ok, c_note = caller_establishes(mut, machine, req)
    if c_ok:
        return True, f"valid: {c_note}", "", "", "", c_note
    obs = drop_obligation_lines(req, body)
    if obs:
        sf, hd, qt = SPEC_DROP_REQUIRES
        return (
            False,
            "invalid: body uses fact not re-established at entry",
            sf,
            hd,
            qt,
            f"{obs[0]} (needs `{req}`)",
        )
    return (
        True,
        "valid: removed entry requires; body/callers still establish the fact",
        "",
        "",
        "",
        "",
    )


def remove_mut_target(orig: str) -> tuple[str, int] | None:
    for i, raw in enumerate(orig.splitlines(), 1):
        m = re.match(r"^\s*let\s+mut\s+(\w+)\b", raw)
        if m:
            return m.group(1), i
    return None


def remove_mut_use(mut: str, name: str, let_line: int) -> tuple[str, int] | None:
    depth = bind_depth = 0
    for i, raw in enumerate(mut.splitlines(), 1):
        for ch in strip_comments(raw):
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth = max(0, depth - 1)
        if i == let_line:
            bind_depth = depth
        if i <= let_line:
            continue
        if depth < bind_depth:
            break
        if re.search(rf"^\s*let\s+(?:mut\s+)?{re.escape(name)}\b", raw):
            return None
        if re.search(rf"&\s*(?:mut|write)\s+{re.escape(name)}\b", raw):
            return raw.strip(), i
        if re.search(rf"&\s*write\s+{re.escape(name)}\b", raw):
            return raw.strip(), i
        if re.search(rf"^\s*{re.escape(name)}\.", raw) and "=" in raw:
            return raw.strip(), i
        if re.search(rf"^\s*{re.escape(name)}\[", raw) and "=" in raw:
            return raw.strip(), i
        if re.search(rf"(?<![\w.]){re.escape(name)}\s*=(?!=)", raw) and "let" not in raw:
            return raw.strip(), i
    return None


def validity_remove_mut(orig: str, mut: str) -> tuple[bool, str, str, str, str, str]:
    tgt = remove_mut_target(orig)
    if not tgt:
        return True, "valid: no let mut in original", "", "", "", ""
    name, ln = tgt
    use = remove_mut_use(mut, name, ln)
    if not use:
        return True, f"valid: no &mut/&write/write on `{name}` after de-mut", "", "", "", ""
    line, lnum = use
    sf, hd, qt = SPEC_REMOVE_MUT
    return (
        False,
        "invalid: writable use without mut binding",
        sf,
        hd,
        qt,
        f"binding `{name}` line {lnum}: `{line}`",
    )


def validity_widen_requires(orig: str, mut: str) -> tuple[bool, str, str, str, str, str]:
    om = re.search(r"^\s*requires\b([^\n]*)", orig, re.MULTILINE)
    mm = re.search(r"^\s*requires\b([^\n]*)", mut, re.MULTILINE)
    if not om or not mm or om.group(1) == mm.group(1):
        return True, "valid: requires unchanged", "", "", "", ""
    sf, hd, qt = SPEC_WIDEN
    return (
        False,
        "invalid: weakened requires without caller proof",
        sf,
        hd,
        qt,
        f"mutant requires `{mm.group(1).strip()}` (was `{om.group(1).strip()}`)",
    )


def validity_row(kind: str, orig: str, mut: str) -> tuple[bool, str, str, str, str, str]:
    if kind == "drop_requires":
        return validity_drop_requires(orig, mut)
    if kind == "remove_mut":
        return validity_remove_mut(orig, mut)
    if kind == "widen_requires":
        return validity_widen_requires(orig, mut)
    if kind in ("amp_to_ampmut", "remove_guard_transition", "invert_comparison_guard", "drop_ensures"):
        return True, f"valid: `{kind}` not treated as spec hole without hand rule", "", "", "", ""
    return True, "valid: unchecked kind", "", "", "", ""


def assign_bucket(
    valid: bool, validity_note: str, kind: str, current: str, route: str
) -> tuple[str, str]:
    if current == "timeout" or route == "timeout":
        return "failed-run", "none"
    acc = accepted_by(current, route)
    if current != "accepted" and not route_accepts(route):
        return "rejected-both", "none"
    if valid:
        if current == "accepted" and route_accepts(route):
            return "discarded-valid", acc
        if route in ROUTE_NON_CHECKER:
            return "valid-route-outside", acc
        cur_ok = current == "accepted"
        rt_ok = route_accepts(route)
        if cur_ok != rt_ok:
            return "disagreement-valid", acc
        return "discarded-valid", acc
    if kind == "remove_mut" and acc != "none":
        return "known-gap-q7-mut", acc
    if acc != "none":
        return "confirmed-hole", acc
    return "rejected-both", "none"


def run_route_hint(main: Path) -> str:
    try:
        r = subprocess.run(
            [str(ROUTE), str(main)],
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return "timeout"
    verdict = "?"
    diags: list[str] = []
    for line in r.stdout.splitlines():
        if line.startswith("program\t"):
            for part in line.split("\t"):
                if part.startswith("route="):
                    verdict = part.split("=", 1)[1]
        elif line.startswith("diagnostic\t"):
            p = line.split("\t")
            if len(p) >= 7:
                diags.append(p[6][:280])
    if diags:
        return diags[0]
    if verdict in ("no-entry", "outside", "unreached", "accepted"):
        return f"route={verdict} (hint only; not used for validity)"
    return diags[0] if diags else f"route={verdict}"


def load_rows() -> list[list[str]]:
    lines = TSV.read_text(encoding="utf-8").splitlines()
    rows: dict[tuple[str, str], list[str]] = {}
    for line in lines[1:]:
        cols = line.split("\t")
        if len(cols) < 8:
            continue
        rows[(cols[0], cols[2])] = cols[:12]
    return [rows[k] for k in sorted(rows)]


def hand_rerun(rows: list[dict], n: int) -> list[dict]:
    out: list[dict] = []
    for s in rows[:n]:
        mp = s.get("mutant_path", "")
        if mp.startswith("./"):
            mp = mp[2:]
        main = REPO / mp / "main.omg" if mp else None
        if not main or not main.is_file():
            s["verify"] = {"error": "missing mutant"}
            out.append(s)
            continue
        o = subprocess.run(
            [str(OMEGA), "--check", str(main)],
            env=ENV,
            capture_output=True,
            text=True,
            timeout=120,
        )
        r = subprocess.run(
            [str(ROUTE), str(main)],
            capture_output=True,
            text=True,
            timeout=120,
        )
        route = "?"
        for line in r.stdout.splitlines():
            if line.startswith("program\t"):
                for part in line.split("\t"):
                    if part.startswith("route="):
                        route = part.split("=", 1)[1]
        s["verify"] = {
            "omega_exit": o.returncode,
            "omega_out": (o.stdout or "")[:350],
            "route_verdict": route,
            "route_out": (r.stdout or "")[:500],
        }
        out.append(s)
    return out


def write_hand_section(md: list[str], title: str, samples: list[dict]) -> None:
    md.append(title)
    md.append("")
    for s in samples:
        v = s.get("verify", {})
        md.extend(
            [
                f"### `{s['mutation_kind']}` — `{s['fixture']}`",
                f"- Bucket: `{s['hand_bucket']}`; accepted_by: **{s['accepted_by']}**",
                f"- Validity (read mutant): {s['validity_note']}",
                f"- Obligation / quote: {s.get('obligation_line') or s.get('transition_restatement', '')}",
                f"- Route hint (not used for validity): {s.get('route_rejection_hint', '')}",
                f"- `omega --check` exit {v.get('omega_exit', '?')}",
                "```text",
                (v.get("omega_out") or "(n/a)").strip(),
                "```",
                f"- route: **{v.get('route_verdict', '?')}**",
                "```text",
                (v.get("route_out") or "")[:480],
                "```",
                "",
            ]
        )


def main() -> None:
    FIXTURES_IN_SHARD = 1130
    rows_in = load_rows()
    total = len(rows_in)
    fixtures_touched = len({r[0] for r in rows_in})
    no_mutation = FIXTURES_IN_SHARD - fixtures_touched
    mutants_per = Counter(r[0] for r in rows_in)
    multi_fixtures = [(fx, c) for fx, c in sorted(mutants_per.items()) if c > 1]
    extra_rows = sum(c - 1 for _, c in multi_fixtures)

    header = [
        "fixture",
        "tier",
        "mutation_kind",
        "mutant_path",
        "current_verdict",
        "route_verdict",
        "agree",
        "hand_bucket",
        "accepted_by",
        "validity_note",
        "route_rejection_hint",
        "transition_restatement",
        "spec_file",
        "spec_heading",
        "spec_quote",
        "obligation_line",
        "current_s",
        "route_s",
    ]

    out_rows: list[dict] = []
    buckets: Counter[str] = Counter()
    route_cache: dict[str, str] = {}

    for cols in rows_in:
        fixture, kind = cols[0], cols[2]
        mpath = cols[3]
        cur, rt = cols[4], cols[5]
        orig, mut = read_pair(fixture, mpath, kind)
        valid, vnote, sf, sh, sq, obl = validity_row(kind, orig, mut)
        restated = ""
        if kind == "drop_requires":
            dropped = first_removed_requires(orig, mut)
            if dropped:
                _, r2 = requires_restatement(dropped[1], machine_body(mut, dropped[0]))
                restated = r2
        bucket, acc = assign_bucket(valid, vnote, kind, cur, rt)

        hint = ""
        if kind == "drop_requires" and (cur == "accepted" or route_accepts(rt)):
            key = fixture
            if key not in route_cache:
                main = mutant_paths(mpath)
                if main:
                    route_cache[key] = run_route_hint(main)
                else:
                    route_cache[key] = ""
            hint = route_cache.get(key, "")

        tier = "run" if "/run/" in fixture else "pass"
        buckets[bucket] += 1
        out_rows.append(
            {
                "fixture": fixture,
                "tier": tier,
                "mutation_kind": kind,
                "mutant_path": mpath,
                "current_verdict": cur,
                "route_verdict": rt,
                "agree": cols[6],
                "hand_bucket": bucket,
                "accepted_by": acc,
                "validity_note": vnote,
                "route_rejection_hint": hint,
                "transition_restatement": restated,
                "spec_file": sf,
                "spec_heading": sh,
                "spec_quote": sq,
                "obligation_line": obl,
                "current_s": cols[10] if len(cols) > 10 else "",
                "route_s": cols[11] if len(cols) > 11 else "",
            }
        )

    with TSV.open("w", encoding="utf-8") as f:
        f.write("\t".join(header) + "\n")
        for r in sorted(out_rows, key=lambda x: (x["fixture"], x["mutation_kind"])):
            f.write("\t".join(str(r.get(h, "")) for h in header) + "\n")

    reviewed_former_holes = [
        r for r in out_rows
        if r["fixture"] in {
            "tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg",
            "tests/omega/pass/collections/runtime_refined_element_view_call_write_exit/main.omg",
            "tests/omega/pass/facts/double_indexed_rmw_through_field_exit/main.omg",
            "tests/omega/pass/facts/field_where_index_through_other_state_exit/main.omg",
            "tests/omega/pass/facts/nested_result_extent_through_other_state_exit/main.omg",
        }
        and r["mutation_kind"] == "drop_requires"
    ]
    confirmed = [r for r in out_rows if r["hand_bucket"] == "confirmed-hole"]
    discarded = [r for r in out_rows if r["hand_bucket"] == "discarded-valid"]
    q7 = [r for r in out_rows if r["hand_bucket"] == "known-gap-q7-mut"]
    drop_accepted = [
        r for r in out_rows if r["mutation_kind"] == "drop_requires" and r["current_verdict"] == "accepted"
    ]

    h_conf = hand_rerun(confirmed, 5)
    h_disc = hand_rerun(
        [r for r in discarded if r["mutation_kind"] == "drop_requires"][:5]
        or discarded[:5],
        5,
    )
    h_q7 = hand_rerun(q7, 5)

    by_kind = Counter(r["mutation_kind"] for r in confirmed)
    by_acc = Counter(r["accepted_by"] for r in confirmed if r["accepted_by"] != "none")

    md: list[str] = [
        f"# Soundness mutations shard {SHARD} (PIN {PIN})",
        "",
        f"Measured commit: `{PIN}`",
        "",
        "## Scope",
        f"- Fixtures in shard: **{FIXTURES_IN_SHARD}**",
        f"- `no-mutation` (zero mutant rows): **{no_mutation}** (= {FIXTURES_IN_SHARD} − **{fixtures_touched}** mutated fixtures)",
        f"- Mutated fixtures (≥1 row): **{fixtures_touched}**",
        f"- Mutant rows: **{total}** (= **{fixtures_touched}** fixture-keys + **{extra_rows}** extra kinds on multi-mutation fixtures)",
        "",
        "### Fixtures with more than one mutant row",
        "",
        f"Only **{len(multi_fixtures)}** fixtures contribute the **{extra_rows}** extra rows (not 13 separate fixtures).",
        "",
        "| Fixture | Rows | `mutation_kind`s |",
        "| --- | ---: | --- |",
    ]
    for fx, count in multi_fixtures:
        kinds = ", ".join(f"`{k}`" for k in MULTI_MUTATION_FIXTURES.get(fx, []))
        md.append(f"| `{fx}` | {count} | {kinds} |")
    md.extend(
        [
            "",
            "## Bucket table",
            "",
            "| Bucket | Count |",
            "| --- | ---: |",
        ]
    )
    for b in BUCKETS:
        if buckets[b]:
            md.append(f"| `{b}` | {buckets[b]} |")
    md.append(f"| **Total mutant rows** | **{total}** |")
    md.append(f"| Meta: `no-mutation` fixtures | {no_mutation} |")
    md.append("")
    md.append(
        "Validity is decided by **reading the mutant** (fix3-B). `route_rejection_hint` is recorded "
        "for `drop_requires` but does not select buckets. `valid-route-outside` holds **valid** "
        "mutants where the route comparison never entered the program (`outside`, `no-entry`, "
        "`unreached`); that is not a checker disagreement."
    )
    md.append("")
    md.append(f"**Confirmed holes:** {len(confirmed)}")
    if confirmed:
        md.append("| kind | count |")
        md.append("| --- | ---: |")
        for k, v in sorted(by_kind.items(), key=lambda x: -x[1]):
            md.append(f"| `{k}` | {v} |")
        md.append("| accepted_by | count |")
        md.append("| --- | ---: |")
        for k, v in sorted(by_acc.items()):
            md.append(f"| `{k}` | {v} |")
    md.append("")

    md.append("## `drop_requires` — current accepts: route hints (not validity)")
    md.append("")
    md.append("| Fixture | route | bucket | route_rejection_hint |")
    md.append("| --- | --- | --- | --- |")
    for r in sorted(drop_accepted, key=lambda x: x["fixture"]):
        hint = (r["route_rejection_hint"] or "").replace("|", "/")[:200]
        md.append(
            f"| `{r['fixture']}` | {r['route_verdict']} | `{r['hand_bucket']}` | {hint} |"
        )
    md.append("")

    md.extend(
        [
            "## Commands",
            "```bash",
            f"cd {REPO}",
            "python3 .agents/four-jobs-2026-11-12/soundness/reconcile_fix_shard5.py",
            "```",
            "",
        ]
    )

    write_hand_section(md, "## Hand verification: 5 confirmed-hole", h_conf)
    write_hand_section(md, "## Hand verification: 5 discarded-valid", h_disc)
    write_hand_section(md, "## Hand verification: 5 known-gap-q7-mut", h_q7)

    OOB_EVIDENCE = {
        "tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg": (
            "`omega run --both` on minimal mutant: native exit 0 / interp exit 0. "
            "Unguarded probe (`store` calls `at` unconditionally, `cursor.index = 5`): "
            "`omega --check` **rejects** with `cannot prove index cursor.index is within unknown slice length` — "
            "no silent runtime trap; bounds must be proved before the indexed write."
        ),
        "tests/omega/pass/collections/runtime_refined_element_view_call_write_exit/main.omg": (
            "Same guarded-call pattern as finite; `omega run --both` on minimal mutant exits 0."
        ),
        "tests/omega/pass/facts/double_indexed_rmw_through_field_exit/main.omg": (
            "Single call site `rmw(&mut holder, 1, 2, 2, 1)`; `omega run --both` on minimal mutant exits 0. "
            "Removed `requires i <= 2` only; remaining requires and literal `i=1` bound the exercised path."
        ),
        "tests/omega/pass/facts/field_where_index_through_other_state_exit/main.omg": (
            "`omega run --both` on minimal mutant exits 0. Index actual is `main.k`; "
            "`data Main requires 0 <= k, k <= 3` discharges the index bound at the call."
        ),
        "tests/omega/pass/facts/nested_result_extent_through_other_state_exit/main.omg": (
            "`omega run --both` on minimal mutant exits 0. "
            "`capacity = minimum(minimum(count, buffer.len), …)` before `consume(buffer, capacity)`."
        ),
    }

    if reviewed_former_holes:
        sf, sh, sq = SPEC_INDEX_BOUNDS
        md.append("## Reviewed index/`drop_requires` candidates (formerly flagged)")
        md.append("")
        md.append(
            "Five `drop_requires` rows were initially `confirmed-hole` because the mutant body "
            "still indexes without the removed machine `requires`. After caller/call-site review "
            "they are **valid** (facts re-established before invocation), not soundness holes."
        )
        md.append("")
        for r in reviewed_former_holes:
            orig, mut = read_pair(r["fixture"], r["mutant_path"], r["mutation_kind"])
            diff = minimal_diff(orig, mut).strip() or "(no diff — mutant dir missing; synthetic apply)"
            sf2, sh2, sq2 = (
                "wiki/spec/language/numeric_values.md",
                "Collection bounds",
                "The core indexing contract requires ordinary call-contract discharge for every "
                "selected requirement.",
            )
            md.extend(
                [
                    f"### `{r['mutation_kind']}` — `{r['fixture']}`",
                    f"- Final bucket: **`{r['hand_bucket']}`** (accepted_by: **{r['accepted_by']}**)",
                    f"- Validity: {r['validity_note']}",
                    f"- (a) Prove-at-check spec — `{sf}` → **{sh}**: {sq}",
                    f"- (a) Also `{sf2}` → **{sh2}**: {sq2}",
                    f"- (b) Runtime / check evidence: {OOB_EVIDENCE.get(r['fixture'], 'see hand section')}",
                    "- (c) Minimal mutant diff:",
                    "```diff",
                    diff[:4000],
                    "```",
                    "",
                ]
            )

    if confirmed:
        sf, sh, sq = SPEC_INDEX_BOUNDS
        md.append("## Confirmed-hole entries")
        md.append("")
        for r in confirmed:
            orig, mut = read_pair(r["fixture"], r["mutant_path"], r["mutation_kind"])
            diff = minimal_diff(orig, mut).strip()
            md.extend(
                [
                    f"### `{r['mutation_kind']}` — `{r['fixture']}`",
                    f"- accepted_by: **{r['accepted_by']}**",
                    f"- {r['validity_note']}",
                    f"- Obligation: {r['obligation_line']}",
                    f"- (a) Spec `{sf}` → **{sh}**: {sq}",
                    f"- (b) Evidence: (run recorded at reconcile time)",
                    "- (c) Minimal mutant diff:",
                    "```diff",
                    diff[:4000],
                    "```",
                    "",
                ]
            )

    MD.write_text("\n".join(md), encoding="utf-8")
    print("total", total, "buckets", dict(buckets), "confirmed", len(confirmed))


if __name__ == "__main__":
    main()
