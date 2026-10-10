# write_only_loans

First sample spelling `&write` — an attenuated
write-only loan: `replace(&write destination, 42)`
stores without read access, and `forward` re-lends the
same qualifier through a second parameter.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
