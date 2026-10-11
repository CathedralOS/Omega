# slice_view_whole

Whole-array reborrow into a mutable slice view: `let cells: &mut [i32 [0..=255]] = &mut main.bag.cells[..]` lends the array's contiguous storage; writes through the view are observed through the owning collection (`main.bag.cells[2]`). The `[..]` full-range reborrow also feeds a shared `&[i32 [0..=255]]` callee.

## Learnings

- `transition { _ -> expr }` arm expressions are atomic-only: identifiers, literals, member projections. Indexed elements (`cells[0]`) and binary operators (`total + observed`) must be bound to a local first.
- A `&mut [T]`/`&[T]` view carries unknown length: indexed element access needs `requires at < cells.len` (or `cells.len == 4`) on the callee, even though the `[..]` reborrowed array has statically-known extent.
- A multi-clause `transition` subject with `&&` at top level must sit on continuation lines before `{`.

Compile-checked only: run a check target, not native.
