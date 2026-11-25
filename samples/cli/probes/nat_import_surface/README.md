# nat import surface

Compile-scope probe. No standard-library dependency.

- exact_sub_guard: exact `remaining - 2` discharged by
  `requires <= 5` + `>= 2` transition guard — no domain, no `as`.
- transitivity_lemma: `requires a < b, b < c` `ensures a<c` — kernel
  entailment hop, empty body.
- nat_import_surface: core::nat imports depend-free and Nat values
  flow through machines. Nat's `-` and `<=` are core's public
  `subtract` and `less_or_equal`, selected after importing both.
- match_mixed_move: one match arm moves a local, the other
  constructs inline — ownership splits per-arm.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/nat_import_surface/main.omg
mbx run -p omega -- packages update --project samples/cli/probes/nat_import_surface
```
