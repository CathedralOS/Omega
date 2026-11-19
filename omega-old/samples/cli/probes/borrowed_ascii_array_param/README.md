# borrowed_ascii_array_param

First sample lending a sized-array domain mutably:
`domain [u8; 8]::Ascii(bytes) requires ascii_only(bytes)`
with `&mut [u8; 8] in Ascii` params — writes through the
borrow must preserve the ASCII predicate, and the
`value < 128` guard keeps the failing arm domain-clean.

`ascii_only` is a compiler-known byte predicate (same
family as `valid_utf8`); `"ab"` folds it at compile time
to establish `[u8; 8] in Ascii`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
