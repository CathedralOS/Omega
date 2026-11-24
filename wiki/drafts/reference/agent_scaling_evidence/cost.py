#!/usr/bin/env python3
"""Cost, speed and steering per landed commit on origin/main (14 days).

Reads commits.tsv (one row per first-parent commit) and prints the tables
used in cost.md. No sampling, no randomness. Run:
    python3 cost.py [path/to/commits.tsv] [chain_ratio]
"""
import csv, sys, statistics, collections
from datetime import datetime

csv.field_size_limit(10**9)
SRC = sys.argv[1] if len(sys.argv) > 1 else \
    '<tables>/commits.tsv'
RATIO = float(sys.argv[2]) if len(sys.argv) > 2 else 0.7
MIN_PRED_TOKENS = 50e6      # predecessor must be a large session to count as a chain
MAX_GAP_H = 72              # predecessor must be at most 72 h earlier
SLACK_MIN = 30              # active-minute growth may exceed wall gap by 30 min

rows = list(csv.DictReader(open(SRC), delimiter='\t'))
for x in rows:
    x['t'] = datetime.fromisoformat(x['date'])
    for k in ('tokens_in', 'tokens_cached', 'tokens_out', 'steps', 'minutes', 'steers'):
        x[k + '_n'] = float(x[k]) if x[k] != '' else None
rows.sort(key=lambda x: x['t'])
DAYS = sorted({x['day'] for x in rows})

FULL = ('tokens_in_n', 'tokens_cached_n', 'tokens_out_n', 'steps_n', 'minutes_n', 'steers_n')
def has_all(x): return all(x[k] is not None for k in FULL)

def agent_group(a):
    if a == '': return '(no provenance line)'
    if a in ('Codex', 'Claude Code'): return a
    return 'other (Cursor, Antigravity, Human/Copilot, Devin, ChatGPT)'

MAIN_LANES = ['psi', 'compiler', 'docs', 'omega', 'repo', 'build', 'tools', 'backend',
              'tests', 'bootstrap-omega', 'bootstrap', 'packages', 'samples', 'library']
def lane_group(l):
    if l == '': return '(no lane prefix)'
    return l if l in MAIN_LANES else 'other lanes'

# ---- chain detection: provenance lines that report a running session total ----
full = [x for x in rows if has_all(x)]
for i, b in enumerate(full):
    best = None
    for a in full[:i]:
        gap = (b['t'] - a['t']).total_seconds() / 60
        if gap < 0 or gap > MAX_GAP_H * 60 or a['agent'] != b['agent']:
            continue
        if a['tokens_in_n'] < MIN_PRED_TOKENS or a['tokens_in_n'] < RATIO * b['tokens_in_n']:
            continue
        if not (a['tokens_in_n'] <= b['tokens_in_n'] and a['steps_n'] <= b['steps_n']
                and a['steers_n'] <= b['steers_n'] and a['tokens_out_n'] <= b['tokens_out_n']):
            continue
        if b['minutes_n'] - a['minutes_n'] > gap + SLACK_MIN:
            continue
        if b['minutes_n'] < a['minutes_n'] - 30:
            continue
        if best is None or a['tokens_in_n'] > best['tokens_in_n']:
            best = a
    b['pred'] = best
    for k in FULL:
        b[k[:-2] + '_d'] = max(0.0, b[k] - best[k]) if best else b[k]
    # An unchained row whose own time exceeds 24 h is a running total that began
    # before any row we can subtract; it cannot be split into per-commit cost.
    b['unres'] = best is None and b['minutes_n'] > 1440
    # Placeholder lines: 0 tokens, 1 step, 1 s.
    b['zero'] = b['tokens_in_n'] == 0
# A = the corrected analysis set.
A = [x for x in full if not x['unres'] and not x['zero']]

def q(v, p):
    v = sorted(v)
    if not v: return float('nan')
    i = (len(v) - 1) * p
    lo = int(i); hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (i - lo)

