# float_match_patterns

`use omega::language::core::float_operations` — floating match subjects
retain IEEE comparison meaning across subject shapes (projected record
field, computed `a * b`) in both `f32`/`f64`. A parameter name used as a
pattern means "subject equals this value"; `_` catches the rest.

Compile-checked only: `core::float_operations` is depend-free.
