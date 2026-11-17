import json, sys, collections
m = json.load(open(sys.argv[1]))
root = m['workspace_root'] + '/'
def rel(p):
    return p.replace(root, '').rsplit('/Cargo.toml', 1)[0]
pk = {p['name']: rel(p['manifest_path']) for p in m['packages']}

def position(path):
    s = path
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

users = collections.defaultdict(list)
devusers = collections.defaultdict(list)
for p in m['packages']:
    for d in p['dependencies']:
        if not d.get('path'):
            continue
        tgt = d['name']
        # rename handling: use path
        tpath = rel(d['path'] + '/Cargo.toml')
        tname = next((n for n, pp in pk.items() if pp == tpath), tgt)
        if (d.get('kind') or '') == 'dev':
            devusers[tname].append(p['name'])
        else:
            users[tname].append(p['name'])

fdn = sorted(n for n, pp in pk.items() if '/foundation/' in pp)
for n in fdn:
    us = sorted(set(users[n]))
    pos = sorted(set(position(pk[u]) for u in us))
    print(f"## {n}  ({pk[n]})  users={len(us)} positions={len(pos)}")
    print("   positions:", ', '.join(pos))
    print("   users:", ', '.join(us))
    print("   dev-only users:", ', '.join(sorted(set(devusers[n]) - set(us))))
    deps = [d['name'] for p in m['packages'] if p['name'] == n for d in p['dependencies'] if d.get('path') and (d.get('kind') or '') != 'dev']
    print("   deps:", ', '.join(sorted(set(deps))))
