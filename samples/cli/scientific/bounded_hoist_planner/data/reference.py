#!/usr/bin/env python3
"""Independent oracle: per-span product search and exact path counting.

Omega enumerates all 10**4 height words and filters them; this oracle instead
searches the per-span allowed ranges, finds the minimum vertical travel,
then counts feasible and optimal-tied profiles and selects the lexicographic
minimum. All quantities are exact integers;
absolute and relative tolerances are both zero. Regeneration is explicit and
forbidden during validation of a frozen suite.
"""
import hashlib
import itertools
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INVALID = 'INVALID: expected H0 HE F1 C1 F2 C2 F3 C3 F4 C4, ten digits; each floor<=ceiling\n'
STEP = 4
SPANS = 4
HEADER = ['clearance_route_planner: synthetic span-height clearance model',
          'explored_words', '10000']


def profiles(allowed, start, end):
    """Every height word meeting bounds, step limit and the end requirement."""
    for heights in itertools.product(*allowed):
        if heights[-1] != end:
            continue
        previous = start
        cost = 0
        possible = True
        for height in heights:
            step = abs(height - previous)
            if step > STEP:
                possible = False
                break
            cost += step
            previous = height
        if possible:
            yield heights, cost


def reference(raw):
    record = raw[:-1] if raw.endswith(b'\n') else raw
    if len(record) != 10 or any(c < 48 or c > 57 for c in record):
        return 64, INVALID, None
    d = [c - 48 for c in record]
    start, end = d[0], d[1]
    floors = [d[2], d[4], d[6], d[8]]
    ceilings = [d[3], d[5], d[7], d[9]]
    if any(f > c for f, c in zip(floors, ceilings)):
        return 64, INVALID, None
    allowed = [range(f, c + 1) for f, c in zip(floors, ceilings)]
    accepted = list(profiles(allowed, start, end))
    if not accepted:
        lines = HEADER + ['feasible_profiles', '00000', 'optimal_ties', '00000',
                          'INFEASIBLE']
        return 2, '\n'.join(lines) + '\n', dict(feasible=0, ties=0)
    optimum = min(cost for _, cost in accepted)
    ties = sum(cost == optimum for _, cost in accepted)
    best_profile = min(heights for heights, cost in accepted if cost == optimum)
    lines = HEADER + ['feasible_profiles', f'{len(accepted):05}',
                      'optimal_ties', f'{ties:05}',
                      'OPTIMAL minimum_vertical_travel lexicographic_profile',
                      'vertical_travel', f'{optimum:03}',
                      'profile span_1 span_2 span_3 span_4']
    lines += [f'{h:03}' for h in best_profile]
    lines += ['final_height', f'{best_profile[-1]:03}']
    return 0, '\n'.join(lines) + '\n', dict(feasible=len(accepted), ties=ties,
                                            cost=optimum, heights=list(best_profile))


def freeze():
    cases = {
        'corridor_flat': b'3309090909\n',
        'open_corridor_ties': b'0209090909\n',
        'dip_required': b'5509020909\n',
        'rise_required': b'1109095909\n',
        'zigzag_bounds': b'4626135766\n',
        'step_limited_ascent': b'0909090999\n',
        'step_limited_descent': b'9009090900\n',
        'narrow_corridor': b'2424354644\n',
        'forced_profile': b'9999999999\n',
        'descent_open': b'9009090909\n',
        'eof_record': b'3309090909',
        'end_unreachable': b'3909090915\n',
        'step_gap_infeasible': b'0599090909\n',
        'trap_infeasible': b'0099090909\n',
        'invalid_floor_above_ceiling': b'3393090909\n',
        'invalid_non_digit': b'33090909x9\n',
        'invalid_letter': b'33a9090909\n',
        'invalid_space': b'33 9090909\n',
        'invalid_short': b'330\n',
        'invalid_long': b'33090909090\n',
        'invalid_crlf': b'3309090909\r\n',
        'invalid_empty': b'',
    }
    manifest = {'model': 'four spans; operating height in [floor,ceiling] per span; hoist step <= 4; final height must equal HE; minimize total vertical travel',
                'oracle': 'forward product search over per-span allowed ranges for feasibility; minimum travel cost; exhaustive counting for feasible/ties; lexicographic optimum',
                'tolerances': {'absolute': 0, 'relative': 0}, 'cases': []}
    for name, raw in cases.items():
        code, text, certificate = reference(raw)
        (ROOT / (name + '.stdin')).write_bytes(raw)
        (ROOT / (name + '.stdout')).write_bytes(text.encode())
        manifest['cases'].append(dict(name=name, exit=code,
                                      stdin_sha256=hashlib.sha256(raw).hexdigest(),
                                      stdout_sha256=hashlib.sha256(text.encode()).hexdigest(),
                                      certificate=certificate))
    manifest['reference_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (ROOT / 'frozen_oracles.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps([(c['name'], c['exit'], c['certificate']) for c in manifest['cases']], indent=2))


if __name__ == '__main__':
    import sys
    if sys.argv[1:] != ['--freeze']:
        raise SystemExit('Only explicit --freeze writes oracles. Validation consumes frozen files.')
    freeze()