def fm(v):
    if v != v: return '-'
    a = abs(v)
    if a >= 1e9: return f'{v/1e9:.2f}B'
    if a >= 1e6: return f'{v/1e6:.1f}M'
    if a >= 1e4: return f'{v/1e3:.0f}k'
    if a >= 100: return f'{v:.0f}'
    return f'{v:.1f}'

def stats(v):
    return (len(v), q(v, .5), statistics.mean(v) if v else float('nan'), q(v, .9), sum(v))

out = []
def P(s=''): out.append(s)
def table(head, body):
    P('| ' + ' | '.join(head) + ' |'); P('|' + '---|' * len(head))
    for r in body: P('| ' + ' | '.join(str(c) for c in r) + ' |')
    P()

N = len(rows)
P(f'commits: {N}; days: {len(DAYS)} ({DAYS[0]} .. {DAYS[-1]}); chain ratio {RATIO}')
miss_line = sum(x['agent'] == '' for x in rows)
unavail = sum(x['agent'] != '' and x['tokens_in_n'] is None for x in rows)
P(f'no provenance line: {miss_line}/{N}; line present but numbers unavailable/unparsed: {unavail}/{N}; '
  f'all six numbers present: {len(full)}/{N}')
zero = [x for x in full if x['tokens_in_n'] == 0]
P(f'rows reporting 0 tokens: {len(zero)}/{len(full)} (steps<=1: {sum(x["steps_n"]<=1 for x in zero)})')
chained = [x for x in full if x['pred']]
P(f'rows chained to an earlier running total: {len(chained)}/{len(full)}')
P(f'unchained running totals over 24 h (excluded from set A): {sum(x["unres"] for x in full)}; '
  f'zero-token placeholders (excluded from set A): {sum(x["zero"] for x in full)}; set A: {len(A)}')
P(f'set A rows whose corrected tokens in is 0 (same totals as predecessor): {sum(x["tokens_in_d"]==0 for x in A)}')
P()

# ---- commits per day ----
P('## per day')
cd = collections.Counter(x['day'] for x in rows)
body = []
for d in DAYS:
    f = [x for x in full if x['day'] == d]
    fa = [x for x in A if x['day'] == d]
    body.append([d, cd[d], len(f), len(fa),
                 fm(q([x['tokens_in_d'] for x in fa], .5)),
                 fm(sum(x['tokens_in_n'] for x in f)), fm(sum(x['tokens_in_d'] for x in fa)),
                 fm(sum(x['tokens_in_d'] - x['tokens_cached_d'] for x in fa)),
                 fm(sum(x['tokens_out_d'] for x in fa)),
                 f"{sum(x['minutes_n'] for x in f)/60:.0f}", f"{sum(x['minutes_d'] for x in fa)/60:.0f}",
                 f"{sum(x['minutes_d'] for x in fa)/60/24:.1f}",
                 fm(q([x['minutes_d'] for x in fa], .5)),
                 f"{sum(x['steers_d']==0 for x in fa)}/{len(fa)}",
                 fm(sum(x['steers_n'] for x in f)), fm(sum(x['steers_d'] for x in fa))])
table(['day', 'commits', 'with numbers', 'in set A', 'median tokens in (A, corr.)', 'tokens in, as reported',
       'tokens in, chain-corrected', 'uncached in, corrected', 'tokens out, corrected',
       'agent-hours as reported', 'agent-hours corrected', 'implied agents at once (corrected h/24)',
       'median minutes (A, corr.)', 'zero-steer commits (A, corr.)', 'steers as reported', 'steers corrected'], body)

