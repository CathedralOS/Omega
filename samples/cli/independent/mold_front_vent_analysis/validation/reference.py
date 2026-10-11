"""Independent set/graph reference, frozen before Omega check or execution.
No Omega output is consumed when producing expected artifacts. Integer tolerance=0.
"""
import itertools
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
CELLS = set(range(16))

def neighbours(cell):
    row, column = divmod(cell, 4)
    return {r * 4 + c for r, c in ((row-1,column),(row+1,column),(row,column-1),(row,column+1))
            if 0 <= r < 4 and 0 <= c < 4}

def flood(air, seeds):
    reached = air & seeds
    pending = list(reached)
    while pending:
        for other in neighbours(pending.pop()) & air - reached:
            reached.add(other)
            pending.append(other)
    return reached

def cells(mask):
    return {index for index in CELLS if mask & (1 << index)}

def mask(values):
    return sum(1 << index for index in values)

def parse(line):
    text = line.rstrip('\r\n')
    if len(text) != 17 or text[4] != ' ' or text[9] != ' ' or text[14] != ' ':
        raise ValueError('format')
    if any(c not in '0123456789abcdefABCDEF' for c in text[:4]+text[5:9]+text[10:14]):
        raise ValueError('hex')
    if any(c not in '0123456789' for c in text[15:]):
        raise ValueError('decimal')
    walls, gates, vents = [cells(int(part,16)) for part in text[:14].split()]
    budget = int(text[15:])
    if not gates or walls & (gates | vents) or gates & vents or budget > 16 or budget < len(gates):
        raise ValueError('model')
    return walls, gates, vents, budget

def simulate(walls, gates, vents, budget):
    filled, trapped = set(gates), set()
    arrival = {cell:0 for cell in gates}
    tick = 0
    batches = []
    while True:
        air = CELLS - walls - filled - trapped
        vented = flood(air, vents)
        trapped |= air - vented
        candidates = vented & set().union(*(neighbours(cell) for cell in filled))
        if len(filled) == budget or not candidates:
            break
        tick += 1
        batch = set(sorted(candidates)[:budget-len(filled)])
        batches.append(mask(batch))
        filled |= batch
        arrival.update({cell:tick for cell in batch})
    regions, rest = [], set(trapped)
    while rest:
        region = flood(rest, {min(rest)})
        regions.append(region)
        rest -= region
    assert walls | filled | trapped | vented == CELLS
    assert len(walls)+len(filled)+len(trapped)+len(vented) == 16
    assert len(filled) <= budget
    return dict(walls=walls,filled=filled,trapped=trapped,vented=vented,arrival=arrival,
                regions=regions,tick=tick,budget=budget,batches=batches)

def render(result):
    output = ['MOLD_FRONT_VENT_V1', 'arrival ticks (# wall, . unfilled):']
    for row in range(4):
        output.append(' '.join('#' if i in result['walls'] else str(result['arrival'][i]).zfill(2)
                               if i in result['arrival'] else '.' for i in range(row*4,row*4+4)))
    output.append('phase (# wall, R resin, T trapped, A vented air):')
    for row in range(4):
        output.append(' '.join('#' if i in result['walls'] else 'R' if i in result['filled'] else
                               'T' if i in result['trapped'] else 'A' for i in range(row*4,row*4+4)))
    output.append('trapped regions (mask, cells):')
    output.extend(f'{mask(region):05d} {len(region):02d}' for region in result['regions'])
    output.append('balance wall resin trapped vented budget unused rounds:')
    values = [len(result[k]) for k in ['walls','filled','trapped','vented']]
    values += [result['budget'], result['budget']-len(result['filled']),result['tick']]
    output.append(' '.join(str(v).zfill(2) for v in values))
    output.append('OK')
    return '\n'.join(output)+'\n'

def tiny_inputs():
    # Enumerate every wall/gate/vent/ordinary assignment on an embedded 2x2.
    tiny = [0,1,4,5]
    for roles in itertools.product(range(4), repeat=4):
        walls = CELLS - set(tiny) | {c for c,r in zip(tiny,roles) if r==0}
        gates = {c for c,r in zip(tiny,roles) if r==1}
        vents = {c for c,r in zip(tiny,roles) if r==2}
        if not gates:
            continue
        for budget in range(len(gates), 5):
            yield f'{mask(walls):04X} {mask(gates):04X} {mask(vents):04X} {budget:02d}\n'

def exhaustive_flood():
    # Independent oracle: enumerate every simple path, instead of a closure loop.
    tiny = {0,1,4,5}
    checks = 0
    for air_mask in range(16):
        air = {cell for bit,cell in enumerate(sorted(tiny)) if air_mask & (1<<bit)}
        for seed_mask in range(16):
            seeds = {cell for bit,cell in enumerate(sorted(tiny)) if seed_mask & (1<<bit)}
            reached = set()
            for length in range(1,5):
                for path in itertools.permutations(sorted(air), length):
                    if path[0] in seeds and all(b in neighbours(a) for a,b in zip(path,path[1:])):
                        reached.add(path[-1])
            assert flood(air,seeds) == reached
            checks += 1
    return checks

FIXTURES = {
    'merging': '0000 0009 F000 16\n',
    'blocked_vents': '0000 0001 0008 16\n',
    'enclosed_air': '0252 0001 8000 16\n',
    'exhausted_budget': '0000 0009 F000 05\n',
    'zero_vents': '0000 0001 0000 16\n',
    'two_trapped_regions': '6666 0001 0000 16\n',
    'seed_budget': '0000 0009 F000 02\n',
    'lowercase_crlf': '0000 0009 f000 05\r\n',
    'full_gate_row': '0000 000F F000 16\n',
    'eof': '0000 0009 F000 05',
}
INVALID = {
    'empty': '', 'bad_hex':'000Z 0001 8000 16\n',
    'bad_separator':'0000,0001 8000 16\n', 'too_large':'0000 0001 8000 17\n',
    'no_gate':'0000 0000 8000 16\n', 'wall_gate':'0001 0001 8000 16\n',
    'wall_vent':'8000 0001 8000 16\n', 'gate_vent':'0000 0001 0001 16\n',
    'too_little':'0000 0009 F000 01\n', 'trailing':'0000 0001 8000 16x\n',
    'negative':'0000 0001 8000 -1\n', 'full_buffer':'x'*32,
}

if __name__ == '__main__':
    checks = exhaustive_flood()
    summary = {}
    for name, line in FIXTURES.items():
        (ROOT/'data'/f'{name}.in').write_bytes(line.encode())
        result = simulate(*parse(line))
        (ROOT/'data'/f'{name}.expected').write_text(render(result))
        summary[name] = {k:mask(result[k]) for k in ['walls','filled','trapped','vented']}
        summary[name].update(tick=result['tick'],batches=result['batches'])
    for name,line in INVALID.items():
        (ROOT/'data'/f'invalid_{name}.in').write_bytes(line.encode())
    freeze = dict(tolerance=0,flood_enumerations=checks,tiny_runtime_cases=len(list(tiny_inputs())),fixtures=summary)
    (ROOT/'validation'/'frozen.json').write_text(json.dumps(freeze,indent=2)+'\n')
    print(json.dumps(freeze,indent=2))
