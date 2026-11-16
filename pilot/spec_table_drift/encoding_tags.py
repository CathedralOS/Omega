#!/usr/bin/env python3
"""Compare encoding.md marked tag tables with the Terminal Psi wire decoders.

Decoders look like `match reader.u8()? { 1 => Space::Variant, ..., tag =>
Err(CodecError::InvalidTag("Space", tag)) }`. Extract (tag, variant) per space
and compare with each `<!-- marker -->` table's (tag, name) rows."""
import os, re, sys, json
ROOT = sys.argv[1] if len(sys.argv) > 1 else '.'
WIRE = os.path.join(ROOT, 'omega-rust/psi/compilation/representations/terminal-psi/wire/src')
PAGE = os.path.join(ROOT, 'wiki/spec/terminal-psi/encoding.md')

def norm(s): return re.sub(r'[^a-z0-9]', '', s.lower())

def match_bounds(text, pos):
    """Innermost `match ... { ... }` block enclosing pos -> (open, close)."""
    for m in reversed(list(re.finditer(r'\bmatch\b', text[:pos]))):
        o = text.find('{', m.end())
        if o < 0 or o > pos: continue
        d = 0; i = o
        while i < len(text):
            c = text[i]
            if c == '{': d += 1
            elif c == '}':
                d -= 1
                if d == 0: break
            i += 1
        if i > pos: return o, i
    return None

ARM = re.compile(r'^\s*(?:\w+\s*@\s*\(?)?((?:\d+\s*\|\s*)*\d+|\d+\s*\.\.=\s*\d+)\)?\s*=>', re.M)
def arms(block):
    # keep only depth-1 text
    out = []; d = 0; buf = []
    for c in block:
        if c == '{': d += 1
        if d <= 1: buf.append(c)
        else: buf.append(c if c == '\n' else ' ')
        if c == '}': d -= 1
    flat = ''.join(buf)
    full = block
    ms = list(ARM.finditer(flat))
    for k, m in enumerate(ms):
        end = ms[k+1].start(1) if k+1 < len(ms) else len(full)
        body = full[m.end():end]
        b = body.strip()
        while True:
            n = re.sub(r'^(?:Ok|Some|Box::new|return)\s*\(?', '', b, count=1).lstrip()
            if n == b: break
            b = n
        p = re.match(r'(?:[A-Z]\w*::)*([A-Z]\w*)', b)
        q = re.search(r'\b(?!CodecError\b)(?:[A-Z]\w*)::([A-Z]\w*)', b)
        name = p.group(1) if p else q.group(1) if q else re.sub(r'\s+', ' ', b)[:30]
        lab = m.group(1)
        if '..=' in lab:
            a, b = map(int, lab.split('..=')); tags = range(a, b+1)
        else: tags = [int(t) for t in lab.split('|')]
        for t in tags: out.append((t, name))
    return out

def decoders():
    """Every `match ... {` block with integer arms, keyed by the InvalidTag
    space it names (if any) and by its enclosing fn name (`fn:name`)."""
    spaces = {}
    for f in sorted(os.listdir(WIRE)):
        if not f.endswith('.rs') or f == 'spec_tag_tables.rs': continue
        text = open(os.path.join(WIRE, f)).read()
        for m in re.finditer(r'\bmatch\b[^{;]*\{', text):
            o = m.end() - 1; d = 0; i = o
            while i < len(text):
                if text[i] == '{': d += 1
                elif text[i] == '}':
                    d -= 1
                    if d == 0: break
                i += 1
            block = text[o:i+1]
            a = arms(block)
            if not a: continue
            line = text[:o].count('\n') + 1
            fns = re.findall(r'fn (\w+)', text[:o])
            keys = set()
            if fns: keys.add('fn:' + fns[-1])
            for sm in re.finditer(r'InvalidTag\(\s*"(\w+)"', block): keys.add(sm.group(1))
            for k in keys: spaces.setdefault(k, []).append((f'{f}:{line}', a))
        # macro tables: `Alias: RustType, "Space" { 1 => Variant ..., }`
        for m in re.finditer(r'(\w+):\s*([\w:]+),\s*"(\w+)"\s*\{', text):
            o = m.end() - 1; d = 0; i = o
            while i < len(text):
                if text[i] == '{': d += 1
                elif text[i] == '}':
                    d -= 1
                    if d == 0: break
                i += 1
            a = arms(text[o:i+1])
            if a:
                line = text[:o].count('\n') + 1
                for k in {m.group(3), 'type:' + m.group(2).split('::')[-1]}:
                    spaces.setdefault(k, []).append((f'{f}:{line}', a))
    return spaces

