# Module File Chain

Depend-free compile-scope probe: directory-module file imports — the first
sample spanning multiple source files under one package.

- `chain/` is a directory module: each file is imported by its own
  `use chain::<file>` spelling, and only `pub` declarations cross the
  file boundary — nothing is re-exported implicitly.
- `chain/right.omg` imports `chain::left` for its field type; the root
  imports both files for its own names.

The machine builds `Right { value: Left { value: 7 } }` across the file
boundary, reads through both levels, and accepts on 7. Compile-checked
only (no std dependency):

```sh
mbx run -p omega -- --check samples/cli/probes/module_file_chain/main.omg
mbx run -p omega -- update --project samples/cli/probes/module_file_chain
```

Both green on Linux x86-64 (18 source files: 16 core + left + right);
`omega.lock` publishes locally, ignored.
