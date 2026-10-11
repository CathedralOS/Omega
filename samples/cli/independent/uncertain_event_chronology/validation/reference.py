"""Independent oracle: Cartesian products of interval members, exact integers.
No floating point tolerances: bytes and exit statuses must match exactly.
Run --freeze before compiler execution; normal validation never changes oracles.
"""
import itertools, json, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]

def parse(raw):
    if raw.endswith(b"\n"): raw = raw[:-1]
    if len(raw) != 19 or any(c < 48 or c > 57 for c in raw): return None
    digits = [c - 48 for c in raw]
    bounds = list(zip(digits[0:6:2], digits[1:6:2]))
    count = digits[6]
    if count > 4 or any(a > b for a,b in bounds): return None
    edges = [tuple(digits[i:i+3]) for i in range(7, 19, 3)][:count]
    if any(a > 2 or b > 2 for a,b,_ in edges): return None
    return bounds, edges

def assignments(bounds, edges, active):
    return [times for times in itertools.product(*(range(a,b+1) for a,b in bounds))
            if all(times[b] >= times[a] + lag for i,(a,b,lag) in enumerate(edges) if i in active)]

def oracle(raw):
    parsed = parse(raw)
    if parsed is None: return 2, "invalid input\n"
    bounds, edges = parsed
    active = set(range(len(edges)))
    feasible = assignments(bounds,edges,active)
    if feasible:
        output = f"feasible count={len(feasible)}\n"
        for event in range(3):
            output += f"event {event} range={min(t[event] for t in feasible)}..{max(t[event] for t in feasible)}\n"
        return 0, output
    for edge in range(len(edges)):
        if not assignments(bounds,edges,active-{edge}): active.remove(edge)
    # Independently verify certificate unsatisfiability and irreducibility.
    assert not assignments(bounds,edges,active)
    assert all(assignments(bounds,edges,active-{edge}) for edge in active)
    mask = ''.join('1' if edge in active else '0' for edge in range(4))
    return 0, f"infeasible witness bounds+edges={mask} count=0\n"

if __name__ == '__main__':
    cases = json.loads((ROOT/'data/cases.json').read_text())
    for case in cases:
        raw = bytes.fromhex(case['input_hex'])
        code, output = oracle(raw)
        destination = ROOT/'data'/case['name']
        if '--freeze' in sys.argv:
            destination.mkdir(exist_ok=True)
            (destination/'input.txt').write_bytes(raw)
            (destination/'expected_stdout.txt').write_text(output)
            (destination/'expected_exit.txt').write_text(str(code)+'\n')
        else:
            assert (destination/'expected_stdout.txt').read_text() == output, case['name']
            assert int((destination/'expected_exit.txt').read_text()) == code, case['name']
    print(f"reference: {len(cases)} exact frozen cases verified")
