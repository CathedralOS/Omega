# Triage packet: unit_017 (area 11, native stages 11-15)

Status: in progress (early receipt).

Scope: the claims in [`records/unit_017.md`](../../records/unit_017.md), which
covers `isa-aarch64/src/selected_form_encoding.rs` lines 1-14400 at the record
checkout, and the lines of the [area 11 digest](../../digests/11-native-stages-11-15.md)
that cite those lines. The gap packet `unit_017_gap_003` owns the supplemental
ranges (`selected_form_encoding.rs` 14401-17625, `callback_field.rs` and the five
`isa-x86_64` files); this packet does not review them. Where a claim here
depends on a caller in the tail, the packet traces only that caller.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. The file has 17,596 lines
at the baseline (17,625 at the record checkout), so line numbers below are
baseline numbers and can sit up to about 30 lines before the record's.
`S` = `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/`;
`A64sfe` = `S/isa-aarch64/src/selected_form_encoding.rs`.

## Settled claims

### U17-01. Generic validator indexes registers without a length check

- **Original:** record §1 "Fail-open risks", second bullet (`:2366-2569`);
  digest line 168.
- **Code path:** `validate_aarch64_selected_form_encoding` (`A64sfe:14377`) calls
  `validate_request` (`:14436`), which takes the operand count from
  `family_and_operand_count` (`:14451`) and refuses `operands.len() != count`
  with `OperandCountMismatch`. `resolve_scalar_registers` (`:14708`) maps
  operands one to one, or returns `[0]` for a one-operand `ReturnScalar`.
  `validate_decoded` (`:2298`) has no other caller (grep for `validate_decoded`
  in `S/isa-aarch64/src`: the definition, its `AccessorScalarAccess` recursion at
  `:2319`, and `:14424`). The four-register saturating arm (`:2483-2486`)
  indexes `registers[3]` only when `realization.operand_count() != 3`, the same
  count `family_and_operand_count` demanded.
- **Contract:** the register slice has exactly the family's operand count
  before any index.
- **Outcome:** intended. A short slice is refused before indexing; no panic
  and no acceptance is reachable from the public entry point.

## Remaining in this packet

Listed individually in the next revision.
