"""Count how many rejected rows for each fact carry a diagnostic line.

Used as a regression guard: a diagnostic can vanish while its KEY survives, so the
LOST check (which counts keys) cannot see it. A lane's second fact reverted its first
fact's diagnostics and `git add -A` committed the revert under the second fact's
message -- nothing reported it.

Usage:
  diag_counts.py <golden>            -> prints "fact count" per fact with any rejected rows
  diag_counts.py <golden> <fact>...  -> prints "fact with_diag/total" for those facts
"""
import sys
from pathlib import Path

def counts(path, facts=None):
    lines = Path(path).read_text().splitlines()
    out = {}
    for i, l in enumerate(lines):
        if 'fail/facts/' in l and 'rejected' in l:
            rel = l.split('fail/facts/')[1].split('/')[0]
            fact = rel
            for m in ('_through_', '_unproved', '_rejected'):
                if m in fact:
                    fact = fact.split(m)[0]
            if facts and fact not in facts:
                continue
            has = i + 1 < len(lines) and (lines[i+1].startswith('\t') or lines[i+1].startswith('    '))
            tot, wd = out.get(fact, (0, 0))
            out[fact] = (tot + 1, wd + (1 if has else 0))
    return out

if __name__ == '__main__':
    golden = sys.argv[1]
    facts = set(sys.argv[2:]) or None
    for fact, (tot, wd) in sorted(counts(golden, facts).items()):
        print(f"{fact} {wd}/{tot}")
