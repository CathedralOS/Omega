# giant_literal_subject

Anonymous numeric match subject: `2^64 / 2^64` comptime-folds to 1 exactly — the match subject never binds a name.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