# ---- per-commit distributions ----
P('## per-commit distribution (rows with all numbers)')
body = []
for k, lab in [('tokens_in', 'tokens in'), ('tokens_cached', 'tokens cached'), ('tokens_out', 'tokens out'),
               ('steps', 'steps'), ('minutes', 'minutes'), ('steers', 'steers')]:
    n, med, mean, p90, tot = stats([x[k + '_n'] for x in full])
    n2, med2, mean2, p902, tot2 = stats([x[k + '_d'] for x in A])
    body.append([lab, n, fm(med), fm(mean), fm(p90), fm(tot), n2, fm(med2), fm(mean2), fm(p902), fm(tot2)])
unc = [x['tokens_in_n'] - x['tokens_cached_n'] for x in full]
uncd = [x['tokens_in_d'] - x['tokens_cached_d'] for x in A]
body.append(['tokens in minus cached', len(unc), fm(q(unc, .5)), fm(statistics.mean(unc)), fm(q(unc, .9)), fm(sum(unc)), len(uncd),
             fm(q(uncd, .5)), fm(statistics.mean(uncd)), fm(q(uncd, .9)), fm(sum(uncd))])
table(['measure', 'n reported', 'median', 'mean', 'p90', 'total', 'n in A', 'median (corr.)', 'mean (corr.)', 'p90 (corr.)', 'total (corr.)'], body)
cache_share = sum(x['tokens_cached_n'] for x in full) / sum(x['tokens_in_n'] for x in full)
P(f'cached share of tokens in (as reported): {cache_share:.3f}')
P()

# ---- steer distribution ----
P('## steers per commit')
bins = [(0, 0), (1, 1), (2, 5), (6, 20), (21, 100), (101, 10**9)]
body = []
for lo, hi in bins:
    a = sum(lo <= x['steers_n'] <= hi for x in full)
    b = sum(lo <= x['steers_d'] <= hi for x in A)
    body.append([f'{lo}' if lo == hi else (f'{lo}-{hi}' if hi < 10**9 else f'>{lo-1}'), f'{a} ({a/len(full):.0%})', f'{b} ({b/len(A):.0%})'])
table(['steers', f'commits as reported (n={len(full)})', f'set A chain-corrected (n={len(A)})'], body)

# ---- by agent and by model ----
def group_table(keyf, title, keys=None):
    P(f'## by {title}')
    g = collections.defaultdict(list); ga = collections.Counter()
    for x in rows: ga[keyf(x)] += 1
    gr = collections.defaultdict(list)
    for x in full: gr[keyf(x)].append(x)
    for x in A: g[keyf(x)].append(x)
    ks = keys or sorted(ga, key=lambda k: -ga[k])
    body = []
    for k in ks:
        f = g.get(k, []); fr = gr.get(k, [])
        if not f:
            body.append([k, ga[k], len(fr), 0] + ['-'] * 8); continue
        body.append([k, ga[k], len(fr), len(f),
                     fm(q([x['tokens_in_n'] for x in fr], .5)), fm(q([x['tokens_in_d'] for x in f], .5)),
                     fm(q([x['tokens_out_d'] for x in f], .5)),
                     fm(q([x['steps_d'] for x in f], .5)), fm(q([x['minutes_d'] for x in f], .5)),
                     f"{sum(x['steers_d']==0 for x in f)/len(f):.0%}",
                     f"{statistics.mean([x['steers_d'] for x in f]):.1f}",
                     f"{sum(x['tokens_in_d'] for x in f)/sum(x['tokens_in_d'] for x in A):.0%}"])
    table([title, 'commits', 'with numbers', 'in set A', 'median tokens in (reported, all with numbers)', 'median tokens in (corr.)',
           'median tokens out (corr.)', 'median steps (corr.)', 'median minutes (corr.)',
           'zero-steer share (A)', 'mean steers (A, corr.)', 'share of set-A corrected tokens in'], body)

group_table(lambda x: agent_group(x['agent']), 'agent')
P('### agents, raw names'); P(', '.join(f'{k or "(none)"}: {v}' for k, v in collections.Counter(x['agent'] for x in rows).most_common())); P()
group_table(lambda x: (x['agent'] or '(none)') + ' / ' + (x['model'] or '(none)')
            if collections.Counter(y['model'] for y in rows)[x['model']] >= 20 else 'models with <20 commits', 'agent / model')
