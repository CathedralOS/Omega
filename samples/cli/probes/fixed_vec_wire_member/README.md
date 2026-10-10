# fixed_vec_wire_member

`#1 samples: FixedVec<i32, 4>` — `use omega::language::core::fixed_vec`
gives the wire codec a length-prefixed element member; encode writes the
run + count, decode rebuilds the same `{ items, length }` shape.

Compile-checked only: `core::fixed_vec` is depend-free.
