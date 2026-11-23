#!/usr/bin/env python3
"""File collisions between commits landing on origin/main (last 14 days).

Reads ../commits.tsv and ../files.tsv; prints Markdown tables to stdout.
Crate list (Cargo.toml directories at HEAD) is read from crates.txt beside this
script, produced by:  git -C /home/user/Omega ls-files '*Cargo.toml' | grep -v ^_recycling
"""
import csv, os, sys, random, itertools, collections, datetime

csv.field_size_limit(10**9)
HERE = os.path.dirname(os.path.abspath(__file__))
BASE = os.path.dirname(HERE)
BULK = 100  # commits changing more than this many files are "bulk"
SEED = 20261123
DRAWS = 5000

commits = {r['hash']: r for r in csv.DictReader(open(os.path.join(BASE, 'commits.tsv')), delimiter='\t')}
files = collections.defaultdict(set)
for r in csv.DictReader(open(os.path.join(BASE, 'files.tsv')), delimiter='\t'):
    files[r['hash']].add(r['path'])
crates = sorted({os.path.dirname(l.strip()) for l in open(os.path.join(HERE, 'crates.txt')) if l.strip() and os.path.dirname(l.strip())},
                key=len, reverse=True)

def is_doc(p):
    b = os.path.basename(p)
    return (p.startswith('wiki/') or b.lower().startswith('readme') or p in ('TASKS.md', 'AGENTS.md', 'CLAUDE.md')
            or (('/' not in p) and b.startswith('OWNER_') and b.endswith('.md')))

def unit(p):
    for c in crates:
        if p.startswith(c + '/'):
            return c
    parts = p.split('/')
    return '/'.join(parts[:2]) if len(parts) > 2 else (parts[0] if len(parts) == 2 else '(root)')

for h in commits:
    commits[h]['lane'] = commits[h]['lane'] or '(none)'
    commits[h]['_files'] = files.get(h, set())
    commits[h]['_t'] = datetime.datetime.fromisoformat(commits[h]['date'])
allc = list(commits.values())
bulk = [c for c in allc if len(c['_files']) > BULK]
empty = [c for c in allc if not c['_files']]
norm = [c for c in allc if 0 < len(c['_files']) <= BULK]
out = []
P = out.append
def pct(a, b): return f"{a}/{b} ({100*a/b:.0f}%)" if b else "0/0"

P(f"Commits: {len(allc)}; bulk (> {BULK} files): {len(bulk)}; no file rows: {len(empty)}; analysed: {len(norm)}")
P(f"Bulk commits touch {sum(len(c['_files']) for c in bulk)} of {sum(len(c['_files']) for c in allc)} file rows.")
P("Bulk by lane: " + ", ".join(f"{k} {v}" for k, v in collections.Counter(c['lane'] for c in bulk).most_common()))

# 1. Top 30 files
fc = collections.Counter(); fd = collections.defaultdict(set)
for c in norm:
    for p in c['_files']:
        fc[p] += 1; fd[p].add(c['day'])
P("\n### Top 30 files by commits (non-bulk)\n\n| # | file | commits | days (of 14) | kind |\n|---|---|---|---|---|")
for i, (p, n) in enumerate(fc.most_common(30), 1):
    P(f"| {i} | `{p}` | {n} | {len(fd[p])} | {'doc' if is_doc(p) else 'code'} |")
topcode = [p for p, _ in fc.most_common() if not is_doc(p)][:10]
P("\n### Top 30 non-document files by commits\n\n| # | file | commits | days |\n|---|---|---|---|")
for i, p in enumerate([p for p, _ in fc.most_common() if not is_doc(p)][:30], 1):
    P(f"| {i} | `{p}` | {fc[p]} | {len(fd[p])} |")

# 2. Directories at depth 4-6 under source-rust
for d in (4, 5, 6):
    dc = collections.Counter(); dd = collections.defaultdict(set)
    for c in norm:
        ds = {'/'.join(p.split('/')[:d]) for p in c['_files'] if p.startswith('source-rust/') and len(p.split('/')) > d}
        for x in ds:
            dc[x] += 1; dd[x].add(c['day'])
    P(f"\n### source-rust directories at depth {d}: top 15 by commits\n\n| directory | commits | days |\n|---|---|---|")
    for x, n in dc.most_common(15):
        P(f"| `{x}` | {n} | {len(dd[x])} |")

