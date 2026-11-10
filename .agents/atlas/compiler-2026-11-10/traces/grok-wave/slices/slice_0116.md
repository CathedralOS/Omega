slice_0116

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0894 | target_providers.rs:643-653, 698-722 | CLOSED | Vacuous target-family and `selecting_declaration` None matches are blocked before roster/admission by empty-candidate, exactly-one-instance, and single-position checks downstream. |
| A0895 | provider_plan.rs:204-209, 284-289 | CLOSED | `OnceLock` memo can go stale after `pub` mutation (read), but grant replay and `PartialEq` compare structure and digest together so a mutated plan is not admitted as the granted plan. |
| A0896 | provider_plan.rs:867-882 | CLOSED | `validate_candidate_against_schema` does not reject `storage: Err`, but `field_offset` at external binding realization errors before lowering. |
| A0897 | provider_plan.rs:841 | CLOSED | Schema validation only checks `u32` range; `conservative_syscall_terminal_mechanism` rejects non-Linux targets during provider admission settlement. |
| A0898 | provider_grants.rs:80-82; binding.rs:68-76 | CLOSED | Unknown grant strings are skipped in provider resolution, but `admit_trust` / `reconstruct_trust_obligations` rejects them via `resolve_non_provider_trust_grant`. |
| A0899 | entry_signature.rs:265-267, 286-288 | CLOSED | `identity()` can hash missing text as `""`, but product paths build signatures through `from_checked_declaration` with co-built identity tables so indices resolve. |
| A0900 | post_handoff_writer.rs:137 | CLOSED | `from_admitted_provider` stores caller flags as-is, but `PreparedPostHandoffWriterDestination::claim` runs `validate_post_handoff_destination_binding` on `pinned`/`unpublished`. |
| A0901 | windows_provider.rs:14 | REACHABLE | Substring scan can treat any image containing `omega_host.dll` bytes as needing the Windows internet provider when staging publications. |

### A0894

1. Read `selected_provider_candidate_rows` in full at `omega-rust/omega/compilation/backend/plans/backend-plan/src/target_providers.rs:600-761` (read) and the target-family predicate at `:643-653` (read): for each `target_machine_families` row, a candidate passes when no family member names that callable, or some member pairs `plan.target` with the callable.
2. Read obligations filter at `:698-722` (read): `obligations.selecting_declaration.as_ref().is_none_or(|declaration| …)` is true when `selecting_declaration` is `None`, so those obligation rows are not filtered out by owner mismatch.
3. Search `selected_provider_candidate_rows` in `omega-rust/`: 7 hits in 4 production files (`target_providers.rs`, `lib.rs`, `program_entry.rs`, `component_verification.rs`) plus tests/support (read grep).
4. Read callers: `checked_candidate_roster` `:432-455` (read) errors when `matched.candidates.is_empty()` after `selected_checked_candidates`; obligation loop `:724-756` errors when instance `count != 1`; `program_entry.rs:427-431` (read) requires exactly one `candidate_position`; `component_verification.rs:319-324` (read) errors on empty `candidate_positions`.
5. Input to accepting branches: Terminal module provider candidates and instance-requirement rows that already matched package/owner/schema filters but may sit outside every target machine family (`:643-653`) or carry `selecting_declaration: None` (`:707-721`).
6. **Verdict: CLOSED** — `checked_candidate_roster` at `target_providers.rs:441-448` and instance resolution at `:747-756` prevent admitting checked providers without exact realizations; `program_entry.rs:427-431` does the same for physical entry adapters.
7. Unsure: I did not read every `selected_checked_candidates` caller beyond roster, close specialization, synchronous-call validation, and the cited consumers.

### A0895

