# Beta executable semantic reference

This directory owns an untrusted executable semantic reference for the Beta
rung. Canonical Beta meaning is the written
[`../SEMANTICS.md`](../SEMANTICS.md). `beta_parser.py` recognizes the Beta
source surface and `beta_interp.py` executes its tuple AST. Fuzz and
exhaustive-I/O gates compare that finite executable observation with programs
produced by the canonical Alpha-written Beta compiler.

The owner contains no compiler backend and imports no refinement machinery.
Its comparisons are diagnostics, not artifact authority. Acceptance of a
compiler artifact still requires the lower-rooted refinement edge described by
the bootstrap lattice.

This Python owner is strictly temporary development scaffolding. It is not
eligible for permanent retention after the checked direct Beta edge subsumes
the bounded diagnostics below, and it is never a prerequisite of the
self-contained bootstrap. While retained, it must consume raw bytes and obey
the exact bootstrap textual-ASCII source envelope rather than Python Unicode or
locale predicates.

Run `ownership-test.sh`, `beta-correctness-fuzz.sh`, and
`beta-io-exhaust.sh` from any working directory.

## Deletion conditions

| Retained files | Bounded role | Deletion condition |
| --- | --- | --- |
| `beta_parser.py`, `test_beta_parser.py`, `ownership-test.sh` | One independently implemented parser and its focused ownership/unit gate. | Delete when direct checked Beta semantics subsumes parser failure detection. |
| `beta_interp.py`, `beta-fuzz-gen.py`, `beta-correctness-fuzz.sh` | One executable semantic reference and deterministic compiler differential. | Delete when direct operational refinement subsumes the observed source shapes. |
| `io-verify.py`, `io-fuzz-gen.py`, `beta-io-exhaust.sh` | Exhaustive bounded byte-I/O comparison. | Delete when the exact I/O observation proof covers the same finite boundary. |
