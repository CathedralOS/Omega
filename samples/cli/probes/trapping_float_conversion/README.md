# trapping_float_conversion

First sample on namespace float-to-int conversions:
`I32::from_f64(v)` / `U8::from_f64(v)` return
`i32 in Trapping` / `u8 in Trapping` — a NaN or
out-of-range operand admits a conversion trap instead of
a normal result; `converted as i32` erases the domain.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
