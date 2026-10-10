"""Explicitly copy the owned kernel into standalone project source directories.
Temporary staging avoids inventing NF01-owned package metadata. Replace imports
with the admitted shared package once NF01 supplies its package signature.
"""
from pathlib import Path
import hashlib
root = Path(__file__).resolve().parent
repository = root.parents[4]
source = repository / 'source/library/numerics/integration/fixed_step_rk4_interface.omg'
payload = source.read_bytes()
for name in ['decay', 'driven_growth', 'invalid_rhs']:
    target = root / name / 'rk4.omg'
    target.write_bytes(payload)
    assert target.read_bytes() == payload
print('staged canonical kernel SHA256', hashlib.sha256(payload).hexdigest())
