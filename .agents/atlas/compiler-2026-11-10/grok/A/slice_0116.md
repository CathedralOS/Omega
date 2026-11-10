# slice_0116

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0894 | target_providers.rs:643-653 | UNDECIDED | Vacuous family filter and permissive obligations owner are read; end-to-end wrong-machine admission not proven. |
| A0895 | provider_plan.rs:204-209 | REACHABLE | Pub `ProviderPlan` fields plus `OnceLock` memo can stale digests after in-place mutation before selection. |
| A0896 | provider_plan.rs:867-882 | CLOSED | `Err` table `storage` passes schema validate but `field_offset` rejects before external rows are emitted. |
| A0897 | provider_plan.rs:841 | CLOSED | Non-Linux syscall plans fail at settlement or evaluated-bindings target checks, not only schema validate. |
| A0898 | provider_grants.rs:80-82 | CLOSED | Skipped provider selectors still must resolve in `reconstruct_trust_obligations` on the product compile path. |
| A0899 | entry_signature.rs:265-267 | REACHABLE | Present callable identity index with empty table text passes capture but hashes as empty in `identity()`. |
| A0900 | post_handoff_writer.rs:137 | CLOSED | Every prepared destination must pass `claim`, which requires `pinned` and `unpublished` at `237-248`. |
| A0901 | windows_provider.rs:14 | REACHABLE | Substring scan in `publication.rs` treats incidental `omega_host.dll` bytes as a real import. |

## A0894

Digest **backend-plan** (`area_03`). Opened `omega-rust/omega/compilation/backend/plans/backend-plan/src/target_providers.rs` at `643-653` and `698-722` (read); lines match digest commit `457adf4a11f` on this checkout (read).

Enclosing function: `selected_provider_candidate_rows` (`600-761`, read). Target filter (`643-653`): for each `target_machine_families` row, if the candidate machine is not a member of that family, the inner `any` is false, so `!any(...)` is true and the whole `all(...)` passes without requiring a matching target (read). Obligations filter (`698-722`): `selecting_declaration.as_ref().is_none_or(...)` is true when `selecting_declaration` is absent, so obligations rows bind with no owner-specific selecting declaration check (read).

Search `selected_provider_candidate_rows` in `omega-rust`: **8** matches in **5** files (read); production call sites: `target_providers.rs` (`579` via `selected_checked_candidates`, definitions/tests in same file), `program_entry.rs:417`, `component_verification.rs:308`; plus `lib.rs` re-export and test support (read). Upward from `579`: `selected_checked_candidates` → `selected_checked_invocation_rows` and publication joins (`target_providers.rs:101+`, read); `selected_checked_adapter_header` at `66-77` requires `header.source_target` consistent with plan target (read); `count != 1` at `747-748` errors on ambiguous instance resolution (read).

**Input reaching the branch:** A checked-adapter plan whose Terminal provider candidate machine is not listed in any `target_machine_families` member for the plan target, or provider-instance obligations with no `selecting_declaration`, while other textual identity filters still align.

**Verdict UNDECIDED (read + inferred):** The permissive filter is confirmed, but no traced path from these branches alone to a full compile that keeps a wrong machine without a later error (header target, invocation proof scope, or instance count) was verified in this slice.

## A0895

Digest **effects** (`area_03`). Opened `provider_plan.rs` `83-95`, `204-209`, `284-289`, `110-114` (read).

`ProviderPlan` exposes mutable `pub` fields including `rows` and `schema`; `identity_digest` and `report_fingerprint` use `digest_memo` `OnceLock` via `get_or_init` (`204-208`, `284-288`, read). `ProviderPlanDigestMemo` clones to default and compares equal regardless of cached values (`104-114`, read). Comment at plan top states construction is free (`5-7`, read).

Search `digest_memo = Default` in `omega-rust`: **2** matches, both in `prepared_providers.rs` (`130` reset after schema attach; `7` comment) (read). `from_selected_plans` calls `validate_against_schema` then `report_fingerprint()` (`provider_evidence.rs:214-222`, read); first fingerprint call after an earlier `identity_digest()` on the same plan would return a stale memo if `pub` fields were edited in between (inferred).

**Input reaching the branch:** A host-assembled provider plan whose `rows` or `schema` are changed after the first `identity_digest()` or `report_fingerprint()` call on that same value.

**Verdict REACHABLE (read + inferred):** Validation reads current fields while memoized digests can remain from the pre-mutation state; downstream commitments that trust those digests without a full replan reset can disagree with the plan content that was validated.

## A0896

Digest **effects** (`area_03`). Opened `provider_plan.rs` `867-882` and `ProviderBinding` at `51-60` (read). `validate_candidate_against_schema` only requires non-empty `table` and `field` strings, not `storage.is_ok()` (read). Test `table_field_correspondence_is_committed_and_received_in_full` (`retained_inputs.rs:392-405`) asserts `validate_against_schema` is empty for both `Ok(storage)` and `Err(...)` storage (read).

Search `VtableField|TableFunction` under `omega-rust/omega/compilation/realization/provider-planning`: realization goes through `external_binding_rows.rs:124-147` (read). Closing check: `field_offset` (`211-217`) does `storage.as_ref().map_err(Clone::clone)?`, rejecting `Err` storage before a row is returned (read). `from_selected_plans` can admit such a plan at schema validate (`provider_evidence.rs:214-220`, read).

