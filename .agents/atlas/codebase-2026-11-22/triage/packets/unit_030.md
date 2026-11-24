# Atlas packet unit_030: native representations (area 12)

Packet complete. Every claim from unit_030 now has a disposition. The review found two wrong
results in the native representation codecs, plus one gap in a constructor check, and fixed all three:

- **Recovery classification round trip:** the codec could not read back a float or callable spill victim.
- **Saturated index count:** one recovery encoder wrote a count that disagreed with the rows it wrote.
- **Program-entry signature:** the constructor accepted an identity index missing from its own table.

Six claims stay open. None of them produces a wrong result today.

The review also found a compile failure on `main` in this area. The crate `optimization-unit` does not build at
the baseline, because #5393 added `Proposition::DeclarationIdentity` without an arm in that crate's identity encoder
(receipt 25). This packet adds the arm, so the stage 01 consumer tests can run again.

The evidence baseline is `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. Claims come from
[records/unit_030.md](../../records/unit_030.md), [units/unit_030.tsv](../../units/unit_030.tsv), and the unit_030
lines of [digests/12-native-representations.md](../../digests/12-native-representations.md), sections 5 to 7 and 10.
The survey read checkout `4f3c255d`. [second_pass_ranges.tsv](../../second_pass_ranges.tsv) assigns no gap packet
to unit 30. Paths below start at `source-rust/subsystems/compiler/psi-to-native/representations/` (written `R/`).
The [joint ledger](../07-13-psi-and-native.md) already records repairs #5343 and #5282. This packet keeps those
dispositions.

## Wrong results fixed in this packet

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | Digest §5 line 269. Record §6 `recovery.rs`, under "How it decides" and unsure item 3: `decode_classification_scalar_type` lacks tags 4 and 5, which the encoder writes | `R/register-homes/src/recovery.rs`: `decode_classification_scalar_type` and `encode_classification_scalar_type` | `RecoveryClassificationPlan::decode(plan.encode())` must return the plan. Stage 06 (`analyses/src/recovery_classification.rs`, `immediate_eligibility::classify`) gives every victim that is not a fixed unsigned u64 the reason `UnsupportedScalarType`, and keeps the victim's own `scalar_type` in the row. A float or callable victim therefore produces tag 4 or 5 | At the baseline, the new test `recovery::tests::unsupported_float_and_callable_victims_round_trip_through_the_codec` failed with `left: Err(UnknownScalarType(4))` for an `IeeeFloat(Binary32)` victim. The decoder now reads tag 4 (format byte 0/1) and tag 5 (nonzero callable contract id). The same test round-trips both float formats, a callable, a Boolean, a signed integer and an address. It also refuses float format 2, callable id 0 and scalar tag 6 | Fixed |
| 2 | Digest §5 line 270, first half. Record §6 `structural_declarations.rs`, fail-open (2): `encode_target_argument` saturates a runtime-index count above `u32::MAX` | `R/register-homes/src/recovery/structural_declarations.rs`, `encode_target_argument` | The decoder (`decode_target_argument`) reads exactly the u32 count of index rows. A saturated count followed by `len` rows misframes the rest of the payload | Traced. `encode_target_argument` already returns `Result<_, terminal_wire::CodecError>`. It now refuses with `CollectionTooLong("structural argument runtime indices")` instead of saturating. This case cannot be built in a unit test, because it needs more than 4 billion index rows. The existing structural round-trip tests pass | Fixed |
| 3 | Digest §5 line 227, first half. Record §4 `entry_signature.rs`, fail-open: the identity hashes a missing identity row as `""`, so two such signatures would collide | `R/program-entry-contracts/entry_signature.rs`, `validate_capture` | `SelectedProgramEntrySignature::identity` hashes the callable and each parameter type by its table text. A signature must therefore name indexes present in its own table. The only producer, stage 01 `program_entry.rs`, already refuses a missing callable text, so this shipped path never reached the gap | The constructor now refuses a callable or parameter-type index that `identities.text` cannot resolve. A new case in `captured_callable_identity_uses_its_table_not_its_ordinal_or_display_name` passes an index one past the table. Without the check, that case fails at `entry_signature.rs:466`. With it, the case passes | Fixed |

## Compile failure on `main` found during validation

| # | Observation | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| 25 | Not an Atlas claim. Building the stage 01 consumer tests failed at the baseline with `error[E0004]: non-exhaustive patterns: &semantic_vocabulary::Proposition::DeclarationIdentity(_) not covered` at `optimization-unit/src/identity.rs:267` | `R/abstract-operations/optimization-unit/src/identity.rs`, `proposition_encoding::encode_proposition` | [#5393](https://github.com/CathedralOS/Omega/pull/5393) added the variant. `origin/main` at `d1a23cb1` still lacks the arm, and no open PR adds it. Psi publication refuses a declaration Identity (`psi/artifact/wire/src/foundation_validation.rs`), so native code should not see one. This encoder returns no `Result` and cannot refuse, so the new arm binds every field under the unused tag 23 rather than panicking | New test `declaration_identity_tests::declaration_identity_binds_its_values_declarations_and_sides` changes a value, a supplier, a member generation, an operand kind and the side order, and checks that each change alters the bytes. `mbx test -p optimization-unit --lib`: 15 passed | Fixed |

## Earlier repairs, confirmed in current source

| # | Original claim | Current source | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| 4 | Digest §6 line 305. Record §6 `structural_calls.rs` fail-open (1) and (2), and `structural_declarations.rs` fail-open (1): `decode_result` drops `qualification_establishments`, `decode_type` drops `requires`, `decode_target_parameter` leaves `projected_qualifications` empty | `structural_calls.rs` `encode_result_establishments` and `decode_result`. `structural_declarations.rs` `decode_type` (reads `requires`). `decode_abi_parameter` fills projected qualifications, and `encode_target_structural_parameter` writes them | Repaired in [#5343](https://github.com/CathedralOS/Omega/pull/5343), as the joint ledger records. Every target parameter is decoded through `decode_abi_parameter` (`program_contracts.rs`, `structural_declarations.rs`). The register-homes suite passes (137 tests) | Fixed (#5343) |
| 5 | Digest §9 line 376. Record §6 `lib.rs`, stated limits: the header says `FixedViewCopyPlan` is v116, but the code says 118 | `R/register-homes/src/lib.rs` header; `fixed_view_copy_codec.rs` `VERSION = 120` | The header no longer states a version number for `OMGFCV`, so it cannot go stale | Fixed |

## Intended behavior

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 6 | Digest §5 line 263, first half. Record §2: `BoundBodyContracts::encoding_subject` skips the foreign-call comparison when `foreign_calls` is `Err` | `R/post-allocation-machine/body-facts/src/lib.rs` `encoding_subject` | The `Err` is a deferred stage result carried as data. Stage 14 refuses it when it builds the object: `body_object.rs` and `body_object/imports.rs` map it to `Error::ForeignCalls`. Stage 15 `function_fragments/validation.rs` requires `foreign_calls.ok() == Some(&artifact.foreign_call_plans)` | Intended |
| 7 | Digest §5 line 263, second half: `needs_installed_branch_checks` is false when there is no installed entry | Same file | The checks compare branches to installed fatal entries. With no installed entry execution, there are no such branches. Stage 13 `frame_application.rs` `check_installed_branches` returns an error when a later stage asks for checks that were not requested, so the default refuses | Intended |
| 8 | Digest §5 line 264. Record §3 `codec.rs`: `PostAllocationMachinePlan` decode accepts any alternative | `R/post-allocation-machine/src/codec.rs`; `codec_tests.rs` `reauthenticated_physical_data_is_still_only_a_proposal` | Decode proves byte identity only, and the test says so. `program_validation.rs` `validate_current_program` performs the correspondence check | Intended |
| 9 | Digest §5 line 265. Record §5: `SelectedAllocationInput::into_legality` widens `None` to all allocatable views | `R/selected-instructions/allocation_input.rs` | The doc comment says so. `None` means that no recovery rule restricted availability, and the policy `AllEnvironmentAllocatableViewsV1` is the unrestricted availability | Intended |
| 10 | Digest §5 line 267. Record §6 `publication.rs`: `AllocationPublicationFacts::retain` hashes the record as given | `R/register-homes/src/publication.rs` | The comment says so. `program_validation.rs` compares `publication.identity` with `recomputed_identity()` | Intended |
| 11 | Digest §5 line 268. Record §6 `recovery.rs`: decoded recovery rows are not checked against the program | `R/register-homes/src/recovery.rs` header | The module header says decode grants no authority, and independent validators replay the rows | Intended |
| 12 | Digest §5 line 270, second half. Record §6 `structural_declarations.rs`, fail-open (3): `encode_projected(.., false)` writes nothing | `structural_declarations.rs`, `structural_calls.rs`, `structural_contracts.rs` | Every live call passes the same `retain` flag to the encoder and the decoder, and every caller in `recovery/` passes `true` | Intended |
| 13 | Digest §5 line 227, second half. Record §4: `push_shape` checks the shape class only with `debug_assert!` | `R/program-entry-contracts/entry_signature.rs` | `validate_capture` (in the constructor; the fields are private) requires each hashed shape to be the exact integer Extent layout `integer(16, 8)` with two `integer(8, 8)` fields, and requires `value_shape` to equal it. `push_shape` only sees validated shapes | Intended |
| 14 | Digest §5 line 228. Record §4 `physical_contract.rs`: any nonzero calling-plan fingerprint is accepted without recomputation | `R/program-entry-contracts/physical_contract.rs` | Its consumers recompute the comparison: `target-bound-program/entry_settlement.rs` and `verified-optimization-input/src/hosted_receiver.rs` both compare it with `contract_report_fingerprint()` of the selected plan | Intended |
| 15 | Record §6 `pressure_rematerialization.rs`: `IntegerValue` tags read Unsigned=0, Signed=1, while `recovery.rs` and `fixed_view_copy_codec.rs` read Signed=0 | `R/register-homes/src/pressure_rematerialization.rs` | Each artifact's encoder and decoder agree: the value is written as 16 bytes and then the tag, and `Signed` is restored by a bit-preserving `as i128`. No codec reads another artifact's layout. The duplicated tables are a §3 cleanup lead | Intended |
| 16 | Record §6 `constraints.rs`: `allocation_legality_identity_from_digests` panics with `assert_eq!` on a count mismatch | `R/register-homes/src/constraints.rs` | The only caller, stage 06 `allocation_legality.rs`, builds one digest per plan function through `carried_digests`. The assert guards a programming error, not input | Intended |
| 17 | Record §3 `identity.rs`: the identity domain is v27 while the wire version is 28 | `R/post-allocation-machine/src/identity.rs`, `codec.rs` | The identity domain and the wire format carry separate version counters, and both files state their number | Intended |
| 18 | Digest §10 item 10. Record §5: no `mod` line for `allocation_input.rs` | Stage 06 `analyses/src/lib.rs` | The file is included with `#[path = ".../allocation_input.rs"] mod allocation_input;` | Intended (digest-verified) |