def tables():
    L = open(PAGE).read().split('\n'); out = {}
    for i, l in enumerate(L):
        m = re.match(r'^<!-- ([\w-]+) -->$', l.strip())
        if not m: continue
        j = i + 1
        while j < len(L) and not L[j].strip(): j += 1
        if not L[j].startswith('|'): continue
        hdr = [c.strip() for c in L[j].strip('|').split('|')]
        rows = []; k = j + 2
        while k < len(L) and L[k].startswith('|'):
            cells = [c.strip() for c in L[k].strip().strip('|').split('|')]
            rows.append((k+1, cells)); k += 1
        out[m.group(1)] = (j+1, hdr, rows)
    return out

HAND = json.load(open(os.path.join(os.path.dirname(__file__), 'encoding_map.json'))) \
    if os.path.exists(os.path.join(os.path.dirname(__file__), 'encoding_map.json')) else {}

def main():
    sp = decoders(); tb = tables()
    report = {'compared': [], 'unmapped': [], 'drift': []}
    for mk, (line, hdr, rows) in tb.items():
        if hdr[0].lower() != 'tag':
            report['unmapped'].append((mk, f'first column is {hdr[0]!r}, not a tag')); continue
        spec = {}
        for ln, c in rows:
            try: spec[int(c[0].strip('`'))] = (c[1].strip('`'), ln)
            except ValueError: pass
        names = {norm(n) for n, _ in spec.values()}
        if mk in HAND:
            cand = [HAND[mk]] if HAND[mk] else []
        else:
            scored = []
            for s, decs in sp.items():
                for loc, a in decs:
                    an = {norm(n) for _, n in a}
                    sc = len(names & an) / max(len(names | an), 1)
                    scored.append((sc, s))
            scored.sort(reverse=True)
            cand = [scored[0][1]] if scored and scored[0][0] >= 0.5 else []
        if not cand:
            report['unmapped'].append((mk, 'no decoder matched')); continue
        s = cand[0]
        key, _, want = s.partition('@')
        for loc, a in sp.get(key, []):
            if want and loc != want: continue
            code = dict(a)
            diffs = []
            for t in sorted(set(spec) | set(code)):
                if spec.get(t, ('', 0))[0] == 'retired' and t not in code: continue
                if t not in code: diffs.append(f'spec line {spec[t][1]} lists tag {t} {spec[t][0]}; decoder has no arm')
                elif t not in spec: diffs.append(f'decoder accepts tag {t} => {code[t]}; spec omits it')
                elif re.fullmatch(r'[A-Z]\w*', code[t]) and norm(spec[t][0]) != norm(code[t]):
                    diffs.append(f'tag {t}: spec line {spec[t][1]} says {spec[t][0]!r}, decoder says {code[t]!r}')
            report['compared'].append((mk, s, loc, len(spec)))
            if diffs: report['drift'].append((mk, s, loc, diffs))
    print(f"markers: {len(tb)}  compared: {len(report['compared'])}  unmapped: {len(report['unmapped'])}")
    for mk, why in report['unmapped']: print('  UNMAPPED', mk, '-', why)
    for mk, s, loc, n in report['compared']: print('  OK-MAPPED', mk, '->', s, loc, f'{n} rows')
    for mk, s, loc, d in report['drift']:
        print(f'DRIFT {mk} vs {s} ({loc})'); [print('   ', x) for x in d]

main()
