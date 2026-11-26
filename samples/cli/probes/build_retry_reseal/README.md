# Retry a promised output

The dependency writes source bytes to a promised file and tries to complete it
while the writer is open. Build returns `OutputCompletion::Retry` with the
same obligation and file. The dependency closes the writer, publishes the
captured bytes as `generated.omg`, then completes the file. This time Build
must return `Sealed`.

The executable calls the dependency’s handwritten `main.omg`, which imports
`generated.omg` by its logical filename. The generated machine in turn calls
the handwritten `seed.omg`. The executable checks that the complete path returns 7.

The [refusal controls](../build_retry_reseal_controls/) are separate projects.

- `invalid_build_imports_its_generated` imports the future Product file into
  the Build that would emit it. Build source is fixed before execution.
- `invalid_foreign_include` tries to publish a file outside the dependency.
- `invalid_path_as_source_bytes` passes a borrowed filesystem handle where
  source publication requires a byte slice.

The `build_package_custody` sample also has a control that calls `fail` after
Build returns `Retry`.
