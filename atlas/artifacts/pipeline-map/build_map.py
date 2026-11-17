#!/usr/bin/env python3
"""Generate the Omega pipeline map page (inline SVG, Omega design tokens)."""
import html

import os
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, 'out', 'index.html')
E = html.escape
parts = []


def box(x, y, w, h, title, sub=None, kind='psi'):
    """kind: psi (outlined), native (filled), proposed (dashed outline), around (thin), hub."""
    cls = {'psi': 'b-psi', 'native': 'b-native', 'proposed': 'b-prop', 'around': 'b-around', 'hub': 'b-hub'}[kind]
    tcls = 't-on' if kind == 'native' else 't'
    s = [f'<rect class="{cls}" x="{x}" y="{y}" width="{w}" height="{h}"/>']
    cx = x + w / 2
    if sub:
        s.append(f'<text class="{tcls} lbl" x="{cx}" y="{y + h/2 - 3}" text-anchor="middle">{E(title)}</text>')
        s.append(f'<text class="{tcls} sub" x="{cx}" y="{y + h/2 + 13}" text-anchor="middle">{E(sub)}</text>')
    else:
        s.append(f'<text class="{tcls} lbl" x="{cx}" y="{y + h/2 + 5}" text-anchor="middle">{E(title)}</text>')
    return '\n'.join(s)


def arrow(d, label=None, lx=0, ly=0, anchor='middle', cls='a'):
    s = [f'<path class="{cls}" d="{d}" marker-end="url(#{"ah-leak" if cls == "leak" else "ah"})"/>']
    if label:
        tcls = 'al-leak' if cls == 'leak' else 'al'
        s.append(f'<text class="{tcls}" x="{lx}" y="{ly}" text-anchor="{anchor}">{E(label)}</text>')
    return '\n'.join(s)


defs = '''<defs>
<marker id="ah" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 z" class="ah"/></marker>
<marker id="ah-leak" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 z" class="ah-leak"/></marker>
<marker id="ah-ok" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 z" class="ah-ok"/></marker>
</defs>'''

# ---------------- Figure 1: the pipeline ----------------
m = []
m.append(defs)
# Around the pipeline (top band)
m.append('<text class="lane" x="30" y="16">AROUND THE PIPELINE</text>')
m.append(box(30, 26, 140, 46, 'omega CLI', kind='around'))
m.append(box(220, 26, 150, 46, 'Build system', 'orchestration', kind='around'))
m.append(box(420, 26, 190, 46, 'Packages', 'acquire · review · lock', kind='around'))
m.append(box(660, 26, 190, 46, 'Build evaluation', 'runs build.omg', kind='around'))
m.append(box(900, 26, 170, 46, 'build requests', 'proposed', kind='proposed'))
m.append(arrow('M170 49 H218', 'runs', 194, 42))
m.append(arrow('M370 49 H418', 'resolves', 394, 42))
m.append(arrow('M610 49 H658', 'sources', 634, 42))
m.append(arrow('M850 49 H898', 'yields', 874, 42))
m.append(arrow('M985 72 V160', 'read by 02–05', 993, 120, 'start'))

# Psi lane
m.append('<text class="lane" x="30" y="122">PSI · TARGET-NEUTRAL</text>')
m.append('<text class="drv" x="30" y="138">driven by source-compiler (00–04) and terminal-production (05–06)</text>')
psi = [('source files', 'proposed rep', 'proposed'), ('tokens', None, 'psi'), ('syntax trees', None, 'psi'),
       ('symbol-resolved trees', None, 'psi'), ('typed trees', None, 'psi')]
xs = [30, 250, 470, 690, 910]
for (t, sub, k), x in zip(psi, xs):
    m.append(box(x, 162, 150, 46, t, sub, kind=k))
labels = ['00 lex', '01 parse', '02 resolve', '03 type']
for i, lab in enumerate(labels):
    x0 = xs[i] + 150
    x1 = xs[i + 1]
    m.append(arrow(f'M{x0} 185 H{x1 - 2}', lab, (x0 + x1) / 2, 178))
# 04 loop below typed trees
m.append(arrow('M955 208 C955 244, 1015 244, 1015 210', '04 specialize', 985, 262))
# 05 lower: typed trees -> Terminal Psi
m.append(arrow('M1060 185 H1105 V338', '05 lower', 1113, 270, 'start'))
# Terminal Psi hub
m.append(box(1020, 340, 160, 58, 'Terminal Psi', 'PSITERM · portable', kind='hub'))
# 06 checker loop on top-left of Terminal Psi
m.append('<path class="ok" d="M1040 340 C1040 312, 1080 312, 1080 338" marker-end="url(#ah-ok)"/>')
m.append('<text class="al-ok" x="1030" y="312" text-anchor="end">06 lowered-form checker</text>')
m.append('<text class="al-ok sub" x="1030" y="327" text-anchor="end">the only verdict</text>')
# interpreter
m.append(box(700, 345, 170, 48, 'Psi interpreter', 'runs Terminal Psi', kind='psi'))
m.append(arrow('M1020 369 H872', 'interprets', 946, 362))
m.append('<text class="al" x="785" y="410" text-anchor="middle">omega run --both: both must exit 0</text>')

