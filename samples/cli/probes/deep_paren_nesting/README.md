# deep_paren_nesting

60 levels of parenthesis nesting in a `let`
initializer — parser recursion tolerance.

## Gaps found while authoring (filed)

The checker thread stack-overflows (`thread has
overflowed its stack`, abort) at 150 AND 300
parenthesis levels under sample-package `--check`,
while corpus `parser/deep_nesting_within_limit` holds
300 levels "well within MAX_NESTING_DEPTH" via the
large-stack compile thread. The large-stack path is
not active for sample packages — landed at 60.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
