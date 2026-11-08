# Checked-tree interpreter

Start at [interpreter.rs](src/interpreter.rs). It owns worker creation, entry
selection, filesystem authority setup, execution, and result/evidence handling.
The crate-root exports preserve the existing callers; `lib.rs` is module wiring.

This Psi-owned source interpreter supports build-time evaluation and differential
execution. Canonical portable execution belongs to `terminal-interpreter`.
[Semantic evaluation](../../../../wiki/spec/language/evaluation.md) and
[build execution](../../../../wiki/spec/build/execution.md) retain distinct
admission contracts. These entrypoints execute caller-admitted inputs; they do
not replace those admission checks.

The lifecycle keeps distinct result and authority paths:

- `interpret_entry` executes a checked entry and returns process
  output, an exit or error, and measured work.
- Constant and structured-return evaluation retain the machine's returned value;
  selected operators and private-layout receipts remain explicit inputs/outputs.
- Observed argument evaluation returns final argument snapshots and build-log
  observations under the pure build policy and optional resource sponsor.
- Granted argument evaluation additionally configures explicit filesystem
  authority and retains partial work/observations on evaluator failure.

All paths retain the existing 256 MiB scoped-worker stack. Pure evaluation uses
the constant-evaluation step ceiling; granted execution uses its full step
ceiling. Result conversion and sponsor accounting happen before returning the
corresponding result. Missing checked service bindings never select a different
machine by name.

Follow the operation into its subordinate owner:

- [interpreter.rs](src/interpreter.rs) owns invocation-local
  runtime state (`Evaluator`); [execution.rs](src/execution.rs),
  [execution_expressions.rs](src/execution_expressions.rs),
  [execution_scalars.rs](src/execution_scalars.rs),
  [program_values.rs](src/program_values.rs), and
  [build_machine.rs](src/build_machine.rs) bind entries, execute machines, and
  recover values from interpreter storage.
- [filesystem.rs](src/filesystem.rs) and
  [filesystem_real.rs](src/filesystem_real.rs) own path authority, metadata
  layouts, operation-attempt vocabulary, call preparation, and the virtual and
  physical providers.
- [evaluation.rs](src/evaluation.rs) owns measured work, observations, included
  sources, result/failure records, build-evaluation sponsor/usage exports, and
  virtual clock bindings. Those records do not grant execution authority.

Focused coverage is in `tests/const_values.rs`, `tests/build_arguments.rs`, and
`tests/resolved_state_execution.rs`, with filesystem preparation, output custody,
and sponsor unit tests beside their implementations.