# Firewall
m.append('<line class="fw" x1="20" y1="440" x2="1180" y2="440"/>')
m.append('<text class="fwl" x="30" y="432">FIREWALL · Psi never depends on Omega</text>')

# Omega lane
m.append('<text class="lane" x="30" y="470">OMEGA · PER TARGET</text>')
m.append('<text class="drv" x="30" y="486">driven by native-compiler (00–09; 10–14 proposed)</text>')
oxs = [1030, 810, 590, 370, 150]
om = ['abstract operations', 'target operations', 'selected instructions', 'register homes', 'post-allocation machine']
for t, x in zip(om, oxs):
    m.append(box(x, 500, 150 if x != 1030 else 150, 46, t, kind='native'))
# 00 lower: Terminal Psi -> abstract operations (the one legal crossing)
m.append(arrow('M1105 398 V498', '00 lower', 1113, 428, 'start'))
# 01 loop under abstract operations
m.append(arrow('M1075 546 C1075 576, 1135 576, 1135 548', '01 optimize', 1105, 592))
olab = ['02 legalize', '03 select', '05 allocate', '06']
for i, lab in enumerate(olab):
    x_from = oxs[i]
    x_to = oxs[i + 1] + 150
    m.append(arrow(f'M{x_from} 523 H{x_to + 2}', lab, (x_from + x_to) / 2, 516))
# 04 loop on selected instructions (above)
m.append(arrow('M635 500 C635 474, 695 474, 695 498', '04', 665, 470))
# row 2: the pipeline continues past 09 through proposed stages 10-14
m.append(box(150, 612, 130, 46, 'machine code', 'encoded · laid out', kind='native'))
m.append(box(330, 612, 150, 46, 'function fragments', 'proposed form', kind='proposed'))
m.append(box(530, 612, 120, 46, 'text section', 'proposed form', kind='proposed'))
m.append(box(700, 612, 110, 46, 'object', 'proposed form', kind='proposed'))
m.append(box(860, 612, 120, 46, 'native image', 'ELF · Mach-O · PE', kind='native'))
m.append(arrow('M215 546 V610', '07 frame + encode · 08 lay out', 207, 590, 'end'))
m.append(arrow('M190 658 C190 690, 250 690, 250 660', '09', 180, 690, 'end'))
m.append(arrow('M280 635 H328', '10', 304, 628))
m.append(arrow('M380 658 C380 690, 440 690, 440 660', '11 frames', 452, 690, 'start'))
m.append(arrow('M480 635 H528', '12', 504, 628))
m.append(arrow('M650 635 H698', '13', 674, 628))
m.append(arrow('M810 635 H858', '14', 834, 628))
# inputs to Omega 00: the selected target and the provider plans
m.append(box(790, 450, 230, 40, 'target description · provider plans', 'proposed inputs · from Build and realization', kind='proposed'))
m.append(arrow('M1020 470 H1060 V498', 'read by 00', 1030, 464, 'start'))

# Foundation
m.append('<rect class="b-fdn" x="20" y="700" width="1050" height="52"/>')
m.append('<text class="lane" x="34" y="721">FOUNDATION · PRIMITIVES ONLY</text>')
m.append('<text class="t sub" x="34" y="740">arena · compiler-storage · diagnostics · source positions · wire · bignum · execution-admission — used by every stage, knows nothing about Omega</text>')

# Today's reach-arounds (toggle)
leak = []
leak.append(arrow('M730 72 V160', 'runs 02–04 itself', 722, 120, 'end', cls='leak'))
leak.append(arrow('M920 612 C920 586, 680 586, 680 548', 'image emission re-runs 02–03', 800, 598, 'middle', cls='leak'))
leak.append(arrow('M250 612 C250 590, 440 590, 440 548', 'emission reads 05–09 types', 345, 576, 'middle', cls='leak'))
leak.append('<rect class="b-leakbox" x="1000" y="612" width="180" height="46"/>')
leak.append('<text class="al-leak" x="1090" y="631" text-anchor="middle">backend today · 222k lines</text>')
leak.append('<text class="al-leak sub" x="1090" y="647" text-anchor="middle">every Omega stage reads it</text>')
leak.append(arrow('M1040 612 C1040 580, 900 580, 900 548', None, cls='leak'))
m.append('<g id="leaks" class="leaks">' + '\n'.join(leak) + '</g>')

fig1 = '<svg viewBox="0 0 1200 760" role="img" aria-label="Omega compiles through one linear pipeline: Psi stages 00 to 06 turn source files into Terminal Psi, the lowered-form checker gives the only verdict, and Omega stages turn Terminal Psi plus the selected target description into machine code, function fragments, a text section, an object and a native image. Foundation holds only primitives. Dashed red arrows mark the reach-arounds that remain today." class="map">' + '\n'.join(m) + '</svg>'

