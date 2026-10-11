"""Independent heap scheduler; produces frozen oracles before Omega execution."""
import heapq
import itertools
from pathlib import Path
ROOT = Path(__file__).resolve().parent
HEADER = 'logic_hazard_timing: mode 0=transport1 1=transport4 2=inertial4\n'
def run(mode):
    delay, inertial = [(1, False), (4, False), (4, True)][mode]
    gates = {3: ('not', 0, 0, 2), 4: ('and', 0, 1, 1),
             5: ('and', 3, 2, 2), 6: ('or', 4, 5, delay),
             7: ('and', 1, 2, 1), 8: ('or', 6, 7, 1)}
    def evaluate(node, values):
        kind, left, right, _ = gates[node]
        return {'not': lambda: not values[left],
                'and': lambda: values[left] and values[right],
                'or': lambda: values[left] or values[right]}[kind]()
    lines = [HEADER.rstrip()]
    for row, inputs in enumerate(itertools.product((False, True), repeat=3)):
        values = list(inputs) + [False]*6
        for node in gates: values[node] = evaluate(node, values)
        a,b,c = inputs
        # Independently select c when a=0 and b when a=1.
        expected = b if a else c
        assert values[6] == values[8] == expected
        lines.append(f'truth row={row:02} value={int(expected)}')
    values = [True]*3 + [False]*6
    for node in gates: values[node] = evaluate(node, values)
    desired = values.copy()
    lines.append('initial nodes 0..8=' + ''.join(str(int(v)) for v in values))
    events = [(10, 0, 0, False)]
    scheduled_slots = {(10, 0)}
    generations = [0]*9
    fall = None
    while events:
        time,node,generation,value = heapq.heappop(events)
        assert time < 32 and node < 9
        if inertial and generation != generations[node]: continue
        if values[node] == value: continue
        values[node] = value
        lines.append(f'wave t={time:02} node={node:02} value={int(value)}')
        if node == 6:
            if not value: fall = time
            else:
                assert fall is not None
                lines.append(f'glitch node=06 start={fall:02} end={time:02} width={time-fall:02}')
        for gate in gates:
            new = evaluate(gate, values)
            if desired[gate] != new:
                desired[gate] = new
                generations[gate] += 1
                arrival = time+gates[gate][3]
                # The bounded calendar model requires one edge per gate/timestamp.
                assert (arrival, gate) not in scheduled_slots
                scheduled_slots.add((arrival, gate))
                heapq.heappush(events, (arrival, gate, generations[gate], new))
    assert values[6] == values[8] == True
    lines.append('PASS truth=08 steady=1 consensus=1')
    return '\n'.join(lines)+'\n'
if __name__ == '__main__':
    for mode in range(3):
        path = ROOT/'data'/f'mode{mode}.expected'
        expected = run(mode)
        if path.exists():
            assert path.read_text() == expected, f'frozen oracle changed: {path}'
        else: path.write_text(expected)
