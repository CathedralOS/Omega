# Atlas packet unit_022_gap_010: image-emission retained checks, `function_fragments.rs` 13801-19800

Packet complete. One claim was a real fail-open check and is fixed in this PR:
`runtime_footprint_retained` accepted indexed memory-access rows on a byte-length
read that has no runtime index. The other claims are intended behavior, are caught
by a second check, or are duplicate code with no wrong result. Two duplicates
remain open as cleanup candidates (receipts 1 and 9).

This packet covers [record unit_022_gap_010](../../records/unit_022_gap_010.md)
(unit file `units/gap_010.tsv`, one range: `function_fragments.rs` 13801-19800)
and the lines of the [area 11 digest](../../digests/11-native-stages-11-15.md)
that cite it (lines 108, 109, 209-211, 237). It owns that whole range; no other
packet shares it. Evidence comes from `24ad2700685e6cc6062026c87ed5fd24ed415265`;
the record's line numbers still match that revision for this file. The
[Psi and native ledger](../07-13-psi-and-native.md) had no earlier disposition for
these claims, and no open PR touched these files when this review started.

Paths: `FF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments.rs`;
`RSV` = `FF`'s `function_fragments/source/retained_source_validation.rs`;
`VAL` = `function_fragments/validation.rs`; `S05` =
`pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/`.
"The audit" is the replay check `RSV::admit`, which runs only on validator
requests with stage 02's output retained (`RSV:1-4`).

## Fail-open leads

| # | Claim (original location) | Code path | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- | --- |
| 2 | `runtime_footprint_retained` returns `true` for an operation with no runtime index without looking at any access row (record §1 fail-open 1; five things item 4; digest line 209) | `FF:15567-15646`; only production caller `RSV:607-609` (`StructuralByteSequenceFieldLength`) | Selection writes one indexed row (`ReadIndexedPrimitive`, `WriteIndexedPrimitive`, `WriteIndexedStructural`) per runtime index and a plain `ReadPlace`/`WritePlace` row when there is none (`S05/selection_structural.rs:4771-4813`, `footprint_at_origin`). The early `return true` at `FF:15612-15614` skipped the `rows.len() == elements.len()` comparison, so a length read with no runtime index accepted any number of indexed rows tagged with its operation. The paired `structural_fields::retained` check reads only target rows, not memory accesses. New test `byte_length_footprint_requires_one_indexed_row_per_runtime_index` fails before the fix on the stray-row assertion and passes after it. | Reproduced and fixed in this PR: the early return is removed, so zero runtime indices require zero indexed rows. |
| 3 | `indexed_read_footprint_retained` returns `true` for an all-`ReadReturnedView` tail without checking the payload read, index, length or obligation (record §1 fail-open 2; five things item 4; digest line 209) | `FF:15883-15900` | Selection's `replay_memory_origins` replaces a `ReadByteSequence` row whose storage comes from a call's returned view by one `ReadReturnedView { producer }` row per producer, with offset 0 and count 0 (`S05/selection_inputs.rs:5484-5524`). That row no longer carries an index, length or obligation, so the check cannot compare them. It still requires every row to name a structural `Call` whose result place is the row's place, and the pointer-slot and descriptor rows before the tail are checked against the selectors. | Intended. The check does not recompute the exact producer set that `collect_calls` would derive; that narrower point is not a demonstrated wrong result. |
| 4 | `decoded_temporary_stack_bytes_by_function` counts 0 for a row whose `effects()` is `None` (record §3 fail-open 1 and "unsure"; digest lines 210, 237) | `FF:17540-17583`; caller `VAL:541-546` | `effects()` is `None` only for the null footprint handle (`machine-code/src/placed_body.rs:57-59`, `encoding.rs:207-209`), used by deferred-control rows (`encoding_identity.rs:410`). The value it is compared with comes from stage 09's selected alternatives, not from decoded footprints (`09_post-allocation-machine-to-framed-machine/src/physical_facts.rs:32-57`). A missing decoded footprint on an instruction that uses temporary stack therefore lowers the decoded peak and `VAL:542` refuses with "temporary stack plan differs from independently decoded instruction effects". | Intended; caught by the comparison |
| 5 | `byte_operation_retained` and the non-float `element_operation_retained` arms match only the variant and `psi_operation`, not operands (record §3 fail-open 3, "unsure"; five things item 3; digest line 211) | `FF:17268-17357`, `:17359-17469`; callers `RSV:706-726` | `RSV::admit` asks whether each optimized operation still has exactly one row in the retained stage 02 target function (`RSV:40-56`). Operand agreement between those target rows and the abstract operations is the stage 04 audit's contract: `verify_retained_target_operations` runs it (`verification/published-objects/src/lib.rs:348`, `verification/stage_validators.rs:63`, `:88`), and it requires the target rows derived from the abstract plan to equal the retained rows (`04_abstract-operations-to-target-operations/src/preparation/legalization.rs:776-789`; the unit_010 packet on open PR #5415, receipt 4, traces the same check). | Intended; operands are checked by the stage 04 audit, not by this membership check |
| 6 | `ieee_float_builtin_retained` matches an `IeeeFloatClassify` target row against a `NearestIeeeFloatArithmetic` selected instruction (record §3 "Cases by shape" and "unsure") | `FF:17149-17189` | `IeeeFloatClassify` carries a `NearestIeeeFloatArithmeticRealization` (`representations/target-operations/src/operations.rs:162-169`). Selection lowers classify to `SelectedInstructionKind::NearestIeeeFloatArithmetic { form }` with `operation: Classify` (`S05/selection_structural.rs:3546-3565`). The check builds the same form from the row's realization and requires exactly one selected instruction with that form and provenance. | Intended |
| 7 | `byte_output::validate` delegates code-byte exactness to `selected_byte_output_bytes_are_exact` (record §5 fail-open, not read) | `FF:19107`; `image-emission/src/object_artifact.rs:1768-1800` | The helper decodes the settlement's code bytes for the target, requires the decoded scratch offset to equal the argument's `scratch_byte_offset`, the decoded register to be the argument's only location (4 bytes at offset 0), and the record shape to be exact. Any failure returns `false`. | Intended; closed |

