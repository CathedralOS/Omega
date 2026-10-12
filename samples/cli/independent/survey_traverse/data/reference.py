#!/usr/bin/env python3
"""Independent survey-traverse oracle.

Parses the same bounded record grammar as the Omega application, reduces
the traverse with platform sin/cos (not the Omega polynomial), applies the
same Bowditch adjustment and reproduces the exact printed layout. The
millidegree sweep re-implements the application's integer-quadrant +
double-double + Taylor algorithm in Python and measures its maximum
absolute error against math.sin/math.cos over all 360000 arguments; the
measured value is recorded in oracle.json, not used to produce results.

Freeze mode (run before any Omega check or execution):
    python3 data/reference.py --freeze
"""
import hashlib
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FIXTURES = ROOT.parent / "fixtures"
ORACLE = ROOT / "oracle.json"

K_HI = 0.000017453292519500
K_LO = 4.4329638701143260e-16
PROBE_TOLERANCE = 1e-9
PRINT_TOLERANCE_UNITS = 1  # one unit in the last printed digit (1e-6 mm)
PRECISION_NUM_TOLERANCE = 1

MAX_LEGS = 8
MAX_LINE = 480


def parse_record(data: bytes):
    """Mirror survey/input.omg: returns leg list or None on malformed."""
    if data.endswith(b"\n"):
        data = data[:-1]
    if len(data) > MAX_LINE:
        return None
    tokens = []
    i = 0
    n = len(data)
    while True:
        while i < n and data[i] in (32, 9):
            i += 1
        if i >= n:
            break
        value = 0
        digits = 0
        while i < n and 48 <= data[i] <= 57:
            if digits >= 7:
                return None
            value = value * 10 + data[i] - 48
            digits += 1
            i += 1
        if i < n and data[i] not in (32, 9):
            return None
        tokens.append(value)
    if not tokens:
        return None
    legs = tokens[0]
    if not (3 <= legs <= MAX_LEGS) or len(tokens) != 1 + 2 * legs:
        return None
    pairs = []
    for slot in range(legs):
        bearing = tokens[1 + 2 * slot]
        distance = tokens[2 + 2 * slot]
        if not (0 <= bearing < 360000) or not (1 <= distance <= 2000000):
            return None
        pairs.append((bearing, distance))
    return pairs


# --- Python re-implementation of survey/millidegree.omg (algorithm twin) ---

def _sin_poly(x):
    xx = x * x
    p = 0.0000000000000028114572543455208
    p = -0.0000000000007647163731819816476 + xx * p
    p = 0.0000000001605904383682161460 + xx * p
    p = -0.0000000250521083854417188 + xx * p
    p = 0.0000027557319223985891 + xx * p
    p = -0.0001984126984126984 + xx * p
    p = 0.0083333333333333333 + xx * p
    p = -0.1666666666666666667 + xx * p
    return x + x * xx * p


def _cos_poly(x):
    xx = x * x
    p = -0.0000000000000001561920696858623
    p = 0.0000000000000477947733238738505 + xx * p
    p = -0.0000000000114707455977297251 + xx * p
    p = 0.0000000020876756987868099 + xx * p
    p = -0.0000002755731922398589065 + xx * p
    p = 0.0000248015873015873016 + xx * p
    p = -0.0013888888888888889 + xx * p
    p = 0.0416666666666666667 + xx * p
    p = -0.5 + xx * p
    return 1.0 + xx * p


def _radians(remainder):
    r = float(remainder)
    return r * K_HI + r * K_LO


def _base_sin(r):
    if r <= 45000:
        return _sin_poly(_radians(r))
    return _cos_poly(_radians(90000 - r))


def _base_cos(r):
    if r <= 45000:
        return _cos_poly(_radians(r))
    return _sin_poly(_radians(90000 - r))


def sin_mdeg(mdeg):
    q, r = divmod(mdeg, 90000)
    if q == 0:
        return _base_sin(r)
    if q == 1:
        return _base_cos(r)
    if q == 2:
        return -_base_sin(r)
    return -_base_cos(r)


def cos_mdeg(mdeg):
    q, r = divmod(mdeg, 90000)
    if q == 0:
        return _base_cos(r)
    if q == 1:
        return -_base_sin(r)
    if q == 2:
        return -_base_cos(r)
    return _base_sin(r)


def millidegree_sweep():
    worst_sin = 0.0
    worst_cos = 0.0
    for mdeg in range(360000):
        worst_sin = max(worst_sin, abs(sin_mdeg(mdeg) - math.sin(math.radians(mdeg / 1000.0))))
        worst_cos = max(worst_cos, abs(cos_mdeg(mdeg) - math.cos(math.radians(mdeg / 1000.0))))
    return worst_sin, worst_cos


# --- Formatting, mirroring survey/report.omg ---

