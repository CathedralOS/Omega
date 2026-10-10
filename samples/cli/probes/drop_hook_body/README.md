# drop_hook_body

`use omega::language::core::drop` — the owner hook body is an ordinary
Unit machine: it reads and writes its subject, calls helpers with
arguments, repeats calls, and reaches nested helpers, on both the
contextual scope-exit edge and an explicit `drop(explicit)` consume.

Compile-checked only: `core::drop` is depend-free.
