# as_slice_views

First sample with the `.as_slice()` view constructor:
`main.entries.as_slice()` yields `&[Entry]`; `view.len`
reads the extent. This is the complete slice-view surface
reachable from a package.

## Gaps found while authoring (filed)

Every core::slice view operator is package-blocked:
`view[i]` rejects "generic machine `Slice::index` has a
concrete specialization tuple that does not satisfy its
authored type bounds" and `view[i..j]` selects private
`Slice::range$specialized$` — same provenance class as the
Nat/TaskRuntime operator gaps. Only array-direct indexing
and `.as_slice()`/`.len` work today.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
