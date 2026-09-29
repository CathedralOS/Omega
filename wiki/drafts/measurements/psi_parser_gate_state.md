# The Omega-written parser gate does not check

`omega --check source/psi/gates/parser/main.omg` is named as acceptance on at
least one board item, and convergence on it is asked for by name. Measured
2026-09-29 on `7102f6917ea`, it fails:

```
cannot check prepared project: checked compilation failed for package
`psi_parser_gate` with 1 diagnostic(s)
  error: selected ProgramEntry establishment rejoins 0 Terminal attachment
  identities; expected one; the machine's unit plan was omitted at an
  unavailable callee (`Parser::parse`), which was itself omitted at local
  construction at `structural field store: pure source: scalar expression row`
  (state 3, statement 1)
```

This is the product compiler -- `source/psi` plus `source/omega` are the
destination the Rust reference producer exists to reach -- so a red gate there
is worth more attention than a red sample. Nothing in `wiki/` or `tools/`
records it.

## Why this blocks the static migration here too

`source/psi` holds **720** receiver occurrences across eight files, the largest
concentration outside the corpora, and `source/omega/main.omg` holds ten. Every
operation in the smaller files reads its receiver, so migrating them means
naming the data and spelling the argument at each call: `self.source.byte_or_nul(index)`
becomes `SourceUnit::byte_or_nul(&self.source, index)`.

That is a shared projection handed on from inside a machine body, which the
[structural-argument transport](static_migration_structural_home.md) still
refuses. And with the gate already red, a migration there cannot be verified by
the gate the board names: the only available check is that the diagnostic is
unchanged, which distinguishes "did not make it worse" from "is correct".

So the product compiler waits on the same transport as everything else, and on
its own `Parser::parse` omission before the gate can witness anything.
