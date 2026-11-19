# const_match_record

First sample with a match expression in a const
initializer: `const ALIAS: Outer = match true { true ->
Outer { inner: SOURCE }, ... }` — arms evaluate at
compile time and the taken arm's record literal may
reference earlier consts.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
