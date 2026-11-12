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
    "discarded-valid",
    "rejected-both",
    "failed-run",
)


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
        return "disagreement-valid", acc
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
    multi = sum(1 for c in mutants_per.values() if c > 1)

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
        f"- Fixtures with ≥1 mutant run: **{fixtures_touched}**",
        f"- `no-mutation` (fixture yielded zero mutants): **{no_mutation}**",
        f"- Mutant rows (deduped fixture×kind): **{total}**",
        f"- Fixtures with >1 mutant kind in TSV: **{multi}** (early harness tried two priorities before single-mut cap).",
        f"- First run logged **1052** checks; final deduped total **{total}** = one row per attempted mutant kind.",
        "",
        "## Bucket table",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for b in BUCKETS:
        if buckets[b]:
            md.append(f"| `{b}` | {buckets[b]} |")
    md.append(f"| **Total mutant rows** | **{total}** |")
    md.append(f"| Meta: `no-mutation` fixtures | {no_mutation} |")
    md.append("")
    md.append(
        "Validity is decided by **reading the mutant** (fix3-B). `route_rejection_hint` is recorded "
        "for `drop_requires` but does not select buckets."
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

    if confirmed:
        md.append("## Confirmed-hole entries")
        md.append("")
        for r in confirmed:
            md.extend(
                [
                    f"### `{r['mutation_kind']}` — `{r['fixture']}`",
                    f"- accepted_by: **{r['accepted_by']}**",
                    f"- {r['validity_note']}",
                    f"- Obligation: {r['obligation_line']}",
                    f"- Spec ({r['spec_file']} → {r['spec_heading']}): {r['spec_quote']}",
                    "",
                ]
            )

    MD.write_text("\n".join(md), encoding="utf-8")
    print("total", total, "buckets", dict(buckets), "confirmed", len(confirmed))


if __name__ == "__main__":
    main()
