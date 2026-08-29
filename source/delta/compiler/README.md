# Delta compiler

This directory owns the canonical Delta-written compiler source. `main.delta` is
the current physical path of the single translation unit consumed by the
lower-rung publication route; it is a compiler input, not a sample or a
separate bootstrap bridge.

D10 retired `.alp` for Delta and selected `.delta`. The atomic migration
updated the locations companion and every consumer while preserving the exact
source bytes and path-independent closure identity. Delta may share source
spelling with Omega, but its meaning is self-contained and independent of both
Alpha and Omega. Splitting the translation unit into ordinary Delta modules is
permitted once the language and source-closure rules define that module
surface; the closure manifest, rather than concatenation in a runner, must own
the resulting source graph.

No unbound compiler executable is checked in here. After the exact lower-rung
producer edge and custody receipt close, the admitted Darwin ARM64 result has
one six-file installation under `artifacts/darwin-arm64-v1/`: the compiler,
the assembly-publication receipt, realization observation, artifact-custody
receipt, one canonical raw execution, and a non-authoritative installation
inventory. `artifact_env.sh` exposes that fixed installation and fails when it
is absent; it never rebuilds or substitutes an ambient compiler.

[`validation/`](validation/) owns the exact source-closure, publication,
artifact-custody, install, and reconstruction checks adjacent to the compiler
they admit. The active work order is in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
