#!/usr/bin/env python3
"""Measure what the Proof lines of landed commits say was run before merge.

Inputs: ../commits.tsv, ../files.tsv (extracted from origin/main, first-parent,
last 14 days) and bodies.raw, the full commit messages, produced by
  git -C /home/user/Omega log --first-parent -n 3100 --format='%H%x1f%B%x1e' origin/main > bodies.raw
(commit messages only; no blobs are fetched). Run from this directory:
  python3 gate.py
"""
import collections
import csv
import os
import random
import re

HERE = os.path.dirname(os.path.abspath(__file__))
csv.field_size_limit(10**9)

bodies = {}
for chunk in open(os.path.join(HERE, "bodies.raw"), encoding="utf-8").read().split("\x1e"):
    chunk = chunk.strip("\n")
    if chunk:
        h, b = chunk.split("\x1f", 1)
        bodies[h[:12]] = b

rows = list(csv.DictReader(open(os.path.join(HERE, "..", "commits.tsv"), encoding="utf-8"), delimiter="\t"))
# rows are newest first; position 0 = newest
pos = {r["hash"]: i for i, r in enumerate(rows)}

v2files = collections.Counter()
allfiles = collections.Counter()
rustfiles = collections.Counter()
for f in csv.DictReader(open(os.path.join(HERE, "..", "files.tsv"), encoding="utf-8"), delimiter="\t"):
    allfiles[f["hash"]] += 1
    if f["path"].endswith((".rs", "Cargo.toml", "Cargo.lock")):
        rustfiles[f["hash"]] += 1
    if f["path"].startswith("omega-rust-v2/"):
        v2files[f["hash"]] += 1

COMPILER = {"psi", "omega", "backend", "compiler", "build", "packages", "library"}
comp = [r for r in rows if r["lane"] in COMPILER]

PROOF_START = re.compile(r"(?m)^\s*(?:\*\*)?Proof(?:\*\*)?:\s*")


def proof_para(body):
    """The Proof: line plus any wrapped continuation lines up to a blank line or the next footer line."""
    m = PROOF_START.search(body)
    if not m:
        return None
    out = []
    for ln in body[m.end():].split("\n"):
        if not ln.strip() or re.match(r"\s*(Agent|Gate|Spec|Co-Authored-By|Claim):", ln):
            break
        out.append(ln.strip())
    return " ".join(out)


for r in rows:
    r["proofp"] = proof_para(bodies[r["hash"]])
    r["v2"] = v2files[r["hash"]] > 0

out = []


def p(*a):
    out.append(" ".join(str(x) for x in a))


def pct(n, d):
    return f"{n}/{d} ({100.0*n/d:.1f}%)" if d else f"{n}/0"


# ---------------------------------------------------------------- periods
# What required CI ran on a PR depends on when it landed. Boundaries are the
# first-parent commits that changed the workflows (a PR that changes a
# workflow is itself checked by its new version, so it opens the new period).
MARKERS = [
    ("cdaf6a4efb76", "P2 #2578 removed compiler.yml etc."),
    ("d4f903b85e", "P3 #3483 admission type-checks merge tree"),
    ("f0f9c99d15", "P4 #4233 root compiler checks paused"),
    ("5c5f0ed472", "P5 #5136 V2 workspace test removed"),
]


def find(prefix):
    for h in pos:
        if h.startswith(prefix[:10]):
            return pos[h]
    raise SystemExit("marker not found " + prefix)


mpos = [(find(h), lab) for h, lab in MARKERS]


def period(r):
    i = pos[r["hash"]]
    per = "P1"
    for mp, lab in mpos:
        if i <= mp:  # at or newer than marker
            per = lab.split()[0]
    return per


PERIOD_DESC = {
    "P1": "compiler.yml required: check-rules, fmt, check-boundary, clippy -p omega, focused tests",
    "P2": "admission: check-rules and fmt (no compile)",
    "P3": "admission: check-rules, fmt, and merge-tree `mbx check --workspace --all-targets` when Rust/Cargo changed",
    "P4": "admission: spec presence + recycling check; root compiler checks paused; V2 workspace tests only when omega-rust-v2/ changed",
    "P5": "admission: spec presence + recycling check only",
}
for r in rows:
    r["period"] = period(r)

