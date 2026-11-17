"""Per-crate, per-first-path-segment usage of foundation crates in non-test code.

Usage: python3 -I modscan.py meta.json out.json
Heuristic: skips tests/ benches/ examples/ dirs, files named tests.rs, *_tests.rs,
*_test.rs, test_*.rs, source_guards.rs, and brace-matched `#[cfg(test)]` items.
Records the first path segment after `crate_ident::` and the names directly
inside `use crate_ident::{..}` (first segment of each).
"""
import json, os, re, sys, collections

m = json.load(open(sys.argv[1]))
root = m['workspace_root'] + '/'

def rel(p):
    return p.replace(root, '').rsplit('/Cargo.toml', 1)[0]

def position(s):
    for pre, lab in [
        ('omega-rust/psi/compilation/pipeline/', 'psi-stage '),
        ('omega-rust/omega/compilation/pipeline/', 'omega-stage '),
        ('omega-rust/psi/compilation/representations/', 'psi-repr '),
        ('omega-rust/omega/compilation/representations/', 'omega-repr '),
        ('omega-rust/psi/foundation/', 'psi-fdn '),
        ('omega-rust/omega/foundation/', 'omega-fdn '),
        ('omega-rust/psi/compilation/', 'psi-comp '),
        ('omega-rust/omega/compilation/backend/', 'backend '),
        ('omega-rust/omega/compilation/realization/', 'realization '),
        ('omega-rust/omega/compilation/', 'omega-comp '),
        ('omega-rust/psi/interpreter', 'psi-interp'),
        ('omega-rust/omega/interpreter/', 'omega-interp '),
        ('omega-rust/omega/packages/', 'packages '),
        ('omega-rust/omega/build-system/', 'build '),
        ('omega-rust/omega/application', 'cli'),
        ('tools/', 'tools '),
        ('samples/', 'samples '),
    ]:
        if s.startswith(pre):
            rest = s[len(pre):]
            first = rest.split('/')[0] if rest else ''
            if lab.strip() in ('psi-stage', 'omega-stage'):
                return lab + first.split('_')[0]
            if lab.strip() in ('psi-repr', 'omega-repr'):
                return lab + rest.replace('/', ':')
            return (lab + rest).strip()
    return s

pkgs = [(rel(p['manifest_path']), p) for p in m['packages']]
pkgs.sort(key=lambda x: -len(x[0]))
fdn = {p['name'].replace('-', '_'): p['name'] for d, p in pkgs if os.environ.get('SCAN_DIR', '/foundation/') in d}

def owner_of(path):
    r = path.replace(root, '')
    for d, p in pkgs:
        if r.startswith(d + '/'):
            return d, p
    return None, None

def strip_cfg_test(text):
    out = []
    lines = text.split('\n')
    i = 0
    while i < len(lines):
        if lines[i].strip() == '#[cfg(test)]':
            # skip the next item, brace-matched
            j = i + 1
            while j < len(lines) and lines[j].strip().startswith('#['):
                j += 1
            if j < len(lines) and '{' not in lines[j] and lines[j].rstrip().endswith(';'):
                i = j + 1
                continue
            depth = 0
            started = False
            k = j
            while k < len(lines):
                depth += lines[k].count('{') - lines[k].count('}')
                if '{' in lines[k]:
                    started = True
                if started and depth <= 0:
                    break
                if not started and lines[k].rstrip().endswith(';'):
                    break
                k += 1
            i = k + 1
            continue
        out.append(lines[i])
        i += 1
    return '\n'.join(out)

skipname = re.compile(r'(^tests\.rs$|_tests?\.rs$|^test_.*\.rs$|^source_guards\.rs$)')
usage = collections.defaultdict(lambda: collections.defaultdict(set))  # crate -> item -> set(user crate)
users_pos = {}
usage2 = collections.defaultdict(lambda: collections.defaultdict(set))

def top_names(group):
    # split group text at top-level commas
    names, depth, cur = [], 0, ''
    for ch in group:
        if ch == '{':
            depth += 1
        elif ch == '}':
            depth -= 1
        if ch == ',' and depth == 0:
            names.append(cur); cur = ''
        else:
            cur += ch
    names.append(cur)
    res = []
    for n in names:
        n = n.strip()
        if not n:
            continue
        seg = re.split(r'::|\s', n)[0].strip('{} ')
        if seg:
            res.append(seg)
    return res

for dirpath, dirnames, filenames in os.walk(root):
    rp = dirpath.replace(root, '')
    if any(part in ('target', '.git', 'tests', 'benches', 'examples', '.agents') for part in rp.split('/')):
        dirnames[:] = []
        continue
    for f in filenames:
        if not f.endswith('.rs') or skipname.search(f):
            continue
        path = os.path.join(dirpath, f)
        d, p = owner_of(path)
        if p is None:
            continue
        try:
            text = open(path, encoding='utf-8', errors='replace').read()
        except OSError:
            continue
        text = strip_cfg_test(text)
        text = re.sub(r'//[^\n]*', '', text)
        me = p['name'].replace('-', '_')
        users_pos[p['name']] = position(d)
        declared = {d['name'] for d in p['dependencies'] if (d.get('kind') or '') != 'dev'}
        for ident, cname in fdn.items():
            if ident == me or ident not in text or cname not in declared:
                continue
            for mm in re.finditer(r'(?<![\w:])(?:::)?' + ident + r'::\{', text):
                start = mm.end()
                depth, k = 1, start
                while k < len(text) and depth:
                    if text[k] == '{': depth += 1
                    elif text[k] == '}': depth -= 1
                    k += 1
                for seg in top_names(text[start:k - 1]):
                    usage[cname][seg].add(p['name'])
            for mm in re.finditer(r'(?<![\w:])(?:::)?' + ident + r'::([A-Za-z_][A-Za-z0-9_]*)', text):
                usage[cname][mm.group(1)].add(p['name'])
            for mm in re.finditer(r'(?<![\w:])(?:::)?' + ident + r'::([a-z_][a-z0-9_]*)::([A-Za-z_][A-Za-z0-9_]*)', text):
                usage2[cname][mm.group(1) + '::' + mm.group(2)].add(p['name'])
            for mm in re.finditer(r'(?<![\w:])(?:::)?' + ident + r'::([a-z_][a-z0-9_]*)::\{', text):
                start = mm.end(); depth, k = 1, start
                while k < len(text) and depth:
                    if text[k] == '{': depth += 1
                    elif text[k] == '}': depth -= 1
                    k += 1
                for seg in top_names(text[start:k - 1]):
                    usage2[cname][mm.group(1) + '::' + seg].add(p['name'])

unused = {}
for d, p in pkgs:
    for dep in p['dependencies']:
        if dep['name'] in fdn.values() and (dep.get('kind') or '') != 'dev' and not any(p['name'] in v for v in usage.get(dep['name'], {}).values()):
            unused.setdefault(dep['name'], []).append(p['name'])
out = {c: {i: sorted(u) for i, u in items.items()} for c, items in usage.items()}
out2 = {c: {i: sorted(u) for i, u in items.items()} for c, items in usage2.items()}
json.dump({'usage2': out2, 'usage': out, 'pos': users_pos, 'declared_unused': unused}, open(sys.argv[2], 'w'), indent=1, sort_keys=True)
