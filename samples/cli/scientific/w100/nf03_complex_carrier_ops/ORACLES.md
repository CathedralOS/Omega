# Frozen NF03 mathematical oracles

Frozen before any NF03 execution, 2026-10-10 UTC. Exact binary fixtures
use equality (zero tolerance); no test threshold is fitted to a run.

Phasor power (F64): voltage 3+4i, current -2+0.5i. Conjugate current
-2-0.5i, voltage times conjugate current -4-9.5i, sum 1+4.5i,
voltage squared norm 25, real-only voltage 7+0i squared norm 49.
Inputs must remain unchanged after all borrowed calls.
Scaled magnitude of 3e200+4e200i is 5e200: accept [4.99999999999999e200,
5.00000000000001e200] (relative tolerance 2e-15, absolute tolerance zero).

Quadrature rotation (F32): rotate 1+0i by 0+1i four times, obtaining
0+1i, -1+0i, 0-1i, 1+0i; sum of four rotated points 0+0i.
Squared norm at each point is 1. Exact equality, zero tolerance.
Scaled magnitude of 3e20+4e20i is 5e20: accept [4.999999e20,
5.000001e20] (relative tolerance 2e-7, absolute tolerance zero).
Zero magnitude is exactly zero. NaN/infinity inputs must return
NonFiniteInput; finite maximum pairs whose magnitude overflows must return
Overflow. Direct squared norms of the large pairs must be nonfinite.

Invalid control: the same carrier is passed as mutable destination and
shared input during one add call. Intended rejection is overlapping
mutable/shared custody, never a numerical-oracle failure.

Each valid project exits 0 only when its checks succeed; otherwise 1.
No output text is required.
