# Computed nominal constants

The record's integer and Boolean fields use ordinary compile-time scalar
evaluation in their declared carriers. Anonymous `7 / 2 * 2` lands as `7u64`;
the later typed multiplication produces `14u64`. The direct field read keeps
the selected constructor and field identity; it does not invent constant storage.

```sh
cargo run -p omega -- --check tests/omega/pass/modules/computed_nominal_constants/main.omg
cargo run -p omega -- inspect-terminal --machine read tests/omega/pass/modules/computed_nominal_constants/main.omg
cargo nextest run -p compiler --test module_machine_indices --no-fail-fast --no-tests fail -E 'test(computed_nominal::)'
```

The integration checks execute the published artifact after dropping source
state and compare a computed nominal index with its literal equivalent under
module-local declaration selection. Evaluator tests cover nested records,
record arrays, case payloads, unused invalid fields, and changed evaluation
receipts. Scalar source replay rejects altered fields, constructors, values,
source bindings, and unselected calls.

This does not establish machine-call or aggregate-producing initializer
evaluation, open generic carriers, or native execution of nominal values.
