"""Omega Serif: a display serif built from Omega's arch geometry.

Every glyph is assembled from a few shapes (stems, rings cut into arches,
stone-cut wedge serifs, beaks) whose sizes come from one weight setting, then
merged into clean outlines. Change a number in `Weight` and every letter
follows.
"""
import sys
import unicodedata
from types import SimpleNamespace

import pathops
from pathops import PathOp
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
import ttfautohint

UPM = 1000
CAP, XH, ASC, DESC = 700, 482, 740, -220
NUM = 680          # lining figure height
AXIS = 300         # operators and logic symbols center on this height, level with the figures' middle
OP_WIDTH = 440     # the width of + = < and the other operators, so they line up in tables
OV, OVX = 12, 10   # overshoot for round caps and lowercase
K = 0.5522847498   # cubic circle constant
BIG = 4000

F = SimpleNamespace()


def setup(weight):
    """Stroke sizes for one weight, before the final SCALE.

    Tuned for reading text, not just headlines: hairlines and serif tips stay at least
    about two thirds of a pixel thick at 16px, and contrast between stems and hairlines
    stays near 2:1 so letters hold together in a paragraph.
    """
    t = (weight - 300) / 200.0          # 0 at Light (300), 1 at Medium (500)
    F.S = 69 + t * 42                   # cap stem
    F.SL = 64 + t * 39                  # lowercase stem
    F.H = 35 + t * 16                   # hairline
    F.CROWN = F.H * 1.3                 # where an arch meets its stems (n, m, h, r, u)
    F.RS, F.RSL = F.S * 1.04, F.SL * 1.05   # round sides run slightly heavier
    F.HN = F.H * 1.5                    # thin stems and diagonals
    # Thick diagonals are measured horizontally; at their slant these widths come out
    # about as thick as a straight stem when measured across the stroke.
    F.TD, F.TDL = F.S * 1.06, F.SL * 1.07
    F.ARM, F.BAR = F.H * 1.1, F.H * 1.1     # arms of E F L T Z and crossbars, near the hairline
    F.SH, F.ST = 32 + t * 5, 17 + t * 3     # serif height at the stem, at the tip
    F.FL, F.TL = 56, 48                 # cap foot and head serif reach
    F.FLL, F.TLL = 46, 40               # lowercase
    F.BW, F.BH = F.S * 0.45, F.S * 0.9      # beak width and drop
    F.DOT = F.SL * 1.15
    F.OP = F.H * 1.5                    # every operator and logic symbol strokes at this weight
    F.WEIGHT = weight
    F.DOTW = F.DOT                      # dot width; widened while a lowercase letter is drawn


# Lowercase letters are drawn at full width, then narrowed by NARROW. While they are
# drawn, every horizontal measurement (stem and round-side thickness, diagonal width,
# serif reach, beak and dot width) is widened by the same factor, so the narrowing
# changes proportions but leaves stroke weight and serifs matching the capitals.
NARROW = 0.86
HORIZONTAL = ('SL', 'RSL', 'TDL', 'HN', 'FLL', 'TLL', 'BW', 'DOTW')

# Every glyph is drawn on a 700-unit cap height, then scaled up so the face sets as
# large as Claude's serif at the same font size (cap height 750).
SCALE = 1.07


def widen_for_narrowing():
    for attr in HORIZONTAL:
        setattr(F, attr, getattr(F, attr) / NARROW)


# ---- shape primitives ------------------------------------------------------

def new():
    return pathops.Path()


def poly(points):
    clean = []
    for x, y in points:
        point = (float(x), float(y))
        if not clean or clean[-1] != point:
            clean.append(point)
    if len(clean) > 1 and clean[-1] == clean[0]:
        clean.pop()
    path = new()
    pen = path.getPen()
    pen.moveTo(clean[0])
    for point in clean[1:]:
        pen.lineTo(point)
    pen.closePath()
    return path


def rect(x0, y0, x1, y1):
    return poly([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])


def ellipse(cx, cy, rx, ry):
    path = new()
    pen = path.getPen()
    kx, ky = rx * K, ry * K
    pen.moveTo((cx + rx, cy))
    pen.curveTo((cx + rx, cy + ky), (cx + kx, cy + ry), (cx, cy + ry))
    pen.curveTo((cx - kx, cy + ry), (cx - rx, cy + ky), (cx - rx, cy))
    pen.curveTo((cx - rx, cy - ky), (cx - kx, cy - ry), (cx, cy - ry))
    pen.curveTo((cx + kx, cy - ry), (cx + rx, cy - ky), (cx + rx, cy))
    pen.closePath()
    return path


def union(*paths):
    out = new()
    for path in paths:
        out = pathops.op(out, path, PathOp.UNION)
    return out


def minus(path, *cuts):
    for cut in cuts:
        path = pathops.op(path, cut, PathOp.DIFFERENCE)
    return path


def clip(path, window):
    return pathops.op(path, window, PathOp.INTERSECTION)


def above(y): return rect(-BIG, y, BIG, BIG)
def below(y): return rect(-BIG, -BIG, BIG, y)
def left_of(x): return rect(-BIG, -BIG, x, BIG)
def right_of(x): return rect(x, -BIG, BIG, BIG)


def xform(path, matrix):
    out = new()
    path.draw(TransformPen(out.getPen(), matrix))
    return out


def ring(cx, cy, rx, ry, side, top, bottom):
    """An ellipse with a counter: thick sides, thin top and bottom (vertical stress)."""
    return ring_sides(cx, cy, rx, ry, side, side, top, bottom)


def ring_sides(cx, cy, rx, ry, left, right, top, bottom):
    """A ring whose left and right sides can differ, so a bowl can thin where it meets a stem."""
    inner_top, inner_bottom = cy + ry - top, cy - ry + bottom
    inner = ellipse(cx + (left - right) / 2, (inner_top + inner_bottom) / 2, rx - (left + right) / 2,
                    (inner_top - inner_bottom) / 2)
    return minus(ellipse(cx, cy, rx, ry), inner)


