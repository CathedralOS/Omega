# Retry a promised output

The dependency writes source bytes to a promised file and tries to complete it
while the writer is open. Build returns `OutputCompletion::Retry` with the
same obligation and file. The dependency closes the writer, publishes the
captured bytes as `generated.omg`, then completes the file. This time Build
must return `Sealed`.

The executable imports `value()` from the dependency’s `generated.omg`. That
machine calls `seed()` in the dependency’s authored source. The executable
checks that it returned 7.

The controls cover three refusals.

- `invalid_authored_selects_generated` tries to import the generated file
  from authored source in the same dependency. The dependency’s own check and
  its consumer must both refuse that import.
- `invalid_foreign_include` tries to publish a file outside the dependency.
- `invalid_path_as_source_bytes` passes a borrowed filesystem handle where
  source publication requires a byte slice.

The `build_package_custody` sample also has a control that calls `fail` after
Build returns `Retry`.
