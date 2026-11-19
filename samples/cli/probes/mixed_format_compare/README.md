# mixed_format_compare

First sample on cross-format float comparison: an `f32`
operand promotes to the wider expression's format, so
`identity(16777216.0f32) == 16777216.0 + 1.0` is FALSE
(f32 plateau vs exact f64 sum), while `widen(1.5f32)`
lands exactly. Also `(calls as u64 in Wrapping) + 1` —
domain-cast + erase for wrapping counter semantics
through `&mut`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
