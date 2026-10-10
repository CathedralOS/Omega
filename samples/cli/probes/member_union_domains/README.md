# Member + Union Domains

Compile-scope probe: union membership `x in A | B` as a bool subject,
and an entry machine carrying its own `requires`. No standard-library
dependency.

- `main.password in Valid | Secure` — union membership evaluates to a
  bool; `(true)` arm pattern on the subject.
- `machine Main::main requires main.password in Valid` — entry
  machines may carry requires contracts (discharged like the corpus
  fixture's entry).
- `accept(length) requires length > 0` — satisfied by the member
  expression `main.password.length` through the domain fact, no
  literal proof.
- `hits` records which union arm ran.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/member_union_domains/main.omg
mbx run -p omega -- update --project samples/cli/probes/member_union_domains
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
