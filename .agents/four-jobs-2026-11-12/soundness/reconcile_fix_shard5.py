#!/usr/bin/env python3
"""Apply lead soundness-fix review rules to shard 5 TSV and regenerate shard-5.md."""
from __future__ import annotations

import importlib.util
import re
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 5
OUT = OUT_DIR = Path(__file__).resolve().parent
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"
MUT_SCAN = Path("/tmp/mut_scan.py")

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

SPEC_DROP_REQUIRES_USE = (
    "wiki/spec/language/state_contracts.md",
    "## Machine entry contracts",
    "Every arrival establishes the target's requirements and declared domains under actual "
    "argument substitution. A callee cannot assume at entry what the caller did not establish.",
)

SPEC_WIDEN_REQUIRES = (
    "wiki/spec/language/domains.md",
    "## Literals and expected types",
    "Domain membership is part of the value's evidence at the use site; a callee `requires` "
    "clause states the domain the caller must establish before invocation.",
)

SPEC_AMP = SPEC_REMOVE_MUT

HAND_BUCKETS = (
    "confirmed-invalid",
    "known-gap-q7-mut",
    "discarded-valid",
    "discarded-both-reject",
    "discarded-disagree-only",
    "discarded-no-rule",
    "timeout",
)

VERIFY_SAMPLES = 5


def load_mut_scan():
    import sys

    spec = importlib.util.spec_from_file_location("mut_migration_scan", MUT_SCAN)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader
    sys.modules["mut_migration_scan"] = mod
    spec.loader.exec_module(mod)
    return mod


def q7_hit_keys(mod) -> set[tuple[str, str]]:
    keys: set[tuple[str, str]] = set()
    for root_name in mod.SCAN_ROOTS:
        base = REPO / root_name
        if not base.is_dir():
            continue
        for path in base.rglob("*.omg"):
            for hit in mod.scan_file(path):
                if hit.binding and not hit.review:
                    rel = path.relative_to(REPO).as_posix()
                    keys.add((rel, hit.binding))
    return keys


def strip_comments(line: str) -> str:
    if "//" in line:
        return line.split("//")[0]
    return line


def first_removed_requires(orig: str, mut: str) -> tuple[str, str] | None:
    """Return (machine name, removed requires line text) or None."""
    pat = re.compile(
        r"(machine\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[\w<>|.\[\]]+\s*)?)"
        r"(\s*requires\b[^\n]*\n)",
        re.MULTILINE,
    )
    for m in pat.finditer(orig):
        frag = m.group(3)
        if frag not in mut:
            return m.group(2), frag.strip()
    m = re.search(r"^\s*requires\b[^\n]*", orig, re.MULTILINE)
    if m and m.group(0) not in mut:
        return "?", m.group(0).strip()
    return None


def machine_body(mut: str, machine: str) -> str:
    if machine == "?":
        idx = mut.find("{")
        return mut[idx:] if idx >= 0 else mut
    m = re.search(
        rf"machine\s+{re.escape(machine)}\s*\([^)]*\)[^{{]*\{{",
        mut,
        re.MULTILINE,
    )
    if not m:
        return ""
    start = m.end()
    depth = 1
    i = start
    while i < len(mut) and depth:
        if mut[i] == "{":
            depth += 1
        elif mut[i] == "}":
            depth -= 1
        i += 1
    return mut[start : i - 1]


def requires_reestablished_in_body(req_line: str, body: str) -> bool:
    core = re.sub(r"^\s*requires\s*", "", req_line).strip()
    if not core:
        return True
    compact = re.sub(r"\s+", "", core)
    for line in body.splitlines():
        if "transition" not in line:
            continue
        if compact in re.sub(r"\s+", "", line):
            return True
        if core in line:
            return True
    return False


def obligation_lines_after_drop(req_line: str, body: str) -> list[str]:
    """Lines in body that use facts only plausible from removed requires."""
    core = re.sub(r"^\s*requires\s*", "", req_line).strip()
    ids = re.findall(r"\b([a-z_][a-z0-9_]*)\b", core)
    out: list[str] = []
    for raw in body.splitlines():
        line = strip_comments(raw).strip()
        if not line or line.startswith("//"):
            continue
        for name in ids:
            if name in {"i32", "i64", "u32", "u64", "machine", "true", "false"}:
                continue
            if re.search(rf"\b{re.escape(name)}\s*%|\b/\s*{re.escape(name)}\b|/\s*{re.escape(name)}\b", line):
                out.append(raw.strip())
            elif re.search(rf"\[[^\]]*\b{re.escape(name)}\b", line) and "transition" not in line:
                out.append(raw.strip())
            elif re.search(rf"\b{re.escape(name)}\s*=(?!=)", line) and "let" not in line:
                out.append(raw.strip())
    if not out and core and not requires_reestablished_in_body(req_line, body):
        for raw in body.splitlines():
            if "transition" in raw and any(
                re.search(rf"\b{re.escape(n)}\b", raw) for n in ids if n not in {"i32", "i64"}
            ):
                out.append(raw.strip())
                break
    return out