1. Read `ProviderPlan` and memo at `provider_plan.rs:82-116`, `identity_digest` / `report_fingerprint` at `:204-208` and `:284-288` (read): first call fills `digest_memo` via `get_or_init`; later changes to `pub` fields do not invalidate the memo.
2. Read `PartialEq` at `:142-160` (read): equality is structural (rows, schema, names), not memo-based.
3. Read `ResolvedSelectedProviderGrant::replays_selected_plan` at `provider_grants.rs:57-60` (read): requires `report_fingerprint`, `identity_digest`, and `==` on the full plan.
4. Read native plan assembly `native_bindings/plans.rs:210-216` (read): rows are pushed before the plan is returned; I did not find a production site that calls `identity_digest()` or `report_fingerprint()` and then mutates the same plan in the same pipeline phase (inferred from grep on `identity_digest` in provider-planning).
5. Input: any `ProviderPlan` value whose fields are edited after the first memoized digest/fingerprint call.
6. **Verdict: CLOSED** — stale memo could mis-report fingerprints, but admission paths that trust plans also require structural `==` (`provider_grants.rs:60`; `binding.rs:97-103`).
7. Unsure: I did not exhaustively prove no validator compares digest alone without structural checks outside grant replay.

### A0896

1. Read `validate_candidate_against_schema` `VtableField` / `TableFunction` arm at `provider_plan.rs:867-882` (read): only `table.is_empty()` and `field.is_empty()` are checked; `storage: Err(_)` is not an error.
2. Read digest encoding at `digest_encoder.rs:348-356` (read): `Err` reason is encoded into the identity digest, so invalid storage is representable but not rejected here.
3. Read consumer `external_binding_rows.rs:124-147` and `field_offset` `:211-218` (read): `storage.as_ref().map_err(Clone::clone)?` turns `Err` into a diagnostic when realizing external binding rows.
4. Read `prepared_providers.rs:278-286` (read): native candidates must pass `validate_candidate_against_schema` before settle; broken table storage still fails later during binding row realization (inferred).
5. Input: a `ProviderPlanRow` whose `ProviderBinding::VtableField` or `TableFunction` carries `storage: Err(String)` with non-empty `table` and `field`.
6. **Verdict: CLOSED** — the accepting validation is real, but compilation does not complete; `field_offset` reports the failure (`external_binding_rows.rs:141-147`).
7. Unsure: I did not trace every entry point that only runs `validate_candidate_against_schema` without ever building external binding rows (e.g. some test-only paths).

### A0897

1. Read `ProviderBinding::Syscall` check at `provider_plan.rs:841-849` (read): only `u32::try_from(*number)` is validated; target OS is not checked.
2. Read admission path `source_imports.rs:118-132` (read) calling `conservative_syscall_terminal_mechanism`.
3. Read `external_mechanisms.rs:110-124` (read): returns error when target is not `LinuxX64` or `LinuxArm64`.
4. Search `ProviderBinding::Syscall` in `omega-rust/`: many hits; production bindings flow through provider admission settlement (read grep summary).
5. Input: a plan row with `Syscall { number }` in `0..=u32::MAX` on a non-Linux `ProviderPlan.target`.
6. **Verdict: CLOSED** — schema validation accepts it; settlement rejects the mechanism for unsupported targets (`external_mechanisms.rs:116-123`).
7. Unsure: I did not read every syscall mint path to confirm all reach settlement.

### A0898

1. Read skip at `provider_grants.rs:80-82` (read): `continue` when grant matches neither plan name nor slot trait identity among candidates.
2. Search `resolve_selected_provider_grants` in `omega-rust/`: 5 hits in 4 files (`provider_grants.rs`, `binding.rs`, `accepted_machine_grants.rs`, `admission_settlement.rs`) (read grep).
3. Read `binding.rs:67-69` (read): unresolved grants simply never appear in `provider_grants`; no error for unknown strings in this phase.
4. Read `admission_settlement.rs:50-68` and `accepted_machine_grants.rs:63-98` (read): grants not in `provider_grants` go to `resolve_non_provider_trust_grant`, which errors at `:96-98` when nothing matches.
5. Read `checked_compilation.rs:161-178` and `product_execution.rs:250-252,317-319,380-382` (read): product completion calls `admit_trust`, which runs `reconstruct_trust_obligations`.
6. Input: a root grant string that names neither a candidate plan, a known slot trait, nor an accepted machine.
7. **Verdict: CLOSED** — `binding.rs` alone would skip silently, but shipped compilation paths call `admit_trust` after binding, which rejects unknown grants (`accepted_machine_grants.rs:96-98`).
8. Unsure: I did not read non-product consumers of `bind_selected_provider_plan_facts` in isolation.

