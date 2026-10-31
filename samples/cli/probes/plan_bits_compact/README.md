# plan_bits_compact

A plan-laid value whose three logical fields share ONE byte:
`FieldPlan::Bits { container, container_width, destination_lsb,
source_lsb, width }` rows pack `present`/`mode`/`code` into `size_fixed:
1`, with `code` itself split across two fragments; `CompactBits::plan`
returns a bare `LayoutPlan` and `CompactBits<PackedFlags>` instantiates it.
Masked-RMW writes preserve siblings and reads reconstruct the scalar.

Compile-checked only: `use omega::language::core::layout` is depend-free.