def joined_bowl(x0, x1, y0, y1, stem_side, side, hair):
    """A bowl that meets a stem on one side: full weight away from the stem, a hairline into it."""
    join = hair * 1.25
    left, right = (side, join) if stem_side == 'right' else (join, side)
    return ring_sides((x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2, (y1 - y0) / 2, left, right, hair, hair)


def arch(x0, x1, spring, crown, side, top):
    """The top half of a ring spanning x0..x1, springing at y=spring: the system's arch."""
    cx, rx = (x0 + x1) / 2, (x1 - x0) / 2
    return clip(ring(cx, spring, rx, crown - spring, side, top, top), above(spring))


def cup(x0, x1, spring, floor, side, bottom, right_side=None):
    """The bottom half of a ring: an arch turned over. right_side thins the side that meets a stem."""
    cx, rx = (x0 + x1) / 2, (x1 - x0) / 2
    right = side if right_side is None else right_side
    return clip(ring_sides(cx, spring, rx, spring - floor, side, right, bottom, bottom), below(spring))


def line(start, end, width):
    """A straight stroke of even thickness, cut square at both ends."""
    (x0, y0), (x1, y1) = start, end
    length = ((x1 - x0) ** 2 + (y1 - y0) ** 2) ** 0.5
    nx, ny = -(y1 - y0) / length * width / 2, (x1 - x0) / length * width / 2
    return poly([(x0 + nx, y0 + ny), (x1 + nx, y1 + ny), (x1 - nx, y1 - ny), (x0 - nx, y0 - ny)])


def rotate(path, degrees, cx, cy):
    import math
    c, s = math.cos(math.radians(degrees)), math.sin(math.radians(degrees))
    return xform(path, (c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy))


def stroke(x0, y0, x1, y1, width):
    """A straight stroke cut flat at both ends, `width` measured horizontally."""
    h = width / 2
    return poly([(x0 - h, y0), (x0 + h, y0), (x1 + h, y1), (x1 - h, y1)])


def foot(xl, xr, y=0, left=None, right=None):
    """Stone-cut wedge serif on the baseline: flat underneath, sloping to a thin tip."""
    left = F.FL if left is None else left
    right = F.FL if right is None else right
    return poly([(xl - left, y), (xr + right, y), (xr + right, y + F.ST), (xr, y + F.SH),
                 (xl, y + F.SH), (xl - left, y + F.ST)])


def head(xl, xr, y, left=None, right=None):
    left = F.TL if left is None else left
    right = F.TL if right is None else right
    return poly([(xl - left, y), (xr + right, y), (xr + right, y - F.ST), (xr, y - F.SH),
                 (xl, y - F.SH), (xl - left, y - F.ST)])


def beak(x, y, direction, side='right'):
    """A vertical wedge at the end of an arm. x is the outer edge, y the arm's outer face."""
    w, h = F.BW, F.BH
    if side == 'right':
        return poly([(x - w, y), (x, y), (x, y + direction * h), (x - w * 0.35, y + direction * h)])
    return poly([(x, y), (x + w, y), (x + w * 0.35, y + direction * h), (x, y + direction * h)])


def dot(x, y):
    return rect(x, y, x + F.DOTW, y + F.DOT)


# ---- glyph registry ----------------------------------------------------------

GLYPHS = {}   # name -> (unicodes, build, sidebearings, fixed advance)


def glyph(name, unicodes=(), sb=(24, 24), advance=None):
    def register(build):
        GLYPHS[name] = (tuple(unicodes), build, sb, advance)
        return build
    return register


def alias(name, unicodes, of):
    unicodes_old, build, sb, advance = GLYPHS[of]
    GLYPHS[of] = (unicodes_old + tuple(unicodes), build, sb, advance)


ROUND = (28, 28)
DIAG = (8, 8)

# ---- shared constructions (caps and lowercase use the same ones) -------------


def v_shape(height, width, thick, thin, reach):
    apex = width / 2
    shape = union(
        poly([(0, height), (thick, height), (apex + thick / 2, 0), (apex - thick / 2, 0)]),
        poly([(apex + thick / 2 - thin, 0), (apex + thick / 2, 0), (width, height), (width - thin, height)]),
        head(0, thick, height, reach, reach),
        head(width - thin, width, height, reach, reach))
    return shape


def a_shape(height, width, thick, thin, reach, crossbar=True):
    apex = width / 2
    top_l, top_r = apex - thick * 0.45, apex + thick * 0.45
    left = poly([(0, 0), (thin, 0), (top_l + thin, height), (top_l, height)])
    right = poly([(top_r - thick, height), (top_r, height), (width, 0), (width - thick, 0)])
    parts = [left, right, foot(0, thin, 0, reach, reach), foot(width - thick, width, 0, reach, reach)]
    if crossbar:
        hull = poly([(0, 0), (width, 0), (top_r, height), (top_l, height)])
        parts.append(clip(rect(0, height * 0.28, width, height * 0.28 + F.BAR), hull))
    return union(*parts)


def x_shape(height, width, thick, thin, foot_reach, head_reach):
    return union(
        poly([(0, height), (thick, height), (width, 0), (width - thick, 0)]),
        poly([(width - thin, height), (width, height), (thin, 0), (0, 0)]),
        head(0, thick, height, head_reach, head_reach),
        head(width - thin, width, height, head_reach, head_reach),
        foot(0, thin, 0, foot_reach, foot_reach),
        foot(width - thick, width, 0, foot_reach, foot_reach))


def z_shape(height, width, thick, arm):
    return union(
        rect(16, height - arm, width, height), beak(16, height, -1, 'left'),
        poly([(width - thick, height - arm), (width, height - arm), (thick, arm), (0, arm)]),
        rect(0, 0, width + 14, arm), beak(width + 14, 0, 1))


def s_shape(height, width, side, hair, spine, ov):
    """Two stacked rings that share one thick horizontal spine."""
    cx = width / 2
    mid = height / 2
    up_bottom, low_top = mid - spine / 2, mid + spine / 2
    up_top, low_bottom = height + ov, -ov
    upper = ring(cx, (up_top + up_bottom) / 2, cx - width * 0.04, (up_top - up_bottom) / 2, side, hair, spine)
    lower = ring(cx, (low_top + low_bottom) / 2, cx, (low_top - low_bottom) / 2, side, spine, hair)
    up_cy, up_ry = (up_top + up_bottom) / 2, (up_top - up_bottom) / 2
    low_cy, low_ry = (low_top + low_bottom) / 2, (low_top - low_bottom) / 2
    upper = minus(upper, rect(cx, -BIG, BIG, up_cy + up_ry * 0.12))
    lower = minus(lower, rect(-BIG, low_cy - low_ry * 0.12, cx, BIG))
    return union(upper, lower)


def bowl(x0, x1, height, side, hair, ov):
    return ring((x0 + x1) / 2, height / 2, (x1 - x0) / 2, height / 2 + ov, side, hair, hair)


# ---- capitals ------------------------------------------------------------------

@glyph('I', [0x49, 0x399])
def cap_I():
    return union(rect(0, 0, F.S, CAP), foot(0, F.S), head(0, F.S, CAP))


@glyph('H', [0x48, 0x397])
def cap_H():
    w = 600
    return union(rect(0, 0, F.S, CAP), rect(w - F.S, 0, w, CAP),
                 rect(F.S, CAP * 0.5 - F.BAR / 2, w - F.S, CAP * 0.5 + F.BAR / 2),
                 foot(0, F.S), head(0, F.S, CAP), foot(w - F.S, w), head(w - F.S, w, CAP))


@glyph('E', [0x45, 0x395])
def cap_E():
    return union(rect(0, 0, F.S, CAP),
                 rect(0, CAP - F.ARM, 470, CAP), beak(470, CAP, -1),
                 rect(0, CAP * 0.5 - F.BAR / 2, 400, CAP * 0.5 + F.BAR / 2),
                 rect(0, 0, 500, F.ARM), beak(500, 0, 1),
                 foot(0, F.S, 0, F.FL, 0), head(0, F.S, CAP, F.TL, 0))


@glyph('F', [0x46])
def cap_F():
    return union(rect(0, 0, F.S, CAP),
                 rect(0, CAP - F.ARM, 470, CAP), beak(470, CAP, -1),
                 rect(0, CAP * 0.48 - F.BAR / 2, 390, CAP * 0.48 + F.BAR / 2),
                 foot(0, F.S), head(0, F.S, CAP, F.TL, 0))


@glyph('L', [0x4C])
def cap_L():
    return union(rect(0, 0, F.S, CAP), rect(0, 0, 490, F.ARM), beak(490, 0, 1),
                 foot(0, F.S, 0, F.FL, 0), head(0, F.S, CAP))


@glyph('T', [0x54, 0x3A4])
def cap_T():
    w, c = 600, 300
    return union(rect(0, CAP - F.ARM, w, CAP), beak(w, CAP, -1), beak(0, CAP, -1, 'left'),
                 rect(c - F.S / 2, 0, c + F.S / 2, CAP), foot(c - F.S / 2, c + F.S / 2))


@glyph('O', [0x4F, 0x39F], sb=ROUND)
def cap_O():
    return bowl(0, 690, CAP, F.RS, F.H, OV)


@glyph('Q', [0x51], sb=ROUND)
def cap_Q():
    cx = 345
    tail = poly([(cx - 40, 40), (cx + 40, 60), (cx + 250, -180), (cx + 170, -180)])
    return union(bowl(0, 690, CAP, F.RS, F.H, OV), tail)


@glyph('C', [0x43], sb=ROUND)
def cap_C():
    cx, rx, cy, ry = 320, 320, CAP / 2, CAP / 2 + OV
    return minus(ring(cx, cy, rx, ry, F.RS, F.H, F.H), aperture(cx, cy, rx, ry, 0.2, 0.4, 0.42))


def aperture(cx, cy, rx, ry, inset, low, high):
    """The opening of c, C and similar letters, cut on a slant so each end tapers like a chisel
    stroke instead of ending in a flat block of the thick side."""
    x_in, x_out = cx + rx * inset, cx + rx + 40
    # The lower cut tilts the other way: following the curve's slant there leaves a spike.
    return poly([(x_in, cy - ry * low), (x_out, cy - ry * (low - 0.08)),
                 (x_out, cy + ry * (high + 0.2)), (x_in, cy + ry * high)])


@glyph('G', [0x47], sb=ROUND)
def cap_G():
    cx, rx, cy, ry = 330, 330, CAP / 2, CAP / 2 + OV
    spur_top, spur_bottom = cy - ry * 0.06, CAP * 0.2
    # The straight spur replaces the ring's curving lower-right side instead of sitting on top of it.
    shape = minus(ring(cx, cy, rx, ry, F.RS, F.H, F.H), rect(cx + rx * 0.2, spur_top, BIG, cy + ry * 0.42),
                  rect(cx + rx * 0.3, spur_bottom, BIG, spur_top))
    return union(shape, rect(cx + rx * 0.05, spur_top - F.BAR, cx + rx - F.RS * 0.2, spur_top),
                 rect(cx + rx - F.RS, spur_bottom - 1, cx + rx, spur_top))


@glyph('D', [0x44])
def cap_D():
    cx, rx = 260, 390
    shape = clip(ring(cx, CAP / 2, rx, CAP / 2, F.RS, F.H, F.H), right_of(cx))
    return union(shape, rect(0, 0, F.S, CAP), rect(0, CAP - F.H, cx + 1, CAP), rect(0, 0, cx + 1, F.H),
                 foot(0, F.S, 0, F.FL, 0), head(0, F.S, CAP, F.TL, 0))


def side_bowl(x_join, x_right, y0, y1, side):
    cx = x_join
    shape = clip(ring(cx, (y0 + y1) / 2, x_right - cx, (y1 - y0) / 2, side, F.H, F.H), right_of(cx))
    return union(shape, rect(0, y1 - F.H, cx + 1, y1), rect(0, y0, cx + 1, y0 + F.H))


@glyph('B', [0x42, 0x392])
def cap_B():
    mid = CAP * 0.52
    return union(rect(0, 0, F.S, CAP),
                 side_bowl(240, 520, mid - F.H, CAP, F.RS * 0.95),
                 side_bowl(250, 570, 0, mid, F.RS),
                 foot(0, F.S, 0, F.FL, 0), head(0, F.S, CAP, F.TL, 0))


@glyph('P', [0x50, 0x3A1])
def cap_P():
    return union(rect(0, 0, F.S, CAP), side_bowl(250, 560, CAP * 0.42, CAP, F.RS),
                 foot(0, F.S), head(0, F.S, CAP, F.TL, 0))


@glyph('R', [0x52])
def cap_R():
    bowl_bottom = CAP * 0.44
    leg_x = 500
    return union(rect(0, 0, F.S, CAP), side_bowl(250, 560, bowl_bottom, CAP, F.RS),
                 poly([(270, bowl_bottom + F.H), (270 + F.TD, bowl_bottom + F.H), (leg_x + F.TD / 2, 0), (leg_x - F.TD / 2, 0)]),
                 foot(0, F.S), head(0, F.S, CAP, F.TL, 0),
                 foot(leg_x - F.TD / 2, leg_x + F.TD / 2, 0, 6, F.FL))


@glyph('U', [0x55])
def cap_U():
    w, spring = 620, 260
    return union(rect(0, spring, F.S, CAP), rect(w - F.S, spring, w, CAP),
                 cup(0, w, spring, -OV, F.S, F.H),
                 head(0, F.S, CAP), head(w - F.S, w, CAP))


@glyph('J', [0x4A])
def cap_J():
    w, spring = 360, 220
    hook = cup(0, w, spring, -OV, F.S, F.H)
    hook = minus(hook, rect(-BIG, spring * 0.45, w / 2, BIG))
    return union(rect(w - F.S, spring, w, CAP), hook, head(w - F.S, w, CAP))


@glyph('N', [0x4E, 0x39D])
def cap_N():
    w = 640
    return union(rect(0, 0, F.HN, CAP), rect(w - F.HN, 0, w, CAP),
                 poly([(0, CAP), (F.TD, CAP), (w, 0), (w - F.TD, 0)]),
                 foot(0, F.HN), head(0, F.HN, CAP, F.TL, 0), head(w - F.HN, w, CAP))


@glyph('M', [0x4D, 0x39C])
def cap_M():
    w = 780
    m = (F.HN + w - F.S) / 2
    return union(rect(0, 0, F.HN, CAP), rect(w - F.S, 0, w, CAP),
                 poly([(0, CAP), (F.TD, CAP), (m + F.TD / 2, 0), (m - F.TD / 2, 0)]),
                 poly([(m + F.TD / 2 - F.HN, 0), (m + F.TD / 2, 0), (w - F.S + F.HN * 0.5, CAP), (w - F.S - F.HN * 0.5, CAP)]),
                 foot(0, F.HN), foot(w - F.S, w), head(0, F.HN, CAP, F.TL, 0), head(w - F.S, w, CAP, 0, F.TL))


@glyph('V', [0x56], sb=DIAG)
def cap_V():
    return v_shape(CAP, 650, F.TD, F.HN, F.TL)


@glyph('A', [0x41, 0x391], sb=DIAG)
def cap_A():
    return a_shape(CAP, 670, F.TD, F.HN, F.FL)


@glyph('W', [0x57], sb=DIAG)
def cap_W():
    w, thick, thin = 930, F.TD, F.HN
    a1, m, a2 = w * 0.27, w * 0.5, w * 0.73
    return union(
        poly([(0, CAP), (thick, CAP), (a1 + thick / 2, 0), (a1 - thick / 2, 0)]),
        poly([(a1 + thick / 2 - thin, 0), (a1 + thick / 2, 0), (m - thick / 2 + thin, CAP), (m - thick / 2, CAP)]),
        poly([(m - thick / 2, CAP), (m + thick / 2, CAP), (a2 + thick / 2, 0), (a2 - thick / 2, 0)]),
        poly([(a2 + thick / 2 - thin, 0), (a2 + thick / 2, 0), (w, CAP), (w - thin, CAP)]),
        head(0, thick, CAP), head(m - thick / 2, m + thick / 2, CAP), head(w - thin, w, CAP))


@glyph('X', [0x58, 0x3A7], sb=DIAG)
def cap_X():
    return x_shape(CAP, 640, F.TD, F.HN, F.FL, F.TL)


@glyph('Y', [0x59, 0x3A5], sb=DIAG)
def cap_Y():
    w, join = 640, CAP * 0.42
    c = w / 2
    return union(
        poly([(0, CAP), (F.TD, CAP), (c + F.S / 2, join), (c - F.S / 2, join)]),
        poly([(c + F.S / 2 - F.HN, join), (c + F.S / 2, join), (w, CAP), (w - F.HN, CAP)]),
        rect(c - F.S / 2, 0, c + F.S / 2, join + 1),
        head(0, F.TD, CAP), head(w - F.HN, w, CAP), foot(c - F.S / 2, c + F.S / 2))


def k_branch(stem, height, width, arm_width, leg_width):
    """The arm and leg of K, k and κ, joined to the stem and to each other.

    The arm starts inside the stem below the join, so it grows out of the stem rather
    than touching it at a point. The leg branches off the arm's lower edge, and its top
    is trimmed along the arm's right edge so the two meet without a step.
    """
    import math
    join = height * 0.36
    base_y = join - 40
    a0, a1 = (stem - 30, base_y), (width - arm_width, height)
    arm = poly([a0, (a0[0] + arm_width, base_y), (width, height), a1])
    leg_top = join + (height - join) * 0.22
    t = (leg_top - base_y) / (height - base_y)
    xa = a0[0] + t * (a1[0] - a0[0])
    foot_right = width + 10
    leg = poly([(xa, leg_top), (xa + leg_width, leg_top), (foot_right, 0), (foot_right - leg_width, 0)])
    p1, p2 = (a0[0] + arm_width, base_y), (width, height)
    dx, dy = p2[0] - p1[0], p2[1] - p1[1]
    length = math.hypot(dx, dy)
    dx, dy = dx / length, dy / length
    nx, ny, far = -dy, dx, 3000
    above_arm = poly([(p1[0] - far * dx, p1[1] - far * dy), (p2[0] + far * dx, p2[1] + far * dy),
                      (p2[0] + far * dx + far * nx, p2[1] + far * dy + far * ny),
                      (p1[0] - far * dx + far * nx, p1[1] - far * dy + far * ny)])
    return union(arm, minus(leg, above_arm)), (foot_right - leg_width, foot_right)


@glyph('K', [0x4B, 0x39A])
def cap_K():
    w = 620
    branch, (leg0, leg1) = k_branch(F.S, CAP, w, F.HN * 1.1, F.TD)
    return union(rect(0, 0, F.S, CAP), branch, foot(0, F.S), head(0, F.S, CAP),
                 head(w - F.HN * 1.1, w, CAP, F.TL * 0.8, F.TL * 0.8), foot(leg0, leg1, 0, 6, F.FL))


@glyph('Z', [0x5A, 0x396])
def cap_Z():
    return z_shape(CAP, 560, F.TD, F.ARM)


@glyph('S', [0x53], sb=ROUND)
def cap_S():
    return s_shape(CAP, 540, F.RS * 0.95, F.H, F.S * 0.82, OV)


# ---- Greek capitals that are not Latin shapes -----------------------------------

@glyph('Gamma', [0x393])
def cap_Gamma():
    return union(rect(0, 0, F.S, CAP), rect(0, CAP - F.ARM, 470, CAP), beak(470, CAP, -1),
                 foot(0, F.S), head(0, F.S, CAP, F.TL, 0))


@glyph('Delta', [0x394], sb=DIAG)
def cap_Delta():
    w = 680
    apex = w / 2
    top_l, top_r = apex - F.TD * 0.45, apex + F.TD * 0.45
    return union(poly([(0, 0), (F.HN, 0), (top_l + F.HN, CAP), (top_l, CAP)]),
                 poly([(top_r - F.TD, CAP), (top_r, CAP), (w, 0), (w - F.TD, 0)]),
                 rect(0, 0, w, F.ARM))


@glyph('Theta', [0x398], sb=ROUND)
def cap_Theta():
    cx = 345
    return union(bowl(0, 690, CAP, F.RS, F.H, OV),
                 rect(cx - 150, CAP / 2 - F.BAR / 2, cx + 150, CAP / 2 + F.BAR / 2))


@glyph('Lambda', [0x39B], sb=DIAG)
def cap_Lambda():
    return a_shape(CAP, 670, F.TD, F.HN, F.FL, crossbar=False)


@glyph('Xi', [0x39E])
def cap_Xi():
    w = 580
    return union(rect(0, CAP - F.ARM, w, CAP), beak(w, CAP, -1), beak(0, CAP, -1, 'left'),
                 rect(70, CAP / 2 - F.BAR / 2, w - 70, CAP / 2 + F.BAR / 2),
                 rect(0, 0, w, F.ARM), beak(w, 0, 1), beak(0, 0, 1, 'left'))


@glyph('Pi', [0x3A0])
def cap_Pi():
    w = 620
    return union(rect(0, 0, F.S, CAP), rect(w - F.S, 0, w, CAP), rect(0, CAP - F.ARM * 1.15, w, CAP),
                 foot(0, F.S), foot(w - F.S, w))


@glyph('Sigma', [0x3A3])
def cap_Sigma():
    w, mid = 580, CAP / 2
    point = w * 0.5
    return union(rect(0, CAP - F.ARM, w, CAP), beak(w, CAP, -1),
                 poly([(0, CAP - F.ARM), (F.TD, CAP - F.ARM), (point + F.TD / 2, mid), (point - F.TD / 2, mid)]),
                 poly([(point - F.TD / 2, mid), (point + F.TD / 2 - F.HN * 0.6, mid + 1), (F.HN * 1.4, F.ARM), (0, F.ARM)]),
                 rect(0, 0, w + 10, F.ARM), beak(w + 10, 0, 1))


@glyph('Phi', [0x3A6], sb=ROUND)
def cap_Phi():
    c = 330
    return union(rect(c - F.S / 2, 0, c + F.S / 2, CAP),
                 ring(c, CAP / 2, 330, CAP * 0.29, F.RS, F.H, F.H),
                 foot(c - F.S / 2, c + F.S / 2), head(c - F.S / 2, c + F.S / 2, CAP))


@glyph('Psi', [0x3A8])
def cap_Psi():
    c, rx, spring, floor = 330, 330, CAP * 0.66, CAP * 0.28
    return union(rect(c - F.S / 2, 0, c + F.S / 2, CAP),
                 cup(c - rx, c + rx, spring, floor, F.S, F.H),
                 rect(c - rx, spring - 1, c - rx + F.S, CAP), rect(c + rx - F.S, spring - 1, c + rx, CAP),
                 head(c - rx, c - rx + F.S, CAP), head(c + rx - F.S, c + rx, CAP),
                 head(c - F.S / 2, c + F.S / 2, CAP), foot(c - F.S / 2, c + F.S / 2))


@glyph('Omega', [0x3A9, 0x2126])
def cap_Omega():
    """The arch on two inward-curving legs, with feet reaching outward."""
    cx, cy, rx = 370, 405, 320
    ry = CAP + OV - cy
    cut = 160
    bowl_part = clip(ring(cx, cy, rx, ry, F.RS, F.H, F.H), above(cut))
    # Where the cut meets the ring: outer and inner x offsets of each leg.
    inner_top, inner_bottom = cy + ry - F.H, cy - ry + F.H
    icy, iry, irx = (inner_top + inner_bottom) / 2, (inner_top - inner_bottom) / 2, rx - F.RS
    dy = cut - cy
    outer = rx * (1 - (dy / ry) ** 2) ** 0.5
    inner = irx * (1 - ((cut - icy) / iry) ** 2) ** 0.5
    legs = [rect(cx - outer, 0, cx - inner, cut + 1), rect(cx + inner, 0, cx + outer, cut + 1)]
    feet = [foot(cx - outer, cx - inner, 0, 120, 0), foot(cx + inner, cx + outer, 0, 0, 120)]
    return union(bowl_part, *legs, *feet)


# ---- lowercase -------------------------------------------------------------------

S_ = lambda: F.SL


@glyph('l', [0x6C])
def low_l():
    return union(rect(0, 0, F.SL, ASC), head(0, F.SL, ASC, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL))


@glyph('i', [0x69])
def low_i():
    return union(rect(0, 0, F.SL, XH), head(0, F.SL, XH, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL),
                 dot((F.SL - F.DOTW) / 2, XH + 90))


@glyph('j', [0x6A], sb=(-24, 28))
def low_j():
    spring, r = -30, 115
    hook = cup(F.SL - 2 * r, F.SL, spring, DESC - OVX, F.SL, F.H)
    # End the hook square, about one stroke tall, rather than letting the cut taper it to a point.
    hook = minus(hook, rect(-BIG, spring - (spring - DESC) * 0.35, F.SL - r, BIG),
                 rect(-BIG, -BIG, F.SL - r - r * 0.72, BIG))
    return union(rect(0, spring, F.SL, XH), hook, head(0, F.SL, XH, F.TLL, 0),
                 dot((F.SL - F.DOTW) / 2, XH + 90))


def n_shape(width, stem_top):
    spring = XH * 0.42
    return union(rect(0, 0, F.SL, stem_top), arch(0, width, spring, XH + OVX, F.SL, F.CROWN),
                 rect(width - F.SL, 0, width, spring + 1),
                 head(0, F.SL, stem_top, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL),
                 foot(width - F.SL, width, 0, F.FLL, F.FLL))


@glyph('n', [0x6E])
def low_n():
    return n_shape(470, XH)


@glyph('h', [0x68])
def low_h():
    return n_shape(470, ASC)


@glyph('m', [0x6D])
def low_m():
    w1 = 400
    spring = XH * 0.42
    x2 = w1 - F.SL
    return union(rect(0, 0, F.SL, XH), arch(0, w1, spring, XH + OVX, F.SL, F.CROWN),
                 rect(x2, 0, w1, spring + 1), arch(x2, x2 + w1, spring, XH + OVX, F.SL, F.CROWN),
                 rect(x2 + w1 - F.SL, 0, x2 + w1, spring + 1),
                 head(0, F.SL, XH, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL),
                 foot(x2, w1, 0, F.FLL * 0.8, F.FLL * 0.8), foot(x2 + w1 - F.SL, x2 + w1, 0, F.FLL, F.FLL))


@glyph('u', [0x75])
def low_u():
    w, spring = 470, XH * 0.58
    return union(rect(0, spring, F.SL, XH), cup(0, w, spring, -OVX, F.SL, F.CROWN, right_side=F.H * 1.25),
                 rect(w - F.SL, 0, w, XH),
                 head(0, F.SL, XH, F.TLL, 0), head(w - F.SL, w, XH, F.TLL, 0), foot(w - F.SL, w, 0, 0, F.FLL))


@glyph('r', [0x72])
def low_r():
    w, spring = 380, XH * 0.42
    shoulder = clip(arch(0, w, spring, XH + OVX, F.SL * 0.9, F.CROWN), rect(-BIG, XH * 0.6, w * 0.86, BIG))
    return union(rect(0, 0, F.SL, XH), shoulder, head(0, F.SL, XH, F.TLL, 0),
                 foot(0, F.SL, 0, F.FLL, F.FLL * 1.4))


@glyph('o', [0x6F], sb=ROUND)
def low_o():
    return bowl(0, 510, XH, F.RSL, F.H, OVX)


@glyph('c', [0x63], sb=ROUND)
def low_c():
    cx, rx, cy, ry = 235, 235, XH / 2, XH / 2 + OVX
    return minus(ring(cx, cy, rx, ry, F.RSL, F.H, F.H), aperture(cx, cy, rx, ry, 0.15, 0.36, 0.4))


@glyph('e', [0x65], sb=ROUND)
def low_e():
    cx, rx, cy, ry = 245, 245, XH / 2, XH / 2 + OVX
    shape = ring(cx, cy, rx, ry, F.RSL, F.H, F.H)
    bar_y = XH * 0.5
    shape = union(shape, clip(rect(-BIG, bar_y, BIG, bar_y + F.BAR), ellipse(cx, cy, rx - 2, ry)))
    return minus(shape, rect(cx + rx * 0.1, cy - ry * 0.4, BIG, bar_y))


@glyph('a', [0x61])
def low_a():
    w = 470
    stem_top = XH * 0.6
    hook = arch(10, w, stem_top, XH + OVX, F.SL, F.H)
    hook = minus(hook, rect(-BIG, -BIG, (10 + w) / 2, stem_top + (XH - stem_top) * 0.42))
    lower = joined_bowl(0, w - F.SL * 0.35, -OVX, XH * 0.56 + OVX, 'right', F.SL, F.H)
    return union(rect(w - F.SL, 0, w, stem_top + 1), hook, lower, foot(w - F.SL, w, 0, 0, F.FLL))


def bowl_with_stem(stem_side, stem_bottom, stem_top):
    w = 540
    if stem_side == 'right':
        lower = joined_bowl(0, w - F.SL * 0.35, -OVX, XH + OVX, 'right', F.RSL, F.H)
        return w, union(lower, rect(w - F.SL, stem_bottom, w, stem_top))
    lower = joined_bowl(F.SL * 0.35, w, -OVX, XH + OVX, 'left', F.RSL, F.H)
    return w, union(lower, rect(0, stem_bottom, F.SL, stem_top))


@glyph('d', [0x64])
def low_d():
    w, shape = bowl_with_stem('right', 0, ASC)
    return union(shape, head(w - F.SL, w, ASC, F.TLL, 0), foot(w - F.SL, w, 0, 0, F.FLL))


@glyph('b', [0x62])
def low_b():
    w, shape = bowl_with_stem('left', 0, ASC)
    return union(shape, head(0, F.SL, ASC, F.TLL, 0), foot(0, F.SL, 0, F.FLL, 0))


@glyph('p', [0x70])
def low_p():
    w, shape = bowl_with_stem('left', DESC, XH)
    return union(shape, head(0, F.SL, XH, F.TLL, 0), foot(0, F.SL, DESC, F.FLL, F.FLL))


@glyph('q', [0x71])
def low_q():
    w, shape = bowl_with_stem('right', DESC, XH)
    return union(shape, head(w - F.SL, w, XH, 0, F.TLL * 0.7), foot(w - F.SL, w, DESC, F.FLL, F.FLL))


@glyph('g', [0x67])
def low_g():
    w = 540
    spring = -20
    lower = joined_bowl(0, w - F.SL * 0.35, -OVX, XH + OVX, 'right', F.RSL, F.H)
    hook = cup(10, w, spring, DESC - OVX, F.SL, F.H)
    hook = minus(hook, rect(-BIG, spring - (spring - DESC) * 0.5, (10 + w) / 2, BIG))
    return union(lower, rect(w - F.SL, spring, w, XH), hook, head(w - F.SL, w, XH, 0, F.TLL * 0.7))


@glyph('f', [0x66], sb=(28, -6))
def low_f():
    r = 125
    spring = ASC - 160
    hook = arch(0, 2 * r, spring, ASC + OVX, F.SL * 0.95, F.H)
    hook = minus(hook, rect(r, -BIG, BIG, spring + (ASC - spring) * 0.38))
    return union(rect(0, 0, F.SL, spring + 1), hook, rect(-60, XH - F.BAR, 200, XH),
                 foot(0, F.SL, 0, F.FLL, F.FLL * 1.4))


@glyph('t', [0x74])
def low_t():
    r, spring, top = 140, 150, XH + 120
    hook = cup(0, 2 * r, spring, -OVX, F.SL, F.H)
    hook = minus(hook, rect(r, spring * 0.5, BIG, BIG))
    stem = minus(rect(0, spring - 1, F.SL, top), poly([(-1, top - 46), (F.SL + 1, top + 1), (-1, top + 1)]))
    return union(stem, hook, rect(-70, XH - F.BAR, 190, XH))


@glyph('k', [0x6B])
def low_k():
    w = 480
    branch, (leg0, leg1) = k_branch(F.SL, XH, w, F.HN * 1.1, F.TDL)
    return union(rect(0, 0, F.SL, ASC), branch, head(0, F.SL, ASC, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL),
                 head(w - F.HN * 1.1, w, XH, F.TLL * 0.8, F.TLL * 0.8), foot(leg0, leg1, 0, 4, F.FLL))


@glyph('v', [0x76], sb=DIAG)
def low_v():
    return v_shape(XH, 500, F.TDL, F.HN * 0.95, F.TLL)


@glyph('w', [0x77], sb=DIAG)
def low_w():
    w, thick, thin = 740, F.TDL, F.HN * 0.95
    a1, m, a2 = w * 0.27, w * 0.5, w * 0.73
    return union(
        poly([(0, XH), (thick, XH), (a1 + thick / 2, 0), (a1 - thick / 2, 0)]),
        poly([(a1 + thick / 2 - thin, 0), (a1 + thick / 2, 0), (m - thick / 2 + thin, XH), (m - thick / 2, XH)]),
        poly([(m - thick / 2, XH), (m + thick / 2, XH), (a2 + thick / 2, 0), (a2 - thick / 2, 0)]),
        poly([(a2 + thick / 2 - thin, 0), (a2 + thick / 2, 0), (w, XH), (w - thin, XH)]),
        head(0, thick, XH, F.TLL, F.TLL), head(m - thick / 2, m + thick / 2, XH, F.TLL, F.TLL),
        head(w - thin, w, XH, F.TLL, F.TLL))


@glyph('x', [0x78], sb=DIAG)
def low_x():
    return x_shape(XH, 500, F.TDL, F.HN * 0.95, F.FLL, F.TLL)


@glyph('y', [0x79], sb=DIAG)
def low_y():
    w, thick, thin = 500, F.TDL, F.HN * 0.95
    apex = w / 2
    slope = (w - (apex + thick / 2)) / XH   # x per unit y of the thin stroke
    # The thin stroke runs past the baseline, then curls left in a small hook instead of
    # ending in a flat foot.
    turn_y = DESC * 0.3
    turn_x = apex + thick / 2 + turn_y * slope
    r = 120
    # Same ending as j: curve left along the descender, then stop square, about one stroke tall.
    hook = cup(turn_x - 2 * r, turn_x, turn_y, DESC - OVX, thin, F.H)
    hook = minus(hook, rect(-BIG, turn_y - (turn_y - DESC) * 0.35, turn_x - r, BIG),
                 rect(-BIG, -BIG, turn_x - r - r * 0.72, BIG))
    return union(
        poly([(0, XH), (thick, XH), (apex + thick / 2, 0), (apex - thick / 2, 0)]),
        poly([(turn_x - thin, turn_y), (turn_x, turn_y), (w, XH), (w - thin, XH)]),
        hook, head(0, thick, XH, F.TLL, F.TLL), head(w - thin, w, XH, F.TLL, F.TLL))


@glyph('z', [0x7A])
def low_z():
    return z_shape(XH, 430, F.TDL, F.ARM * 0.95)


@glyph('s', [0x73], sb=ROUND)
def low_s():
    return s_shape(XH, 420, F.RSL * 0.92, F.H, F.SL * 0.8, OVX)


# ---- figures (tabular: one advance) ------------------------------------------------

FIG = 555


def fig(name, code):
    return glyph(name, [code], advance=FIG)


@fig('zero', 0x30)
def fig_0():
    return bowl(0, 460, NUM, F.RS * 0.95, F.H, OV)


@fig('one', 0x31)
def fig_1():
    c = 0
    flag = poly([(c - F.S / 2, NUM - F.H * 2.4), (c - F.S / 2 - 130, NUM - 160), (c - F.S / 2 - 130, NUM - 160 + F.H * 1.5), (c + F.S / 2, NUM)])
    return union(rect(c - F.S / 2, 0, c + F.S / 2, NUM), flag, foot(c - F.S / 2, c + F.S / 2, 0, F.FL * 1.7, F.FL * 1.7))


@fig('two', 0x32)
def fig_2():
    rx, spring = 220, NUM * 0.66
    top = arch(0, 2 * rx, spring, NUM + OV, F.RS * 0.95, F.H)
    top = minus(top, rect(-BIG, -BIG, rx, spring + (NUM - spring) * 0.4))
    side = F.RS * 0.95
    diagonal = poly([(2 * rx - side, spring + 1), (2 * rx, spring + 1), (side * 0.9, F.ARM), (0, F.ARM)])
    return union(top, diagonal, rect(0, 0, 2 * rx + 10, F.ARM * 1.1), beak(2 * rx + 10, 0, 1))


@fig('three', 0x33)
def fig_3():
    waist = NUM * 0.47
    cx = 210
    up_cy, up_ry = (NUM + OV + waist) / 2, (NUM + OV - waist) / 2
    lo_top = waist + F.H
    lo_cy, lo_ry = (lo_top - OV) / 2, (lo_top + OV) / 2
    upper = minus(ring(cx, up_cy, 190, up_ry, F.RS * 0.92, F.H, F.H), rect(-BIG, -BIG, cx, up_cy + up_ry * 0.3))
    lower = minus(ring(cx + 10, lo_cy, 220, lo_ry, F.RS * 0.95, F.H, F.H), rect(-BIG, lo_cy - lo_ry * 0.3, cx + 10, BIG))
    return union(upper, lower, rect(cx - 70, waist, cx + 12, waist + F.H))


@fig('four', 0x34)
def fig_4():
    stem_x = 300
    return union(rect(stem_x, 0, stem_x + F.S, NUM),
                 poly([(stem_x, NUM), (stem_x + F.HN * 1.2, NUM), (F.HN * 1.2, NUM * 0.27 + F.BAR), (0, NUM * 0.27 + F.BAR)]),
                 rect(0, NUM * 0.27, 470, NUM * 0.27 + F.BAR * 1.2),
                 foot(stem_x, stem_x + F.S, 0, F.FL, F.FL))


@fig('five', 0x35)
def fig_5():
    left = 30
    bowl_top = NUM * 0.6
    cx, rx = 230, 225
    cy, ry = (bowl_top - OV) / 2, (bowl_top + OV) / 2
    lower = minus(ring(cx, cy, rx, ry, F.RS * 0.95, F.H, F.H), rect(-BIG, cy - ry * 0.32, cx - 40, BIG))
    return union(lower, rect(left, bowl_top - F.H, cx + 1, bowl_top),
                 rect(left, bowl_top - F.H, left + F.S * 0.85, NUM),
                 rect(left, NUM - F.ARM, 430, NUM), beak(430, NUM, -1))


def six_shape():
    cx, rx = 225, 225
    lower = bowl(0, 2 * rx, NUM * 0.64, F.RS * 0.95, F.H, OV)
    sweep = clip(ring(cx, NUM / 2, rx, NUM / 2 + OV, F.RS * 0.95, F.H, F.H), rect(-BIG, NUM * 0.32, cx + 70, BIG))
    return union(lower, sweep)


@fig('six', 0x36)
def fig_6():
    return six_shape()


@fig('nine', 0x39)
def fig_9():
    return xform(six_shape(), (-1, 0, 0, -1, 450, NUM))


@fig('seven', 0x37)
def fig_7():
    w = 450
    return union(rect(0, NUM - F.ARM, w, NUM), beak(0, NUM, -1, 'left'),
                 poly([(w - F.TD, NUM - F.ARM), (w, NUM - F.ARM), (150 + F.TD / 2, 0), (150 - F.TD / 2, 0)]))


@fig('eight', 0x38)
def fig_8():
    waist = NUM * 0.53
    upper = ring(225, (NUM + OV + waist - F.H) / 2, 190, (NUM + OV - waist + F.H) / 2, F.RS * 0.9, F.H, F.H)
    lower = ring(225, (waist - OV) / 2, 225, (waist + OV) / 2, F.RS * 0.95, F.H, F.H)
    return union(upper, lower)


# ---- punctuation -------------------------------------------------------------------

P_SB = (40, 40)


@glyph('period', [0x2E], sb=P_SB)
def punct_period():
    return dot(0, 0)


@glyph('comma', [0x2C], sb=P_SB)
def punct_comma():
    d = F.DOT
    return union(dot(0, 0), poly([(d * 0.25, 1), (d, 1), (d * 0.45, -d * 1.25), (d * 0.05, -d * 1.25)]))


@glyph('colon', [0x3A], sb=P_SB)
def punct_colon():
    return union(dot(0, 0), dot(0, XH - F.DOT))


@glyph('semicolon', [0x3B], sb=P_SB)
def punct_semicolon():
    return union(punct_comma(), dot(0, XH - F.DOT))


@glyph('periodcentered', [0xB7], sb=P_SB)
def punct_middot():
    return dot(0, XH / 2 - F.DOT / 2)


@glyph('exclam', [0x21], sb=P_SB)
def punct_exclam():
    d = F.DOT
    return union(poly([(0, CAP), (d, CAP), (d * 0.68, CAP * 0.28), (d * 0.32, CAP * 0.28)]), dot(0, 0))


@glyph('question', [0x3F], sb=(30, 30))
def punct_question():
    rx, spring = 190, CAP * 0.66
    top = arch(0, 2 * rx, spring, CAP + OV, F.S * 0.92, F.H)
    top = minus(top, rect(-BIG, -BIG, rx, spring + (CAP - spring) * 0.4))
    turn = clip(ring(rx, spring, rx, spring - CAP * 0.42, F.S * 0.92, F.H, F.H), rect(rx, -BIG, BIG, spring + 1))
    c = rx
    return union(top, turn, rect(c - F.SL / 2, CAP * 0.27, c + F.SL / 2, CAP * 0.42 + F.H), dot(c - F.DOT / 2, 0))


@glyph('quotesingle', [0x27], sb=P_SB)
def punct_quotesingle():
    d = F.DOT
    return poly([(0, CAP), (d, CAP), (d * 0.7, CAP - 230), (d * 0.3, CAP - 230)])


@glyph('quotedbl', [0x22], sb=P_SB)
def punct_quotedbl():
    one = punct_quotesingle()
    return union(one, xform(one, (1, 0, 0, 1, F.DOT + 50, 0)))


def raised_comma():
    return xform(punct_comma(), (1, 0, 0, 1, 0, CAP - F.DOT))


@glyph('quoteright', [0x2019], sb=P_SB)
def punct_quoteright():
    return raised_comma()


@glyph('quoteleft', [0x2018], sb=P_SB)
def punct_quoteleft():
    return xform(punct_comma(), (-1, 0, 0, -1, F.DOT, CAP - F.DOT * 1.25 + F.DOT))


@glyph('quotedblright', [0x201D], sb=P_SB)
def punct_quotedblright():
    one = raised_comma()
    return union(one, xform(one, (1, 0, 0, 1, F.DOT + 50, 0)))


@glyph('quotedblleft', [0x201C], sb=P_SB)
def punct_quotedblleft():
    one = punct_quoteleft()
    return union(one, xform(one, (1, 0, 0, 1, F.DOT + 50, 0)))


@glyph('hyphen', [0x2D], sb=(36, 36))
def punct_hyphen():
    t = F.H * 1.7
    return rect(0, XH / 2 - t / 2, 250, XH / 2 + t / 2)


@glyph('endash', [0x2013], sb=(20, 20))
def punct_endash():
    t = F.H * 1.4
    return rect(0, XH / 2 - t / 2, 460, XH / 2 + t / 2)


@glyph('emdash', [0x2014], sb=(0, 0))
def punct_emdash():
    t = F.H * 1.4
    return rect(0, XH / 2 - t / 2, 1000, XH / 2 + t / 2)


@glyph('parenleft', [0x28], sb=(46, 24))
def punct_parenleft():
    cy, ry = (ASC + DESC) / 2 + 20, (ASC - DESC) / 2 + 40
    rx = 420
    return clip(ring(rx, cy, rx, ry, F.S * 0.8, F.H, F.H), left_of(rx * 0.42))


@glyph('parenright', [0x29], sb=(24, 46))
def punct_parenright():
    return xform(punct_parenleft(), (-1, 0, 0, 1, 0, 0))


@glyph('slash', [0x2F], sb=(10, 10))
def punct_slash():
    return stroke(0, DESC * 0.6, 330, ASC, F.S * 0.9)


@glyph('plus', [0x2B], sb=(40, 40))
def punct_plus():
    h = F.OP / 2
    return union(rect(0, AXIS - h, OP_WIDTH, AXIS + h), rect(OP_WIDTH / 2 - h, AXIS - 200, OP_WIDTH / 2 + h, AXIS + 200))


def bars(*offsets):
    h = F.OP / 2
    return union(*[rect(0, AXIS + o - h, OP_WIDTH, AXIS + o + h) for o in offsets])


@glyph('equal', [0x3D], sb=(40, 40))
def punct_equal():
    return bars(-80, 80)


@glyph('numbersign', [0x23], sb=(20, 20))
def punct_numbersign():
    t = F.S * 0.75
    return union(stroke(170, 30, 240, 650, t), stroke(350, 30, 420, 650, t),
                 rect(40, 210, 540, 210 + F.BAR * 1.3), rect(60, 440, 560, 440 + F.BAR * 1.3))


@glyph('dollar', [0x24], sb=ROUND)
def punct_dollar():
    shape = cap_S()
    c = 270
    return union(shape, rect(c - F.H * 0.8, -90, c + F.H * 0.8, CAP + 90))


@glyph('percent', [0x25], sb=(24, 24))
def punct_percent():
    side = F.S * 0.8
    return union(ring(115, CAP - 150, 115, 150, side, F.H, F.H), ring(465, 150, 115, 150, side, F.H, F.H),
                 line((110, -10), (470, CAP + 10), F.S * 0.62))


@glyph('ampersand', [0x26], sb=(30, 20))
def punct_ampersand():
    upper = ring(210, 560, 125, 152, F.S * 0.85, F.H, F.H)
    lower = minus(ring(240, 205, 240, 205 + OV, F.RS, F.H, F.H), rect(330, 140, BIG, BIG))
    leg = poly([(120, 440), (120 + F.TD, 440), (600, 0), (600 - F.TD, 0)])
    arm = union(rect(250, 330 - F.BAR, 600, 330), beak(600, 330 - F.BAR, -1))
    return union(upper, lower, leg, arm, foot(600 - F.TD, 600, 0, 4, F.FL * 0.6))


@glyph('asterisk', [0x2A], sb=(30, 30))
def punct_asterisk():
    cx, cy = 180, CAP - 170
    petal = poly([(cx - F.SL * 0.42, cy + 170), (cx + F.SL * 0.42, cy + 170), (cx + F.H * 0.5, cy), (cx - F.H * 0.5, cy)])
    return union(*[rotate(petal, angle, cx, cy) for angle in (0, 72, 144, 216, 288)])


@glyph('less', [0x3C], sb=(40, 40))
def punct_less():
    return chevron(AXIS, 210)


def chevron(y, half):
    """The < shape around height y; > and the bracket-like logic signs are built from it."""
    t = F.OP
    return union(line((40, y), (OP_WIDTH, y + half), t), line((40, y), (OP_WIDTH, y - half), t),
                 rect(28, y - t * 0.35, 60, y + t * 0.35))


@glyph('greater', [0x3E], sb=(40, 40))
def punct_greater():
    return xform(punct_less(), (-1, 0, 0, 1, 460, 0))


@glyph('at', [0x40], sb=(30, 30))
def punct_at():
    cx, cy, r = 420, 260, 420
    outer = minus(ring(cx, cy, r, r, F.S * 0.75, F.H, F.H), rect(cx + 120, cy - 300, BIG, cy - 30))
    inner = ring(cx - 40, cy + 20, 130, 160, F.SL * 0.85, F.H, F.H)
    stem_x = cx + 92
    stem = rect(stem_x, cy - 120, stem_x + F.SL * 0.85, cy + 180)
    tail = cup(stem_x, cx + r - 2, cy - 30, cy - 180, F.SL * 0.85, F.H)
    return union(outer, inner, stem, tail)


def bracket(left=True):
    top, bottom, t = ASC + 20, DESC + 20, F.S * 0.8
    shape = union(rect(0, bottom, t, top), rect(0, top - F.ARM, 210, top), rect(0, bottom, 210, bottom + F.ARM))
    return shape if left else xform(shape, (-1, 0, 0, 1, 210, 0))


@glyph('bracketleft', [0x5B], sb=(60, 20))
def punct_bracketleft():
    return bracket(True)


@glyph('bracketright', [0x5D], sb=(20, 60))
def punct_bracketright():
    return bracket(False)


@glyph('backslash', [0x5C], sb=(10, 10))
def punct_backslash():
    return xform(punct_slash(), (-1, 0, 0, 1, 0, 0))


@glyph('asciicircum', [0x5E], sb=(30, 30))
def punct_asciicircum():
    t = F.HN * 1.3
    return union(line((0, 330), (210, CAP), t), line((210, CAP), (420, 330), t))


@glyph('underscore', [0x5F], sb=(0, 0))
def punct_underscore():
    return rect(0, DESC + 70, 520, DESC + 70 + F.H * 1.4)


def brace(left=True):
    t, r, x = F.SL * 0.8, 120, 120
    mid = (ASC + DESC) / 2 + 30
    top_hook = clip(arch(x, x + 2 * r, ASC - 110, ASC + 20, t, F.H), left_of(x + r))
    bottom_hook = clip(cup(x, x + 2 * r, DESC + 130, DESC, t, F.H), left_of(x + r))
    stem = union(rect(x, DESC + 129, x + t, mid - 40), rect(x, mid + 40, x + t, ASC - 109))
    nub = union(poly([(x + t, mid + 70), (x, mid + 40), (0, mid), (x, mid - 40), (x + t, mid - 70)]))
    shape = union(top_hook, bottom_hook, stem, nub)
    return shape if left else xform(shape, (-1, 0, 0, 1, 0, 0))


@glyph('braceleft', [0x7B], sb=(40, 30))
def punct_braceleft():
    return brace(True)


@glyph('braceright', [0x7D], sb=(30, 40))
def punct_braceright():
    return brace(False)


@glyph('bar', [0x7C], sb=(80, 80))
def punct_bar():
    return rect(0, DESC, F.SL * 0.8, ASC + 40)


@glyph('asciitilde', [0x7E], sb=(40, 40))
def punct_asciitilde():
    t = F.H * 1.7
    return union(arch(0, 230, 300, 400, t, t * 0.8), cup(230 - t, 460, 330, 220, t, t * 0.8))


@glyph('ellipsis', [0x2026], sb=(50, 50))
def punct_ellipsis():
    gap = F.DOT + 110
    return union(dot(0, 0), dot(gap, 0), dot(2 * gap, 0))


def arch_stone(x, y, size):
    """A small stone with an arched top: the system's radius-arch shape."""
    return union(rect(x, y, x + size, y + size * 0.5), clip(ellipse(x + size / 2, y + size * 0.5, size / 2, size / 2), above(y + size * 0.5 - 1)))


@glyph('bullet', [0x2022], sb=(50, 50))
def punct_bullet():
    size = F.DOT * 1.7
    return arch_stone(0, XH / 2 - size / 2, size)


def guillemet(offset=0):
    t = F.HN * 1.5
    mid = XH / 2
    return union(line((offset, mid), (offset + 170, mid + 175), t), line((offset, mid), (offset + 170, mid - 175), t),
                 rect(offset - 6, mid - 15, offset + 30, mid + 15))


@glyph('guilsinglleft', [0x2039], sb=(40, 40))
def punct_guilsinglleft():
    return guillemet()


@glyph('guilsinglright', [0x203A], sb=(40, 40))
def punct_guilsinglright():
    return xform(guillemet(), (-1, 0, 0, 1, 170, 0))


@glyph('guillemotleft', [0xAB], sb=(40, 40))
def punct_guillemotleft():
    return union(guillemet(), guillemet(160))


@glyph('guillemotright', [0xBB], sb=(40, 40))
def punct_guillemotright():
    return xform(union(guillemet(), guillemet(160)), (-1, 0, 0, 1, 330, 0))


@glyph('degree', [0xB0], sb=(40, 40))
def punct_degree():
    t = F.H * 1.7
    return ring(115, CAP - 115, 115, 115, t, t * 0.85, t * 0.85)


@glyph('multiply', [0xD7], sb=(50, 50))
def punct_multiply():
    t, r = F.OP, 170
    return union(line((0, AXIS - r), (2 * r, AXIS + r), t), line((0, AXIS + r), (2 * r, AXIS - r), t))


@glyph('divide', [0xF7], sb=(40, 40))
def punct_divide():
    return union(bars(0), dot(OP_WIDTH / 2 - F.DOT / 2, AXIS + 70), dot(OP_WIDTH / 2 - F.DOT / 2, AXIS - 70 - F.DOT))


@glyph('plusminus', [0xB1], sb=(40, 40))
def punct_plusminus():
    return union(xform(punct_plus(), (1, 0, 0, 1, 0, 60)), bars(-260))


@glyph('Euro', [0x20AC], sb=ROUND)
def punct_euro():
    return union(cap_C(), rect(-70, 290, 380, 290 + F.BAR), rect(-70, 400, 400, 400 + F.BAR))


@glyph('sterling', [0xA3], sb=(30, 30))
def punct_sterling():
    x, r = 120, 170
    hook = arch(x, x + 2 * r, 500, CAP + OV, F.S, F.H)
    hook = minus(hook, rect(x + r, -BIG, BIG, 500 + (CAP - 500) * 0.35))
    return union(rect(x, F.ARM - 1, x + F.S, 501), hook, rect(10, 320, 400, 320 + F.BAR),
                 rect(0, 0, 500, F.ARM), beak(500, 0, 1))


@glyph('exclamdown', [0xA1], sb=P_SB)
def punct_exclamdown():
    return xform(punct_exclam(), (-1, 0, 0, -1, F.DOT, XH))


@glyph('questiondown', [0xBF], sb=(30, 30))
def punct_questiondown():
    return xform(punct_question(), (-1, 0, 0, -1, 380, XH))


def arrow_right(tip=620):
    t = F.OP
    y = XH / 2
    return union(rect(0, y - F.OP / 2, tip - 20, y + F.OP / 2),
                 line((tip, y), (tip - 190, y + 190), t), line((tip, y), (tip - 190, y - 190), t),
                 rect(tip - 30, y - 15, tip + 6, y + 15))


@glyph('arrowright', [0x2192], sb=(40, 40))
def punct_arrowright():
    return arrow_right()


@glyph('arrowleft', [0x2190], sb=(40, 40))
def punct_arrowleft():
    return xform(arrow_right(), (-1, 0, 0, 1, 0, 0))


@glyph('arrowup', [0x2191], sb=(40, 40))
def punct_arrowup():
    return rotate(arrow_right(), 90, 310, XH / 2)


@glyph('arrowdown', [0x2193], sb=(40, 40))
def punct_arrowdown():
    return rotate(arrow_right(), -90, 310, XH / 2)


# ---- lowercase Greek ------------------------------------------------------------------

@glyph('alpha', [0x3B1])
def gr_alpha():
    w = 540
    lower = joined_bowl(0, w - F.SL * 0.35, -OVX, XH + OVX, 'right', F.RSL, F.H)
    return union(lower, rect(w - F.SL, 0, w, XH + OVX), foot(w - F.SL, w, 0, 0, F.FLL))


@glyph('beta', [0x3B2])
def gr_beta():
    waist = XH * 0.95
    upper = ring(190, (ASC + OVX + waist) / 2, 190, (ASC + OVX - waist) / 2, F.SL, F.H, F.H)
    lower = ring(235, waist / 2, 235, waist / 2 + OVX, F.RSL, F.H, F.H)
    return union(rect(0, DESC, F.SL, (ASC + waist) / 2), upper, lower, foot(0, F.SL, DESC, F.FLL, F.FLL))


@glyph('gamma', [0x3B3], sb=DIAG)
def gr_gamma():
    w = 500
    return union(v_shape(XH, w, F.TDL, F.HN * 0.95, F.TLL), rect(w / 2 - F.SL * 0.4, DESC + 30, w / 2 + F.SL * 0.4, 2))


@glyph('delta', [0x3B4], sb=ROUND)
def gr_delta():
    cx = 250
    neck_top = ASC - 140
    top = arch(cx - 120 - F.SL * 0.45, cx + 160, neck_top, ASC + OVX, F.SL * 0.85, F.H)
    top = minus(top, rect(cx + 40, -BIG, BIG, ASC - 60))
    return union(bowl(0, 500, XH, F.RSL, F.H, OVX), line((cx + 10, XH - F.H), (cx - 120, neck_top + 1), F.SL * 0.85), top)


def three_shape(height, width):
    waist = height * 0.48
    cx = width * 0.45
    up_cy, up_ry = (height + OVX + waist) / 2, (height + OVX - waist) / 2
    lo_top = waist + F.H
    lo_cy, lo_ry = (lo_top - OVX) / 2, (lo_top + OVX) / 2
    upper = minus(ring(cx, up_cy, cx * 0.92, up_ry, F.RSL * 0.92, F.H, F.H), rect(-BIG, -BIG, cx, up_cy + up_ry * 0.3))
    lower = minus(ring(cx + 8, lo_cy, cx, lo_ry, F.RSL * 0.95, F.H, F.H), rect(-BIG, lo_cy - lo_ry * 0.3, cx + 8, BIG))
    return union(upper, lower, rect(cx - 55, waist, cx + 10, waist + F.H))


@glyph('epsilon', [0x3B5], sb=ROUND)
def gr_epsilon():
    return xform(three_shape(XH, 420), (-1, 0, 0, 1, 0, 0))


def greek_tail(cx):
    """The descending hook under ζ, ξ and ς: from the baseline curve, turn down and right."""
    side = F.SL * 0.85
    ry = 80 + F.H - OVX
    rx = 140
    turn = clip(ring(cx, -80, rx, ry, side, F.H, F.H), rect(cx, -80, BIG, BIG))
    return union(turn, rect(cx + rx - side, DESC + 50, cx + rx, -79))


@glyph('zeta', [0x3B6])
def gr_zeta():
    w = 400
    cx, spring = w * 0.45, 150
    sweep = clip(ring(cx, spring, cx, spring + OVX, F.SL, F.H, F.H), rect(-BIG, -BIG, cx + 1, spring + 1))
    return union(rect(40, ASC - F.ARM, w, ASC), beak(w, ASC, -1),
                 poly([(w - F.TDL, ASC - F.ARM), (w, ASC - F.ARM), (F.SL, spring), (0, spring)]),
                 sweep, greek_tail(cx))


@glyph('eta', [0x3B7])
def gr_eta():
    w, spring = 470, XH * 0.42
    return union(rect(0, 0, F.SL, XH), arch(0, w, spring, XH + OVX, F.SL, F.CROWN), rect(w - F.SL, DESC, w, spring + 1),
                 head(0, F.SL, XH, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL))


@glyph('theta', [0x3B8], sb=ROUND)
def gr_theta():
    return union(ring(230, ASC / 2, 230, ASC / 2 + OVX, F.RSL, F.H, F.H), rect(20, ASC / 2 - F.BAR / 2, 440, ASC / 2 + F.BAR / 2))


@glyph('iota', [0x3B9])
def gr_iota():
    r, spring = 120, 130
    hook = minus(cup(0, 2 * r, spring, -OVX, F.SL, F.H), rect(r, spring * 0.5, BIG, BIG))
    return union(rect(0, spring - 1, F.SL, XH), hook, head(0, F.SL, XH, F.TLL, 0))


@glyph('kappa', [0x3BA])
def gr_kappa():
    w = 450
    branch, (leg0, leg1) = k_branch(F.SL, XH, w, F.HN * 1.1, F.TDL)
    return union(rect(0, 0, F.SL, XH), branch, head(0, F.SL, XH, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL),
                 foot(leg0, leg1, 0, 4, F.FLL))


@glyph('lambda', [0x3BB], sb=DIAG)
def gr_lambda():
    w = 500
    join = XH * 0.72
    t = (join / ASC)   # where the thin leg meets the thick stroke, as a fraction down from the top
    jx = F.TDL / 2 + (w - F.TDL) * (1 - t)
    return union(poly([(0, ASC), (F.TDL, ASC), (w, 0), (w - F.TDL, 0)]),
                 poly([(0, 0), (F.HN, 0), (jx + F.HN / 2, join), (jx - F.HN / 2, join)]),
                 head(0, F.TDL, ASC, F.TLL, 0), foot(0, F.HN, 0, F.FLL, F.FLL), foot(w - F.TDL, w, 0, 4, F.FLL))


@glyph('mu', [0x3BC])
def gr_mu():
    w, spring = 470, XH * 0.58
    return union(rect(0, DESC, F.SL, XH), cup(0, w, spring, -OVX, F.SL, F.CROWN, right_side=F.H * 1.25),
                 rect(w - F.SL, 0, w, XH),
                 head(0, F.SL, XH, F.TLL, 0), head(w - F.SL, w, XH, F.TLL, 0), foot(w - F.SL, w, 0, 0, F.FLL))


@glyph('nu', [0x3BD], sb=DIAG)
def gr_nu():
    return v_shape(XH, 480, F.TDL, F.HN * 0.95, F.TLL)


@glyph('xi', [0x3BE])
def gr_xi():
    return union(gr_zeta(), rect(90, XH * 0.98, 330, XH * 0.98 + F.BAR))


@glyph('pi', [0x3C0])
def gr_pi():
    w = 600
    return union(rect(0, XH - F.ARM, w, XH), rect(100, 0, 100 + F.SL, XH), rect(w - 160, 0, w - 160 + F.SL, XH),
                 foot(100, 100 + F.SL, 0, F.FLL, F.FLL), foot(w - 160, w - 160 + F.SL, 0, F.FLL, F.FLL))


@glyph('rho', [0x3C1])
def gr_rho():
    return union(bowl(0, 520, XH, F.RSL, F.H, OVX), rect(0, DESC, F.SL, XH / 2), foot(0, F.SL, DESC, F.FLL, F.FLL))


@glyph('sigma', [0x3C3])
def gr_sigma():
    return union(bowl(0, 480, XH, F.RSL, F.H, OVX), rect(240, XH - F.ARM, 580, XH))


@glyph('sigmafinal', [0x3C2], sb=ROUND)
def gr_sigmafinal():
    cx, rx, cy, ry = 235, 235, XH / 2, XH / 2 + OVX
    open_c = minus(ring(cx, cy, rx, ry, F.RSL, F.H, F.H), rect(cx + 1, -BIG, BIG, cy + ry * 0.4))
    return union(open_c, greek_tail(cx))


@glyph('tau', [0x3C4])
def gr_tau():
    r, spring, x = 120, 130, 160
    hook = minus(cup(x, x + 2 * r, spring, -OVX, F.SL, F.H), rect(x + r, spring * 0.5, BIG, BIG))
    return union(rect(0, XH - F.ARM, 460, XH), rect(x, spring - 1, x + F.SL, XH), hook)


@glyph('upsilon', [0x3C5])
def gr_upsilon():
    w, spring = 500, XH * 0.55
    return union(cup(0, w, spring, -OVX, F.SL, F.H), rect(0, spring, F.SL, XH), rect(w - F.SL, spring, w, XH),
                 head(0, F.SL, XH, F.TLL, 0))


@glyph('phi', [0x3C6], sb=ROUND)
def gr_phi():
    c = 270
    return union(ring(c, XH / 2, 270, XH / 2 + OVX, F.RSL, F.H, F.H), rect(c - F.SL / 2, DESC, c + F.SL / 2, ASC - 40))


@glyph('chi', [0x3C7], sb=DIAG)
def gr_chi():
    w = 500
    return union(poly([(0, XH), (F.TDL, XH), (w, DESC), (w - F.TDL, DESC)]),
                 poly([(w - F.HN, XH), (w, XH), (F.HN, DESC), (0, DESC)]),
                 head(0, F.TDL, XH, F.TLL, F.TLL), foot(w - F.TDL, w, DESC, 4, F.FLL))


@glyph('psi', [0x3C8])
def gr_psi():
    w, spring = 560, XH * 0.5
    return union(cup(0, w, spring, -OVX, F.SL, F.H), rect(0, spring, F.SL, XH), rect(w - F.SL, spring, w, XH),
                 rect(w / 2 - F.SL / 2, DESC, w / 2 + F.SL / 2, ASC - 60),
                 head(0, F.SL, XH, F.TLL, 0), head(w - F.SL, w, XH, 0, F.TLL))


@glyph('omega', [0x3C9])
def gr_omega():
    w, spring = 720, XH * 0.5
    return union(cup(0, 380, spring, -OVX, F.SL, F.H), cup(340, w, spring, -OVX, F.SL, F.H),
                 rect(0, spring, F.SL, XH), rect(w - F.SL, spring, w, XH),
                 head(0, F.SL, XH, F.TLL, 0), head(w - F.SL, w, XH, 0, F.TLL))


alias('o', [0x3BF], 'o')


# ---- accents and the Latin-1 letters built from them -------------------------------

@glyph('dotlessi', [0x131])
def low_dotlessi():
    return union(rect(0, 0, F.SL, XH), head(0, F.SL, XH, F.TLL, 0), foot(0, F.SL, 0, F.FLL, F.FLL))


def mark_acute():
    return poly([(-50, 0), (-50 + F.H * 1.25, 0), (70, 150), (70 - F.SL, 150)])


def mark_grave():
    return xform(mark_acute(), (-1, 0, 0, 1, 0, 0))


def mark_circumflex():
    half = poly([(-115, 0), (-115 + F.H * 1.35, 0), (F.SL * 0.5, 150), (-F.SL * 0.5, 150)])
    return union(half, xform(half, (-1, 0, 0, 1, 0, 0)))


def mark_dieresis():
    d = F.DOT * 0.92
    return union(rect(-d - 34, 0, -34, d), rect(34, 0, 34 + d, d))


def mark_tilde():
    t = F.H * 1.7
    return union(arch(-150, -5, 40, 140, t, t * 0.8), cup(-5 - t, 145, 100, 0, t, t * 0.8))


def mark_ring():
    return ring(0, 80, 80, 80, F.H * 1.5, F.H * 1.25, F.H * 1.25)


def anchor(path, height):
    """Horizontal centre of a letter's main body, where marks are hung."""
    body = clip(path, rect(-BIG, height * 0.3, BIG, height * 0.8))
    x0, _, x1, _ = body.bounds
    return (x0 + x1) / 2


def with_mark(base, mark, cap):
    path = base()
    height = CAP if cap else XH
    x = anchor(path, height)
    if cap:
        placed = xform(mark(), (1, 0, 0, 0.78, x, CAP + 48))
    else:
        placed = xform(mark(), (1, 0, 0, 1, x, XH + 72))
    return union(path, placed)


def cedilla(base, cap):
    path = base()
    x = anchor(path, CAP if cap else XH)
    t = F.SL * 0.55
    tail = union(rect(x - t / 2, -70, x + t / 2, 20),
                 clip(ring(x + 30, -120, 85, 62, t, F.H, F.H), right_of(x - t / 2)),
                 rect(x - 50, -182, x + 30, -182 + F.H))
    return union(path, tail)


def slashed(base, height):
    path = base()
    x0, _, x1, _ = clip(path, rect(-BIG, 0, BIG, height)).bounds
    return union(path, line((x0 + 20, -40), (x1 - 20, height + 40), F.S * 0.62))


MARKS = {'grave': mark_grave, 'acute': mark_acute, 'circumflex': mark_circumflex,
         'tilde': mark_tilde, 'dieresis': mark_dieresis, 'ring': mark_ring}

ACCENTED = [
    ('A', cap_A, True, [('grave', 0xC0), ('acute', 0xC1), ('circumflex', 0xC2), ('tilde', 0xC3), ('dieresis', 0xC4), ('ring', 0xC5)]),
    ('E', cap_E, True, [('grave', 0xC8), ('acute', 0xC9), ('circumflex', 0xCA), ('dieresis', 0xCB)]),
    ('I', cap_I, True, [('grave', 0xCC), ('acute', 0xCD), ('circumflex', 0xCE), ('dieresis', 0xCF)]),
    ('N', cap_N, True, [('tilde', 0xD1)]),
    ('O', cap_O, True, [('grave', 0xD2), ('acute', 0xD3), ('circumflex', 0xD4), ('tilde', 0xD5), ('dieresis', 0xD6)]),
    ('U', cap_U, True, [('grave', 0xD9), ('acute', 0xDA), ('circumflex', 0xDB), ('dieresis', 0xDC)]),
    ('Y', cap_Y, True, [('acute', 0xDD), ('dieresis', 0x178)]),
    ('a', low_a, False, [('grave', 0xE0), ('acute', 0xE1), ('circumflex', 0xE2), ('tilde', 0xE3), ('dieresis', 0xE4), ('ring', 0xE5)]),
    ('e', low_e, False, [('grave', 0xE8), ('acute', 0xE9), ('circumflex', 0xEA), ('dieresis', 0xEB)]),
    ('i', low_dotlessi, False, [('grave', 0xEC), ('acute', 0xED), ('circumflex', 0xEE), ('dieresis', 0xEF)]),
    ('n', low_n, False, [('tilde', 0xF1)]),
    ('o', low_o, False, [('grave', 0xF2), ('acute', 0xF3), ('circumflex', 0xF4), ('tilde', 0xF5), ('dieresis', 0xF6)]),
    ('u', low_u, False, [('grave', 0xF9), ('acute', 0xFA), ('circumflex', 0xFB), ('dieresis', 0xFC)]),
    ('y', low_y, False, [('acute', 0xFD), ('dieresis', 0xFF)]),
]

for letter, base, is_cap, marks in ACCENTED:
    base_sb = GLYPHS[letter][2]
    for mark_name, code in marks:
        glyph(letter + mark_name, [code], sb=base_sb)(
            (lambda b=base, m=MARKS[mark_name], c=is_cap: lambda: with_mark(b, m, c))())

glyph('Ccedilla', [0xC7], sb=ROUND)(lambda: cedilla(cap_C, True))
glyph('ccedilla', [0xE7], sb=ROUND)(lambda: cedilla(low_c, False))
glyph('Oslash', [0xD8], sb=ROUND)(lambda: slashed(cap_O, CAP))
glyph('oslash', [0xF8], sb=ROUND)(lambda: slashed(low_o, XH))


@glyph('AE', [0xC6], sb=(12, 28))
def cap_AE():
    e_x = 330
    e = xform(cap_E(), (1, 0, 0, 1, e_x, 0))
    diagonal = poly([(0, 0), (F.HN, 0), (e_x + F.HN * 0.5, CAP), (e_x - F.HN * 0.5, CAP)])
    hull = poly([(0, 0), (e_x + F.S, 0), (e_x + F.S, CAP), (e_x, CAP)])
    crossbar = clip(rect(0, CAP * 0.28, e_x + 1, CAP * 0.28 + F.BAR), hull)
    return union(e, diagonal, crossbar, foot(0, F.HN, 0, F.FL, F.FL))


@glyph('OE', [0x152], sb=(34, 28))
def cap_OE():
    e_x = 380
    left = minus(bowl(0, 2 * (e_x + F.S / 2), CAP, F.RS, F.H, 0), right_of(e_x + F.S * 0.5))
    e = xform(cap_E(), (1, 0, 0, 1, e_x, 0))
    return union(left, minus(e, poly([(e_x - F.FL - 2, -2), (e_x, -2), (e_x, F.SH + 2), (e_x - F.FL - 2, F.SH + 2)]),
                             poly([(e_x - F.TL - 2, CAP + 2), (e_x, CAP + 2), (e_x, CAP - F.SH - 2), (e_x - F.TL - 2, CAP - F.SH - 2)])))


@glyph('ae', [0xE6])
def low_ae():
    # The e loses its left side and hangs on the a's stem, so the two share one stroke.
    e = minus(low_e(), left_of(F.RSL))
    return union(low_a(), xform(e, (1, 0, 0, 1, 470 - F.SL * 0.5 - F.RSL, 0)))


@glyph('oe', [0x153])
def low_oe():
    # The e loses its left side and its bowl takes over the o's right side.
    e = minus(low_e(), left_of(F.RSL))
    return union(low_o(), xform(e, (1, 0, 0, 1, 510 - F.RSL * 0.5 - F.RSL, 0)))


@glyph('germandbls', [0xDF])
def low_germandbls():
    w, spring = 400, ASC - 190
    waist = XH * 0.82
    top = arch(0, w, spring, ASC + OVX, F.SL, F.H)
    right_down = clip(ring(w / 2, spring, w / 2, spring - waist, F.SL * 0.95, F.H, F.H), rect(w / 2, waist - 1, BIG, spring + 1))
    lower = minus(ring(w / 2 + 20, (waist + F.H - OVX) / 2, w / 2 - 10, (waist + F.H + OVX) / 2, F.RSL * 0.95, F.H, F.H),
                  rect(-BIG, (waist - OVX) * 0.2, w / 2 + 20, BIG))
    return union(rect(0, 0, F.SL, spring + 1), top, right_down, lower, rect(F.SL, waist, w / 2 + 21, waist + F.H),
                 foot(0, F.SL, 0, F.FLL, F.FLL))


@glyph('Eth', [0xD0])
def cap_Eth():
    return union(cap_D(), rect(-50, CAP * 0.48 - F.BAR / 2, 200, CAP * 0.48 + F.BAR / 2))


@glyph('Thorn', [0xDE])
def cap_Thorn():
    return union(rect(0, 0, F.S, CAP), side_bowl(230, 540, CAP * 0.2, CAP * 0.8, F.RS),
                 foot(0, F.S), head(0, F.S, CAP))


@glyph('thorn', [0xFE])
def low_thorn():
    w, shape = bowl_with_stem('left', DESC, ASC)
    return union(shape, head(0, F.SL, ASC, F.TLL, 0), foot(0, F.SL, DESC, F.FLL, F.FLL))


@glyph('eth', [0xF0], sb=ROUND)
def low_eth():
    return union(gr_delta(), line((120, ASC - 230), (380, ASC - 120), F.H * 1.3))


# ---- notation used in Omega's docs: proofs, logic, mappings -------------------------
# Every sign here strokes at F.OP, centres on AXIS and spans OP_WIDTH like + and =, so a
# line such as `committed ≤ bound ⊢ fact` sits level and even.

@glyph('grave', [0x60], sb=(60, 60))
def punct_grave():
    return xform(mark_grave(), (1, 0, 0, 1, 0, CAP - 150))


@glyph('minus', [0x2212], sb=(40, 40))
def math_minus():
    return bars(0)


@glyph('lessequal', [0x2264], sb=(40, 40))
def math_lessequal():
    return union(chevron(AXIS + 70, 160), bars(-200))


@glyph('greaterequal', [0x2265], sb=(40, 40))
def math_greaterequal():
    return xform(math_lessequal(), (-1, 0, 0, 1, 460, 0))


@glyph('notequal', [0x2260], sb=(40, 40))
def math_notequal():
    return union(bars(-80, 80), line((120, AXIS - 230), (320, AXIS + 230), F.OP))


def wave(y):
    t = F.OP
    return union(arch(0, 230, y, y + 75, t, t * 0.85), cup(230 - t, OP_WIDTH, y + 20, y - 55, t, t * 0.85))


@glyph('approxequal', [0x2248], sb=(40, 40))
def math_approxequal():
    return union(wave(AXIS + 45), wave(AXIS - 105))


@glyph('equivalence', [0x2261], sb=(40, 40))
def math_equivalence():
    return bars(-130, 0, 130)


def wedge():
    top = XH
    return union(line((0, 20), (OP_WIDTH / 2, top), F.OP), line((OP_WIDTH / 2, top), (OP_WIDTH, 20), F.OP),
                 rect(OP_WIDTH / 2 - F.OP * 0.45, top - F.OP * 0.9, OP_WIDTH / 2 + F.OP * 0.45, top))


@glyph('logicaland', [0x2227], sb=(30, 30))
def math_and():
    return clip(wedge(), rect(-BIG, 0, BIG, XH))


@glyph('logicalor', [0x2228], sb=(30, 30))
def math_or():
    return xform(math_and(), (1, 0, 0, -1, 0, XH))


@glyph('intersection', [0x2229], sb=(40, 40))
def math_intersection():
    return union(arch(0, OP_WIDTH, 260, XH + 10, F.OP, F.OP), rect(0, 0, F.OP, 261), rect(OP_WIDTH - F.OP, 0, OP_WIDTH, 261))


def open_set(y, r):
    """The left-opening curve of ∈ and ⊆: a half ring whose ends run straight to the right."""
    cx = r
    return union(clip(ring(cx, y, r, r, F.OP, F.OP, F.OP), left_of(cx + 1)),
                 rect(cx, y + r - F.OP, OP_WIDTH, y + r), rect(cx, y - r, OP_WIDTH, y - r + F.OP))


@glyph('element', [0x2208], sb=(40, 40))
def math_element():
    return union(open_set(AXIS, 210), bars(0))


@glyph('reflexsubset', [0x2286], sb=(40, 40))
def math_subseteq():
    return union(open_set(AXIS + 60, 170), bars(-210))


@glyph('emptyset', [0x2205], sb=(40, 40))
def math_emptyset():
    return union(ring(220, AXIS, 190, 210, F.OP, F.OP, F.OP), line((70, AXIS - 270), (370, AXIS + 270), F.OP))


@glyph('ringoperator', [0x2218], sb=(50, 50))
def math_ring():
    return ring(110, AXIS, 110, 110, F.OP * 0.9, F.OP * 0.9, F.OP * 0.9)


@glyph('universal', [0x2200], sb=(20, 20))
def math_forall():
    t = F.OP * 1.2
    return union(line((0, CAP), (260, 0), t), line((260, 0), (520, CAP), t),
                 rect(260 - t * 0.5, 0, 260 + t * 0.5, t * 0.8), rect(140, CAP * 0.5 - F.OP / 2, 380, CAP * 0.5 + F.OP / 2))


@glyph('existential', [0x2203], sb=(30, 30))
def math_exists():
    w = OP_WIDTH
    return union(rect(w - F.OP * 1.2, 0, w, CAP), rect(0, CAP - F.OP, w, CAP), rect(70, CAP / 2 - F.OP / 2, w, CAP / 2 + F.OP / 2),
                 rect(0, 0, w, F.OP))


@glyph('turnstile', [0x22A2], sb=(50, 40))
def math_turnstile():
    return union(rect(0, 0, F.OP * 1.2, CAP), rect(0, CAP / 2 - F.OP / 2, OP_WIDTH, CAP / 2 + F.OP / 2))


@glyph('mapsto', [0x21A6], sb=(40, 40))
def math_mapsto():
    return union(arrow_right(), rect(0, XH / 2 - 150, F.OP, XH / 2 + 150))


@glyph('arrowboth', [0x2194], sb=(40, 40))
def math_arrowboth():
    return union(arrow_right(), xform(arrow_right(), (-1, 0, 0, 1, 620, 0)))


@glyph('arrowdblright', [0x21D2], sb=(40, 40))
def math_implies():
    t, y, tip = F.OP, XH / 2, 620
    return union(rect(0, y + 55, tip - 120, y + 55 + t * 0.85), rect(0, y - 55 - t * 0.85, tip - 120, y - 55),
                 line((tip, y), (tip - 220, y + 220), t), line((tip, y), (tip - 220, y - 220), t),
                 rect(tip - 30, y - 15, tip + 6, y + 15))


@glyph('longrightarrow', [0x27F6], sb=(40, 40))
def math_longarrow():
    return arrow_right(900)


def angle_bracket():
    mid = (ASC + DESC) / 2 + 20
    return union(line((210, ASC + 20), (20, mid), F.OP), line((20, mid), (210, DESC + 20), F.OP),
                 rect(8, mid - F.OP * 0.35, 40, mid + F.OP * 0.35))


@glyph('angleleft', [0x27E8], sb=(40, 30))
def math_angleleft():
    return angle_bracket()


@glyph('angleright', [0x27E9], sb=(30, 40))
def math_angleright():
    return xform(angle_bracket(), (-1, 0, 0, 1, 230, 0))


@glyph('whitebracketleft', [0x27E6], sb=(60, 20))
def math_whiteleft():
    inner = F.S * 0.8 + 50
    return union(bracket(True), rect(inner, DESC + 20, inner + F.OP * 0.8, ASC + 20))


@glyph('whitebracketright', [0x27E7], sb=(20, 60))
def math_whiteright():
    return xform(math_whiteleft(), (-1, 0, 0, 1, 210, 0))


@glyph('section', [0xA7], sb=ROUND)
def punct_section():
    s = s_shape(440, 380, F.RS * 0.8, F.H, F.S * 0.6, 0)
    return union(xform(s, (1, 0, 0, 1, 0, 250)), xform(s, (1, 0, 0, 1, 0, -40)))


alias('mu', [0xB5], 'mu')


def superior(make):
    """A figure drawn at 62% size and raised to the cap height, drawn heavier first so it
    keeps the text's weight after shrinking."""
    weight, scale = F.WEIGHT, 0.62
    for attr in ('S', 'H', 'RS', 'HN', 'TD', 'ARM', 'BAR', 'BW', 'BH'):
        setattr(F, attr, getattr(F, attr) * 1.25)
    path = make()
    setup(weight)
    return xform(path, (scale, 0, 0, scale, 0, CAP + 20 - NUM * scale))


glyph('onesuperior', [0xB9], sb=(24, 24))(lambda: superior(fig_1))
glyph('twosuperior', [0xB2], sb=(24, 24))(lambda: superior(fig_2))
glyph('threesuperior', [0xB3], sb=(24, 24))(lambda: superior(fig_3))


@glyph('space', [0x20, 0xA0], advance=240)
def space():
    return new()


@glyph('.notdef', [], sb=(50, 50))
def notdef():
    return minus(rect(0, 0, 500, CAP), rect(F.H * 2, F.H * 2, 500 - F.H * 2, CAP - F.H * 2))


# ---- kerning -------------------------------------------------------------------------
#
# Spacing gives every letter fixed sidebearings, which leaves gaps where shapes are
# open: under a T's arm, between A and V, after r or before a period. Kerning closes
# those gaps pair by pair. For each pair we slice both outlines into horizontal bands
# and average the white space between them, counting cavities deeper than DEPTH as
# DEPTH so a letter tucks under an arm without sliding into the stem. A pair whose
# average is wider than its reference pair (HH for capitals, nn for lowercase, the
# punctuation itself against H or n) is pulled in by the difference, but never closer
# than CLEARANCE at its nearest point, measured on the real outlines.

BAND, DEPTH = 20, 140
FACTOR, THRESHOLD, CLEARANCE = 0.85, 20, 30
TIGHTEST, LOOSEST = -110, 40


def profile(path):
    bands = {}
    if not len(path):
        return bands, 0, 0
    for y in range(DESC - 60, ASC + 80, BAND):
        slice_ = clip(path, rect(-BIG, y, BIG, y + BAND))
        if len(slice_):
            x0, _, x1, _ = slice_.bounds
            bands[y] = (x0, x1)
    x_min, _, x_max, _ = path.bounds
    return bands, x_min, x_max


def kernable(name):
    if len(name) == 1 and name.isalpha():
        return True
    return name in KERN_PUNCTUATION or any(name.startswith(base) and name[len(base):] in MARKS
                                           for base in ('A', 'E', 'I', 'N', 'O', 'U', 'Y', 'a', 'e', 'i', 'n', 'o', 'u', 'y'))


KERN_PUNCTUATION = {'period', 'comma', 'quoteright', 'quotedblright', 'quoteleft', 'quotedblleft',
                    'quotesingle', 'quotedbl', 'hyphen', 'colon', 'semicolon', 'endash', 'emdash'}


def reference(name):
    if name in KERN_PUNCTUATION:
        return name
    return 'H' if name[0].isupper() else 'n'


def measure(left, right, profiles, metrics):
    """Average clipped gap and nearest real gap between two placed glyphs."""
    left_bands, _, left_max = profiles[left]
    right_bands, right_min, _ = profiles[right]
    advance = metrics[left][0]
    total, count, nearest = 0.0, 0, None
    for y, (_, x1) in left_bands.items():
        if y not in right_bands:
            continue
        x0 = right_bands[y][0]
        total += (advance - max(x1, left_max - DEPTH)) + min(x0, right_min + DEPTH)
        count += 1
        real = advance - x1 + x0
        nearest = real if nearest is None else min(nearest, real)
    return (total / count, nearest) if count else (None, None)


def kern_pairs(outlines, metrics):
    names = [name for name in outlines if kernable(name)]
    profiles = {name: profile(outlines[name]) for name in names}
    references = {}
    pairs = {}
    for left in names:
        for right in names:
            average, nearest = measure(left, right, profiles, metrics)
            if average is None:
                continue
            key = (reference(left), reference(right))
            if key not in references:
                references[key] = measure(key[0], key[1], profiles, metrics)[0]
            expected = references[key]
            value = 0.0
            if expected is not None and average - expected > THRESHOLD:
                value = FACTOR * (expected - average)
            value = max(value, CLEARANCE - nearest) if nearest < CLEARANCE or value < 0 else value
            value = int(round(max(TIGHTEST, min(LOOSEST, value)) / 5.0) * 5)
            if abs(value) >= 10:
                pairs[(left, right)] = value
    return pairs


# ---- assembly ----------------------------------------------------------------------

def build(weight, style, out_stem):
    setup(weight)
    order, cmap, charstrings, metrics, outlines, tt_glyphs = ['.notdef'], {}, {}, {}, {}, {}
    order += [name for name in GLYPHS if name != '.notdef']
    for name in order:
        unicodes, make, (lsb, rsb), fixed = GLYPHS[name]
        lowercase = any(unicodedata.category(chr(code)) == 'Ll' for code in unicodes)
        if lowercase:
            widen_for_narrowing()
        path = make()
        if lowercase:
            path = xform(path, (NARROW, 0, 0, 1, 0, 0))
            setup(weight)
        path = pathops.simplify(xform(path, (SCALE, 0, 0, SCALE, 0, 0)), clockwise=False)
        lsb, rsb = lsb * SCALE, rsb * SCALE
        fixed = None if fixed is None else round(fixed * SCALE)
        if fixed is None:
            if len(path):
                x0, _, x1, _ = path.bounds
                dx, advance = lsb - x0, round(x1 - x0 + lsb + rsb)
            else:
                dx, advance = 0, 240
        else:
            advance = fixed
            if len(path):
                x0, _, x1, _ = path.bounds
                dx = (fixed - (x1 - x0)) / 2 - x0
            else:
                dx = 0
        placed = xform(path, (1, 0, 0, 1, dx, 0))
        pen = T2CharStringPen(advance, None)
        placed.draw(pen)
        charstrings[name] = pen.getCharString()
        tt_pen = TTGlyphPen(None)
        placed.draw(Cu2QuPen(tt_pen, max_err=0.8, reverse_direction=True))
        tt_glyphs[name] = tt_pen.glyph()
        left = round(placed.bounds[0]) if len(placed) else 0
        metrics[name] = (advance, left)
        outlines[name] = placed
        for code in unicodes:
            cmap[code] = name

    kerning = kern_pairs(outlines, metrics)

    family = 'Omega Serif'
    ps_name = 'OmegaSerif-' + style
    lines = ['languagesystem DFLT dflt;', 'languagesystem latn dflt;', 'languagesystem grek dflt;', 'feature kern {']
    lines += ['  pos %s %s %d;' % (left, right, value) for (left, right), value in sorted(kerning.items())]
    lines.append('} kern;')
    features = '\n'.join(lines)

    def assemble(truetype):
        builder = FontBuilder(UPM, isTTF=truetype)
        builder.setupGlyphOrder(order)
        builder.setupCharacterMap(cmap)
        if truetype:
            builder.setupGlyf(tt_glyphs)
        else:
            builder.setupCFF(ps_name, {'FullName': family + ' ' + style}, charstrings, {})
        builder.setupHorizontalMetrics(metrics)
        builder.setupHorizontalHeader(ascent=950, descent=-280)
        builder.setupNameTable({'familyName': family, 'styleName': style, 'uniqueFontIdentifier': ps_name + '-0.6',
                                'fullName': family + ' ' + style, 'psName': ps_name, 'version': 'Version 0.600'})
        builder.setupOS2(sTypoAscender=round(ASC * SCALE + 10), sTypoDescender=round(DESC * SCALE - 15), sTypoLineGap=200,
                         usWinAscent=1000, usWinDescent=320,
                         sxHeight=round(XH * SCALE), sCapHeight=round(CAP * SCALE), usWeightClass=weight, achVendID='NONE',
                         fsSelection=0x40 if style == 'Regular' else 0)
        builder.setupPost()
        builder.addOpenTypeFeatures(features)
        return builder

    # The CFF build is kept for outline checks; the shipped font is TrueType, autohinted
    # so stems and the x-height snap to whole pixels at reading sizes on Windows.
    assemble(False).save(out_stem + '.otf')
    assemble(True).save(out_stem + '-unhinted.ttf')
    ttfautohint.ttfautohint(in_file=out_stem + '-unhinted.ttf', out_file=out_stem + '.ttf',
                            windows_compatibility=True, hint_composites=True)
    print(style, len(kerning), 'kerning pairs')
    font = TTFont(out_stem + '.ttf')
    font.flavor = 'woff2'
    font.save(out_stem + '.woff2')
    return len(order)


if __name__ == '__main__':
    styles = {'Light': 300, 'Regular': 400, 'Medium': 500}
    chosen = sys.argv[1:] or ['Regular']
    for style in chosen:
        count = build(styles[style], style, 'OmegaSerif-' + style)
        print(style, count, 'glyphs')
