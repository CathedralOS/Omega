# offset_integer_at

An unaligned `FieldPlan::IntegerAt` placement: `offset: 1` with
`stored_width: 16` inside a `size_fixed: 4, align: 1` plan — the encoded
field straddles byte 1..2 of the word.

Compile-checked only: `use omega::language::core::layout` is depend-free.
