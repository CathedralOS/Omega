# File imports and qualified owners

The source is the language target for [file imports](../../../../wiki/spec/language/modules.md#names-and-imports).
`result()` must return 7. The package-private machine in `m/value-cell.omg`
reads the stored 7. The callable supplied in the record returns 9. Both the
record and the imported file are named `cell`, and `cell::Stored` names a case
of a data type with that same spelling.

Each file imports its own dependencies. Removing the `cell` import from
`main.omg` must fail even though `m/b.omg` imports the same file. Removing
`as cell` must also fail because the filename stem contains a hyphen.
The `numeric` import selects the bundled core file through the toolchain root.