# ---------------- Figure 2: the foundation side channel ----------------
def panel(today):
    p = [defs.replace('id="ah"', 'id="ah2"' if today else 'id="ah3"').replace('id="ah-leak"', 'id="ahl2"' if today else 'id="ahl3"').replace('id="ah-ok"', 'id="ahok2"' if today else 'id="ahok3"')]
    a = 'ah2' if today else 'ah3'
    al = 'ahl2' if today else 'ahl3'

    def ar(d, label=None, lx=0, ly=0, anchor='middle', leak=False):
        s = [f'<path class="{"leak" if leak else "a"}" d="{d}" marker-end="url(#{al if leak else a})"/>']
        if label:
            s.append(f'<text class="{"al-leak" if leak else "al"}" x="{lx}" y="{ly}" text-anchor="{anchor}">{E(label)}</text>')
        return '\n'.join(s)
    p.append(f'<text class="lane" x="10" y="18">{"TODAY" if today else "TARGET"}</text>')
    p.append(box(10, 40, 165, 50, 'symbol-resolved trees', kind='psi'))
    p.append(box(207, 40, 165, 50, 'typed trees', kind='psi'))
    p.append(box(404, 40, 165, 50, 'Terminal Psi', kind='hub'))
    p.append(ar('M175 65 H205', '03', 191, 58))
    p.append(ar('M372 65 H402', '05', 388, 58))
    if today:
        p.append('<rect class="b-fdn" x="10" y="170" width="559" height="46"/>')
        p.append('<text class="t lbl" x="289" y="198" text-anchor="middle">foundation · language-semantics :: declaration_selection</text>')
        p.append(ar('M85 90 V168', '02 writes', 93, 132, 'start', leak=True))
        p.append(ar('M486 168 V92', '05 reads', 478, 132, 'end', leak=True))
        p.append('<text class="al-leak" x="289" y="132" text-anchor="middle">skips typed trees: no stage carries it</text>')
    else:
        for x, w in ((10, 165), (207, 165), (404, 165)):
            p.append(f'<rect class="tag" x="{x + 6}" y="98" width="{w - 12}" height="18"/>')
            p.append(f'<text class="tagt" x="{x + w/2}" y="111" text-anchor="middle">declaration_selection</text>')
        p.append('<text class="al" x="289" y="146" text-anchor="middle">02 writes it into symbol-resolved trees; each stage carries it forward</text>')
        p.append('<rect class="b-fdn" x="10" y="170" width="559" height="46"/>')
        p.append('<text class="t lbl" x="289" y="198" text-anchor="middle">foundation · primitives only (no arrows in or out)</text>')
    return '<svg viewBox="0 0 580 226" role="img" aria-label="' + (
        "Today: stage 02 writes declaration_selection into a foundation crate and stage 05 reads it from there, skipping typed trees." if today else
        "Target: declaration_selection lives in symbol-resolved trees and is carried through typed trees to Terminal Psi; foundation has no arrows.") + '" class="pair">' + '\n'.join(p) + '</svg>'

fig2 = '<div class="pairs">' + panel(True) + panel(False) + '</div>'

# ---------------- Figure 3: foundation size ----------------
def bar(y, label, lines, note, scale, cls):
    w = max(lines * scale, 2)
    return (f'<text class="t lbl" x="0" y="{y + 18}">{E(label)}</text>'
            f'<rect class="{cls}" x="200" y="{y}" width="{w:.1f}" height="26"/>'
            f'<text class="t sub num" x="{200 + w + 10:.1f}" y="{y + 18}">{E(note)}</text>')
scale = 760 / 221793
rows = [
    (8, 'Backend today', 221793, '221,793 lines · 29 crates beside the Omega pipeline', 'bar-now'),
    (44, 'Backend target', 0, '0 · each part moves into a stage, an input or its owner', 'bar-target'),
    (96, 'Foundation today', 96223, '96,223 lines · 26 crates', 'bar-now'),
    (132, 'Foundation target', 14800, 'about 14,800 lines · 7 primitive crates', 'bar-target'),
]
fig3 = ('<svg viewBox="0 0 1200 170" role="img" aria-label="Code outside the pipeline: backend goes from 221,793 lines to none; foundation goes from 96,223 lines to about 14,800." class="bars">'
        + ''.join(bar(y, l, n, note, scale, c) for y, l, n, note, c in rows) + '</svg>')

page = open(os.path.join(HERE, 'omap_template.html')).read()
page = page.replace('%%FIG1%%', fig1).replace('%%FIG2%%', fig2).replace('%%FIG3%%', fig3)
import os
os.makedirs(os.path.dirname(OUT), exist_ok=True)
open(OUT, 'w').write(page)
print('wrote', OUT, len(page))