p("## periods (compiler-lane commits)")
pc = collections.Counter(r["period"] for r in comp)
pd = {}
for r in rows:
    pd.setdefault(r["period"], []).append(r["day"])
for k in sorted(pc):
    p(k, PERIOD_DESC[k], "| days", min(pd[k]), "to", max(pd[k]), "| compiler commits", pc[k], "| all commits", len(pd[k]))

# ---------------------------------------------------------------- Proof presence
p("\n## Proof presence")
withp = [r for r in comp if r["proofp"]]
p("compiler-lane commits", len(comp), "with Proof line", pct(len(withp), len(comp)))
for lane in sorted(COMPILER):
    L = [r for r in comp if r["lane"] == lane]
    p("  lane", lane, pct(sum(1 for r in L if r["proofp"]), len(L)))
for k in sorted(pc):
    L = [r for r in comp if r["period"] == k]
    p("  period", k, pct(sum(1 for r in L if r["proofp"]), len(L)))
nonc = [r for r in rows if r["lane"] not in COMPILER]
p("non-compiler commits with Proof line", pct(sum(1 for r in nonc if r["proofp"]), len(nonc)))
p("compiler commits touching omega-rust-v2/ (V2 workspace)", sum(1 for r in comp if r["v2"]))

# ---------------------------------------------------------------- hosts named
HOSTS = [
    ("macOS (any)", r"(?i)mac\s?os|darwin|apple silicon"),
    ("Linux (any)", r"(?i)linux"),
    ("Linux x86_64", r"(?i)linux[ _-]?x86[ _-]?64"),
    ("Linux arm64", r"(?i)linux[ _-]?(?:arm64|aarch64)"),
    ("Windows (any)", r"(?i)windows"),
    ("Windows x86_64", r"(?i)windows[ _-]?x86[ _-]?64"),
    ("Windows arm64", r"(?i)windows[ _-]?(?:arm64|aarch64)"),
    ("named LAN VM (omega-linux, omega-win, omega-remote, vm-heavy, 'VM')", r"(?i)omega-linux|omega-win\b|omega-remote|vm-heavy|\bLAN VMs?\b|\bVMs?\b"),
    ("GitHub Actions run cited (actions/runs/ URL or 'Actions <id>')", r"(?i)actions/runs/\d|\bActions \d{8,}"),
    ("GitHub admission named as passing", r"(?i)admission (?:job )?(?:passe|passed|is green)|required admission"),
]
p("\n## hosts named in Proof paragraphs (compiler-lane, denominator = commits with Proof)")
hostflags = {}
for name, rx in HOSTS:
    R = re.compile(rx)
    hits = [r for r in withp if R.search(r["proofp"])]
    hostflags[name] = set(r["hash"] for r in hits)
    p(f"  {name}: {pct(len(hits), len(withp))}")
none_named = [r for r in withp if not any(r["hash"] in hostflags[n] for n, _ in HOSTS[:7] + HOSTS[7:9])]
p("  names no host/OS, VM or Actions run:", pct(len(none_named), len(withp)))
fams = ["macOS (any)", "Linux (any)", "Windows (any)"]
multi = [r for r in withp if sum(r["hash"] in hostflags[f] for f in fams) >= 2]
p("  names two or more OS families:", pct(len(multi), len(withp)))
other = [r for r in withp if r["hash"] in hostflags[HOSTS[7][0]] or r["hash"] in hostflags[HOSTS[8][0]]]
p("  names a LAN VM or cites an Actions run (another machine):", pct(len(other), len(withp)))
p("  ... of all compiler-lane commits:", pct(len(other), len(comp)))
for k in sorted(pc):
    L = [r for r in withp if r["period"] == k]
    p("    period", k, pct(sum(1 for r in L if r in other), len(L)))

p("\n## some check ran on a machine other than the author's (CI by period, or Proof cites VM/Actions run)")
ci = {}
for r in comp:
    reason = None
    if r["period"] == "P1":
        reason = "P1 compiler.yml (clippy -p omega, focused tests, boundary) on GitHub runner"
    elif r["period"] == "P3" and rustfiles[r["hash"]]:
        reason = "P3 admission merge-tree type-check on GitHub runner (no tests)"
    elif r["period"] == "P4" and r["v2"]:
        reason = "P4 admission V2 workspace tests on GitHub runner"
    elif r in other:
        reason = "Proof cites a LAN VM or a GitHub Actions run"
    if reason:
        ci[r["hash"]] = reason