group_table(lambda x: lane_group(x['lane']), 'lane')

# ---- trend: first 7 days vs last 7 days, and per-day medians ----
P('## trend')
half1, half2 = DAYS[:7], DAYS[7:]
body = []
for lab, ds in [('first 7 days ' + half1[0] + '..' + half1[-1], half1), ('last 7 days ' + half2[0] + '..' + half2[-1], half2)]:
    f = [x for x in A if x['day'] in ds]
    body.append([lab, sum(cd[d] for d in ds), len(f),
                 fm(q([x['tokens_in_d'] for x in f], .5)),
                 fm(statistics.mean([x['tokens_in_d'] for x in f])),
                 fm(q([x['minutes_d'] for x in f], .5)),
                 f"{sum(x['steers_d']==0 for x in f)/len(f):.0%}", f"{statistics.mean([x['steers_d'] for x in f]):.2f}",
                 f"{len([x for x in f if x['pred']])}/{len(f)}"])
table(['period', 'commits', 'in set A', 'median tokens in (corr.)', 'mean tokens in (corr.)',
       'median minutes (corr.)', 'zero-steer share', 'mean steers (corr.)', 'chained rows'], body)

P('### per day and agent (set A, corrected)')
body = []
for d in DAYS:
    row = [d]
    for ag in ('Codex', 'Claude Code'):
        f = [x for x in A if x['day'] == d and x['agent'] == ag]
        row += [len(f), fm(q([x['tokens_in_d'] for x in f], .5)) if f else '-',
                f"{sum(x['steers_d']==0 for x in f)/len(f):.0%}" if f else '-']
    row.append(sum(1 for x in rows if x['day'] == d and x['agent'] == 'Codex' and x['tokens_in_n'] is None))
    body.append(row)
table(['day', 'Codex n', 'Codex median tokens in', 'Codex zero-steer', 'Claude n', 'Claude median tokens in',
       'Claude zero-steer', 'Codex commits with "unavailable" line'], body)

def spearman(xs, ys):
    def rank(v):
        o = sorted(range(len(v)), key=lambda i: v[i]); r = [0] * len(v); i = 0
        while i < len(o):
            j = i
            while j + 1 < len(o) and v[o[j + 1]] == v[o[i]]: j += 1
            for k in range(i, j + 1): r[o[k]] = (i + j) / 2
            i = j + 1
        return r
    a, b = rank(xs), rank(ys)
    ma, mb = statistics.mean(a), statistics.mean(b)
    num = sum((p - ma) * (q_ - mb) for p, q_ in zip(a, b))
    den = (sum((p - ma) ** 2 for p in a) * sum((q_ - mb) ** 2 for q_ in b)) ** .5
    return num / den
ti = [x['t'].timestamp() for x in A]
for k in ('tokens_in_d', 'minutes_d', 'steers_d', 'steps_d'):
    P(f'spearman(commit time, {k}) over set A, {len(A)} rows: {spearman(ti, [x[k] for x in A]):+.3f}')
for ag in ('Codex', 'Claude Code'):
    f = [x for x in A if x['agent'] == ag]
    P(f'  {ag}: spearman(time, tokens_in_d)={spearman([x["t"].timestamp() for x in f],[x["tokens_in_d"] for x in f]):+.3f}, '
      f'(time, steers_d)={spearman([x["t"].timestamp() for x in f],[x["steers_d"] for x in f]):+.3f}, n={len(f)}')
P()

# ---- concentration ----
P('## concentration')
for k, lab, S in [('tokens_in_n', 'reported, all with numbers', full), ('tokens_in_d', 'set A chain-corrected', A)]:
    v = sorted((x[k] for x in S), reverse=True)
    t = sum(v); n10 = round(len(v) * .1); n1 = round(len(v) * .01)
    P(f'{lab}: top 10% ({n10} of {len(v)}) hold {sum(v[:n10])/t:.1%} of tokens in; top 1% ({n1}) hold {sum(v[:n1])/t:.1%}; '
      f'bottom 50% hold {sum(v[len(v)//2:])/t:.1%}')
