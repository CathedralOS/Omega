"""Report letters whose outline falls apart into more pieces than the letter has.

A piece is an outer contour (counter-clockwise in these CFF fonts); holes such as the
inside of o wind the other way and are not counted. Most letters are one piece; i and j
are two (stem and dot), and a few symbols are several by design.
"""
import sys
from fontTools.pens.recordingPen import RecordingPen
from fontTools.ttLib import TTFont

EXPECTED = {'i': 2, 'j': 2, 'Theta': 2, 'Xi': 3}


def signed_area(points):
    return sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(points, points[1:] + points[:1])) / 2


def pieces(glyph_set, name):
    pen = RecordingPen()
    glyph_set[name].draw(pen)
    contours, current = [], []
    for op, args in pen.value:
        if op == 'moveTo':
            current = [args[0]]
        elif op in ('lineTo', 'curveTo', 'qCurveTo'):
            current.append(args[-1])
        elif op in ('closePath', 'endPath'):
            contours.append(current)
    return sum(1 for c in contours if len(c) > 2 and signed_area(c) > 0)


font = TTFont(sys.argv[1])
glyph_set = font.getGlyphSet()
cmap = font.getBestCmap()
bad = []
for code, name in sorted(cmap.items()):
    character = chr(code)
    if not character.isalpha() or name in bad:
        continue
    has_mark = name not in ('i', 'j') and any(name.endswith(m) for m in ('grave', 'acute', 'circumflex', 'tilde', 'dieresis', 'ring'))
    if has_mark:
        continue
    expected = EXPECTED.get(name, 1)
    found = pieces(glyph_set, name)
    if found != expected:
        bad.append(name)
        print(f'{name} ({character}): {found} pieces, expected {expected}')
print('checked', len(cmap), 'mapped characters;', len(bad), 'letters in the wrong number of pieces')
