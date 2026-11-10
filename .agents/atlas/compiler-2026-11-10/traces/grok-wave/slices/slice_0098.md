slice_0098

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0752 | machine-code/src/layout/text_section.rs:282-308 | REACHABLE | Foreign-call identity hashing omits `callback_materializations`, so two placements can share one digest while differing on callbacks. |
| A0753 | register-homes/src/constraints.rs:48,112 | REACHABLE | `AllocatorAvailabilityPlan::decode` reads class/view lists without enforcing comment-stated sortedness. |
| A0754 | register-homes/src/post_allocation_manifest.rs:171-265 | CLOSED | Statistics are bound into manifest identity via `encode_manifest_content` and checked at decode (`recomputed_identity`). |
| A0755 | register-homes/src/recovery/fixed_view_copy_codec.rs:688,696 | UNDECIDED | Optional `decode_contracts` signature may leave `structural: None`; did not trace whether every downstream path requires it. |
| A0756 | register-homes/src/recovery/selected_blocks.rs:436 | REACHABLE | `decode_case` accepts any non-negative `case_tag` and cleanup list without cross-check to the structural case. |
| A0757 | register-homes/src/recovery/selected_instructions.rs:243-249 | CLOSED | Transformed-plan digest check plus later `ReturnAggregate`/`Store` checks close semantic gaps on the compile path. |
| A0758 | register-homes/src/recovery/structural_contracts.rs:184 | UNDECIDED | Memory-access rows decode with unchecked place/offset/count; no caller trace to a program accept/reject gate. |
| A0759 | register-homes/src/recovery/structural_calls.rs:1003-1009 | CLOSED | Import/Table skip decode-time `has_valid_entry_mechanism`, but emission re-checks via `calls::instantiate_foreign_call_constraint`. |

### A0752

1. Read `relocation_free_text_section_identity` in full (`text_section.rs:211-321`, read); `PlacedNormalizedForeignCallResolution` includes `callback_materializations` at `text_section.rs:208` (read).
2. Search `rg 'relocation_free_text_section_identity' omega-rust` → 5 hits (4 defs/re-exports + `impl::recomputed_identity`, read); search `rg 'callback_materializations' omega-rust/omega/compilation/backend/machine-emission/src/text_placement.rs` → hits at placement and fixup walk (read).
3. Callers: `text_placement.rs:724` sets `section.identity = section.recomputed_identity()`; `text_placement.rs:1160-1173` `check_placed_section` requires `section.identity == section.recomputed_identity()`; foreign fixups walked at `text_placement.rs:1360-1431`; `check_foreign_call` at `text_placement.rs:1599-1684` does not compare `row.callback_materializations` (read).
4. Input: in-memory `RelocationFreeTextSectionPlacement` or foreign-resolution rows whose only delta is callback materialization vectors still match the same identity bytes.
5. **Verdict: REACHABLE** — Two placements that differ only in callback materialization metadata can share one text-section identity; anything that treats that digest alone as proof of placement can accept the wrong one. **Unsure:** Whether any shipped path uses only the weak digest without `object_artifact.rs:2417-2420` or full section bytes.

### A0753

1. Read `AllocatorAvailabilityPlan::decode` in full (`constraints.rs:58-106`, read); field comments at `constraints.rs:48` and `constraints.rs:112` (read).
2. Search `rg 'AllocatorAvailabilityPlan::decode' omega-rust` → 6 hits, all in `constraints.rs` tests (read).
3. Callers: production builds via `selected-instructions/analyses/src/allocator_availability.rs:30-74` `compute_terminal_allocator_availability` → `seal_allocator_availability` (`allocator_availability.rs:143-150`, read); no production `decode` caller found.
4. Input: OMGAVA-framed bytes with class or `unconstrained_views` rows permuted but self-consistent identity framing.
5. **Verdict: REACHABLE** — Unsorted wire rows decode successfully; identity only matches the tampered order, not canonical sortedness. **Unsure:** Whether any non-test loader feeds decoded availability directly into allocation without recomputing from the register model.

### A0754

1. Read `PostAllocationOptimizationManifest::decode` in full (`post_allocation_manifest.rs:176-264`, read).
2. Search `rg 'PostAllocationOptimizationManifest::decode' omega-rust` → 12 hits, all in `post_allocation_manifest.rs` tests (read).
3. Callers: in-process manifest construction; decode path checks `manifest.identity != manifest.recomputed_identity()` at `post_allocation_manifest.rs:261-262` (read); statistics included in hash at `post_allocation_manifest.rs:409-417` (read).
4. Input: five arbitrary `u64` statistics fields inside a tamper-consistent manifest body.
5. **Verdict: CLOSED** — Wrong statistics cannot pass decode unless the envelope identity is forged to match; no read evidence that compilation decisions use statistics without the digest-bound manifest.

