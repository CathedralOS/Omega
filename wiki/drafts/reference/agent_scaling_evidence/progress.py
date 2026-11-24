#!/usr/bin/env python3
"""Progress measures over 14 days of origin/main (first-parent).

Reads commits.tsv, reads TASKS.md / OWNER_QUESTIONS.md / OWNER_CODE_CONCERNS.md
once per day at that day's last commit (git show <hash>:<path>), reads all
commit bodies with one `git log` call (no diffs), and prints JSON-ish tables.
Rerun: python3 progress.py > progress.out
"""
import csv, re, subprocess, collections, json, random, sys, os

csv.field_size_limit(10**9)
REPO = "/home/user/Omega"
BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
rows = list(csv.DictReader(open(os.path.join(BASE, "commits.tsv")), delimiter="\t"))
rows.sort(key=lambda r: r["date"])  # ISO with offset; good enough for per-day order after tz-normalising below

from datetime import datetime
def ts(r):
    return datetime.fromisoformat(r["date"]).timestamp()
rows.sort(key=ts)
days = sorted({r["day"] for r in rows})
hashes = {r["hash"] for r in rows}

def show(h, path):
    p = subprocess.run(["git", "-C", REPO, "show", f"{h}:{path}"], capture_output=True, text=True)
    return p.stdout if p.returncode == 0 else None

ID_ITEM = re.compile(r"^- \*\*([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+)\.?\*\*", re.M)
ID_ANY = re.compile(r"\*\*([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+)\.?\*\*")

def tasks_stats(t):
    return dict(lines=t.count("\n"), item_ids=len(ID_ITEM.findall(t)),
                distinct_bold_ids=len(set(ID_ANY.findall(t))),
                bullets=len(re.findall(r"^\s*- ", t, re.M))), set(ID_ITEM.findall(t))

def oq_stats(t):
    open_part = t.split("## Open questions", 1)[1] if "## Open questions" in t else t
    return dict(lines=t.count("\n"), open_questions=len(re.findall(r"^### ", open_part, re.M)))

def cc_stats(t):
    ent = t.split("## Entries", 1)[1] if "## Entries" in t else ""
    return dict(lines=t.count("\n"), entries=len(re.findall(r"^### ", ent, re.M)))

# ---- 1. daily snapshots
last_of_day = {}
for r in rows:
    last_of_day[r["day"]] = r  # rows sorted by time, so last write wins
snap = []
prev_ids = None
for d in days:
    h = last_of_day[d]["hash"]
    t = show(h, "TASKS.md"); q = show(h, "OWNER_QUESTIONS.md"); c = show(h, "OWNER_CODE_CONCERNS.md")
    ts_, ids = tasks_stats(t) if t else ({}, set())
    added = sorted(ids - prev_ids) if prev_ids is not None else []
    removed = sorted(prev_ids - ids) if prev_ids is not None else []
    prev_ids = ids
    snap.append(dict(day=d, hash=h, tasks=ts_, oq=oq_stats(q) if q else None,
                     cc=cc_stats(c) if c else None, ids_added=added, ids_removed=removed))

# ---- 2. gate mix
GATES = ["RC-REPOSITORY", "RC-SOURCE-SEMANTICS", "RC-BUILD-AND-PACKAGES", "RC-NATIVE-MATRIX",
         "RC-DIAGNOSTICS", "RC-REPRESENTATIVE-PROGRAMS"]
ALIASES = {"repository": 0, "source semantics": 1, "build and packages": 2, "native matrix": 3,
           "diagnostics": 4, "representative programs": 5}
COMPILER_LANES = {"psi", "omega", "backend", "compiler", "build", "packages", "library"}

def gate_of(g):
    head = g.split("|")[0].strip()
    u = head.upper().replace("`", "")
    for name in GATES:
        if u.startswith(name): return name
    low = head.lower().replace("`", "").replace("rc-", "").replace("-", " ").strip()
    for k, i in ALIASES.items():
        if low.startswith(k): return GATES[i]
    return "other-text" if g.strip() else "none"

gate_day = collections.defaultdict(collections.Counter)
gate_day_compiler = collections.defaultdict(collections.Counter)
red = collections.defaultdict(list)
RED = re.compile(r"red rows:\s*([+-]?\d+)")
for r in rows:
    gname = gate_of(r["gate"])
    gate_day[r["day"]][gname] += 1
    if r["lane"] in COMPILER_LANES:
        gate_day_compiler[r["day"]][gname] += 1
    m = RED.search(r["gate"])
    if m: red[r["day"]].append(int(m.group(1)))
first_red = min((r["day"] for r in rows if RED.search(r["gate"])), default=None)

# ---- 3. proof-line counts
NUM = r"(\d[\d,]*)"
NOUN = r"(?:[\w-]+\s+){0,3}?(tests?|samples?|programs?|fixtures?|cases?)"
P_OF = re.compile(NUM + r"\s*(?:/|of)\s*" + NUM + r"\s+" + NOUN, re.I)
P_ALL = re.compile(r"(?:all\s+)?" + NUM + r"\s+" + NOUN + r"[^.;|]{0,40}?\b(pass(?:ed)?|ok|succeeded|exited 0|agree[ds]?)", re.I)
P_PASSED_N = re.compile(r"(pass(?:ed)?)[:\s]+" + NUM, re.I)
proof_hits = []
for r in rows:
    p = r["proof"]
    for m in P_OF.finditer(p):
        proof_hits.append((r["day"], r["hash"], "of", int(m.group(1).replace(",", "")),
                           int(m.group(2).replace(",", "")), m.group(3).lower(), m.group(0)))
    for m in P_ALL.finditer(p):
        proof_hits.append((r["day"], r["hash"], "n", int(m.group(1).replace(",", "")), None,
                           m.group(2).lower(), m.group(0)))
