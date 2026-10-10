# wrapping_array_literal

`[u32; 1] in Wrapping` array literals retain the element
domain at the whole-array type: explicit `as u32 in
Wrapping` element grants, nested `[[u32; 1]; 1] in
Wrapping`, record member `values: [u32;1] in Wrapping`,
and `values[0] + 1u32` wrapping arithmetic on projected
elements (`4294967295 + 1` folds to `0`).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
