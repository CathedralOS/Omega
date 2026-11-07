#!/usr/bin/env python3
"""Independent oracle: enumerate 24 permutations, not Omega's 256 event words.
All quantities are integers; exact equality (absolute and relative tolerance 0).
Regeneration is explicit and forbidden during validation of a frozen suite.
"""
import hashlib
import itertools
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INVALID = 'INVALID: expected C A B RA RB LA(2) LB(2) H(2), eleven digits; C=1..2, A/B=1..9, ready<=latest\n'

def reference(raw):
    record = raw[:-1] if raw.endswith(b'\n') else raw
    if len(record) != 11 or any(c < 48 or c > 57 for c in record):
        return 64, INVALID, None
    d = [c - 48 for c in record]
    capacity, a, b, ra, rb = d[:5]
    la, lb, horizon = 10*d[5]+d[6], 10*d[7]+d[8], 10*d[9]+d[10]
    if capacity not in (1, 2) or min(a,b) < 1 or ra > la or rb > lb:
        return 64, INVALID, None
    accepted = []
    for route in itertools.permutations(range(4)):
        picked, delivered = set(), set()
        position = time = distance = load = peak = waiting = 0
        arrivals = []
        valid = True
        for event in route:
            if event < 2:
                if load == capacity:
                    valid = False; break
                sample = event
                destination = (a,b)[sample]
            else:
                sample = event - 2
                if sample not in picked:
                    valid = False; break
                destination = 0
            length = abs(destination-position)
            distance += length; time += length; position = destination
            if event < 2:
                start = max(time, (ra,rb)[sample])
                waiting += start-time; time = start
                if start > (la,lb)[sample]:
                    valid = False; break
                picked.add(sample); load += 1; peak = max(peak,load)
            else:
                delivered.add(sample); load -= 1
            arrivals.append(time); time += 1
        if valid and delivered == {0,1} and position == 0 and load == 0 and time <= horizon:
            accepted.append(dict(route=route, arrivals=arrivals, distance=distance, completion=time, waiting=waiting, peak=peak))
    accepted.sort(key=lambda r:(r['distance'],r['completion'],r['route']))
    best = accepted[0] if accepted else None
    ties = sum((r['distance'],r['completion']) == (best['distance'],best['completion']) for r in accepted) if best else 0
    lines = ['sample_collection_routing: synthetic integer-minute model','explored_words','256','feasible_tours',f'{len(accepted):03}','optimal_ties',f'{ties:03}']
    if not best:
        return 2, '\n'.join(lines+['INFEASIBLE'])+'\n', dict(feasible=0,ties=0)
    lines += ['OPTIMAL distance_then_completion lexicographic_route']
    for label, value in [('distance',best['distance']),('completion',best['completion']),('waiting',best['waiting']),('peak_load',best['peak'])]:
        lines += [label, f'{value:03}']
    lines += ['route 000=depot 001=pickup_A 002=pickup_B 003=drop_A 004=drop_B','000']
    lines += [f'{event+1:03}' for event in best['route']] + ['000','service_starts']
    lines += [f'{t:03}' for t in best['arrivals']] + ['final_load','000','final_position','000']
    return 0, '\n'.join(lines)+'\n', dict(feasible=len(accepted),ties=ties,**best)

def freeze():
    cases = {
        'joint_capacity_two': b'23700999999\n',
        'separate_capacity_one': b'13700999999\n',
        'symmetric_ties': b'24400999999\n',
        'capacity_one_ties': b'14400999999\n',
        'waiting': b'21298999999\n',
        'urgent_a': b'23700039999\n',
        'urgent_b': b'23700990799\n',
        'window_infeasible': b'23700029999\n',
        'horizon_exact': b'23700999918\n',
        'horizon_infeasible': b'23700999917\n',
        'zero_latest': b'21100000099\n',
        'equal_window': b'21111010199\n',
        'exact_pickup_windows': b'21112010299\n',
        'reversed_sites': b'27300999999\n',
        'far_sites': b'29900999999\n',
        'eof_record': b'23700999999',
        'invalid_capacity_zero': b'03700999999\n',
        'invalid_capacity_three': b'33700999999\n',
        'invalid_site_zero': b'20700999999\n',
        'invalid_window_order': b'23790009999\n',
        'invalid_non_digit': b'237009x9999\n',
        'invalid_short': b'237\n',
        'invalid_long': b'237009999990\n',
        'invalid_crlf': b'23700999999\r\n',
        'invalid_empty': b'',
    }
    manifest = {'model':'two unit vials; line travel; 1 minute each service; pickup-start windows; depot drops; horizon includes final service', 'oracle':'itertools.permutations(range(4)); distance, completion, then lex route; ties count distance/completion only', 'tolerances':{'absolute':0,'relative':0},'cases':[]}
    for name, raw in cases.items():
        code, text, certificate = reference(raw)
        (ROOT/(name+'.stdin')).write_bytes(raw)
        (ROOT/(name+'.stdout')).write_bytes(text.encode())
        manifest['cases'].append(dict(name=name,exit=code,stdin_sha256=hashlib.sha256(raw).hexdigest(),stdout_sha256=hashlib.sha256(text.encode()).hexdigest(),certificate=certificate))
    manifest['reference_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (ROOT/'frozen_oracles.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps([(c['name'],c['exit'],c['certificate']) for c in manifest['cases']],indent=2))

if __name__ == '__main__':
    import sys
    if sys.argv[1:] != ['--freeze']:
        raise SystemExit('Only explicit --freeze writes oracles. Validation consumes frozen files.')
    freeze()
