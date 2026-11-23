# Retry a promised output

The dependency writes source bytes to a promised file and tries to complete it
while the writer is open. Build returns `OutputCompletion::Retry` with the
same obligation and file. The dependency closes the writer, publishes the
captured bytes as `generated.omg`, then completes the file. This time Build
must return `Sealed`.

The executable imports `value()` from the dependency’s `generated.omg`. That
machine calls `seed()` in the dependency’s authored source. The executable
checks that it returned 7.

The current compiler refuses these three controls.

- `invalid_authored_selects_generated` imports the generated file from
  handwritten Product source in the same dependency. Its expected refusal
  predates the [Product source view](../../../../wiki/spec/build/execution.md#generated-source-boundary).
  Under that rule the Product import succeeds. The running Build still cannot
  import source it generates for the Product.
- `invalid_foreign_include` tries to publish a file outside the dependency.
- `invalid_path_as_source_bytes` passes a borrowed filesystem handle where
  source publication requires a byte slice.

The `build_package_custody` sample also has a control that calls `fail` after
Build returns `Retry`.
