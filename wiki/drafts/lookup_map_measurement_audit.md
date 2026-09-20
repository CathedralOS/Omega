# Lookup-map measurement audit

Companion measurement note to
[lookup_map_justification.md](lookup_map_justification.md) (that audit's census:
no name-keyed declaration-lookup map exists beside the scoped `SymbolTable`;
every retained name-keyed map is a substitution environment, evaluation frame,
canonical-identity rejoin, package registry, or diagnostic index). This note
measures the cardinalities that the "extra lookup maps require a measured
reason" clause ([pipeline.md](../../omega-rust/pipeline.md)) actually gates on:
a map needs justification only when its key space or hit rate makes a linear
scoped walk measurably wrong. Measured at `03be2ee302`; re-measured at
`dcb807e190` on linux x86-64 (counts below updated). Since `976e05f74b8` the
census is also mechanically enforced: `tests/architecture/scoped_lookup_maps.rs`
fails on any production `HashMap`/`BTreeMap` keyed by an authored-spelling token
that is absent from its `JUSTIFIED_LOOKUP_MAP_FILES` catalog, and on stale
catalog rows.

## Scoped-tree key spaces (what a map would index over)

Per-scope declaration counts bound every lookup key space: a `SymbolTable`
lookup scans one parent's child list, so the binding bound is per-scope
fan-out, not corpus-wide declaration volume. Counted at `dcb807e190`
(column-0 `machine`/`data`/`trait`/`define`/`proposition`/`const`/`measure`
declarations, `pub` included):

- `source/library/` (the whole core+std package): 784 declarations across
  88 files — largest single file 171 (`core/numeric_conversion.omg`).
  (The earlier figure, 241 / largest 42 (`core/nat.omg`), under-counted:
  the library had already grown past it at `03be2ee302`. Per-scope key
  spaces stay bounded by member fan-out below, not file totals.)
- `samples/cli/games/dungeon_crawler_cli/`: 139 declarations in 28 files —
  the largest multi-file maintained sample.
- Largest machine member fan-out: 203 members
  (`samples/gui/windowed_calculator/main.omg`); typical ≤ 30 (median 5).

`SymbolTable` resolves a reference by scanning its parent's child list
(`child_handles` + name compare) — worst case ≲ 200 comparisons at the
observed corpus extremes, median ≲ 40. A hash map saves nothing at these
cardinalities and adds build-phase allocation; the measured reason the
policy demands does not exist for declaration lookup.

## Retained-map key spaces (already justified)

- **Generic substitution maps** (`preparation/generic_data/*`,
  `trait_defaults.rs`, `type_equations.rs`): key space = one declaration's
  parameter list. Maximum observed authored arity in `source/library` +
  `tests/omega`: **3 parameters** per generic header. A per-application map
  over ≤3 keys is the scoped environment itself.
- **Evaluation frames** (`checked-interpreter`, `contract_entailment`,
  `build-time-evaluation` layouts): key space = one activation's live
  binders — bounded by the frame's own authored locals, again ≤ tens.
- **Identity rejoins / package registries**
  (`terminal_artifact/behavior_exclusions.rs` identities map,
  `package-compilation` dependency maps): key space = the module's provider
  candidates / declared dependencies — bounded by authored rows, and the key
  domain (normalized identities, package names) is not a scoped-name space
  at all, so the symbol tree cannot serve it regardless of size.

## Conclusion

Measured cardinalities confirm the justification audit: every name-keyed map
keyspace is bounded by a single declaration, activation, module, or package —
≤ ~240 keys observed anywhere — and no declaration-lookup map duplicates
scoped symbol-tree authority. A future map needs a measured reason only when
its key domain is unbounded (cross-program or cross-corpus indexes); none
exists today.
