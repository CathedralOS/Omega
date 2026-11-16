#!/usr/bin/env python3
"""compiler_request.md outcome tables vs source/omega/application/ocreq/outcome.omg."""
import re, sys, os
ROOT = sys.argv[1] if len(sys.argv) > 1 else '.'
PAGE = os.path.join(ROOT, 'wiki/spec/build/compiler_request.md')
OMG = os.path.join(ROOT, 'source/omega/application/ocreq/outcome.omg')
L = open(PAGE).read().split('\n'); T = open(OMG).read()

def table(hdr):
    i = next(k for k, l in enumerate(L) if l.startswith(hdr)); rows = []; k = i + 2
    while k < len(L) and L[k].startswith('|'):
        rows.append((k + 1, [c.strip() for c in L[k].strip().strip('|').split('|')])); k += 1
    return rows

def camel(s): return ''.join(p.capitalize() for p in s.split('_'))
def transition(machine, enum):
    m = re.search(r'machine ' + machine + r'\b.*?\n\}', T, re.S); body = m.group(0)
    line0 = T[:m.start()].count('\n') + 1
    out = {}
    for mm in re.finditer(enum + r'::(\w+) -> (\w+)\(([^)]*)\)', body):
        out[mm.group(1)] = (mm.group(2), mm.group(3), line0 + body[:mm.start()].count('\n'))
    dflt = re.search(r'_ -> (\w+)\(', body)
    return out, (dflt.group(1) if dflt else None)

def enum_cases(name):
    m = re.search(r'data ' + name + r' \[copy\] \{(.*?)\n\}', T, re.S)
    return re.findall(r'case (\w+);', m.group(1))

SPACE = {'0': 'none', '1': 'request', '2': 'emitted_artifact', '3': 'internal_row', '4': 'canonical_source'}
issues = []
def check(rows, enum, num_m, space_m, limit_m=None, code_col=0, name_col=1, space_col=2, limit_col=None):
    nums, _ = transition(num_m, enum); spaces, sdef = transition(space_m, enum)
    limits = transition(limit_m, enum)[0] if limit_m else {}
    cases = enum_cases(enum); seen = set()
    for ln, c in rows:
        name = camel(c[name_col].strip('`')); seen.add(name)
        if name not in cases: issues.append(f'spec {ln}: {c[name_col]} has no {enum} case'); continue
        n = nums.get(name)
        if not n or n[1] != c[code_col]: issues.append(f'spec {ln}: {c[name_col]} code {c[code_col]}, omg says {n and n[1]}')
        want = SPACE.get(c[space_col].split()[0], '?'); got = spaces.get(name, (sdef,))[0]
        if want != got: issues.append(f'spec {ln}: {c[name_col]} space {c[space_col]!r}, omg says {got}')
        if limit_m:
            lim = c[limit_col].replace(',', ''); got = limits.get(name, (None, None))[1]
            if lim != got: issues.append(f'spec {ln}: {c[name_col]} limit {c[limit_col]}, omg says {got}')
    for k in cases:
        if k not in seen: issues.append(f'outcome.omg {enum}::{k} has no spec row')
    return len(rows)

n1 = check([r for r in table('| Tag | Code | Name |') if r[1][0].startswith('1')], 'RejectCode',
           'reject_code_number', 'reject_code_space', code_col=1, name_col=2, space_col=3)
n2 = check(table('| Code | Resource | Limit |'), 'Provision', 'provision_number', 'provision_space',
           'provision_limit', code_col=0, name_col=1, space_col=3, limit_col=2)
other = [r for r in table('| Tag | Code | Name |') if not r[1][0].startswith('1')]
print(f'Reject rows {n1}, Incomplete rows {n2}, non-Reject rows in the Reject table: {len(other)}')
for ln, c in other: print('  unchecked row', ln, c[:3])
for i in issues: print('DRIFT', i)
