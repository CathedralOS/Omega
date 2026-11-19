# Delta tests

This directory owns selected Delta compiler validation. Complete compiler
conformance and proof admission remain open.

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `staged-compiler/` | Exercises selected source-envelope, lexical-atom, global-census, recursive multi-field, rope, exhaustive-match, malformed-source, and scale stages through the selected Gamma evaluator. | Extend with each admitted Delta stage and replace with complete Delta conformance. |
| `emission/` | Compares source-owned Gamma-plan extents, cached unary publication, fallback, ordering, and capture reconstruction against exact bytes. | Retain while the selected compiler owns this private serialization cache; replace when complete conformance directly covers the same invariants. |
| `internal-boundary/` | Exercises retained typing-continuation contradictions through actual dispatch and canonical failure publication, with valid and malformed companions. | Replace when complete compiler invariant conformance directly covers these owned internal rows. |

Run the focused serializer gate with `sh tests/delta/emission/run.sh`; its
[README](emission/README.md) distinguishes private representation controls from
admitted Delta sources and executable Gamma programs.

Selected scalar, recursive-data, malformed-source, proper-tail, mutual-tail,
nested-match, scale, and reordered-match coverage belongs to
[`staged-compiler/`](staged-compiler/run.sh). The evaluator and augmentation
route are checked under [Gamma tests](../gamma/README.md). These selected
controls are conformance evidence, not proof admission for the complete edge.
