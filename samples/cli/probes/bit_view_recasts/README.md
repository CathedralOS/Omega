# bit_view_recasts

First sample with equal-width integer re-views: `&i32 as
&u32` modular image, inline `&i8 as &u8` in a guard
operand, `&bytes[offset] as &i16` little-endian assembly
at a runtime offset, and recasts nested inside binary
expressions in both `&` and `&mut` spellings (no
reference-typed let binding needed).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
