# Denotation Lets

Compile-scope probe: `pub let` denotations and single-file
`module` declarations. No standard-library dependency.

- `module lets;` — a single file declares its module identity;
  `main.omg` reaches it via `use lets`.
- `pub let reflexive(value: u8): core::Strict<0>` — proposition
  lets at the proof-judgment level; `wrapping_reflexive` carries a
  domain-qualified parameter.
- Denotation lets are PROOF-ONLY: not callable as executable
  values, and a `requires` quoting one needs the fact established
  (they participate like ordinary contract members).
- Learnings: `pub let` params REQUIRE a `:` annotation;
  `A: core::Type<0>` bounds inside `<>` don't parse in this
  toolchain; arrow-returning lets are not spellable here.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/denotation_lets/main.omg
mbx run -p omega -- update --project samples/cli/probes/denotation_lets
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