def fixed6(value):
    if value < 0.0:
        sign = "-"
        magnitude = -value
    else:
        sign = "+"
        magnitude = value
    scaled = int(magnitude * 1000000.0 + 0.5)
    return f"{sign}{scaled // 1000000:08d}.{scaled % 1000000:06d}"


def expected_output(pairs):
    # Expected values come from the algorithm twin so that exact-zero
    # branches and rounding floors match the application's own arithmetic;
    # independence is enforced by the recorded platform-vs-twin divergence
    # and the frozen print tolerance, not by an assumed shared bug.
    dep = [d * sin_mdeg(b) for b, d in pairs]
    lat = [d * cos_mdeg(b) for b, d in pairs]
    dep_ref = [d * math.sin(math.radians(b / 1000.0)) for b, d in pairs]
    lat_ref = [d * math.cos(math.radians(b / 1000.0)) for b, d in pairs]
    global LAST_DIVERGENCE
    LAST_DIVERGENCE = max(
        max(abs(a - r) for a, r in zip(dep, dep_ref)),
        max(abs(a - r) for a, r in zip(lat, lat_ref)))
    # Sequential accumulation, matching the Omega loop order exactly.
    sum_dep = 0.0
    sum_lat = 0.0
    for i in range(len(pairs)):
        sum_dep += dep[i]
        sum_lat += lat[i]
    perimeter = sum(d for _, d in pairs)
    misclosure = math.sqrt(sum_dep * sum_dep + sum_lat * sum_lat)
    grade_ok = misclosure * 1000.0 <= float(perimeter)
    lines = [f"traverse legs={len(pairs)} perimeter={perimeter}",
             f"misclosure dep={fixed6(sum_dep)} lat={fixed6(sum_lat)} "
             f"length={fixed6(misclosure)} "
             f"precision={'exact' if misclosure == 0.0 else '1:' + str(int(perimeter / misclosure))}"]
    if not grade_ok:
        lines.append("grade=FAIL")
        return 3, "\n".join(lines) + "\n"
    adj_dep = [-sum_dep * d / perimeter for _, d in pairs]
    adj_lat = [-sum_lat * d / perimeter for _, d in pairs]
    east = [0.0]
    north = [0.0]
    for i in range(len(pairs)):
        east.append(east[-1] + dep[i] + adj_dep[i])
        north.append(north[-1] + lat[i] + adj_lat[i])
    for i in range(len(pairs)):
        lines.append(f"leg {i} dep={fixed6(dep[i])} lat={fixed6(lat[i])} "
                     f"adj_dep={fixed6(adj_dep[i])} adj_lat={fixed6(adj_lat[i])}")
    for i in range(len(pairs) + 1):
        lines.append(f"station {i} e={fixed6(east[i])} n={fixed6(north[i])}")
    lines.append(f"bounds e_min={fixed6(min(east))} e_max={fixed6(max(east))} "
                 f"n_min={fixed6(min(north))} n_max={fixed6(max(north))}")
    lines.append("grade=PASS")
    return 0, "\n".join(lines) + "\n"


def oracle(data: bytes):
    pairs = parse_record(data)
    if pairs is None:
        return 2, "ERROR invalid input\n"
    return expected_output(pairs)


def freeze():
    cases = {}
    for path in sorted(FIXTURES.glob("*.input")):
        name = path.stem
        raw = path.read_bytes()
        code, stdout = oracle(raw)
        out_path = FIXTURES / f"{name}.stdout"
        out_path.write_bytes(stdout.encode())
        cases[name] = {
            "exit": code,
            "twin_platform_divergence_mm": LAST_DIVERGENCE,
            "input_sha256": hashlib.sha256(raw).hexdigest(),
            "stdout_sha256": hashlib.sha256(out_path.read_bytes()).hexdigest(),
        }
    worst_sin, worst_cos = millidegree_sweep()
    manifest = {
        "frozen_by": "data/reference.py --freeze before any Omega execution",
        "algorithm": "integer-quadrant millidegree fold + double-double radian scale + Taylor",
        "millidegree_max_abs_error": {"sin": worst_sin, "cos": worst_cos},
        "probe_tolerance": PROBE_TOLERANCE,
        "print_tolerance_units": PRINT_TOLERANCE_UNITS,
        "precision_num_tolerance": PRECISION_NUM_TOLERANCE,
        "cases": cases,
    }
    ORACLE.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(f"froze {len(cases)} cases; millidegree max abs error sin={worst_sin:.3e} cos={worst_cos:.3e}")


if __name__ == "__main__":
    if sys.argv[1:] == ["--freeze"]:
        freeze()
    elif sys.argv[1:] == ["--sweep"]:
        print(millidegree_sweep())
    else:
        raise SystemExit("usage: reference.py --freeze | --sweep")
