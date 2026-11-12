# int_literal_suffixes

Typed integer literal suffixes select the literal's primitive type:
`100i64`, `7u32`, `9usize` round-trip through storage and compare
equal to the same suffixed literal.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
