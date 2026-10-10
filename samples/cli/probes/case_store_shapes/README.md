# case store shapes

Compile-scope probe. No standard-library dependency (relation_trait_machine
imports omega::language::core::relation — depend-free proof module).

- shared_mut_borrow_params: `strike(attacker: &Player, defender: &mut
  Player, damage)` — shared+mutable borrows of the SAME type as
  sibling params.
- self_compare_fold: `transition value < value` self-compare —
  always-false flag folds to the untaken edge at 43.
- case_store_shapes: case replacement naming no common field zeroes
  them; `i32 in Ten` interval-domained payload discharges
  `scale`'s `requires level in Ten` at the call.
- relation_trait_machine: core Reflexive/Symmetric/Transitive/
  Antisymmetric traits take `proposition Relation` params — user
  propositions bind through `satisfies` (declaration surface).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/case_store_shapes/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_store_shapes
```