# 3. Per-day collision share
byday = collections.defaultdict(list)
for c in norm: byday[c['day']].append(c)
P("\n### Per day: commits sharing a file with another commit landed the same day\n\n| day | commits | any file | code file | document only | doc file at all |\n|---|---|---|---|---|---|")
tot = collections.Counter()
for day in sorted(byday):
    cs = byday[day]; cnt = collections.Counter(p for c in cs for p in c['_files'])
    a = cd = do = dany = 0
    for c in cs:
        shared = {p for p in c['_files'] if cnt[p] > 1}
        code = any(not is_doc(p) for p in shared); doc = any(is_doc(p) for p in shared)
        a += bool(shared); cd += code; do += (doc and not code); dany += doc
    tot.update(n=len(cs), a=a, cd=cd, do=do, dany=dany)
    P(f"| {day} | {len(cs)} | {pct(a,len(cs))} | {pct(cd,len(cs))} | {pct(do,len(cs))} | {pct(dany,len(cs))} |")
P(f"| all | {tot['n']} | {pct(tot['a'],tot['n'])} | {pct(tot['cd'],tot['n'])} | {pct(tot['do'],tot['n'])} | {pct(tot['dany'],tot['n'])} |")

# 3b. Same measure excluding the top 10 code files and TASKS-style docs (how much is driven by a few hubs)
hub = set(topcode)
a = 0
for day, cs in byday.items():
    cnt = collections.Counter(p for c in cs for p in c['_files'] if not is_doc(p) and p not in hub)
    a += sum(any(cnt[p] > 1 for p in c['_files'] if not is_doc(p) and p not in hub) for c in cs)
P(f"\nCode collision share if the top 10 code files are ignored: {pct(a, tot['n'])}")

# 3b2. Tighter windows: another commit landed within W hours either side
srt = sorted(norm, key=lambda c: c['_t'])
for W in (1, 3, 6):
    w = datetime.timedelta(hours=W); hit = hitc = 0; nb = []
    for i, c in enumerate(srt):
        others = [o for o in srt if o is not c and abs(o['_t'] - c['_t']) <= w]
        nb.append(len(others))
        hit += any(c['_files'] & o['_files'] for o in others)
        codef = {p for p in c['_files'] if not is_doc(p)}
        hitc += any(codef & o['_files'] for o in others)
    nb.sort()
    P(f"Window +/-{W}h: median {nb[len(nb)//2]} other commits landed in window; shares any file {pct(hit,len(srt))}; shares a code file {pct(hitc,len(srt))}")
tp = sum(1 for cs in byday.values() for a_, b_ in itertools.combinations(cs, 2))
cp = sum(1 for cs in byday.values() for a_, b_ in itertools.combinations(cs, 2) if {p for p in a_['_files'] if not is_doc(p)} & b_['_files'])
P(f"Same-day commit pairs sharing a code file: {pct(cp, tp)}")
hpd = collections.Counter(c['_t'].strftime('%Y-%m-%d %H') for c in norm)
P(f"Landings per clock hour with at least one landing: median {sorted(hpd.values())[len(hpd)//2]}, max {max(hpd.values())} (over {len(hpd)} hours)")

# 3c. Overlapping work intervals [landed - active minutes, landed]
withm = [c for c in norm if c['minutes']]
for c in withm: c['_s'] = c['_t'] - datetime.timedelta(minutes=float(c['minutes']))
withm.sort(key=lambda c: c['_t'])
ov = ovcode = ovany = 0; conc = []
for i, c in enumerate(withm):
    others = [o for o in withm if o is not c and o['_s'] < c['_t'] and c['_s'] < o['_t']]
    conc.append(len(others))
    codef = {p for p in c['_files'] if not is_doc(p)}
    ovany += bool(others)
    ovcode += any(codef & o['_files'] for o in others)