cc = collections.Counter(ci.values())
for k, v in cc.most_common():
    p("  ", k, pct(v, len(comp)))
p("  any of these:", pct(len(ci), len(comp)))
tested = sum(1 for h, v in ci.items() if "type-check" not in v)
p("  excluding type-check-only:", pct(tested, len(comp)))
p("  P3 compiler commits that changed no Rust/Cargo file:", sum(1 for r in comp if r["period"] == "P3" and not rustfiles[r["hash"]]))

p("\n## hosts by agent (compiler-lane commits with Proof)")
for ag in ["Codex", "Claude Code"]:
    L = [r for r in withp if r["agent"] == ag]
    p(" ", ag, "n=", len(L), " ".join(f"{n.split(' (')[0]}={pct(sum(1 for r in L if r['hash'] in hostflags[n]), len(L))}" for n in ["macOS (any)", "Linux (any)", "Windows (any)", HOSTS[7][0], HOSTS[8][0]]))

# ---------------------------------------------------------------- breadth sample
SEED, N = 20261123, 120
pop = sorted(withp, key=lambda r: r["hash"])
sample = random.Random(SEED).sample(pop, N)
# Hand labels for the sample, by sample index, read from the Proof paragraph.
# Breadth (broadest run stated): N = one or a few named tests/controls/programs;
# C1 = one crate's suite; CM = several crates' suites; W = workspace-wide;
# S = sample or sweep run (sample-gate, samples/, many fixture programs);
# X = no test or program run stated (only lint, format, boundary, audit).
# Flags: F = states a failure (pre-existing or otherwise, excluding a new
# test failing before the fix as intended); U = states an unverified, pending,
# skipped or unclaimed case.
LABELS = {
    0: ("N", "", "v2"), 1: ("N", ""), 2: ("N", "U"), 3: ("C1", "F"), 4: ("N", "U"), 5: ("N", ""),
    6: ("N", "U"), 7: ("CM", ""), 8: ("N", "F"), 9: ("N", ""), 10: ("CM", ""), 11: ("CM", "U"),
    12: ("N", ""), 13: ("CM", "F"), 14: ("N", ""), 15: ("N", "U"), 16: ("CM", ""), 17: ("N", ""),
    18: ("N", "F"), 19: ("N", "U"), 20: ("N", ""), 21: ("N", ""), 22: ("C1", ""), 23: ("X", "U"),
    24: ("S", ""), 25: ("C1", "FU"), 26: ("N", ""), 27: ("N", "U"), 28: ("N", "U"), 29: ("N", "F"),
    30: ("CM", ""), 31: ("N", "U"), 32: ("N", ""), 33: ("S", ""), 34: ("S", "F"), 35: ("N", ""),
    36: ("C1", ""), 37: ("N", "U"), 38: ("N", ""), 39: ("N", ""), 40: ("CM", "U"), 41: ("N", ""),
    42: ("C1", ""), 43: ("C1", ""), 44: ("N", ""), 45: ("N", "F"), 46: ("N", ""), 47: ("N", "F"),
    48: ("C1", "F"), 49: ("N", "F"), 50: ("X", ""), 51: ("N", ""), 52: ("N", ""), 53: ("N", ""),
    54: ("N", ""), 55: ("CM", "F"), 56: ("N", ""), 57: ("N", ""), 58: ("N", ""), 59: ("N", "F"),
    60: ("C1", ""), 61: ("N", ""), 62: ("N", ""), 63: ("N", ""), 64: ("C1", "F"), 65: ("N", ""),
    66: ("CM", ""), 67: ("C1", "F"), 68: ("S", ""), 69: ("N", ""), 70: ("N", "F"), 71: ("CM", "F"),
    72: ("N", "U"), 73: ("N", ""), 74: ("N", "U"), 75: ("N", ""), 76: ("CM", "F"), 77: ("N", "F"),
    78: ("N", "F"), 79: ("CM", ""), 80: ("CM", "F"), 81: ("N", ""), 82: ("N", "U"), 83: ("N", ""),
    84: ("N", ""), 85: ("N", ""), 86: ("N", ""), 87: ("S", "F"), 88: ("N", "F"), 89: ("N", ""),
    90: ("N", "U"), 91: ("CM", "F"), 92: ("N", "F"), 93: ("N", "U"), 94: ("N", ""), 95: ("N", ""),
    96: ("N", "F"), 97: ("N", "U"), 98: ("N", "U"), 99: ("N", ""), 100: ("C1", "U"), 101: ("CM", ""),
    102: ("N", "F"), 103: ("N", "U"), 104: ("N", ""), 105: ("N", ""), 106: ("N", "U"), 107: ("S", ""),
    108: ("N", "U"), 109: ("N", "U"), 110: ("CM", "F"), 111: ("W", "", "v2"), 112: ("CM", "F"),
    113: ("N", "U"), 114: ("C1", "U"), 115: ("CM", "F"), 116: ("C1", ""), 117: ("CM", "F"),
    118: ("N", ""), 119: ("CM", ""),
}
# Hash prefixes at the labelled positions, so a changed sample is detected.
EXPECT = {0: "dc9026af05ad", 59: "d2ebe480c978", 119: "63e410eb6ee5"}
for i, h in EXPECT.items():
    assert sample[i]["hash"] == h, (i, sample[i]["hash"])
