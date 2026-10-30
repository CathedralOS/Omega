# seq_consuming_combinators

Consuming combinators over a recursive sum: `map<T, U, machine F>`/`filter<T, machine Predicate>` with `requires machine F(value: T) -> U` / `requires machine Predicate(value: &T) -> bool` contracts, `terminates by items` structural rank, and closed wrappers `map<increment>`/`filter<positive>` instantiating the machine-symbol params.

## Learnings

- `Seq<T>` is proof-only — `&Seq<T>` params are impossible (no runtime layout); consume by value.
- `terminates by items` ranks only through a recursive edge whose subject is a pattern-bound subterm — multi-param accumulator shapes and state-routed recursion lose the rank ("no builtin well-founded order").
- Runtime recursion is TAIL-ONLY: `1 + length(tail)` rejects ("result feeds the surrounding computation"); the recursive call must be the arm's produced value path — `bump(depth(tail))` inside a call arg is fine.
- `(value as u64 in Wrapping) + 1` re-tags a single operand; `as u64` strips back — no overflow obligation.
- `match` does NOT admit record/case destructure — `transition` dispatch only.

Compile-checked only: run a check target, not native.
