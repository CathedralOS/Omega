# proposition surface

Compile-scope probe. No standard-library dependency.

- proposition_surface: user propositions — parameterized
  `proposition ... <T, const Limit, machine Generator>(v)`,
  proposition alias forwarding, proposition instances in
  requires/ensures, typed literal args, `127u64 as u8` literal
  narrowing. Proposition facts are evidence-borne (decl surface).
- trait_operator_crown: `trait Ranked<T> { operator < before }`
  crowned token, generic evidence binder `Order satisfies`,
  `Ascending::before` named call, concrete `machine <` crown.
- mutable_loan_return: `-> &mut Room` transition-selected loan
  return; sibling fields read before the loan forms; writes
  through the returned borrow.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/proposition_surface/main.omg
mbx run -p omega -- update --project samples/cli/probes/proposition_surface
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
