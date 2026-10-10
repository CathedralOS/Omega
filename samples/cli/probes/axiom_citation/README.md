# axiom citation

Compile-scope probe. No standard-library dependency.

- axiom_citation: `boundary machine mul_comm_axiom(a,b: Nat) ->
  Nat ensures mul(a,b)==mul(b,a)` — axiom cited inside a proof
  machine's body; `core::nat` imports depend-free. Nat is
  proof-only (no runtime result flows out).
- boundary_requirement: `boundary requirement X::settle(state)`
  bodyless consuming-contract decl beside an affine sum carrying
  a `[linear]` payload.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/axiom_citation/main.omg
mbx run -p omega -- update --project samples/cli/probes/axiom_citation
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
