# slice_0098

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0752 | omega-rust/omega/compilation/representations/machine-code/src/layout/text_section.rs:282-308 | CLOSED | Object custody rebuilds foreign-call rows from the full text placement (including callbacks) and requires equality. |
| A0753 | omega-rust/omega/compilation/representations/register-homes/src/constraints.rs:48 | CLOSED | `AllocatorAvailabilityPlan::decode` has no production callers; the producer builds rows from the physical model. |
| A0754 | omega-rust/omega/compilation/representations/register-homes/src/post_allocation_manifest.rs:230-236 | CLOSED | Manifest `decode` is test-only; identity recomputation binds statistics bytes but nothing cross-checks them against homes. |
| A0755 | omega-rust/omega/compilation/representations/register-homes/src/recovery/fixed_view_copy_codec.rs:688 | UNDECIDED | `structural` stays `None` when the wire omits the optional block; whether that decoded plan can reach compile acceptance was not traced. |
| A0756 | omega-rust/omega/compilation/representations/register-homes/src/recovery/selected_blocks.rs:436 | UNDECIDED | `decode_case` accepts arbitrary `case_tag` on the fixed-view-copy wire; no production `FixedViewCopyPlan::decode` caller was found. |
| A0757 | omega-rust/omega/compilation/representations/register-homes/src/recovery/selected_instructions.rs:243-249 | UNDECIDED | Recovery `decode_kind` does not bound `ReturnAggregate`/`Store` payloads; same wire-only surface as A0756. |
| A0758 | omega-rust/omega/compilation/representations/register-homes/src/recovery/structural_contracts.rs:219-327 | UNDECIDED | Memory-access decode fills place/offset/count without structural cross-check; production ingest path not confirmed. |
| A0759 | omega-rust/omega/compilation/representations/register-homes/src/recovery/structural_calls.rs:1003-1009 | CLOSED | Import/Table wire decode skips `has_valid_entry_mechanism`, but source lowering already checks bindings before register homes. |

## A0752

**Cited site (read):** `relocation_free_text_section_identity` at `machine-code/src/layout/text_section.rs:211-322`. The loop over `unresolved_normalized_foreign_calls` at `282-308` hashes kind, state, caller coordinates, invocation site, boundary, ordinal, offsets, and addend, but not `PlacedNormalizedForeignCallResolution::callback_materializations` (field at `188-208`).

**Identity use (read):** `RelocationFreeTextSectionPlacement::recomputed_identity` at `text_section.rs:80-81` calls this hasher. `callback_materializations` are populated in production at `machine-emission/src/text_placement.rs:1109-1119`.

**Callers of hasher (read):** `rg '\brelocation_free_text_section_identity\b' omega-rust` → 4 matches: definition `text_section.rs:211`, re-exports `layout.rs:54`, `machine-code/src/lib.rs:473`, `object-file/src/lib.rs:80`.

**Closing check (read):** `validate_relocation_free_object_from_text` at `object-file/src/relocation_free_object/text_section.rs:72-83` calls `construct_relocation_free_object_from_text`, which clones each `resolution` including callbacks at `111-117`, then requires `*object == construct_relocation_free_object_from_text(text, selections)?`. `validate_optimized_relocation_free_object_container` at `fragment_container.rs:58-62` always runs this before accepting a staged container.

**Input (plain words):** A hostile edit that changes only callback materialization vectors cannot pass custody while the source text section is the compiled placement, because object equality is rebuilt from the full text struct, not from the text-section identity digest alone.

**Verdict:** CLOSED.

## A0753

**Cited site (read):** `AllocatorAvailabilityPlan` field comment at `constraints.rs:48` marks `classes` as class-ID-sorted; `RegisterClassAvailability::unconstrained_views` at `112` is documented sorted. `AllocatorAvailabilityPlan::decode` at `58-105` decodes lists with no sortedness check (read `78-92`).

**Enclosing function (read):** Full `decode` body `58-105`; encode path `137-169` also does not assert order.

**Producer (read):** `compute_terminal_allocator_availability` at `selected-instructions/analyses/src/allocator_availability.rs:55-68` walks `physical.model().classes` in definition order and filters views—canonical order comes from the validated physical model, not from wire.

**Decode callers (read):** `rg 'AllocatorAvailabilityPlan::decode' omega-rust` → 7 matches, all in `constraints.rs` inside `#[cfg(test)]` (`659-686`).

**Verdict:** CLOSED for Omega-source compile (plan is computed, not decoded); unsorted wire could decode in tests only.

## A0754

**Cited site (read):** `PostAllocationOptimizationManifest::decode` at `post_allocation_manifest.rs:176-265` reads five `u64` statistics fields at `230-236` into `PostAllocationStatistics` with no comparison to digested `homes`, `ranges`, or `liveness`.

**Identity (read):** `encode_manifest_content` at `409-417` includes statistics in the recomputed digest; `decode` rejects at `261-262` on `IdentityMismatch` if framed identity bytes disagree with recomputation including those counts.

**Producer (read):** Pipeline `05_selected-instructions-to-register-homes/src/assignment.rs` builds manifests in memory (statistics referenced at `477` per grep); I did not read the full assignment function.

