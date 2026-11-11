# Checked-tree interpreter

This evaluator runs build-time machines on checked and typed trees. It is
migration debt: whole programs and the corpus oracle already run on
`terminal-interpreter`, and the remaining build-time jobs move there under
`UNIVERSAL-INTERPRETER-CLOSURE`. Do not add new callers.

Start at [interpreter.rs](src/interpreter.rs)
(`evaluate_prepared_build_time_machine`). It owns worker creation, entry
selection, execution, and result/evidence handling. The crate-root exports
preserve the existing callers; `lib.rs` is module wiring.

[Semantic evaluation](../../../../wiki/spec/language/evaluation.md) and
[build execution](../../../../wiki/spec/build/execution.md) retain distinct
admission contracts. These entrypoints execute caller-admitted inputs; they do
not replace those admission checks. Constant and structured-return evaluation
retain the machine's returned value; selected operators and private-layout
receipts remain explicit inputs/outputs.

All paths run on the 256 MiB scoped-worker stack under the constant-evaluation
step ceiling. Result conversion and sponsor accounting happen before returning
the corresponding result. Only the virtual filesystem is installed: no caller
supplies host filesystem grants, clocks, standard input, boundary adapters or a
target flags environment. Missing checked service bindings never select a
different machine by name.

Follow the operation into its subordinate owner:

- [interpreter.rs](src/interpreter.rs) owns invocation-local
  runtime state (`Evaluator`); [execution.rs](src/execution.rs),
  [execution_expressions.rs](src/execution_expressions.rs),
  [execution_scalars.rs](src/execution_scalars.rs),
  [program_values.rs](src/program_values.rs), and
  [build_machine.rs](src/build_machine.rs) bind entries, execute machines, and
  recover values from interpreter storage.
- [filesystem.rs](src/filesystem.rs) owns path validation, metadata layouts,
  operation-attempt vocabulary, call preparation, and the virtual provider.
- [evaluation.rs](src/evaluation.rs) owns measured work, observations,
  result/failure records, and build-evaluation sponsor/usage exports. Those
  records do not grant execution authority.

Focused coverage is in `tests/`, with filesystem preparation, output custody,
and sponsor unit tests beside their implementations.