assert len(LABELS) == N
p(f"\n## breadth sample: n={N}, seed={SEED}, population = compiler-lane commits with Proof ({len(pop)}) sorted by hash")
cat = collections.Counter(LABELS[i][0] for i in range(N))
NAMES = {"N": "one or a few named tests/controls/programs", "C1": "one crate's suite", "CM": "several crates' suites",
         "W": "workspace-wide run", "S": "sample or sweep run", "X": "no test or program run stated"}
for k in ["N", "C1", "CM", "W", "S", "X"]:
    p(f"  {NAMES[k]}: {pct(cat[k], N)}")
F = sum(1 for i in range(N) if "F" in LABELS[i][1])
U = sum(1 for i in range(N) if "U" in LABELS[i][1])
FU = sum(1 for i in range(N) if LABELS[i][1])
p("  states a failure:", pct(F, N))
p("  states an unverified/pending/unclaimed case:", pct(U, N))
p("  states either:", pct(FU, N))
p("  sample items that are V2-workspace commits:", sum(1 for i in range(N) if sample[i]["v2"]))
p("  sample by period:", dict(collections.Counter(s["period"] for s in sample)))
cl = sum(1 for s in sample if re.search(r"(?i)clippy|\blint", s["proofp"]))
p("  sample Proofs that mention Clippy/lint:", pct(cl, N))
nm = sum(1 for s in sample if re.search(r"(?i)native|run --both|\bPsi exit|both engines", s["proofp"]))
p("  sample Proofs that mention a native or native/Psi run:", pct(nm, N))

# ---------------------------------------------------------------- workspace-wide runs
WS = re.compile(r"(?i)(?<!-p )(?:--workspace|workspace[- ]wide|whole[- ]workspace|full[- ]workspace|entire workspace|"
                r"workspace (?:tests?|test suite|nextest|clippy|all-target|lib tests|check)|\d[\d,]* (?:standalone |experimental |V2 )?(?:Rust )?workspace tests)")
NEG = re.compile(r"(?i)(not run|was not run|not been run|blocked|unbuildable|cannot|could not|still depend|listed for the next|still reports)")
p("\n## workspace-wide run statements (keyword match on Proof paragraph, compiler lanes)")
wsc = [r for r in withp if WS.search(r["proofp"])]
wsc_pos = [r for r in wsc if not NEG.search(r["proofp"][max(0, WS.search(r["proofp"]).start() - 80): WS.search(r["proofp"]).end() + 80])]
p("  Proof mentions workspace-wide run:", pct(len(wsc), len(withp)), "| without a nearby 'not run/blocked' phrase:", len(wsc_pos))
p("    of which V2-workspace commits (touch omega-rust-v2/ or say V2/omega-rust-v2):",
  sum(1 for r in wsc_pos if r["v2"] or re.search(r"V2|omega-rust-v2|standalone|experimental", r["proofp"])))
