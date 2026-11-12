# slice_core_surface

`use omega::language::core::slice` imports the slice surface:
`&main.entries[..]` borrows a field array as `&[T]` and `.len` reports the
runtime length.

## Gaps found while authoring (filed)

- `view[1..3]` / `view[4..4]` range slicing selects PRIVATE
  `Slice::range$specialized...` machines — corpus fixtures compile the
  full surface but user packages get 'selects private machine' (same
  corpus-vs-package class as whole-collection view element ops and
  `Slice::index`).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
