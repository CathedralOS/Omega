# mut_field_subview

A mutable sub-range view of a field array lends `&mut [u8]` to a
callee that writes through the slice; when the view dies the owner
reads the writes at their original coordinates.

## Gaps found while authoring (filed)

- Whole-collection `&mut xs[..]` element writes stay corpus-only: `exits[index] = v`
  on the local `&mut [T]` reports "assignment target must be a named
  place" — the specialized `Slice::index` write
  path is private (same class as `Slice::range`/`Slice::index` reads).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