for r in wsc_pos:
    m = WS.search(r["proofp"])
    p("    ", r["hash"][:10], r["period"], "v2" if r["v2"] else "  ", r["title"][:70], "||", r["proofp"][max(0, m.start() - 50): m.end() + 40])
wsall = [r for r in rows if WS.search(bodies[r["hash"]])]
p("  any commit (all lanes) whose body mentions a workspace-wide run:", pct(len(wsall), len(rows)))
TW = re.compile(r"(?i)(full baseline|workspace (?:validation|baseline|run|test run)|validate the workspace|record the baseline)")
tw = [r for r in rows if TW.search(r["title"])]
p("  commits whose title is a whole-workspace validation/baseline run:", len(tw), [r["title"] for r in tw])

# ---------------------------------------------------------------- main broken
p("\n## main broken / red / failing when the change started")
TITLE_REPAIR = re.compile(r"(?i)\brestores? (?:the )?(?:build|compil\w*|compiler builds)|\brestores? .{0,30}test compilation|broke (?:test )?compilation|repair what #\d+ broke|compiles? again")
BODY_REPAIR = re.compile(r"(?i)(?:workspace lint|post-merge check|build|compilation|clippy) on main (?:fails|failed|is broken|was broken)|turning main's post-merge check red|main fails with E0\d{3}|original main E0\d{3}|restores? compilation|failed with E0\d{3} until")
rep = [r for r in rows if TITLE_REPAIR.search(r["title"]) or BODY_REPAIR.search(bodies[r["hash"]])]
p("  title or body says it repairs main's build/compilation/lint (keyword):", len(rep))
for r in rep:
    p("    ", r["hash"][:10], r["day"], r["lane"], r["title"][:90])
PRE = re.compile(r"(?i)(already (?:red|failing|failed|broken|fail)|(?:red|failing|fails|failed|broken|fail) (?:identically )?on (?:the )?(?:unchanged |unmodified |clean |current |exact |fixed )?(?:origin/)?(?:main|base)\b|main (?:is|was|stays|remains) (?:red|broken|failing)|pre-?existing (?:failure|failures|failing)|fail(?:s|ed|ing)? (?:identically |the same way )?(?:on|at) (?:the )?(?:exact |clean |fixed |unchanged |unmodified )?(?:base|main)\b|reproduces? (?:identically )?(?:on|at) (?:the )?(?:clean |fixed |exact )?(?:base|main)\b|(?:base|main) (?:also )?fails)")
pre = [r for r in rows if PRE.search(bodies[r["hash"]])]
LEDGER = re.compile(r"(?i)(?:rows|fixtures) (?:that )?(?:already )?regressed on main|regressions already on main")
led = [r for r in rows if LEDGER.search(r["title"])]
p("  titles recording corpus rows that already regressed on main:", len(led), "days", sorted(set(r["day"] for r in led)))
UNB = re.compile(r"(?i)(root (?:workspace|cargo build)[^.]{0,40}(?:unbuildable|cannot|blocked|prevent)|prevent a root cargo build|full workspace build still|workspace (?:is|remains) unbuildable|unbuildable)")
unb = [r for r in rows if UNB.search(bodies[r["hash"]])]
p("  bodies saying the root workspace could not be built:", len(unb), "days", sorted(set(r["day"] for r in unb)))
prec = [r for r in pre if r["lane"] in COMPILER]
p("  body says something already failed on main/base (keyword):", pct(len(pre), len(rows)), "| compiler lanes:", pct(len(prec), len(comp)))
for k in sorted(pc):
    L = [r for r in comp if r["period"] == k]
    p("    period", k, pct(sum(1 for r in L if r in prec), len(L)))
# precision check sample (seed 7, 30 hits) judged by hand: 25 of 30 genuine
p("  precision, hand-judged random 30 hits (seed 7 over the all-lane hit list): 25/30")

# ---------------------------------------------------------------- write
open(os.path.join(HERE, "gate_out.txt"), "w", encoding="utf-8").write("\n".join(out) + "\n")
print("\n".join(out))
