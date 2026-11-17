#!/usr/bin/env python3
"""PILOT ONLY: mutate accepted Omega programs to stress facts and aliasing.

Reads one single-file sample (main.omg + build.omg), finds machine bodies,
their top-level statements and the typed variables in scope, and writes
mutants that put a write, a `&mut` loan, a reorder or a runtime index between
where a fact is established and where it is used. A sound checker either
refuses a mutant or the mutant still runs with every proved fact true.

Usage: soundness_mutator.py <sample_dir> <out_dir> [--per-program N] [--seed S]
"""
import json
import os
import random
import re
import sys

SCALARS = {
    "u8": (0, 255), "u16": (0, 65535), "u32": (0, 4294967295),
    "u64": (0, 18446744073709551615), "i8": (-128, 127), "i16": (-32768, 32767),
    "i32": (-2147483648, 2147483647), "i64": (-9223372036854775808, 9223372036854775807),
    "usize": (0, 18446744073709551615), "bool": None,
}


SWAPS = os.environ.get("PILOT_SWAPS", "1") == "1"


def strip_comments(text):
    return re.sub(r"//[^\n]*", "", text)


def match_brace(text, start):
    """Index just past the brace matching text[start] == '{'."""
    depth = 0
    for i in range(start, len(text)):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i + 1
    return None


def split_top(text, sep=","):
    parts, depth, cur = [], 0, ""
    for c in text:
        if c in "([{<":
            depth += 1
        elif c in ")]}>":
            depth -= 1
        if c == sep and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += c
    if cur.strip():
        parts.append(cur)
    return [p.strip() for p in parts]


def parse_data(text):
    datas = {}
    for m in re.finditer(r"\bdata\s+(\w+)\s*(<[^>{]*>)?\s*(\[[^\]]*\])?([^{;]*)\{", text):
        name = m.group(1)
        end = match_brace(text, m.end() - 1)
        if end is None:
            continue
        body = text[m.end():end - 1]
        fields, cases = {}, []
        depth = 0
        flat = ""
        for c in body:
            if c == "{":
                depth += 1
            if depth == 0:
                flat += c
            if c == "}":
                depth -= 1
        for stmt in flat.split(";"):
            stmt = stmt.strip()
            if stmt.startswith("case "):
                cases.append(stmt)
                continue
            fm = re.match(r"^(\w+)\s*:\s*(.+)$", stmt)
            if fm:
                fields[fm.group(1)] = fm.group(2).strip()
        datas[name] = {
            "fields": fields,
            "cases": cases,
            "generic": bool(m.group(2)),
            "copy": bool(m.group(3) and "copy" in m.group(3)),
            "requires": "requires" in (m.group(4) or ""),
        }
    return datas


MACHINE = re.compile(r"\bmachine\s+([\w:]+)\s*(<[^>(]*>)?\s*\(")


def parse_machines(text):
    machines = []
    for m in MACHINE.finditer(text):
        # parameter list
        i = m.end() - 1
        depth = 0
        for j in range(i, len(text)):
            if text[j] == "(":
                depth += 1
            elif text[j] == ")":
                depth -= 1
                if depth == 0:
                    break
        params_text = text[i + 1:j]
        brace = text.find("{", j)
        semi = text.find(";", j)
        if brace < 0 or (0 <= semi < brace and "requires" not in text[j:semi]):
            continue
        header = text[j + 1:brace]
        end = match_brace(text, brace)
        if end is None:
            continue
        params = []
        for p in split_top(params_text):
            pm = re.match(r"^(\w+)\s*:\s*(.+)$", p)
            if pm:
                params.append((pm.group(1), pm.group(2).strip()))
        machines.append({
            "name": m.group(1).split("::")[-1],
            "qualified": m.group(1),
            "params": params,
            "requires": "requires" in header,
            "ensures": "ensures" in header,
            "body_start": brace + 1,
            "body_end": end - 1,
        })
    return machines


def statements(text, start, end):
    """Top-level statement spans (start, end) of one body."""
    out = []
    i = start
    while i < end:
        while i < end and text[i] in " \t\r\n":
            i += 1
        if i >= end:
            break
        s = i
        depth = 0
        while i < end:
            c = text[i]
            if c in "([":
                depth += 1
            elif c in ")]":
                depth -= 1
            elif c == "{":
                close = match_brace(text, i)
                if close is None:
                    return out
                i = close
                # `let x = match y { ... };` continues to its semicolon
                k = i
                while k < end and text[k] in " \t\r\n":
                    k += 1
                if depth == 0 and (k >= end or text[k] != ";") and not re.match(
                    r"\s*(else|\.|[-+*/%&|<>=!])", text[i:end]
                ):
                    break
                continue
            elif c == ";" and depth == 0:
                i += 1
                break
            i += 1
        out.append((s, i))
    return out