vs = sorted((x['steers_d'] for x in A), reverse=True); t = sum(vs)
P(f'steers corrected: top 10% of commits hold {sum(vs[:round(len(vs)*.1)])/t:.1%} of steers')
P()

# ---- large reported times ----
P('## reported time per commit')
m = [x['minutes_n'] for x in full]
for th in (60, 240, 1440, 10080):
    P(f'reported minutes > {th}: {sum(v > th for v in m)}/{len(m)}')
P(f'max reported minutes {max(m):.0f} ({max(m)/60:.0f} h); window length {len(DAYS)} days = {len(DAYS)*24} h')
big = [x for x in full if x['minutes_n'] > 1440]
P(f'of the {len(big)} rows >24 h, chained to an earlier running total: {sum(1 for x in big if x["pred"])}')
P()
print('\n'.join(out))

# ---- split at the provenance-counting fix (PR #3707, merged 2026-11-15 19:26:49 -08:00) ----
FIX = datetime.fromisoformat('2026-11-15T19:26:49-08:00')
out2 = ['## before / after the Claude Code provenance fix (#3707, 8b2f3c251c)']
hdr = ['agent', 'period', 'commits', 'in set A', 'median tokens in (corr.)', 'mean tokens in (corr.)', 'p90 tokens in (corr.)',
       'median minutes (corr.)', 'median steps (corr.)', 'zero-steer share', 'mean steers', 'p90 steers', 'max steers']
out2.append('| ' + ' | '.join(hdr) + ' |'); out2.append('|' + '---|' * len(hdr))
for ag in ('Codex', 'Claude Code', None):
    for lab, cond in (('before fix', lambda x: x['t'] < FIX), ('after fix', lambda x: x['t'] >= FIX)):
        sel = lambda x: (ag is None or x['agent'] == ag) and cond(x)
        f = [x for x in A if sel(x)]
        nall = sum(1 for x in rows if sel(x))
        s_ = [x['steers_d'] for x in f]
        out2.append('| ' + ' | '.join(str(c) for c in [ag or 'all agents', lab, nall, len(f),
            fm(q([x['tokens_in_d'] for x in f], .5)), fm(statistics.mean([x['tokens_in_d'] for x in f])),
            fm(q([x['tokens_in_d'] for x in f], .9)), fm(q([x['minutes_d'] for x in f], .5)),
            fm(q([x['steps_d'] for x in f], .5)), f"{sum(v==0 for v in s_)/len(s_):.0%}",
            f"{statistics.mean(s_):.2f}", fm(q(s_, .9)), fm(max(s_))]) + ' |')
fa = [x for x in A if x['t'] >= FIX]
out2.append('')
out2.append('after fix, steer distribution (set A, corrected): ' + ', '.join(
    f'{lab}: {n}' for lab, n in [('0', sum(x["steers_d"]==0 for x in fa)), ('1', sum(x["steers_d"]==1 for x in fa)),
                                  ('2-5', sum(2<=x["steers_d"]<=5 for x in fa)), ('6-20', sum(6<=x["steers_d"]<=20 for x in fa)),
                                  ('>20', sum(x["steers_d"]>20 for x in fa))]) + f' (n={len(fa)})')
dd = sorted({x['day'] for x in fa})
fd = [x for x in fa if x['day'] > '2026-11-15']
out2.append(f'after fix, total corrected steers on 2026-11-16..2026-11-23: {sum(x["steers_d"] for x in fd):.0f} over {len(fd)} set-A commits '
            f'({sum(x["steers_d"] for x in fd)/8:.0f} per day)')
print('\n'.join(out2))
