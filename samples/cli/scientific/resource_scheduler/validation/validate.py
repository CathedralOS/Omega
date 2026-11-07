#!/usr/bin/env python3
"""Check frozen independent oracles and exported native/Terminal observations.

No execution result can rewrite a golden. This checker independently reconstructs
precedence, resource use, duration, tie order and makespan from the printed rows.
"""
import hashlib
import json
import re
import sys
from pathlib import Path

from reference import ROOT, calculate, decode, feasible


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(output):
    frozen = json.loads((ROOT / 'data' / 'expected.json').read_text())
    vectors = 0
    for name, expected in frozen.items():
        raw = (ROOT / 'data' / f'{name}.input').read_bytes()
        independent = calculate(raw)
        # Round-trip tuples so the expected JSON's list representation compares.
        assert json.loads(json.dumps(independent)) == expected, f'{name}: reference drift'
        assert (ROOT / 'data' / f'{name}.stdout').read_text() == expected['stdout']
        vectors += independent['vectors']
        for engine in ('native', 'terminal'):
            directory = output / engine
            actual = (directory / f'{name}.stdout').read_bytes()
            assert actual == expected['stdout'].encode(), f'{engine}/{name}: output differs: {actual!r}'
            assert int((directory / f'{name}.exit').read_text()) == expected['exit'], f'{engine}/{name}: exit differs'
            assert (directory / f'{name}.stderr').read_bytes() == b'', f'{engine}/{name}: guest stderr'
            if expected['exit'] == 0:
                problem = decode(raw)
                matches = re.findall(rb'TASK (\d+) START (\d+) END (\d+)\n', actual)
                assert len(matches) == len(problem[2])
                starts = []
                ends = []
                for index, (identifier, start, end) in enumerate(matches):
                    assert int(identifier) == index
                    starts.append(int(start))
                    ends.append(int(end))
                    assert ends[-1] == starts[-1] + problem[2][index][0]
                assert feasible(problem, starts), f'{engine}/{name}: resource/precedence violation'
                assert max(ends, default=0) == expected['makespan']
                assert starts == expected['starts'], f'{engine}/{name}: deterministic tie violation'
    summary = dict(fixtures=len(frozen), engines=['native', 'terminal'],
                   start_vectors=vectors, tolerance=0, oracle_sha256=digest(ROOT / 'data' / 'expected.json'),
                   result='all exact stdout, stderr, exit and scheduling checks passed')
    (output / 'validation.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('usage: validate.py OBSERVATION_DIR')
    validate(Path(sys.argv[1]))