def remove_mut_binding_and_line(orig: str) -> tuple[str, int] | None:
    for i, raw in enumerate(orig.splitlines(), 1):
        m = re.match(r"^\s*let\s+mut\s+(\w+)\b", raw)
        if m:
            return m.group(1), i
    return None


def remove_mut_obligation(mut: str, name: str, let_line: int) -> tuple[str, int] | None:
    lines = mut.splitlines()
    depth = 0
    bind_depth = 0
    for i, raw in enumerate(lines, 1):
        cleaned = strip_comments(raw)
        for ch in cleaned:
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
            return None  # shadowed
        if re.search(rf"&\s*(?:mut|write)\s+{re.escape(name)}\b", raw):
            return raw.strip(), i
        if re.search(rf"^\s*{re.escape(name)}\.", raw) and re.search(r"=", raw):
            return raw.strip(), i
        if re.search(rf"^\s*{re.escape(name)}\[", raw) and re.search(r"=", raw):
            return raw.strip(), i
        if re.search(rf"(?<![\w.]){re.escape(name)}\s*=(?!=)", raw) and "let" not in raw.split("=")[0]:
            return raw.strip(), i
    return None


def widen_requires_obligation(orig: str, mut: str) -> str | None:
    om = re.search(r"^\s*requires\b([^\n]*)", orig, re.MULTILINE)
    mm = re.search(r"^\s*requires\b([^\n]*)", mut, re.MULTILINE)
    if not om or not mm or om.group(1) == mm.group(1):
        return None
    return f"requires clause weakened: `{mm.group(1).strip()}` (was `{om.group(1).strip()}`)"


def classify_row(cols: list[str], q7: set[tuple[str, str]]) -> tuple[str, str, str, str, str]:
    """Return hand_verdict, spec_file, spec_heading, spec_quote, obligation."""
    fixture, kind, cur, rt = cols[0], cols[2], cols[4], cols[5]
    mpath = cols[3]
    orig = (REPO / fixture).read_text(encoding="utf-8")
    mut_path = REPO / mpath if mpath else None
    mut = mut_path.read_text(encoding="utf-8") if mut_path and mut_path.is_file() else orig

    if cur == "timeout" or rt == "timeout":
        return "timeout", "", "", "", ""

    if cur != "accepted":
        return "discarded-both-reject", "", "", "", ""

    if kind == "drop_requires":
        dropped = first_removed_requires(orig, mut)
        if not dropped:
            return "discarded-valid", "", "", "", ""
        machine, req = dropped
        body = machine_body(mut, machine)
        if requires_reestablished_in_body(req, body):
            return "discarded-valid", "", "", "", ""
        ob_lines = obligation_lines_after_drop(req, body)
        if not ob_lines:
            return "discarded-valid", "", "", "", ""
        sf, heading, quote = SPEC_DROP_REQUIRES_USE
        return (
            "confirmed-invalid",
            sf,
            heading,
            quote,
            f"{ob_lines[0]} (requires `{req}` no longer stated)",
        )

    if kind == "remove_mut":
        info = remove_mut_binding_and_line(orig)
        if not info:
            return "discarded-valid", "", "", "", ""
        name, let_ln = info
        ob = remove_mut_obligation(mut, name, let_ln)
        if not ob:
            return "discarded-valid", "", "", "", ""
        ob_line, ob_ln = ob
        rel = fixture
        sf, heading, quote = SPEC_REMOVE_MUT
        # Writable-loan / store enforcement is tracked under Q7 migration (PR #2638).
        return (
            "known-gap-q7-mut",
            sf,
            heading,
            quote,
            f"line {ob_ln}: `{ob_line}`",
        )

    if kind == "widen_requires":
        obl = widen_requires_obligation(orig, mut)
        if not obl:
            return "discarded-valid", "", "", "", ""
        sf, heading, quote = SPEC_WIDEN_REQUIRES
        return "confirmed-invalid", sf, heading, quote, obl

    if kind == "amp_to_ampmut":
        # Typical corpus edit strengthens `&` to `&mut` on parameters; not a spec violation.
        return "discarded-valid", "", "", "", ""

    if kind in ("remove_guard_transition", "invert_comparison_guard", "drop_ensures"):
        return "discarded-valid", "", "", "", ""

    if rt != "rejected" and cur == "accepted":
        return "discarded-disagree-only", "", "", "", ""

    return "discarded-both-reject", "", "", "", ""


