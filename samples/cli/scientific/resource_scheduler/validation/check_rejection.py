#!/usr/bin/env python3
"""Verify the distinct complete intended-invalid source rejection."""
import hashlib
import json
import sys
from pathlib import Path
sample = Path(__file__).resolve().parents[1]
prefix = Path(sys.argv[1])
record = json.loads(Path(str(prefix) + '.resources.json').read_text())
error = Path(str(prefix) + '.stderr').read_text()
expected = (sample / 'intended_invalid/expected_diagnostic.txt').read_text().strip()
assert record['exit'] == 1 and expected in error
assert 'needs an accepted program-entry binding' not in error
assert 'no executable body' not in error
print(json.dumps({'result': 'intended declared-range rejection verified',
                  'source_sha256': hashlib.sha256((sample / 'intended_invalid/invalid.omg').read_bytes()).hexdigest(),
                  'diagnostic': expected}, indent=2))
