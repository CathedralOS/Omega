# match_reference_local

Match arms yield references: `let selected: &i32 = match cond { ... }`
binds a borrowed local chosen between two referents; reading a snapshot
ends the loan and frees both sources for mutation. `crash Trap;` is the
explicit trap statement.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