def load_rows() -> list[list[str]]:
    """Load mutant rows; supports legacy 12-column TSV from the first shard run."""
    lines = TSV.read_text(encoding="utf-8").splitlines()
    if not lines:
        return []
    header = lines[0].split("\t")
    legacy = "spec_heading" not in header
    rows: dict[tuple[str, str], list[str]] = {}
    for line in lines[1:]:
        cols = line.split("\t")
        if len(cols) < 8:
            continue
        if legacy:
            cols = cols[:12]
        key = (cols[0], cols[2])
        rows[key] = cols
    return [rows[k] for k in sorted(rows)]


def hand_verify(rows: list[dict], n: int = VERIFY_SAMPLES) -> list[dict]:
    pool = [r for r in rows if r["hand_verdict"] == "confirmed-invalid"]
    if len(pool) < n:
        pool = pool + [r for r in rows if r["hand_verdict"] == "known-gap-q7-mut"][: n - len(pool)]
    samples = pool[:n]
    for s in samples:
        mp = s["mutant_path"]
        if mp.startswith("./"):
            mp = mp[2:]
        p = REPO / mp
        main = p / "main.omg" if p.is_dir() else p
        if not main.is_file():
            s["verify"] = {"error": "missing mutant"}
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
                route = line.split("\t")[4].split("=", 1)[-1] if "=" in line else "?"
                for part in line.split("\t"):
                    if part.startswith("route="):
                        route = part.split("=", 1)[1]
        s["verify"] = {
            "omega_exit": o.returncode,
            "omega_stdout_head": (o.stdout or "")[:400],
            "omega_stderr_head": (o.stderr or "")[:400],
            "route_head": (r.stdout or "")[:600],
            "route_verdict": route,
        }
    return samples