**Verdict CLOSED (read):** Schema validation alone accepts `Err` storage, but every production path that materializes table bindings hits `field_offset` and errors; compilation does not proceed to a complete external binding row.

## A0897

Digest **effects** (`area_03`). Opened `provider_plan.rs:841-849` (read). `ProviderBinding::Syscall` validation only checks `u32::try_from(*number)` (read).

Search `ProviderBinding::Syscall` in realization/admission: `external_binding_rows.rs:123`, `setlements.rs:518-531`, `binding_evaluation.rs:135-150`, `native_bindings/plans.rs:257-262` (read). Closing checks: `rejoin_system_call` requires `CallingPolicy::native_syscall_for_target(target)` (`setlements.rs:518-519`, read); ordinary `via` syscall evaluation rejects non-Linux targets at `binding_evaluation.rs:142-149` (read); native Build evaluation matches Linux only at `native_bindings/plans.rs:258-262` (read). `validate_candidate_against_schema` has no target guard (read).

**Verdict CLOSED (read):** A directly built syscall row on a non-Linux plan can pass candidate schema validation but fails on syscall realization or settlement paths before a normalized foreign call is admitted.

## A0898

Digest **effects** (`area_03`). Opened `provider_grants.rs:64-82` (read). At `80-81`, when the grant string matches neither a candidate plan name nor a trait slot name, the loop `continue`s without diagnostic (read).

Search `resolve_selected_provider_grants(` in `omega-rust`: **5** matches in **4** files (read): `binding.rs:68`, `accepted_machine_grants.rs:29`, `admission_settlement.rs:41`, plus definition/tests in `provider_grants.rs` (read). `binding.rs:67-69` uses the resolved list only for qualification grants on entries that were not skipped (read). Product path: `checked_compilation.rs:161-177` calls `reconstruct_trust_obligations` on root grants (read). `admission_settlement.rs:50-68` iterates every `root_grants` entry; if not in `provider_grants`, `resolve_non_provider_trust_grant` runs and errors on unknown names (`67-68`, read).

**Verdict CLOSED (read):** Silent skip in `resolve_selected_provider_grants` does not let a mistyped root grant through full compilation; trust reconstruction still requires each grant to resolve as provider or non-provider.

## A0899

Digest **program-entry-plan** (`area_03`). Opened `entry_signature.rs:262-267`, `286-288`, `313-320` (read). `identity()` hashes `normalized_callable_identity` with `identities.text(...).unwrap_or_default()` (`262-267`, read); parameter types use the same pattern (`284-288`, read). `from_checked_declaration` calls `validate_capture` (`190`, read), which rejects only `normalized_callable_identity.is_absent()` (`317`, read), not missing table text for a present index (read).

Search `SelectedProgramEntrySignature::from_checked_declaration`: **2** production uses (`program_entry.rs:236`, `retained_terminal_codec.rs:919`) (read). Typical production interns callable text from `machine_callable_identity_text` (`program_entry.rs:241-245`, read).

**Input reaching the branch:** A program entry whose callable identity index is present but resolves to no string in the identity table (empty canonical text).

**Verdict REACHABLE (read + inferred):** Capture accepts the signature; `identity()` commits an empty callable string, distinct from rejecting a missing index.

## A0900

Digest **executable-installation** (`area_03`). Opened `executable-installation/src/post_handoff_writer.rs:137-151`, `180-204`, `237-264` (read). `DestinationPreparationReceipt::from_admitted_provider` stores caller-supplied `pinned` and `unpublished` without validation (read).

Search `DestinationPreparationReceipt::from_admitted_provider` in `omega-rust`: definition only in `post_handoff_writer.rs:137` (read). Search `PreparedPostHandoffWriterDestination::claim`: **1** implementation (`181-204`, read). `validate_post_handoff_destination_binding` requires `receipt.pinned` and `receipt.unpublished` be true (`245-248`, read). `program-entry-plan` consumes only `ValidatedPreparedPostHandoffWriterDestination` (`program-entry-plan/src/post_handoff_writer.rs:72+`, read).

**Verdict CLOSED (read):** Unchecked constructor flags are validated at `claim` on every prepared post-handoff destination; false flags yield `InstallationDiagnostic` errors.

## A0900

(Reserved — no duplicate row.)

## A0901

Digest **runtime-abi** (`area_03`). Opened `runtime-abi/src/windows_provider.rs:12-17` (read). `image_imports_windows_internet_provider` scans any sliding window equal to `WINDOWS_INTERNET_PROVIDER_LIBRARY` bytes (read).

Search `image_imports_windows_internet_provider` in `omega-rust`: **6** matches in **3** files (read): consumer `products/src/publication.rs:221-226` (when COFF, bundles helper DLL if scan true on output or any component image); re-export in `runtime-abi/lib.rs`; tests in `windows_provider.rs` (read). `publication.rs:227-243` stages `windows_internet_provider_dll()` beside the executable when `imported` is true (read).

**Input reaching the branch:** A Windows PE image or component whose bytes embed the 14-byte sequence `omega_host.dll` in a data or padding region without a real import of that library.

**Verdict REACHABLE (read):** Publication treats raw substring presence as import evidence and may bundle the synthetic DLL though the image is not actually linked to that provider.

## What I did not read

- Full `target_providers.rs` test fixtures for family-filter edge cases.
- Every `selected_checked_candidates` downstream publication join in isolation.
- Terminal interpreter paths for table-field `storage` outside `external_binding_rows`.
- Complete PE import-table semantics versus substring heuristic.

SLICE COMPLETE
