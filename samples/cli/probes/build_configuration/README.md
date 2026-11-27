# build_configuration

This probe writes application configuration through the `builder` value.

- `subsystem` accepts all four cases, including `Unspecified { value: u16 }`.
  Use a record literal for a case with a payload.
- `identifier` sets the macOS `CFBundleIdentifier`.
- `elf_interpreter.linux_x86_64` sets the exact `PT_INTERP` pathname.
- `x86_deployment_features` selects `Baseline` or `AvxFma3`.
- `Optimizations::enable(@mutable builder.optimizations, Optimization::<CaseName>)`
  selects an optimization from the shared 55-name vocabulary.
  `Optimizations::emit_report` increments `human_report`. Check mode rejects
  `--disable-optimization` because it executes no optimization stage.

The selected target and provider contracts determine which hardware operations
each output can use.
[Build permissions](../../../../wiki/spec/build/permissions.md#where-the-operation-can-run)

Package admission currently stops at `no product route yet: selected Psi
optimizations have no second lowered-form check`. The probe exercises the two
selected optimizations and their Build settings.

## Rejection controls

- `invalid_optimization_case` tries `Optimization::NoSuchOpt` and receives
  "data `Optimization` has no member `NoSuchOpt`".
- `invalid_subsystem_call` tries `Subsystem::Unspecified(7)` and receives
  "this call names a case of a sum, and a case is not a machine".
- `invalid_identifier_charset` uses an underscore in `builder.identifier`.
  Build accepts letters, digits, dots, and hyphens. The control receives
  "not a valid application identifier".
