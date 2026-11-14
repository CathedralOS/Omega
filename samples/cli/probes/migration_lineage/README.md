# migration_lineage

A format package selects checked migration machines through a nominal
lineage parameter: `satisfies FormatMigration<CounterDisk, CounterV1,
Counter>::migrate` binds the machine to the disk lineage's migrate
requirement while the same old/new pair under `CounterNetwork` gets its
own non-aliased requirement identity.

## Learnings

- `use omega::language::core::format_lineage` + `Build::dependency(builder, dependency::from_source(Source::Path{location: "../../../../source/library/std"}))` — the
  std-package path dependency route.
- Package review staleness ("accepted package compiler identity
  changed"): `mbx run` rebuilds the compiler binary on sibling commits,
  invalidating a just-run `omega packages update`. Invoke `target/debug/omega`
  directly after `mbx build -p omega` for update→check pairs.

Compile-checked only: path-depends on `source/library/std`; run a
check target, not native.
