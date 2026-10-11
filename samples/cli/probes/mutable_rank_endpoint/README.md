# mutable_rank_endpoint

The pinned view bound `limit` is a mutable input (`mut limit`), but
its declared range forms the computed endpoint `limit + 1` at every
evaluation; the premise-preserving prefix proves `limit` still
denotes its arrival value across the named-state cycle.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
