#!/usr/bin/env python3
"""Build the pilot corpus: every standalone, library-free program, plus small
textual mutations. usage: build_corpus.py <list.txt> <out.jsonl> [per-mutation cap]"""
import json, random, re, sys

LIST, OUT = sys.argv[1:3]
CAP = int(sys.argv[3]) if len(sys.argv) > 3 else 300
files = [l.strip() for l in open(LIST) if l.strip()]
random.seed(7)


def m_builtin_data(s):
    # Rename the first data type to a builtin integer spelling (modules.md#L146).
    m = re.search(r'^(?:public\s+)?data\s+([A-Z]\w*)\b(?!\s*::)', s, re.M)
    if not m: return None
    return re.sub(r'\b%s\b' % m.group(1), 'u7', s)


def m_builtin_binder(s):
    # Rename the first generic type binder to `u32`.
    m = re.search(r'\b(?:machine|data)\s+[\w:]+\s*<\s*([A-Z]\w*)\s*[,>]', s)
    if not m: return None
    return re.sub(r'\b%s\b' % m.group(1), 'u32', s)


ARM = re.compile(r'^(\s*)(\w+::\w+)\s*\{\s*(\w+)\s*\}\s*->(.*)$', re.M)


def m_field_value(s):
    # `Case { v } -> body` where body ignores v  =>  `Case { v: 0 } -> body`.
    for m in ARM.finditer(s):
        if not re.search(r'\b%s\b' % m.group(3), m.group(4)):
            return s[:m.start()] + f"{m.group(1)}{m.group(2)} {{ {m.group(3)}: 0 }} ->{m.group(4)}" + s[m.end():]
    return None


CASE_ARM = re.compile(r'^(\s*)(\w+::\w+(?:\s*\{[^}\n]*\})?)\s*->', re.M)


def m_guard_case_arm(s):
    # Add a trivially true guard to the first case arm; a guarded arm no
    # longer counts toward coverage (patterns.md, coverage).
    m = CASE_ARM.search(s)
    if not m: return None
    return s[:m.end() - 2] + " if true " + s[m.end() - 2:]


WILD = re.compile(r'^\s*_\s*->[^\n]*\n', re.M)


def m_drop_wildcard(s):
    m = WILD.search(s)
    if not m: return None
    return s[:m.start()] + s[m.end():]


TRUE_WILD = re.compile(r'^(\s*)(true\s*->[^\n]*)\n(\s*)(_\s*->[^\n]*)$', re.M)


def m_swap_arms(s):
    # `true -> a` / `_ -> b`  =>  `_ -> b` / `true -> a` (the true arm is now
    # unreachable).
    m = TRUE_WILD.search(s)
    if not m: return None
    return s[:m.start()] + f"{m.group(1)}{m.group(4)}\n{m.group(3)}{m.group(2)}" + s[m.end():]


PARAM_MACHINE = re.compile(r'machine\s+\w+\s*\(\s*(\w+)\s*:\s*(u8|u16|u32|u64|i32|i64|bool)\s*[,)][^{;]*\{', re.S)


def m_shadow_param(s):
    # Re-declare the first scalar parameter as a local (state_contracts.md:
    # a name is declared at most once among parameters and locals).
    m = PARAM_MACHINE.search(s)
    if not m: return None
    zero = "false" if m.group(2) == "bool" else "0"
    return s[:m.end()] + f" let {m.group(1)}: {m.group(2)} = {zero};" + s[m.end():]


LET = re.compile(r'\blet\s+(mut\s+)?([a-z]\w*)\s*:')


def m_local_builtin_name(s):
    # Rename the first local to a builtin type spelling.
    m = LET.search(s)
    if not m or m.group(2) in ("u8", "u32"): return None
    return re.sub(r'\b%s\b' % m.group(2), 'u8', s)


MUTATIONS = [m_builtin_data, m_builtin_binder, m_field_value, m_guard_case_arm,
             m_drop_wildcard, m_swap_arms, m_shadow_param, m_local_builtin_name]

rows = []
sources = {}
for f in files:
    try:
        sources[f] = open(f, encoding="utf-8").read()
    except UnicodeDecodeError:
        continue
    rows.append({"id": f"orig:{f}", "origin": f, "mutation": "none", "source": sources[f]})
order = list(sources)
for mut in MUTATIONS:
    random.shuffle(order)
    n = 0
    for f in order:
        if n >= CAP: break
        out = mut(sources[f])
        if out is None or out == sources[f]: continue
        rows.append({"id": f"{mut.__name__}:{f}", "origin": f, "mutation": mut.__name__, "source": out})
        n += 1
    print(mut.__name__, n)
with open(OUT, "w") as o:
    for r in rows:
        o.write(json.dumps(r) + "\n")
print("total", len(rows))
