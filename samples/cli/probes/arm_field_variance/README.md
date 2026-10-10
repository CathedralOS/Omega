# arm field variance

Compile-scope probe. No standard-library dependency.

- member_projection_consume: path-indexed linear claims —
  `consume(pair.left)` projects one member while the sibling
  keeps its claim.
- owner_snapshot_restore: read-before-drop + whole-record
  re-establishment (`g = Guard{...}`) + read-after.
- linear_branch_transfer: transition arms are alternatives —
  both transfer the same linear obligation into consuming states.
- arm_field_variance: `pack` places inputs in DIFFERENT fields
  per arm; the claim map is arm-independent.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/arm_field_variance/main.omg
mbx run -p omega -- update --project samples/cli/probes/arm_field_variance
```
