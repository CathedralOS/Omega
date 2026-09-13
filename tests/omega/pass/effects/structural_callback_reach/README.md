# Linear structural callback reach

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/structural_callback_reach/main.omg
cargo run -p omega -- inspect-terminal --machine Main::demand tests/omega/pass/effects/structural_callback_reach/main.omg
```

The source checks and publishes a verified three-machine Terminal module.
Frontend normalization hoists the selected callback call into a local. Ordinary
statement sequencing binds that result and completion returns it, preserving the
exact `Region::Owned` qualification and input-origin claim. The demanding wrapper
uses the same transitive call closure as its selected callback.

The source-free tests cover publication, reload and identity-preserving execution,
including a scalar binding before the callback, the exact nominal reach application,
and rejection of altered call coordinates, same-shaped targets, qualifications,
result bindings, source claim events and returned-claim transfers. They also retain
the direct const-generic callee regression:

```sh
cargo nextest run -p checked-trees-to-lowered-psi --test structural_return_source service_reach --no-fail-fast --no-tests fail
```

Custody reconstruction lives in `validation/src/structural_call_custody.rs`;
ordinary binding and completion belong to
`typed-trees-to-checked-trees/src/flow/terminal_unit/control/statement_sequence.rs`.
Whole-root linear input-origin forwarding uses the existing `CallStructural`
operation. Projected or freshly established returned claims, multiple transferred
linear claims and mixed scalar/linear call arguments require further implementation.
These checks do not establish generic PCC or independently authenticated original
contract projections.
