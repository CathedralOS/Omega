# array_operand_epochs

Owned array operands snapshot contents at their exact evaluation
positions: `before` captures pre-write contents, the effectful `marker`
rewrites index 0, and `after` captures post-write contents — the same
source home stays writable between the independent copies.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
