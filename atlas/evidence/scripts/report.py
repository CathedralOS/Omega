"""For one foundation crate: item -> defining file[::inline mod path] -> user positions.
Usage: python3 -I report.py usage.json meta.json <crate> [--by-file] [--depth N]"""
import json, os, re, sys, collections
u = json.load(open(sys.argv[1]))
m = json.load(open(sys.argv[2]))
crate = sys.argv[3]
byfile = '--by-file' in sys.argv
depth_limit = int(sys.argv[sys.argv.index('--depth') + 1]) if '--depth' in sys.argv else 1
pk = next(p for p in m['packages'] if p['name'] == crate)
src = os.path.join(os.path.dirname(pk['manifest_path']), 'src')
defs = {}
DEF = re.compile(r'^\s*pub(?:\([^)]*\))?\s+(?:unsafe\s+)?(?:struct|enum|fn|trait|type|const|static|mod|union)\s+([A-Za-z_][A-Za-z0-9_]*)')
MAC = re.compile(r'^\s*[a-z_]+!\s*[\(\{]\s*(?:pub\s+)?([A-Z][A-Za-z0-9_]*)')
MACRULE = re.compile(r'macro_rules!\s+([A-Za-z_][A-Za-z0-9_]*)')
MOD = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{')
for dp, dn, fn in os.walk(src):
    for f in fn:
        if not f.endswith('.rs'):
            continue
        path = os.path.join(dp, f)
        relp = path.replace(src + '/', '')
        stack = []
        depth = 0
        for line in open(path, encoding='utf-8', errors='replace'):
            code = line.split('//')[0]
            mm = MOD.match(code)
            where = relp + ''.join('::' + s for s, _ in stack[:depth_limit])
            for rx in (DEF, MAC, MACRULE):
                d = rx.search(code) if rx is MACRULE else rx.match(code)
                if d:
                    defs.setdefault(d.group(1), where)
            if mm:
                stack.append((mm.group(1), depth))
            depth += code.count('{') - code.count('}')
            while stack and depth <= stack[-1][1]:
                stack.pop()
items = u['usage'].get(crate, {})
pos = u['pos']
if byfile:
    agg = collections.defaultdict(lambda: [set(), set(), []])
    for it, users in items.items():
        f = defs.get(it, '?')
        agg[f][0].update(users)
        agg[f][1].update(pos.get(x, x) for x in users)
        agg[f][2].append(it)
    for f, (us, ps, its) in sorted(agg.items(), key=lambda kv: -len(kv[1][1])):
        print(f"{f}: {len(ps)} positions | {len(its)} items: {', '.join(sorted(its))[:400]}")
        print(f"    positions: {', '.join(sorted(ps))}")
else:
    for it, users in sorted(items.items(), key=lambda kv: -len(kv[1])):
        ps = sorted(set(pos.get(x, x) for x in users))
        print(f"{it} [{defs.get(it,'?')}] ({len(ps)}): {', '.join(ps)}")