## Duplicate code and record questions

| # | Claim (original location) | Code path | Check | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `frame_location` and `argument_matches_source` are a test-only copy of stage 09 (record §6 "Same job elsewhere"; five things item 5; digest line 109) | `FF:19418-19496` (`#[cfg(test)] mod frame_location`), used only by tests at `FF:19500`, `:19886`; production copy `09_post-allocation-machine-to-framed-machine/src/internal_call_plans.rs:1164-1230` | A whitespace-normalized `diff` of the two bodies differs only in the module wrapper, line wrapping and the import path of `TargetStructuralArgument`. The stage 15 tests exercise a copy that no production code calls. | Still open (duplicate, no wrong result). Proposed fix: move those tests to stage 09 against the production functions and delete the stage 15 copy. |
| 8 | Temporary stack size is computed three ways (record §3; digest line 108) | `FF:17531` (published), `:17540` (decoded placed effects), `:17585` (replayed original encoding); compared at `VAL:396-397`, `:541-542` | The published value is stage 09's; the other two are independent recomputations from different inputs used by two validators. They share only `temporary_stack_peak`. | Intended |
| 9 | `HomeInputs::resolve` and the free `resolve` duplicate the lookup (record §2 "Same job elsewhere"; digest line 108) | `FF:16818-16888`, `:16890-16945` | Both build the same three candidate sources (structural result homes, function parameters, block parameters) and the same allocations, then call one `resolve_candidates` that makes the decision. The indexed form serves repeated lookups (`FF:10994`, `:11002`, `:11397`); the free form serves single lookups (`FF:4339`, `:10309`, `:10461`). | Still open as a cleanup candidate (duplicate candidate construction); no wrong result |
| 10 | `parameter_abi` and `abi.parameter_abi()` both exist (record §3 "Same job elsewhere", not read) | `FF:17471-17490`; `representations/target-operations/target-vocabulary/src/function_abi.rs:68-82` | Both apply `publishes_parameter_abi` with the same four inputs, one to the retained target function and one to the published ABI record; `VAL:410-416` and `:571-588` compare the two sides. | Intended |
| 11 | `replacement_footprints_retained` source offset, `read_access` unknown place, `thin_address_join`, `operation_retained` other operations (record §1, §2, §6 "none seen") | `FF:16248-16280`, `:16664`, `:16991`, `:19198` | The record found no fail-open; each unknown case returns `false`, `None` or an error. | Intended, as the record found |

## Main build regression found while testing

The focused image-emission test could not build on `main` (`24ad2700` through
`139787f1`). [#5393](https://github.com/CathedralOS/Omega/pull/5393) added
`Proposition::DeclarationIdentity` and left two exhaustive matches in the native
optimization unit without an arm: `encode_proposition`
(`optimization-unit/src/identity.rs:267`) and the place-root `collect`
(`optimization-unit-semantics/src/unit_validation.rs:10668`). No open PR or
branch fixed it. This PR adds both arms in a separate commit. Psi publication
refuses a declaration identity outside checking-identity mode
(`psi/artifact/declaration-encoding/src/terms/proposition.rs:266-272`), so the new
arms keep the native crates compiling and encode the clause faithfully if one
ever arrives.

## Unresolved claims

- Receipt 1: test-only `frame_location` copy in stage 15 (cleanup).
- Receipt 9: duplicate candidate construction in `structural_homes::resolve` (cleanup).
