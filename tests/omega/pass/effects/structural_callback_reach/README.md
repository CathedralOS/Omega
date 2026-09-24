# Linear structural callback reach

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/structural_callback_reach/main.omg
cargo run -p omega -- inspect-terminal --machine Main::demand tests/omega/pass/effects/structural_callback_reach/main.omg
```

The source checks and publishes a verified Terminal module.
Ordinary statement sequencing binds the selected callback's result, transfers it
into a mixed scalar/linear call and forwards that call's result again. Scalar
helper invocations before and after the owned argument retain authored order
through ordinary computation continuations. Each structural handoff
preserves the exact `Region::Owned` qualification and input-origin claim at a
distinct result place. The demanding wrapper uses the same transitive call closure
as its selected callback.

The source-free tests cover publication, reload and identity-preserving execution,
including a scalar binding before the callback, direct and named completion,
the exact nominal reach application, and rejection of altered call coordinates,
same-shaped targets, qualifications,
result bindings, duplicate moves, source claim events and returned-claim transfers.
Mixed-call controls reject reordered pure or computed source operands, a claim
indexed by authored rather than structural position, missing or mistyped scalar
operands, and scalar values used before their defining operation.
The consumed input cannot substitute for the live callback result. They also retain
the direct const-generic callee regression:

```sh
cargo nextest run -p checked-trees-to-lowered-psi --test structural_return_source service_reach --no-fail-fast --no-tests fail
```

Custody reconstruction lives in `validation/src/structural_call_custody.rs`;
ordinary binding and completion belong to
`04_typed-trees-to-checked-trees/src/execution/unit/control/statement_sequence.rs`.
Whole-root linear input-origin forwarding uses the existing `CallStructural`
and `CallStructuralWithScalarArguments` operations. Both encodings share claim
and content verification; scalar operand collection does not depend on the
enclosing call's result category. Source admission in
`validation/src/calls/expression_scanning/result_realization.rs` uses one owned
result classifier for the containing state, immutable local and exact callee;
it does not reject a named result merely because the caller returns ownership.
A consumer's content projection comes from the producing callee's
verified normal-return identity guarantee, not a matching claim ID. Stale and
future result places and missing content guarantees reject in module validation,
independently of proof-bundle identity.

`projected.omg` carries a whole `[Region in Owned; 2]` through the same callback,
named local, mixed scalar/structural call and ordinary forward:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/structural_callback_reach/projected.omg
cargo run -p omega -- inspect-terminal --machine Main::demand tests/omega/pass/effects/structural_callback_reach/projected.omg
cargo nextest run -p checked-trees-to-lowered-psi --test structural_return_source --no-fail-fast --no-tests fail -E 'test(projected_)'
```

Each indexed claim keeps its entry identity, qualification and content path.
The source-free tests vary the array length through one, two and three claims;
they reject changed semantic input/output paths, entry/transfer identity swaps,
incomplete claim sets, invalid result paths, dropped qualifications and missing
callee content guarantees. This forwards a whole aggregate; it does not extract
an indexed result or establish fresh claims. Those cases and returning claims
from distinct owned inputs require further implementation. Native execution is not established
by these source-free interpreter checks.
These checks do not establish generic PCC or independently authenticated original
contract projections.

`projected_three.omg` carries the same whole-aggregate callback reach over
`[Region in Owned; 3]`, pinning the source-level join at three indexed claims:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/structural_callback_reach/projected_three.omg
cargo run -p omega -- inspect-terminal --machine Main::demand tests/omega/pass/effects/structural_callback_reach/projected_three.omg
```

Measured frontier at the commit that added `projected_three.omg`: packing two
distinct owned `Region` inputs into `[Region in Owned; 2]` and forwarding the
array through `Selected` still fails Terminal production with
`InvalidUnitMachinePlan` ("`Main::demand` has no admitted body (local
construction stopped at result type)"), and extracting `forwarded[0]` while the
`[1]` sibling stays live rejects at source check with the linear
reaches-scope-exit diagnostic. Both remain upstream implementation work in
`typed-trees-to-checked-trees`, not test-only gaps.
