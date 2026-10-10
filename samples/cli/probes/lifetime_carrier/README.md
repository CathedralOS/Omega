# lifetime_carrier

First sample with lifetime-parameterized data
(`Carrier<'storage>` holding a `&'storage i32`), lifetime
pass-through signatures on `transform`/`repair`, and a whole
extract-and-restore repair through `&mut`.

## Learnings

`'_` is not spellable in a local annotation — the lifetime
must be declared on a generic worker machine
(`run<'storage>`) the entry delegates to; `let` requires an
explicit `: Type` here, inference alone doesn't bind it.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
