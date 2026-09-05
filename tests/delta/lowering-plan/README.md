# Delta lowering-plan gate

Run `sh tests/delta/lowering-plan/run.sh` from the repository root. The private
Gamma prefix invokes production `prepare_admitted_source(0)` with raw Delta
source. The complete frontend and expanded-Gamma planner finish before it
publishes any diagnostic bytes. It reports each authored function body's
height as unsigned little-endian u32, in declaration order, followed by the
unmarked evaluator's final scalar byte `00`. This is not DCREQ, DCOUT, or a
generated application envelope.

The 13 authored controls pin expression-list height: atoms have height zero;
applications and lets add one to their maximum expression-child height.
Function signatures, call heads, and binder/type atoms add no levels. Fixtures
cover ordinary calls, let bodies, branching, checked arithmetic guards, raw
arithmetic, constructor product spines, payload projections, and nested mixed
payload matches. The three-let checked arithmetic expansion has height 7.
Nested right additions reach 265, a 128-binder payload match reaches 258, and
ordinary calls reach 1,023 at the admitted Delta expression-depth boundary.

Expected heights are manually derived fixture facts. The host only constructs
the authored bytes, frames execution, and compares the diagnostic bytes; it
does not parse Delta or Gamma, lower expressions, or compute their heights.
The diagnostic compiler includes the complete declared source closure and its
prefix under one pinned identity in `compiler.tsv`.

Heights above 255 deliberately remain observable here. This gate establishes
expanded-plan construction and measurement, not a normalization transform,
successful execution of those deep Gamma programs, resource conformance, or
closure of the Delta bootstrap edge. Existing byte receipts and executable
emitter regressions remain owned by the [staged gate](../staged-compiler/README.md).
