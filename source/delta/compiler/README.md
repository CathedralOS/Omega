# Delta compiler

This directory owns the canonical Delta-written compiler source. `main.alp` is
the current single translation unit consumed by the lower-rung publication
route; it is a compiler input, not a sample or a separate bootstrap bridge.

The historical `.alp` suffix is retained until Delta v1's independent source
contract and file convention are ratified. Its spelling does not make Delta an
Alpha or Omega subset. Splitting the translation unit into ordinary Delta
modules is permitted once the ratified language and source-closure rules define
that module surface; the closure manifest, rather than concatenation in a
runner, must own the resulting source graph.

No unbound compiler executable is checked in here. A published artifact belongs
under `artifacts/` only after its exact lower-rung producer edge and custody
receipt exist. The currently active publication machinery remains temporarily
at `source/delta/` while its exact run is live; once cold, it moves together
under `validation/`. The active work order is in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
