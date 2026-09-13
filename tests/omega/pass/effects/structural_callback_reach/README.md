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
`typed-trees-to-checked-trees/src/flow/terminal_unit/control/statement_sequence.rs`.
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

Projected or freshly established returned claims and multiple transferred
linear claims require further implementation. Native execution is not established
by these source-free interpreter checks.
These checks do not establish generic PCC or independently authenticated original
contract projections.