def scalar_paths(datas, type_name, prefix, depth=0):
    """(path, scalar_type) for scalar fields reachable through records."""
    t = type_name.strip()
    if t in SCALARS:
        yield prefix, t
        return
    if depth > 2 or t not in datas or datas[t]["cases"]:
        return
    for field, ftype in datas[t]["fields"].items():
        yield from scalar_paths(datas, ftype, f"{prefix}.{field}", depth + 1)


def literal_values(scalar, text):
    if scalar == "bool":
        return ["true", "false"]
    lo, hi = SCALARS[scalar]
    values = {0, 1, 99, hi}
    if lo < 0:
        values |= {-1, lo}
    nums = [int(n) for n in re.findall(r"(?<![\w.])(\d{1,6})(?![\w.])", text)]
    for n in nums[:20]:
        if lo <= n + 1 <= hi:
            values.add(n + 1)
        if lo <= n - 1 <= hi:
            values.add(n - 1)
    return sorted(values)


def scope_at(text, machine, stmt_start, stmts):
    """Mutable typed variables visible before a statement."""
    scope = []
    for name, ptype in machine["params"]:
        if ptype.startswith("&mut "):
            scope.append((name, re.sub(r"\s+in\s+.*$", "", ptype[5:].strip()), None))
        elif ptype.startswith("&"):
            continue
    for s, e in stmts:
        if s >= stmt_start:
            break
        lm = re.match(r"let\s+(mut\s+)?(\w+)\s*:\s*([^=;]+?)\s*(=|;)", text[s:e])
        if lm:
            vtype = re.sub(r"\s+in\s+.*$", "", lm.group(3).strip())
            scope.append((lm.group(2), vtype, None if lm.group(1) else s))
    return scope


def helpers(scalars, carriers, datas):
    out = ["", "// ---- pilot mutation helpers ----"]
    for t in sorted(scalars):
        out.append(f"machine pilot_poke_{t}(slot: &mut {t}, value: {t}) {{ slot = value; }}")
    out.append("machine pilot_opaque_u64(value: u64) -> u64 { value }")
    out.append("machine pilot_pick_u64(taken: u64, other: u64) -> u64 ensures result == taken || result == other { taken }")
    for t, path, st in sorted(carriers):
        tag = path.replace(".", "_")
        out.append(f"data PilotCarrier_{t}<'storage> {{ cell: &'storage mut {t}; }}")
        out.append(
            f"machine pilot_via_{t}{tag}(carrier: &mut PilotCarrier_{t}, value: {st}) "
            f"{{ carrier.cell{path} = value; }}"
        )
    return "\n".join(dict.fromkeys(out)) + "\n"


