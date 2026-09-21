# EVALUATED-FOREIGN-BINDINGS — re-verification ledger

Board row: `TASKS.md` `**EVALUATED-FOREIGN-BINDINGS.**` (~:5958). Scope verified
at `201d58c5915`; re-verified on this host (linux x86-64) at `f72122f71e` by
Zergling-126 (claim `230a8a52`, draft path only).

## (a) Consumed half landed — confirmed

- `machine-code` functions carry `port_effects: Vec<PortEffectRecord>`:
  `omega-rust/omega/representations/machine-code/src/machine_code/functions.rs`,
  record type at `machine_code/boundary/ports.rs:6`.
- Object-artifact construction rebase + provenance/uniqueness/exact-bytes
  validation: `image-emission/src/object_artifact/` (`function_validation.rs`,
  `carriers.rs`).
- Installation record constructs, codecs and replays effects:
  `installation_record/record_construction.rs:276`,
  `codec/port_effect_codec.rs`, `record_shape.rs:141` `validate_port_effects`,
  record-vs-image equality at `record_validation.rs:101`.
- Native physical custody replays each effect against machine/object/final-image
  bytes: `native-artifact/src/physical/derivation/{provider_custody,evidence,
  children,settlement_identity,hashing}.rs`.

## (b) Locator custody landed — confirmed

`NormalizedForeignLocator` pairs from PE / versioned-ELF / Mach-O import tables
feed `FinalImageImportPlan::Normalized` via `image/src/builder/copies.rs`,
`image/src/final_image/symbols.rs`, `image-elf/src/imports.rs`,
`image-macho/src/dyld_linking/imports.rs`, plus admission policy in
`native_realization/terminal_authority_policy/normalized_foreign.rs`. Raw
foreign bytes remain locator data.

## (c) Producer still missing — confirmed

Only two `PortEffectRecord {}` literals exist and both are consumers/replay:
`port_effect_codec.rs:82` (decode) and `derivation/hashing.rs:766` (custody
hash). No selected instruction or machine-emission fragment ever emits port
bytes, and both production writers still hardcode empty vectors:

- `image-emission/src/function_fragments/production.rs:226` — `port_effects: Vec::new()`
- `native-realization/src/native_realization/callback_thunks.rs:171` — `port_effects: Vec::new()`

`TargetUnitOperation::PortWrite`, `MetadataOnlyPortRealization` and
`DirectPortReadU8Realization` exist (`realization_request.rs:34-35`,
`boundary_settlements` record shape) with the settlement-must-follow-port-write
custody check in `lowering/unit/boundary_call.rs`, but nothing constructs an
effect record.

## Fence disposition

Row's fence map re-verified: PRIVILEGED-PORT-EFFECT-SETTLEMENTS owns
`machine-code/boundary` + admission policy + ports corpus; PSI-NATIVE-FIELD-STORES
owns `function_fragments`; PHYSICAL-ACCESS-PROFILES owns
`native-artifact/src/physical`; the normalized-import evidence tests sit under
the NORMALIZED-ABI-LOWERING imports leg. The missing production writer lives
inside those fences. No unfenced slice remains on this host; the correct
in-fence artifact is this ledger.
