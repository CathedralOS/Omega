# foreign_byte_recast

`use omega::language::core::{external_binding, layout}` — a plan-laid
`SignedByte<PortableByte>` record view recasts to the plain byte-shaped
`ForeignByte` ABI that `bzero`/`memcmp` bindings consume:
`reaches CompareByte + ClearByte` unions two service ceilings, and
`as &ForeignByte` / `&mut value.value as &mut ForeignByte` carry the
recast through shared and mutable member projections. `FieldPlan::IntegerAt`
with `stored_width: 8, interpretation: Signed` forces the byte width past
the field's natural 64 bits.

## Gaps found while authoring (filed)

- `value as &mut ForeignByte` on a `&mut RawView` parameter is refused
  ("requires mutable source access") — the member-projection route
  `&mut value.value as &mut ForeignByte` is required; same class as the
  `encoded_recast_views` second-mutable-recast gap, now confirmed on a
  call-parameter source.

Compile-checked only: `core::external_binding`/`core::layout` are depend-free.