conc.sort()
P(f"\nCommits with a provenance Time value: {len(withm)}. Overlapping-interval partners per commit: median {conc[len(conc)//2]}, p90 {conc[int(len(conc)*.9)]}, max {conc[-1]}.")
P(f"Commits whose active-work interval overlaps another's: {pct(ovany,len(withm))}; overlaps another commit that changed one of the same code files: {pct(ovcode,len(withm))}")

# 4. Top 10 code files: commits per day
days = sorted(byday)
P("\n### Commits per day touching each of the top 10 code files\n\n| file | " + " | ".join(d[5:] for d in days) + " | max |\n|---|" + "---|" * (len(days) + 1))
for p in topcode:
    row = [sum(p in c['_files'] for c in byday[d]) for d in days]
    P(f"| `{p}` | " + " | ".join(str(x) for x in row) + f" | {max(row)} |")

# 5. Spread
def spread(c, docs):
    fs = c['_files'] if docs else {p for p in c['_files'] if not is_doc(p)}
    return len({unit(p) for p in fs}), len(fs)
P("\n### Spread: distinct crates/directories per commit\n\n| units | all files | excluding documents |\n|---|---|---|")
s1 = collections.Counter(); s2 = collections.Counter(); nodoc = 0
for c in norm:
    s1[min(spread(c, True)[0], 5)] += 1
    u, n = spread(c, False)
    if n == 0: nodoc += 1
    else: s2[min(u, 5)] += 1
n2 = len(norm) - nodoc
for k in range(1, 6):
    P(f"| {k if k < 5 else '5+'} | {pct(s1[k], len(norm))} | {pct(s2[k], n2)} |")
P(f"\nDocument-only commits (excluded from the right column): {nodoc}")
docany = sum(any(is_doc(p) for p in c['_files']) for c in norm)
P(f"Commits touching at least one document: {pct(docany, len(norm))}; touching TASKS.md: {pct(sum('TASKS.md' in c['_files'] for c in norm), len(norm))}")

# 6. Lane pairs sharing a code file on the same day
pairs = collections.Counter(); lanepairs_total = collections.Counter(); lc = collections.Counter(c['lane'] for c in norm)
for day, cs in byday.items():
    for a_, b_ in itertools.combinations(cs, 2):
        k = tuple(sorted((a_['lane'], b_['lane'])))
        lanepairs_total[k] += 1
        if {p for p in a_['_files'] if not is_doc(p)} & b_['_files']:
            pairs[k] += 1
P("\n### Lane pairs: same-day commit pairs that share a code file (top 20)\n\n| lane A | lane B | colliding pairs | of same-day pairs | rate |\n|---|---|---|---|---|")
for k, n in pairs.most_common(20):
    P(f"| {k[0]} | {k[1]} | {n} | {lanepairs_total[k]} | {100*n/lanepairs_total[k]:.1f}% |")
same = sum(n for k, n in pairs.items() if k[0] == k[1]); P(f"\nColliding pairs within one lane: {same} of {sum(pairs.values())}")

# 7. k commits drawn from the same day: chance that none shares a code file
rng = random.Random(SEED)
dlist = [d for d in days if len(byday[d]) >= 20]
P(f"\n### Draw k commits from one day (days with >= 20 commits: {len(dlist)}), {DRAWS} draws per k, seed {SEED}\n\n| k | all k pairwise disjoint on code files | ... ignoring top 10 code files | disjoint on any file incl. documents |\n|---|---|---|---|")
codeset = {c['hash']: frozenset(p for p in c['_files'] if not is_doc(p)) for c in norm}
codehub = {h: s - hub for h, s in codeset.items()}
allset = {c['hash']: frozenset(c['_files']) for c in norm}
def disjoint(sample, m):
    seen = set()
    for c in sample:
        s = m[c['hash']]
        if seen & s: return False
        seen |= s
    return True
for k in (2, 3, 4, 5, 6, 8, 10, 15, 20):
    r = [0, 0, 0]
    for _ in range(DRAWS):
        s = rng.sample(byday[rng.choice(dlist)], k)
        r[0] += disjoint(s, codeset); r[1] += disjoint(s, codehub); r[2] += disjoint(s, allset)
    P(f"| {k} | {100*r[0]/DRAWS:.0f}% | {100*r[1]/DRAWS:.0f}% | {100*r[2]/DRAWS:.0f}% |")

print("\n".join(out))
