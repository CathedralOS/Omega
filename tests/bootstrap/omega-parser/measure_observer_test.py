"""A profiler read failure must reap its real native child, not become a verdict."""
import json
import os
from pathlib import Path
import struct
import sys
import time

import measure_observer as observer

directory = Path(sys.argv[1]).resolve()
source = b'(def main () Int 42)'
request = struct.pack('<I', len(source)) + source
started = time.monotonic()
original_cursor = observer.cursor

def unavailable_cursor(process_id):
    raise ValueError('injected profiler read failure')

try:
    observer.cursor = unavailable_cursor
    try:
        observer.observe(directory, 'observer-cleanup-error', request, 30)
    except ValueError as error:
        assert str(error) == 'injected profiler read failure'
    else:
        raise AssertionError('profiler failure was accepted')
finally:
    observer.cursor = original_cursor

children = Path(f'/proc/self/task/{os.getpid()}/children').read_text().strip()
assert not children, f'profiler left native children: {children}'
assert observer.observe(directory, 'observer-cleanup-positive', request, 30) == b'*'
assert json.loads((directory / 'observer-cleanup-positive.json').read_text())['cumulative_pairs'] == 0
record = {'role': 'Profiler cleanup control, not a language rejection.',
          'injected_read_failure_raised': True, 'native_child_reaped': True,
          'positive_gamma_42_exact': True, 'wall_seconds': time.monotonic() - started}
(directory / 'observer-cleanup.check.json').write_text(json.dumps(record, indent=2) + '\n')
print('observer cleanup: injected failure reaped child; positive Gamma control passed')