## Still open

None of these produces a wrong result for a program today.

| # | Original claim | Current source | Trace | Outcome |
| --- | --- | --- | --- | --- |
| 19 | Digest §5 line 266. Record §6 `constraints.rs`: the decoder does not check the sorted order that comments claim for availability classes and views | `R/register-homes/src/constraints.rs` `AllocatorAvailabilityPlan::decode` | The decoder checks magic, version, policy, trailing bytes and identity, but not order. The only producer, stage 06 `allocator_availability.rs`, derives the order from the physical model. `AllocatorAvailabilityPlan::decode` has no caller outside its own tests (`git grep`). A future reader of saved artifacts must check canonical order | Still open (latent) |
| 20 | Digest §5 line 271. Record §3 `adapters.rs`: `X86PushPop` indexes a 16-entry table with a `u8` | `R/post-allocation-machine/src/adapters.rs` `effects` | The only producer, stage 09 `native_entry_bridge.rs` `push_or_pop`, passes the constant `SAVED_GPRS` (0 to 15). The stage 11 encoder (`adapters.rs`) refuses `register >= 16`, but `effects` would panic first on an out-of-range value | Still open (latent panic) |
| 21 | Digest §7 line 314. Record §3 `costs.rs`: `target_cost_model`, `TargetCostModel`, `observe` and the `NonAuthoritative*` types have no caller outside the file and its re-export | `R/post-allocation-machine/src/costs.rs` | `git grep -w` at the baseline finds only the re-export in `lib.rs`. The other hits are the `optimization-core` identity of the same name | Still open (unused) |
| 22 | Digest §7 line 331. Record §6 `post_allocation_manifest.rs`: `PostAllocationOptimizationManifestError::{ContentMismatch, IdentityMismatch}` are never constructed | `R/register-homes/src/post_allocation_manifest.rs` | `git grep` finds no construction site | Still open (unused) |
| 23 | Digest §7 line 332, §10 item 11. Record five-things item 1: no production reader of the register-homes and post-allocation wire `decode` | `R/register-homes/`, `R/post-allocation-machine/` | Matches the digest's check. The decoders have no caller except their own tests, so item 1 above affected only tests and saved artifacts | Still open (unused) |
| 24 | Record §6 `codec_tests.rs`: test names say `artifact_v15` while the version is 120 | `R/register-homes/src/recovery/codec_tests.rs` | Naming only, with no effect on behavior | Still open (naming) |

