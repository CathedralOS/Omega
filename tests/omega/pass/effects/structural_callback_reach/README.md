# Linear structural callback reach

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/structural_callback_reach/main.omg
cargo run -p omega -- inspect-terminal --machine Main::demand tests/omega/pass/effects/structural_callback_reach/main.omg
```

The source checks and publishes a verified three-machine Terminal module.
Ordinary statement sequencing binds the selected callback's result, transfers it
into a subsequent ordinary call and returns that call's result. Each handoff
preserves the exact `Region::Owned` qualification and input-origin claim at a
distinct result place. The demanding wrapper uses the same transitive call closure
as its selected callback.

The source-free tests cover publication, reload and identity-preserving execution,
including a scalar binding before the callback, direct and named completion,
the exact nominal reach application, and rejection of altered call coordinates,
same-shaped targets, qualifications,
result bindings, duplicate moves, source claim events and returned-claim transfers.
The consumed input cannot substitute for the live callback result. They also retain
the direct const-generic callee regression:

```sh
cargo nextest run -p checked-trees-to-lowered-psi --test structural_return_source service_reach --no-fail-fast --no-tests fail
```

Custody reconstruction lives in `validation/src/structural_call_custody.rs`;
ordinary binding and completion belong to
`typed-trees-to-checked-trees/src/flow/terminal_unit/control/statement_sequence.rs`.
Whole-root linear input-origin forwarding uses the existing `CallStructural`
operation. A consumer's content projection comes from the producing callee's
verified normal-return identity guarantee, not a matching claim ID. Stale and
future result places and missing content guarantees reject in module validation,
independently of proof-bundle identity.

Projected or freshly established returned claims, multiple transferred
linear claims and mixed scalar/linear call arguments require further implementation.
These checks do not establish generic PCC or independently authenticated original
contract projections.
