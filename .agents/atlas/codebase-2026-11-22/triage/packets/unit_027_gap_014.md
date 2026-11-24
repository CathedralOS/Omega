# Triage packet: unit_027_gap_014 (area 12, `unit_validation.rs` lines 15601-18242)

Status: In progress. Early receipt: one claim settled.

This packet reviews the correctness observations in
[record unit_027_gap_014](../../records/unit_027_gap_014.md) and the matching
lines in [digest 12](../../digests/12-native-representations.md). The assigned
range is [units/gap_014.tsv](../../units/gap_014.tsv): `UV` lines 15601-18242.
The baseline is `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`; at that
revision and at `a0152d26` the file still has 18242 lines and the record's line
numbers hold.

`UV` is
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/unit_validation.rs`.

## Receipts

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| D | Record §1 item 5 and "What you are unsure about" bullet 3: `unreachable!()` at `UV:15766` assumes `StructuralReferenceOrigin` has only `Parameter` and `Invocation` | `UV:15757-15767`; enum at `source-rust/subsystems/compiler/psi/types/values.rs:1507-1515` | The `let ... else` chain must name every origin kind. | Read the enum: exactly `Parameter(ReferenceOriginSource)` and `Invocation { carrier_type, path, extent, access }`. The second `let ... else` therefore cannot fail. A future third variant would compile and then panic here instead of failing to compile. | Intended (unreachable at this revision) |