def mutate(text, rng, budget):
    clean = strip_comments(text)
    datas = parse_data(clean)
    machines = parse_machines(clean)
    callees_with_req = {m["name"] for m in machines if m["requires"]}
    candidates = []
    for machine in machines:
        if machine["name"].startswith("pilot_"):
            continue
        stmts = statements(clean, machine["body_start"], machine["body_end"])
        for idx, (s, e) in enumerate(stmts):
            stmt = clean[s:e]
            calls_req = any(re.search(rf"\b{re.escape(c)}\s*\(", stmt) for c in callees_with_req)
            weight = 3 if calls_req else (2 if re.match(r"(transition|match|let .*= *match)", stmt) else 1)
            scope = scope_at(clean, machine, s, stmts)
            for var, vtype, promote in scope:
                bare = re.sub(r"<.*", "", vtype)
                mentions = re.search(rf"\b{re.escape(var)}\b", stmt) is not None
                w = weight * (3 if mentions else 1)
                for path, st in scalar_paths(datas, bare, ""):
                    candidates.append((w, "store", s, var, path, st, bare, promote))
                    candidates.append((w, "poke", s, var, path, st, bare, promote))
                    if path and bare in datas and not datas[bare]["generic"]:
                        candidates.append((w, "carrier", s, var, path, st, bare, promote))
            if idx + 1 < len(stmts) and SWAPS:
                candidates.append((weight, "swap", idx, machine, stmts))
            if re.search(r"\[\s*\d+\s*\]", stmt):
                candidates.append((weight * 2, "runtime_index", s, e))
                body = clean[machine["body_start"]:machine["body_end"]]
                indices = sorted(set(re.findall(r"\[\s*(\d+)\s*\]", body)))
                if len(indices) > 1:
                    candidates.append((weight * 3, "pick_index", s, e, indices))
            if re.match(r"[\w.\[\]]+\s*=[^=]", stmt) or re.match(r"[\w:]+\s*\(", stmt):
                candidates.append((weight, "delete", s, e))
    if not candidates:
        return []
    mutants = []
    seen = set()
    tries = 0
    while len(mutants) < budget and tries < budget * 20:
        tries += 1
        total = sum(c[0] for c in candidates)
        pick = rng.uniform(0, total)
        for cand in candidates:
            pick -= cand[0]
            if pick <= 0:
                break
        kind = cand[1]
        scalars, carriers = set(), set()
        if kind in ("store", "poke", "carrier"):
            _, _, s, var, path, st, bare, promote = cand
            value = rng.choice(literal_values(st, clean))
            if kind == "store":
                snippet = f"{var}{path} = {value};"
            elif kind == "poke":
                scalars.add(st)
                snippet = f"pilot_poke_{st}(&mut {var}{path}, {value});"
            else:
                carriers.add((bare, path, st))
                n = len(mutants)
                snippet = (
                    f"let mut pilot_carrier{n}: PilotCarrier_{bare} = PilotCarrier_{bare} "
                    f"{{ cell: &mut {var} }}; pilot_via_{bare}{path.replace('.', '_')}"
                    f"(&mut pilot_carrier{n}, {value});"
                )
            new = clean[:s] + snippet + "\n    " + clean[s:]
            if promote is not None:
                new = new[:promote] + re.sub(r"^let\s+", "let mut ", new[promote:], count=1)
            desc = f"{kind} {snippet}" + (" (let made mutable)" if promote is not None else "")
        elif kind == "swap":
            _, _, idx, machine, stmts = cand
            (a0, a1), (b0, b1) = stmts[idx], stmts[idx + 1]
            new = clean[:a0] + clean[b0:b1] + clean[a1:b0] + clean[a0:a1] + clean[b1:]
            desc = f"swap {clean[a0:a1].strip()[:60]!r} <-> {clean[b0:b1].strip()[:60]!r}"
        elif kind == "delete":
            _, _, s, e = cand
            new = clean[:s] + clean[e:]
            desc = f"delete {clean[s:e].strip()[:80]!r}"
        elif kind == "pick_index":
            _, _, s, e, indices = cand
            stmt = clean[s:e]
            hits = list(re.finditer(r"\[\s*(\d+)\s*\]", stmt))
            h = rng.choice(hits)
            other = rng.choice([i for i in indices if i != h.group(1)])
            replaced = stmt[:h.start()] + f"[pilot_pick_u64({other}, {h.group(1)})]" + stmt[h.end():]
            new = clean[:s] + replaced + clean[e:]
            desc = f"pick_index runs {other} for {h.group(1)} in {stmt.strip()[:60]!r}"
        else:
            _, _, s, e = cand
            stmt = clean[s:e]
            hits = list(re.finditer(r"\[\s*(\d+)\s*\]", stmt))
            h = rng.choice(hits)
            replaced = stmt[:h.start()] + f"[pilot_opaque_u64({h.group(1)})]" + stmt[h.end():]
            new = clean[:s] + replaced + clean[e:]
            desc = f"runtime_index {stmt.strip()[:60]!r}"
        if desc in seen:
            continue
        seen.add(desc)
        mutants.append((new + helpers(scalars | {c[2] for c in carriers}, carriers, datas), desc))
    return mutants


def main():
    sample, out_dir = sys.argv[1], sys.argv[2]
    per = 8
    seed = 1
    args = sys.argv[3:]
    if "--per-program" in args:
        per = int(args[args.index("--per-program") + 1])
    if "--seed" in args:
        seed = int(args[args.index("--seed") + 1])
    rng = random.Random(f"{seed}:{sample}")
    text = open(os.path.join(sample, "main.omg")).read()
    build = open(os.path.join(sample, "build.omg")).read()
    for n, (mutant, desc) in enumerate(mutate(text, rng, per)):
        d = os.path.join(out_dir, f"m{n}")
        os.makedirs(d, exist_ok=True)
        open(os.path.join(d, "main.omg"), "w").write(mutant)
        open(os.path.join(d, "build.omg"), "w").write(build)
        json.dump({"source": sample, "mutation": desc}, open(os.path.join(d, "mutation.json"), "w"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