**Decode callers (read):** `rg 'PostAllocationOptimizationManifest::decode' omega-rust` → 12 matches, all in `post_allocation_manifest.rs` tests (`547-762`).

**Verdict:** CLOSED for compile acceptance (statistics are not verified against homes, but manifest wire is not decoded on the traced production path).

## A0755

**Cited site (read):** `decode_function` at `fixed_view_copy_codec.rs:659-700` sets `structural: None` at `688`, then calls `decode_contracts` at `696`.

**`decode_contracts` (read):** `structural_contracts.rs:184-192` optionally decodes a signature via `optional(..., decode_signature, ...)` into `function.structural`; if the wire tag is `0`, `structural` remains `None`.

**Caller (read):** `decode_function` is reached from `decode_v7` / `FixedViewCopyPlan::decode` in the same crate (`fixed_view_copy_codec.rs:66-88`).

**Production ingest (read):** `rg 'FixedViewCopyPlan::decode' omega-rust` → matches only in `recovery/codec_tests.rs` (no matches outside tests). `compute_terminal_fixed_view_copies` at `pipeline/05/.../fixed_view_copy_recovery.rs:617-711` clones `selected.plan()` in memory and never calls `decode`.

**Verdict:** UNDECIDED — digest “not traced” stands; I did not find a production path that decodes a plan missing `structural` and then accepts a program.

## A0756

**Cited site (read):** `decode_case` at `selected_blocks.rs:436-492` reads `case_tag: cursor.i32()?` at `456` and decodes payloads and `cleanup_actions` via `decode_list(cursor, decode_cleanup)` at `485-491` without matching the tag to the named `case` or checking cleanup against the case layout.

**Call path (read):** Used from successor/block decode in the fixed-view-copy codec (e.g. `decode_successor` region above `436`); entire module is under `register-homes/src/recovery/`.

**Production (read):** Same as A0755 — `FixedViewCopyPlan::decode` is test-only per repository search.

**Verdict:** UNDECIDED — wire can carry inconsistent case tags, but no traced link from that decode to source-program acceptance.

## A0757

**Cited site (read):** `decode_kind` at `recovery/selected_instructions.rs:204-249` (tags `36` and `24`): `ReturnAggregate { fragment_count: cursor.byte()? }` at `243-245` and `Store { byte_offset, byte_size: cursor.byte()? }` at `246-249` with no range checks against ABI or memory contracts.

**Parallel (read):** Canonical selected-plan encoding uses the same shape at `selected-instructions/src/effects/program/encoding.rs:603-605` for tag `36` (read snippet only).

**Callers (read):** `decode_kind` is used from fixed-view-copy instruction decode inside `recovery/` (not fully listed); `FixedViewCopyPlan::decode` is test-only.

**Verdict:** UNDECIDED — unbounded wire values decode, but production compile uses in-memory `SelectedProgram` from pipeline `03`, not this recovery decoder.

## A0758

**Cited site (read):** `decode_contracts` memory-access loop at `structural_contracts.rs:210-327`: after reading `place`, `byte_offset`, and `byte_count` at `219-221`, rows are pushed at `320-327` without validating against function structural layout or instruction bounds.

**Contrast (read):** Some transport decoders in `selected_blocks.rs` reject invalid shapes (e.g. `InvalidStructuralTransport` at `407` in the same file family).

**Production (read):** Invoked only through fixed-view-copy function decode; `FixedViewCopyPlan::decode` has no non-test callers.

**Verdict:** UNDECIDED — acceptance of arbitrary memory-access metadata on wire was not connected to program compile success.

## A0759

**Cited site (read):** `decode_call_binding` at `structural_calls.rs:875-1010`. SystemCall path checks `has_valid_entry_mechanism` at `909-914`. After building Import/Table/Component mechanism at `984-1001`, only `Component` is filtered at `1003-1008` with `!binding.has_valid_entry_mechanism()`.

**Mechanism check (read):** `has_valid_entry_mechanism` at `target-operations/src/boundary/realizations.rs:215-261`; Import/Table arms require native policy and call-return entry at `238-242`.

**Source compile (read):** `legalization_target.rs:3260` and `selection_call_abi.rs:846` (grep) reject foreign calls when `!binding.has_valid_entry_mechanism()` during lowering to selected instructions—before register-home recovery codecs run.

**Decode callers (read):** `rg '\bdecode_call_binding\b' omega-rust` → 8 matches: definition/call at `746` in `structural_calls.rs` plus unit tests in the same file (`1845+`); no pipeline references.

**Verdict:** CLOSED for Omega-source compilation; gap remains on fixed-view-copy wire decode only.

## What I did not read

- Full `PostAllocationOptimizationManifest` test module and full `assignment.rs` statistics computation.
- Entire `validate_relocation_free_object` foreign-field loop beyond `mod.rs:449-467` (callback byte placement in text).
- Full `legalization_target.rs` and `selection_call_abi.rs` call sites (only grep hits).
- `wiki/spec` or digest reader notes beyond `area_05.md` preamble and items 89-96.
- Psi frontend validation for case tags or memory accesses on the source path.

SLICE COMPLETE
