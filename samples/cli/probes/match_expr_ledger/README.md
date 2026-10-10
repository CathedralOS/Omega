# Match Expr Ledger

Depend-free compile-scope probe: match expressions — the first sample
producing values from `match` instead of dispatching transitions.

- Literal patterns pick a case: `match code { 2 -> Missing, _ -> Other }`
  yields the expression's value directly.
- A nested match fills a record field — `category: match code { ... }`
  inside the `_` arm's construction.
- Duplicate value patterns keep authored first-match order: in `choose`,
  `0 -> 7` shadows `0 -> 99`.
- Match results bind through `let` and through fields on `&mut`
  carriers (`Holder::replace` writes `holder.category` from a match).

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/match_expr_ledger/main.omg
mbx run -p omega -- update --project samples/cli/probes/match_expr_ledger
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
