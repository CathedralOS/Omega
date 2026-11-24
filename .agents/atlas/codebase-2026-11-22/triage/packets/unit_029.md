# Triage packet: unit_029 (area 12, machine-code checkers and records)

Status: in progress. Receipts below are settled; the rest of the slice follows.

This packet reviews the correctness observations in
[record unit_029](../../records/unit_029.md) and the matching lines in
[digest 12](../../digests/12-native-representations.md). The baseline is
`main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. No file in the unit
changed between that revision and `a0152d26`, so the record's line numbers
still hold. No supplemental gap range is assigned to unit_029.

`R/` is `source-rust/subsystems/compiler/psi-to-native/representations/`;
`EA/` is `R/machine-code/encoding-admission/src/`; `MC/` is
`R/machine-code/src/`.

## Receipts

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | Record §A2 item 9(a); digest 12 line 274 and §10 item 7: `(_, NoneV1, _)` in `validate_non_return_effects` accepts any unlisted kind with any address (`EA/exit_requirements.rs:1035-1048`) | `EA/exit_requirements.rs:155-176`, `:1035-1048`; `EA/validation.rs:178-200`, `:435-446`, `:504-515` | The exit check refuses a non-return instruction whose stack, memory or control effect differs from its kind. The memory effect itself comes from the target's alternative catalog. | Trace. `prepare_non_return` reads the effects from the row's footprint. The encoding checker requires that footprint to equal the ISA validator's footprint (`admit_decoded_footprint`), and that footprint's effects to equal `machine.alternative.encoded` (`validate_machine_footprint`). A row may carry an address only for the 15 address kinds in `validate_row` (`:178-200`), each matched to its own `PhysicalAddressOperation` by `admits_address_operation`. Every other kind goes to `validate_baseline`, which refuses any address (`:444-446`). `NoneV1` with an address reaches this arm only for `FrameAddress` (`AddressOffset` has its own arm at `:913-924`), whose address `frame_address::validate_address` checks. A memory-reading kind cannot reach this arm with `NoneV1` unless its catalog entry says so. | Intended |