### A0755

1. Read `decode_function` and `decode_contracts` (`fixed_view_copy_codec.rs:659-700`, `structural_contracts.rs:184-209`, read).
2. Search `rg 'FixedViewCopyPlan::decode' omega-rust` → 66 hits, all `codec_tests.rs` (read).
3. Callers: pipeline builds copies in memory (`pipeline/05_selected-instructions-to-register-homes/`, read headers only); decode is test/artifact boundary.
4. Input: wire function with `structural: None` after optional signature tag 0 in `decode_contracts`.
5. **Verdict: UNDECIDED** — `None` is valid for some functions (`image-emission` allows it when parameters empty, `function_fragments.rs:18633`, read); did not trace every path from OMGFCV decode to a hard reject when calls/memory need a signature.

### A0756

1. Read `decode_case` in full (`selected_blocks.rs:436-493`, read).
2. Search `rg 'decode_case' omega-rust/omega/compilation/representations/register-homes` → 5 hits (`selected_blocks.rs`, `structural_declarations.rs`, read names only).
3. Callers: successor decode inside fixed-view-copy payload → `FixedViewCopyPlan::decode` → post-decode liveness `validate_case_transport` (`liveness.rs:1998-2033`, read).
4. Input: structural case edge with arbitrary `case_tag` ≥ 0 and unconstrained `cleanup_actions`.
5. **Verdict: REACHABLE** — Liveness rejects only `case_tag < 0` and continuation cleanup (`liveness.rs:2011-2015`, read), not mismatch to the named case variant. **Unsure:** Whether codegen always derives tags from the case definition so hostile wire is the only concern.

### A0757

1. Read `decode_kind` arms (`selected_instructions.rs:243-249`, read).
2. Search `rg 'fragment_count' omega-rust/omega/compilation/backend/machine-emission/src/exit_contract.rs` and `machine_effects.rs:377` (read).
3. Callers: `FixedViewCopyPlan::decode` checks `selected_instruction_plan_identity` (`fixed_view_copy_codec.rs:80-83`, read); emission checks `ReturnAggregate` at `exit_contract.rs:2629-2632` and `Store` width at `machine_effects.rs:377-381` (read).
4. Input: wire `ReturnAggregate` / `Store` kinds with arbitrary `fragment_count` / `byte_size`.
5. **Verdict: CLOSED** — Decode accepts raw bytes, but digest mismatch or later machine-effect/exit-contract checks reject inconsistent shapes on paths that reach native emission.

### A0758

1. Read `decode_contracts` memory-access loop (`structural_contracts.rs:210-327`, read).
2. Search `rg 'memory_accesses' omega-rust/omega/compilation/representations/selected-instructions/analyses` → sparse hits, no validation helper found (read grep only).
3. Callers: `decode_function` → `decode_contracts` inside OMGFCV payload (read); did not read machine-emission validators for place/offset/count agreement.
4. Input: memory-access row with any `PlaceId`, `byte_offset`, `byte_count`, and role tag.
5. **Verdict: UNDECIDED** — Decode is permissive; no traced check from decoded row to program rejection was read in this slice.

### A0759

1. Read `decode_call_binding` in full (`structural_calls.rs:875-1010`, read); entry description matches: Component/System guarded, Import/Table not at `1003-1009` (read).
2. Search `rg 'has_valid_entry_mechanism' omega-rust/omega/compilation/representations/register-homes/src/recovery/structural_calls.rs` (read).
3. Callers: decoded bindings land in `SelectedNormalizedForeignCall`; `calls::instantiate_foreign_call_constraint` requires `call.binding.has_valid_entry_mechanism()` and `validate_boundary_entry_plan` (`calls.rs:101-115`, read).
4. Input: Import/Table wire binding that decodes but fails `has_valid_entry_mechanism` or boundary plan validation.
5. **Verdict: CLOSED** — Decode-time gap is harmless for emission because constraint instantiation returns `None` when mechanism or plan validation fails (`calls.rs:101-115`, read).

### What I did not read

- Full `check_placed_functions` after `text_placement.rs:1432`.
- `pressure_rematerialization_recovery.rs` spill/recovery wire decode.
- `allocation_legality.rs` for sorted `candidates` vectors (comment at `constraints.rs:212` only; no `AllocationLegalityPlan::decode` in `constraints.rs`).
- Corpus/xtask validators that might call these decodes outside `omega-rust/omega/compilation`.

SLICE COMPLETE
