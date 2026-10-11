# build_configuration

The `builder` value exposes authored configuration as writable fields
that compose independently on a freestanding application:

- `subsystem` — all four cases write cleanly, including
  `Unspecified { value: u16 }` (payload cases use record-literal
  braces; `Subsystem::Unspecified(7)` call-style rejects — see
  `invalid_subsystem_call`).
- `identifier` — the macOS application identity / CFBundleIdentifier.
- `elf_interpreter.linux_x86_64` — the exact `PT_INTERP` pathname.
- `privileged_services.port_io` / `.interrupt_table` — granular
  privileged-service admissions that compose independently.
- `x86_deployment_features` — `Baseline` or `AvxFma3`.
- `optimizations` — the per-optimization counter roster;
  `Optimizations::enable(&mut builder.optimizations,
  Optimization::<CaseName>)` selects a case from the shared 55-name
  vocabulary and `Optimizations::emit_report` bumps `human_report`.
  Build-authored enables are distinct from the CLI's
  `--disable-optimization` rollback (which rejects every name under
  `--check` because Check executes no optimization stage).

Controls:

- `invalid_optimization_case`: `Optimization::NoSuchOpt` rejects
  "`Optimization` names a declaration, not storage" — the consistent
  shape for a `Data::Case` reference whose case does not exist
  (identical to `CrashCause::<unknown>`).
- `invalid_subsystem_call`: `Subsystem::Unspecified(7)` call-style
  rejects "value call `Subsystem.Unspecified(..)` does not resolve to a
  callable machine entry or requirement" — payload construction is
  record-literal only.
- `invalid_identifier_charset`: `builder.identifier` validates its
  byte vocabulary — `A-Z a-z 0-9 . -` only; an underscore rejects at
  build-time Build validation ("not a valid application identifier").

- `invalid_fabricated_inputs`: an authored `BuildInputs {}` value
  passed to `BuildInputs::get` rejects "named build inputs require a
  compiler-issued facet" — fabricating the compiler-owned facet shape
  grants no input access.
