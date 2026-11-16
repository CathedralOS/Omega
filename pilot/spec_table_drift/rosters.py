#!/usr/bin/env python3
"""Compare name-roster tables in wiki/spec with Rust enums.

Each check names: spec page, the first line of the table's header, which
column holds backticked names, and a Rust file + enum whose variants the
roster must equal. Hand-mapped; one line per table."""
import re, sys, os
ROOT = sys.argv[1] if len(sys.argv) > 1 else '.'

CHECKS = [
  # (label, spec page, header prefix, column, rust file, enum name, exclude-from-code)
  ('classicality rule roster vs AcceptedProofRule',
   'wiki/spec/proofs/classicality.md', '| Rule | Foundation', 0,
   'omega-rust/psi/compilation/proof-admission/src/proof.rs', 'AcceptedProofRule', set()),
  ('classicality rule roster vs terminal_psi::ProofRule',
   'wiki/spec/proofs/classicality.md', '| Rule | Foundation', 0,
   'omega-rust/psi/compilation/representations/terminal-psi/src/artifacts.rs', 'ProofRule', set()),
]

def enum_variants(path, name):
    t = open(os.path.join(ROOT, path)).read()
    m = re.search(r'enum\s+' + name + r'\b[^{]*\{', t)
    o = m.end(); d = 1; i = o
    while d:
        d += {'{': 1, '}': -1}.get(t[i], 0); i += 1
    body = t[o:i-1]
    body = re.sub(r'//[^\n]*', '', body)
    body = re.sub(r'#\[[^\]]*\]', '', body)
    # depth-0 identifiers starting a variant
    out = []; d = 0; tok = ''
    for line in body.split('\n'):
        s = line.strip()
        if d == 0:
            mm = re.match(r'([A-Z]\w*)', s)
            if mm: out.append(mm.group(1))
        d += s.count('{') + s.count('(') - s.count('}') - s.count(')')
    return out, t[:m.start()].count('\n') + 1

def table_names(path, header, col):
    L = open(os.path.join(ROOT, path)).read().split('\n')
    i = next(k for k, l in enumerate(L) if l.startswith(header))
    names = {}
    k = i + 2
    while k < len(L) and L[k].startswith('|'):
        cell = L[k].strip('|').split('|')[col]
        for n in re.findall(r'`([A-Za-z]\w*)`', cell): names[n] = k + 1
        k += 1
    return names, i + 1

for label, page, hdr, col, rs, enum, excl in CHECKS:
    spec, sl = table_names(page, hdr, col)
    code, cl = enum_variants(rs, enum)
    code = [c for c in code if c not in excl]
    miss_spec = [c for c in code if c not in spec]
    miss_code = [s for s in spec if s not in code]
    print(f'{label}: spec {page}:{sl} ({len(spec)} names) vs {rs}:{cl} ({len(code)} variants)')
    for c in miss_spec: print(f'   code has {c}; spec table omits it')
    for s in miss_code: print(f'   spec line {spec[s]} lists {s}; enum has no such variant')