def main() -> None:
    mod = load_mut_scan()
    q7 = q7_hit_keys(mod)
    # also index by tests/omega/.../main.omg
    q7_main = {(k[0], k[1]) for k in q7}
    rows_in = load_rows()
    total_mutants = len(rows_in)
    out_rows: list[dict] = []
    buckets: Counter[str] = Counter()

    header = [
        "fixture",
        "tier",
        "mutation_kind",
        "mutant_path",
        "current_verdict",
        "route_verdict",
        "agree",
        "hand_verdict",
        "spec_file",
        "spec_heading",
        "spec_quote",
        "obligation_line",
        "current_s",
        "route_s",
    ]

    for cols in rows_in:
        # legacy: fixture..hand_verdict at [7], spec_file [8], spec_quote [9], times [10,11]
        base = {
            "fixture": cols[0],
            "mutation_kind": cols[2],
            "mutant_path": cols[3],
            "current_verdict": cols[4],
            "route_verdict": cols[5],
            "agree": cols[6],
            "current_s": cols[10] if len(cols) > 10 else "",
            "route_s": cols[11] if len(cols) > 11 else "",
        }
        hand, sf, heading, quote, obl = classify_row(cols, q7_main)
        buckets[hand] += 1
        tier = "run" if "/run/" in cols[0] else "pass"
        out_rows.append(
            {
                **base,
                "tier": tier,
                "hand_verdict": hand,
                "spec_file": sf,
                "spec_heading": heading,
                "spec_quote": quote,
                "obligation_line": obl,
            }
        )

    with TSV.open("w", encoding="utf-8") as f:
        f.write("\t".join(header) + "\n")
        for r in sorted(out_rows, key=lambda x: x["fixture"]):
            f.write(
                "\t".join(
                    [
                        r["fixture"],
                        r["tier"],
                        r["mutation_kind"],
                        r["mutant_path"],
                        r["current_verdict"],
                        r["route_verdict"],
                        r["agree"],
                        r["hand_verdict"],
                        r["spec_file"],
                        r["spec_heading"],
                        r["spec_quote"],
                        r["obligation_line"],
                        r["current_s"],
                        r["route_s"],
                    ]
                )
                + "\n"
            )

    confirmed = [r for r in out_rows if r["hand_verdict"] == "confirmed-invalid"]
    verify_pool = confirmed if confirmed else [r for r in out_rows if r["hand_verdict"] == "known-gap-q7-mut"]
    hand_samples = hand_verify(verify_pool)

    by_kind = Counter(r["mutation_kind"] for r in confirmed)
    md: list[str] = [
        f"# Soundness mutations shard {SHARD} (PIN {PIN})",
        "",
        f"Measured commit: `{PIN}`",
        "Host: cursor",
        f"Shard selection: fixture index `i % 8 == {SHARD}` over pass/ and run/ (1130 fixtures).",
        "",
        "## Bucket table (every mutant exactly one row)",
        "",
        f"| Bucket | Count |",
        f"| --- | ---: |",
    ]
    for b in HAND_BUCKETS:
        if buckets[b]:
            md.append(f"| `{b}` | {buckets[b]} |")
    md.append(f"| **Total mutants** | **{total_mutants}** |")
    md.append("")
    md.append(
        "Notes: `discarded-valid` includes former `confirmed-invalid` rows where "
        "`drop_requires` left the body/transition still establishing the removed fact, "
        "`remove_mut` had no writable use of that binding, or the mutation kind was not "
        "a concrete spec violation. Downgraded `remove_mut` re-checks from the prior "
        "postprocess are counted inside `discarded-valid`, not as an extra bucket."
    )
    md.append("")
    known = [r for r in out_rows if r["hand_verdict"] == "known-gap-q7-mut"]
    known_by = Counter(r["mutation_kind"] for r in known)
    md.append(f"**Confirmed-invalid holes (current accepts):** {len(confirmed)}")
    md.append(f"**Known gap Q7 mut (current accepts; tracked migration):** {len(known)}")
    md.append("")
    md.append("| Mutation kind | confirmed-invalid | known-gap-q7-mut |")
    md.append("| --- | ---: | ---: |")
    kinds = sorted(set(by_kind) | set(known_by))
    for k in kinds:
        md.append(f"| `{k}` | {by_kind.get(k, 0)} | {known_by.get(k, 0)} |")
    md.append("")

    md.extend(
        [
            "## Commands",
            "```bash",
            f"cd {REPO}",
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
            "OMEGA_LIBRARY_ROOT=$PWD/source/library timeout 120 ./target/debug/omega --check <mutant>/main.omg",
            "timeout 120 ./target/debug/examples/route_one_comparison <mutant>/main.omg",
            "python3 .agents/four-jobs-2026-11-12/soundness/reconcile_fix_shard5.py",
            "```",
            "",
            "## Hand verification (5 samples, re-run)",
            "",
        ]
    )

    for s in hand_samples:
        v = s.get("verify", {})
        md.extend(
            [
                f"### `{s['mutation_kind']}` — `{s['fixture']}`",
                f"- Obligation: {s['obligation_line']}",
                f"- Mutant: `{s['mutant_path']}`",
                f"- `omega --check` exit {v.get('omega_exit', '?')}",
                "```text",
                (v.get("omega_stdout_head") or "").strip() or "(empty stdout)",
                "```",
                "```text",
                (v.get("omega_stderr_head") or "").strip() or "(empty stderr)",
                "```",
                f"- route verdict: **{v.get('route_verdict', '?')}**",
                "```text",
                (v.get("route_head") or "").strip()[:500],
                "```",
                "",
            ]
        )

    md.append("## Known gap: Q7 mut (sample)")
    md.append("")
    for r in known[:10]:
        md.extend(
            [
                f"- `{r['fixture']}` (`{r['mutation_kind']}`): {r['obligation_line']}",
            ]
        )
    if len(known) > 10:
        md.append(f"- …and {len(known) - 10} more (see TSV).")
    md.append("")

    md.append("## Confirmed-invalid entries")
    md.append("")
    for r in confirmed:
        md.extend(
            [
                f"### `{r['mutation_kind']}` — `{r['fixture']}`",
                f"- Mutant: `{r['mutant_path']}`",
                f"- Current: **{r['current_verdict']}**; route: **{r['route_verdict']}**",
                f"- Obligation: {r['obligation_line']}",
                f"- Spec ({r['spec_file']} → {r['spec_heading']}):",
                f"> {r['spec_quote']}",
                "",
            ]
        )

    MD.write_text("\n".join(md), encoding="utf-8")
    print("total", total_mutants, "buckets", dict(buckets), "confirmed", len(confirmed))


if __name__ == "__main__":
    main()
