# Delta tests

This directory owns retained Delta compiler comparisons and the experiments
that led to typed scalar/effect Gamma. The selected Delta compiler remains open.

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `staged-compiler/` | Exercises selected source-envelope, lexical-atom, global-census, recursive multi-field, rope, exhaustive-match, malformed-source, and scale stages through the selected Gamma evaluator. | Extend with each admitted Delta stage and replace with complete Delta conformance. |
| `emission/` | Compares source-owned Gamma-plan extents, cached unary publication, fallback, ordering, and capture reconstruction against exact bytes. | Retain while the selected compiler owns this private serialization cache; replace when complete conformance directly covers the same invariants. |
| `internal-boundary/` | Exercises retained typing-continuation contradictions through actual dispatch and canonical failure publication, with valid and malformed companions. | Replace when complete compiler invariant conformance directly covers these owned internal rows. |
| `state-machine-experiment/` | Exercises the speculative source-owned typed state-machine compiler across nominal data, fixed storage, states, exhaustive transitions, calls, and direct Alpha emission. | Delete after its evidence is superseded by a representative canonical Delta compiler comparison. |

Run the focused serializer gate with `sh tests/delta/emission/run.sh`; its
[README](emission/README.md) distinguishes private representation controls from
admitted Delta sources and executable Gamma programs.

Completed direct-Beta topology comparisons are recorded in
[D88](../../wiki/pre_migration/architecture/bootstrap_chain/decisions.md#d88--typed-functional-gamma-is-evaluated-directly-by-beta)
and [D92](../../wiki/pre_migration/architecture/bootstrap_chain/decisions.md#d92--matched-direct-delta-does-not-displace-minimized-gamma).
Their prototypes are not maintained as alternate implementations. Selected
recursive-data, malformed-source, proper-tail, scale, and reordered-match
coverage belongs to `staged-compiler/`; the selected evaluator and augmentation
route are checked under `tests/gamma/`. See the
[retirement boundary](../../wiki/pre_migration/design_briefs/bootstrap_cost_review.md#completed-topology-discriminators)
for the coverage mapping and remaining legacy dependencies.

The owner-retired Forth-Gamma experiment and its unused symbolic-label resolver
are also removed. D93/D94 retain its historical comparison; the
[Forth retirement review](../../wiki/pre_migration/design_briefs/bootstrap_cost_review.md#forth-experiment)
maps useful behavior to the selected tests without retaining the alternate dialect.

The scalar direct-Alpha and streaming compiler prototypes are retired as well:
D80/D82/D88 retain their architectural findings, and their shared recursive and
scalar-surface sources now belong to `staged-compiler/`. See the
[scalar retirement review](../../wiki/pre_migration/design_briefs/bootstrap_cost_review.md#scalar-compiler-experiments)
for coverage and the remaining concatenative dependencies.

The concatenative compiler-slice gate and schema-specific elaborator are retired.
Its mutual-tail and nested-match sources run in `staged-compiler/`; the two
concatenative receipts still used by the Gamma-to-Beta comparison now live with
that consumer. The [slice retirement review](../../wiki/pre_migration/design_briefs/bootstrap_cost_review.md#concatenative-delta-compiler-slice)
maps the remaining assertions. The now-unconsumed concatenative Delta compiler
and helper retire with that gate; legacy Gamma compiler/evaluator tools still
have other consumers.
