#!/usr/bin/env python3
"""Independent all-predecessor audit of observed allocator facts; no admission."""
from pathlib import Path
import collections
import hashlib
import json

ROOT = Path(__file__).resolve().parent

def calculate(data):
    result = {}
    for register, row in data.items():
        points = {}
        for point in row['points']:
            points.setdefault(point['block'], []).append(point)
        incoming = collections.defaultdict(list)
        for edge in row['connectors']:
            if edge['target'] in points:
                incoming[edge['target']].append(edge['source'])
        closing = {}
        boundaries = []
        pending = set(points)
        while pending:
            ready = sorted(block for block in pending
                           if all(source in closing for source in incoming[block]))
            assert ready, 'observed range has a cycle or missing predecessor'
            for block in ready:
                block_points = points[block]
                domain = set(block_points[0]['views'])
                for source in incoming[block]:
                    domain.intersection_update(closing[source])
                assert domain, (register, block, 'empty incoming domain')
                for point in block_points[1:]:
                    compatible = domain.intersection(point['views'])
                    if not compatible:
                        boundaries.append({'block': block, 'point': point['point'],
                                           'from_views': sorted(domain),
                                           'to_views': point['views']})
                        domain = set(point['views'])
                    else:
                        domain = compatible
                closing[block] = domain
                pending.remove(block)
        result[register] = {
            'acyclic': True, 'all_predecessors_processed': True,
            'closing_domains': {str(block): sorted(views)
                                for block, views in closing.items()},
            'boundaries': boundaries,
        }
    return result

if __name__ == '__main__':
    oracle = ROOT.parent.parent / 'data/expected.json'
    assert hashlib.sha256(oracle.read_bytes()).hexdigest() == (
        '7dd3788aec16d4870152fcce959069808534df441029a14eb2628d8f805707c3')
    result = calculate(json.loads((ROOT / 'range-summary.json').read_text()))
    assert result == json.loads((ROOT / 'independent-domain-audit.json').read_text())
    assert not result['6']['boundaries']
    assert result['7']['boundaries'] == [
        {'block': 13, 'point': 82, 'from_views': [10], 'to_views': [0]}]
    print(json.dumps(result, indent=2))
