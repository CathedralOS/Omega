# fixed_token_operators

Fixed-token declaration heads: `operator + Cell::add;` binds a
fixture-owned operator, since primitive `i32 +` belongs to the core
semantic home; `boundary machine []` and `boundary machine [..]`
bind fixture-owned collection operators — generic over `T`, with
requires contracts that stay provider-agnostic.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
