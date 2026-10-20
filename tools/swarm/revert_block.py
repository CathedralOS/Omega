"""Restore ONE fact's row blocks in the outcomes golden from a revision, leaving every
other byte alone.

Why this exists: the revert path used `git checkout HEAD -- <golden>`, a whole-file
restore. After a rebase, HEAD holds MAIN's golden, where this run's earlier facts are
still bare -- so reverting one bad fact discarded every earlier fact's diagnostic in
the same run. Nothing reported it: the tool's LOST check counts KEYS, and the key
survives with its diagnostic line removed.

Usage: revert_block.py <golden> <revision> <fact-fragment>
Prints how many row blocks it restored.
"""
import subprocess, sys, re
from pathlib import Path

golden, rev, fragment = sys.argv[1], sys.argv[2], sys.argv[3]
base = subprocess.run(['git', 'show', f'{rev}:{golden}'], capture_output=True, text=True).stdout
cur = Path(golden).read_text()

def blocks(text):
    """Map fixture key -> full block (key line plus its tab-indented continuation)."""
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

hdr = [l for l in cur.splitlines() if not l.strip() or l.startswith('#')]
origin_b, cur_b = blocks(base), blocks(cur)
restored = 0
for k, blk in origin_b.items():
    if fragment in k and k in cur_b:
        cur_b[k] = blk
        restored += 1
body = [l for k in sorted(cur_b) for l in cur_b[k]]
Path(golden).write_text('\n'.join(hdr + body) + '\n')
print(restored)
