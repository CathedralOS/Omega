# fixed_token_operators

Fixed-token declaration heads: `operator + i32::add;` names the builtin
arithmetic surface; `boundary machine []` and `boundary machine [..]`
bind fixture-owned collection operators — generic over `T`, with
requires contracts that stay provider-agnostic.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
