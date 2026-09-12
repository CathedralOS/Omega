# Linear structural callback reach

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/structural_callback_reach/main.omg
cargo run -p omega -- inspect-terminal --machine Main::demand tests/omega/pass/effects/structural_callback_reach/main.omg
```

The source checks. Terminal publication remains an implementation dependency:
frontend normalization hoists the callback call into a local and returns that local.
The checked call, input-origin claim outcome and content identity reshuffle remain
present, but structural call-return planning requires a single call expression.
The demanding wrapper also needs the ordinary transitive call closure.

Reuse the structural result binding and completion joins in
`typed-trees-to-checked-trees/src/flow/terminal_unit/control/statement_sequence.rs`
and its consumers. Do not add another source-family recognizer for the hoisted
local. Acceptance is publication, reload and identity-preserving interpretation
of this same qualified linear call chain, including its exact nominal binder
reach record and independent rejection of stale call/claim evidence.

The currently supported direct call to a const-generic linear identity callee
uses the existing two-body producer and preserves that callee's closed reach
application. Its source-free execution and stale source-coordinate controls run
with:

```sh
cargo nextest run -p checked-trees-to-lowered-psi --test structural_return_source service_reach --no-fail-fast --no-tests fail
```

These checks do not establish generic PCC or independently authenticated
original-contract projections.
