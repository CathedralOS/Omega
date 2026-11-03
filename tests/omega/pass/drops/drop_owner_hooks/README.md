# drop_owner_hooks

`use omega::language::core::drop` — `drop(g)` is the ordinary generic
consuming machine: the caller's path is proved by transfer, and the
checked cleanup edge selects the `dropped by` hook `drop_guard`. A
`proof [erased]` member carries no runtime representation, so `drop(c)`
consumes `Carrier` without requiring a `dropped by` hook on `Carrier` or running
`drop_evidence`.

Compile-checked only: `use omega::language::core::drop` is depend-free.