with_count = {h[1] for h in proof_hits}
proof_nonempty = sum(1 for r in rows if r["proof"].strip())

# ---- 4. full bodies: whole-compiler measures
log = subprocess.run(["git", "-C", REPO, "log", "--first-parent", "--format=%h%x00%B%x1e", "-n", "3400", "origin/main"],
                     capture_output=True, text=True).stdout
bodies = {}
for chunk in log.split("\x1e"):
    chunk = chunk.strip("\n")
    if "\x00" not in chunk: continue
    h, b = chunk.split("\x00", 1)
    h = h.strip()
    for full in hashes:
        pass
    bodies[h] = b
# commits.tsv hashes are 12 chars; %h may be 10; match by prefix
short = {}
for r in rows:
    for k in (r["hash"][:10], r["hash"][:11], r["hash"][:12]):
        short[k] = r
body_of = {}
for h, b in bodies.items():
    r = short.get(h[:12]) or short.get(h[:11]) or short.get(h[:10])
    if r: body_of[r["hash"]] = b
WHOLE = re.compile(r"(sample[- ]gate|xtask sweep|\bsweep\b)", re.I)
whole_hits = []
for r in rows:
    b = body_of.get(r["hash"], "")
    for line in b.splitlines():
        if WHOLE.search(line) and re.search(r"\d", line):
            whole_hits.append((r["day"], r["hash"], r["title"][:70], line.strip()[:300]))
unfiltered = [w for w in whole_hits if "sample-gate" in w[3].lower() and "--filter" not in w[3]]
sweep_totals = [w for w in whole_hits if re.search(r"sweep", w[3], re.I) and re.search(r"\d[\d,]{2,}\s+(programs|rows)", w[3])]

# ---- sample of proof hits to read
random.seed(20261123)
sample = random.sample(proof_hits, min(30, len(proof_hits)))

out = dict(n_commits=len(rows), days=days, snapshots=snap,
           gate_day={d: dict(gate_day[d]) for d in days},
           gate_day_compiler={d: dict(gate_day_compiler[d]) for d in days},
           red_rows={d: dict(n=len(red[d]), sum=sum(red[d]), nonzero=sum(1 for x in red[d] if x)) for d in days},
           first_red_rows_day=first_red,
           proof_nonempty=proof_nonempty, proof_commits_with_count=len(with_count), proof_hits=len(proof_hits),
           proof_hits_by_day=dict(collections.Counter(h[0] for h in proof_hits)),
           proof_noun=dict(collections.Counter(h[5].rstrip("s") for h in proof_hits)),
           proof_big=[h for h in proof_hits if h[3] >= 500],
           proof_sample=sample, bodies_matched=len(body_of),
           whole_hits=len(whole_hits), whole_hit_list=whole_hits,
           unfiltered_sample_gate=unfiltered, sweep_totals=sweep_totals)
json.dump(out, sys.stdout, indent=1, default=list)

# ---- 5. extra series (printed to stderr as plain text)
def P(*a): print(*a, file=sys.stderr)
reds = [int(m.group(1)) for r in rows for m in [RED.search(r["gate"])] if m]
P("red-rows values:", collections.Counter(reds).most_common(), "sum", sum(reds), "n", len(reds))
P("red-rows last day:", max(r["day"] for r in rows if RED.search(r["gate"])))
READY = re.compile(r"(\d[\d,]*)\s+(?:L0.L5 and CLI|layer and CLI|L0.L5|layer|workspace|Rust)\s+tests", re.I)
BLOCK = re.compile(r"(\w+) (?:total |registered |remaining )*(?:readiness )?blockers?|sole (?:remaining )?(?:registered )?(?:readiness )?blocker", re.I)
P("\nreadiness/workspace test totals in bodies (day hash lane value | blocker phrase):")
for r in rows:
    b = body_of.get(r["hash"], "")
    if "readiness" not in b.lower() and "workspace tests" not in b.lower(): continue
    vals = [m.group(1) for m in READY.finditer(b)]
    bl = [m.group(0) for m in BLOCK.finditer(b)]
    if vals: P(r["day"], r["hash"], r["lane"], vals[:3], "|", bl[:2])
CHK = re.compile(r"(\d[\d,]*)\s+(?:cases|checker tests)[^.]{0,30}?(\d[\d,]*)\s+passed", re.I)
P("\nlowered-form checker suite totals:")
for r in rows:
    b = body_of.get(r["hash"], "")
    for m in CHK.finditer(b):
        if "checker" in b[max(0, m.start()-80):m.end()].lower():
            P(r["day"], r["hash"], m.group(0)[:100])
# gate share in compiler lanes
P("\ngate share, compiler lanes, by day (none/other excluded from denominator shown separately)")
for d in days:
    c = gate_day_compiler[d]; tot = sum(c.values()); named = sum(v for k, v in c.items() if k.startswith("RC-"))
    P(d, tot, named, {g.replace("RC-", ""): c.get(g, 0) for g in GATES}, "none", c.get("none", 0), "other", c.get("other-text", 0))
tot = collections.Counter()
for d in days: tot.update(gate_day[d])
P("all lanes total", dict(tot))
totc = collections.Counter()
for d in days: totc.update(gate_day_compiler[d])
P("compiler lanes total", dict(totc), sum(totc.values()))
