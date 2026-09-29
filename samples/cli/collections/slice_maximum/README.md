# Slice maximum

This sample finds the maximum of `[3, 7, 2, 9, 4]` by tail recursion over a
shrinking borrowed slice. Its console pause also lends a local, zero-initialized
byte array to a mutable boundary call. Both operations must compose without
special treatment of this sample in the compiler.

From the repository root, with a current `omega` binary on `PATH`:

```sh
omega run --both samples/cli/collections/slice_maximum/main.omg
```

Press Enter when prompted. Both native execution and the checked interpreter
must return **9**. For unattended execution, macOS's shell can redirect stdin
from `/dev/null`; PowerShell can pipe `''` to the command. Exit 9 is the authored
result, not a compiler failure. Omit `--target` to execute on the current host.

## Remaining native dependency

The local array loan passes independent Terminal and abstract contract checks,
but target lowering still rejects the program with
`UnsupportedOperationInScalarFunction(MachineId(1))` on macOS ARM64. The
`established_source` resolver in Omega's
`02_abstract-operations-to-target-operations/src/lowering/scalar/element_views.rs`
currently locates backing only among signature parameters. It must also carry
the existing local array's structural home through view establishment and
independent native replay. The command above remains the acceptance check;
passing an internal contract test alone does not close this sample.