### A0899

1. Read `identity()` at `entry_signature.rs:226-310` (read): `identities.text(...).unwrap_or_default()` at `:265-267` and `:286-288` hashes empty string when lookup returns `None`.
2. Read `validate_capture` at `:313-336` (read): rejects `normalized_callable_identity.is_absent()` and parameter `normalized_type_identity.is_absent()`, but does not assert `text(index).is_some()`.
3. Read `TerminalIdentityIndex::is_absent` at `psi/compilation/representations/terminal-psi/src/identity.rs:251-253` (read): only index `0` is absent; out-of-range indices yield `None` from `text` at `:354-356`.
4. Read production builder `program_entry.rs:236-248` (read): `from_checked_declaration` after interning callable and types into the same `identities` table built in that function.
5. Input: a signature with non-absent identity index whose table has no entry (hypothetical bypass of `from_checked_declaration`).
6. **Verdict: CLOSED** for the normal compile path — indices are interned with the table before validation; `from_checked_declaration` gates construction (`entry_signature.rs:190-191`).
7. Unsure: I did not fully audit codec decode paths in `retained_terminal_codec.rs` for mismatched index/table pairs.

### A0900

1. Read `DestinationPreparationReceipt::from_admitted_provider` at `executable-installation/post_handoff_writer.rs:137-151` (read): copies `pinned` and `unpublished` without checking them.
2. Read `PreparedPostHandoffWriterDestination::claim` at `:181-197` (read) calling `validate_post_handoff_destination_binding` at `:237-248` (read), which requires `receipt.pinned` and `receipt.unpublished`.
3. Search `DestinationPreparationReceipt::from_admitted_provider(` in `omega-rust/`: only the definition in `post_handoff_writer.rs` (read grep); no production call sites found.
4. Search `validate_post_handoff_destination_binding`: only used from `claim` in the same file (read).
5. Input: receipt built with `pinned: false` or `unpublished: false` passed into `claim`.
6. **Verdict: CLOSED** — the pub constructor accepts arbitrary flags, but the only validation entry for prepared destinations enforces pinning and unpublished state (`post_handoff_writer.rs:245-248`).
7. Unsure: future callers might use the receipt without `claim`; none are wired today.

### A0901

1. Read `image_imports_windows_internet_provider` at `runtime-abi/windows_provider.rs:14-18` (read): sliding window equality against `b"omega_host.dll"`.
2. Search `image_imports_windows_internet_provider` in `omega-rust/`: 4 hits — `lib.rs` re-export, `publication.rs:221-225`, and unit test in `windows_provider.rs` (read grep).
3. Read consumer `products/publication.rs:219-244` (read): for COFF targets, if main output or any component image matches, stages `omega_host.dll` beside the publication.
4. Input: any byte image containing the 13-byte substring `omega_host.dll` (e.g. embedded in a string section), even without a real PE import.
5. **Verdict: REACHABLE** — publication will bundle the synthesized provider DLL though the program may not import those symbols.
6. Unsure: I did not trace whether later install steps reject the extra DLL; staging still occurs (read `publication.rs:227-243`).

### What I did not read

- Full bodies of all `selected_checked_candidates` callers beyond those cited for A0894.
- Complete `retained_terminal_codec` decode validation for program entry signatures (A0899).
- Provider-admission paths that might stop after `validate_candidate_against_schema` without external binding realization (A0896).
- Post-handoff writer call graph outside `executable-installation/post_handoff_writer.rs` (A0900).

SLICE COMPLETE
