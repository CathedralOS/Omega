"""Merge a golden with a revision's golden, keeping BOTH sides' rows.

Why: the record tool re-synced the golden by OVERWRITING it from origin/main. A lane
that had committed fact A and then ran the tool for fact B lost A at the sync, because
main did not yet carry A's diagnostics -- and `git add -A` committed the revert under
B's message. This merges instead: every row present in either side survives.

A row is a key line plus its indented continuation (diagnostic lines). When a key
exists on both sides, the side with a DIAGNOSTIC wins, on the principle that a bare
`rejected` row carries no evidence and a diagnostic-bearing row does. A key present on
only one side is kept as-is. Output is sorted by key, which is the file's own order.

Usage: merge_rows.py <golden> <revision>
"""
import subprocess, sys
from pathlib import Path

def blocks(text):
    out, key, buf = {}, None, []
    for line in text.splitlines():
        if line and not line[0].isspace() and not line.startswith('#'):
            if key is not None:
                out[key] = buf
            key, buf = line.split()[0], [line]
        elif key is not None:
            buf.append(line)
    if key is not None:
        out[key] = buf
    return out

golden, rev = sys.argv[1], sys.argv[2]
theirs = blocks(subprocess.run(['git','show',f'{rev}:{golden}'],capture_output=True,text=True).stdout)
cur_text = Path(golden).read_text()
ours = blocks(cur_text)
hdr = [l for l in cur_text.splitlines() if not l.strip() or l.startswith('#')]

def has_diag(blk):
    return len(blk) > 1 and any(l[:1] in ('\t', ' ') and l.strip() for l in blk[1:])

merged, kept_theirs, kept_ours, both = {}, 0, 0, 0
for k in set(theirs) | set(ours):
    a, b = theirs.get(k), ours.get(k)
    if a and b:
        both += 1
        merged[k] = a if (has_diag(a) and not has_diag(b)) else b
        if merged[k] is a: kept_theirs += 1
        elif merged[k] is b: kept_ours += 1
    elif a:
        merged[k] = a; kept_theirs += 1
    else:
        merged[k] = b; kept_ours += 1

body = [l for k in sorted(merged) for l in merged[k]]
Path(golden).write_text('\n'.join(hdr + body) + '\n')
print(f"merged {len(merged)} rows (from {rev}: {kept_theirs}, kept ours: {kept_ours}, in both: {both})")
