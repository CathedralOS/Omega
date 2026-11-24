# Packet unit_031: selected instructions, foreign-call admission and physical recipes

In progress. Receipts below are settled; the rest of the record is still being traced.

## Scope and method

- **Packet:** [`records/unit_031.md`](../../records/unit_031.md) and [`units/unit_031.tsv`](../../units/unit_031.tsv), area `12-native-representations`. The five `SI/src/effects/` files the record did not read belong to gap packets `unit_031_gap_016` and `unit_031_gap_017` ([`second_pass_ranges.tsv`](../../second_pass_ranges.tsv) lines 46-51), so this packet makes no claim about them.
- **Baseline:** current paths and lines are at `24ad2700685e6cc6062026c87ed5fd24ed415265`.
- **Abbreviations:** `SI/` = `source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions/`; `EP09/` = `source-rust/subsystems/compiler/psi-to-native/pipeline/09_post-allocation-machine-to-framed-machine/entry-planning/src/`; `IE15/` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/`.

## Receipts

| # | Original claim (record section) | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `identity_transport` zips internal and request parameters, so a hand-built request with fewer `parameters` passes (B2.9) | `SI/physical-recipes/src/callable_entry.rs:115-134` | Both production callers build the request with `native_entry_request_from_abi`, which refuses unequal counts (`callable_entry.rs:73-84`): stage 09 `EP09/callable_entry.rs:82,115` and stage 15 `IE15/entries.rs:887-888` through `native_entry_request` (`:907-923`). The only hand-built request (`IE15/entries.rs:3647`) is a unit test and goes to `plan_native_entry_bridge`, which itself refuses `request.parameters.len() != internal.parameters.len()` (`EP09/native_entry_bridge.rs:275-276`). | Intended (single builder checks the count) |
| 2 | `NativeEntryParameter::scalar_codec == 0` means "no conversion", so a codec defaulted to 0 would pass (B7.9) | `SI/physical-recipes/src/native_entry_bridge.rs:8-12` | `SelectedProgramContracts::new` requires codec `Some(0)` for every non-`BorrowedScalarReference` parameter and a decodable codec or `None` for a borrowed scalar reference (`SI/src/program_contracts.rs:62-80`). `native_entry_request_from_abi` refuses `None` (`callable_entry.rs:94-100`). `value_transport` refuses codec 0 for a `BorrowedScalarReference` destination with `MissingScalarCodec` and refuses a nonzero codec on any other class (`EP09/native_entry_bridge.rs:219-246`). | Intended (zero cannot stand in for a needed conversion) |
