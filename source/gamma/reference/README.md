# Gamma executable semantic reference

This directory owns an untrusted executable semantic reference for the Gamma
rung. Canonical Gamma meaning is the written
[`../SEMANTICS.md`](../SEMANTICS.md). `gamma_parser.py` recognizes the Gamma
source surface and `gamma_interp.py` executes its tuple AST. Fuzz and
exhaustive-I/O gates compare that finite executable observation with programs
produced by the canonical Beta-written Gamma compiler.

The owner contains no compiler backend and imports no refinement machinery.
Its comparisons are diagnostics, not artifact authority. Acceptance of a
compiler artifact still requires the lower-rooted refinement edge described by
the bootstrap lattice.

This Python owner is strictly temporary development scaffolding. It is not
eligible for permanent retention after the checked direct Gamma edge subsumes
the bounded diagnostics below, and it is never a prerequisite of the
self-contained bootstrap. While retained, it must consume raw bytes and obey
the exact bootstrap textual-ASCII source envelope rather than Python Unicode or
locale predicates.

Run `ownership-test.sh`, `gamma-correctness-fuzz.sh`, and
`gamma-io-exhaust.sh` from any working directory.

## Deletion conditions

| Retained files | Bounded role | Deletion condition |
| --- | --- | --- |
| `gamma_parser.py`, `test_gamma_parser.py`, `ownership-test.sh` | One independently implemented parser and its focused ownership/unit gate. | Delete when direct checked Gamma semantics subsumes parser failure detection. |
| `gamma_interp.py`, `gamma-fuzz-gen.py`, `gamma-correctness-fuzz.sh` | One executable semantic reference and deterministic compiler differential. | Delete when direct operational refinement subsumes the observed source shapes. |
| `io-verify.py`, `io-fuzz-gen.py`, `gamma-io-exhaust.sh` | Exhaustive bounded byte-I/O comparison. | Delete when the exact I/O observation proof covers the same finite boundary. |
