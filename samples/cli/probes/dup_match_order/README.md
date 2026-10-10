# dup_match_order

Repeated value patterns retain authored first-match order — the second `0` arm is unreachable but compiles; first match wins.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