`QualifiedPhysicalRead` (digest §7 line 330) has its one external use in `machine-code/src/encoding.rs:238`.
That claim only noted a single caller, so it is intended. The tables of repeated work in record §3 and §6
(target encoders, `decode_invocation_site`, the origin and constraint-key tables) are code-repair leads, not
correctness claims. This packet does not change them.

A follow-on trace for receipt 1: `encode_classification_scalar_type` writes a rigid (parameterized) integer
width through `write_identity`, which the decoder does not read back. Selected plans do not carry rigid widths:
`fixed_view_copy_codec.rs` `encode_scalar` refuses them as "not an executable integer descriptor". A misread
plan would then re-encode to different bytes and fail the identity check, so decode refuses rather than accepts.

## Validation

All runs are at the baseline plus this packet's commits. Commands ran through `mbx`.

- `mbx test -p register-homes --lib`: 137 passed. Before the decoder fix, the new round-trip test failed with `Err(UnknownScalarType(4))`.
- `mbx test -p program-entry-contracts`: 3 passed. With the new table check removed, the new negative case fails at `entry_signature.rs:466`.
- `mbx test -p admitted-native-inputs-to-target-bound-program --lib`: stage 01 is the only caller of `from_checked_declaration`. 14 passed. Before receipt 25, this command failed to compile `optimization-unit`.
- `mbx test -p optimization-unit --lib`: 15 passed.
- `mbx clippy -p register-homes -p program-entry-contracts -p optimization-unit --all-targets -- -D warnings`: clean.
- Not run: a workspace build. Other crates that match on `Proposition` exhaustively were not compiled. The packet forbids a global rebuild.
- `mbx xtask fmt --check`: 3,213 diffs at the baseline and 3,213 with this packet, so it adds none. The baseline diffs are in files this packet does not change, and the formatter here does not match the repository's formatted state.
