# Packet unit_046: Psi descriptor results and replay

Status: in progress.

This packet reviews the correctness observations in [record unit_046](../../records/unit_046.md) (area `07-psi-semantics`, crate `terminal-semantics`, files under `source-rust/subsystems/compiler/psi/semantics/src/descriptors/`, written `D/` below) and the matching bullets in [digest 07](../../digests/07-psi-semantics.md). The survey read commit `4f3c255d`; every receipt below was checked against `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. No supplemental gap ranges are assigned to unit_046 in `second_pass_ranges.tsv`. Items in digest section 5 that cite files outside this unit (`D/replay/case_flow.rs`, `D/replay/engine.rs:2632`, `D/replay/summary_interface.rs`) belong to other packets and are not reviewed here.

Outcomes use the packet vocabulary: fixed, intended, duplicate, feature gap, reproduced, still open. "Intended" means the code refuses or stays conservative for the case the record worried about, with the source or test that shows it.

## Receipts

### 1. Descriptor graphs default to an empty list

- **Evidence:** record `D/descriptor_results.rs` "Fail-open risks" bullet 1 and "Five things" item 2; digest 07 lines 132, 171, 269 and section 10 item 12 (line 291).
- **Current source:** `D/descriptor_results.rs:1214-1223` (`terminal_graph`, `terminal_graph_from`) and `:1662-1668` (`terminal_reference_graph`, `terminal_borrowed_storage_graph`) still call `unwrap_or_default()`.
- **Contract:** a machine whose descriptor graph could not be built must not be read as "nothing to check".
- **Trace:** every production caller of the four builders reaches the graph through `descriptor_machine` or `DescriptorAnalysis`, and both report a missing machine. `Replay::machine` (`D/replay/engine.rs:763-770`) returns `None` for a missing or duplicated row, so `payload_targets` (`D/replay/analysis.rs:827-839`) and the other analysis queries return `None`, and `case_is_established` (`:340-351`) returns `false`. The callers turn that into a refusal:
  - `psi/semantics/src/content_transformations.rs:137` → `current_selected_payload_contract` errors with "selected payload backing is unresolved" (`D/descriptor_results/payload_history.rs:520-523`).
  - `psi/semantics/src/calls/evidence_calls/terminal_inputs.rs:160-161` errors with "runtime evidence body has no current descriptor machine".
  - `psi/artifact/wire/src/semantic_module.rs:1119-1140` errors when `analysis.result_slots(row)` is `None`.
  - `psi/artifact/wire/src/foundation_validation.rs:3186` refuses when `case_is_established` is false, and `:7033` refuses when `element_payload_memberships` is `None`.
  - `psi/semantics/src/references/reference_origins/memberships.rs:110` and `psi-to-native/verification/provider-admission/src/access_lifecycles.rs:109` pass the graph to replay-based queries that error on a missing input (`bind_profile` maps each `None` to a lifecycle diagnostic).
  - `psi-to-native/.../ranked_cycles_validation.rs:819` calls a different, native `graph::terminal_graph`.
- **Outcome: intended.** The empty list loses the reason for the failure, but no traced consumer accepts a program because of it. The digest's correction that `terminal_graph_from` also defaults (section 10 item 12) still holds.

### 2. `terminal_definition` treats unlisted place kinds as locals

- **Evidence:** record `D/descriptor_results.rs` "Fail-open risks" bullet 2 and "What you are unsure about" bullet 2; digest 07 line 133.
- **Current source:** `D/descriptor_results.rs:1723-1782`; the catch-all is `:1781`. The unlisted kinds of `StructuralPlaceKind` (`foundation/semantic-records/src/storage.rs:735-762`) are `Result`, `ProviderAttachment` and `TrivialAffineLocal`.
- **Contract:** `DescriptorDefinition::Local` lets `Replay` leave a write out of the callee's published write frame (`D/replay/engine.rs:727-728`), so only storage the caller cannot reach may be `Local`.
- **Trace:** `TrivialAffineLocal` is callee-local storage, and `terminal_flow.rs:77` already treats it as independently owned. A `Result` place reaches the caller through result slots, not as caller storage. `ProviderAttachment` is the only kind that could name storage outside the callee, and published Psi never contains one: wire validation refuses every machine that has one with "provider-backed attachment specialization is incomplete" (`psi/artifact/wire/src/foundation_validation.rs:3582`, `:12300-12311`), the lowered-form checker refuses it as "a root place kind outside this route" (`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:2007-2010`), and `plain_affine` refuses "affine provider storage" (`psi/semantics/src/plain_affine.rs:1739-1741`).
- **Outcome: intended** for current inputs. If provider-backed attachments are admitted later, this arm needs an explicit `ProviderAttachment` row; that is part of the same feature, not a present defect.

### 3. `edge_feasibility` and `case_feasibility` disagree on a missing state

- **Evidence:** record `D/replay/analysis.rs` "Fail-open risks" bullets 1-3; digest 07 line 143 (first half).
- **Current source:** `D/replay/analysis.rs:243-273` (`flow_feasibility`), `:274-315` (`edge_feasibility`, `ProvedImpossible` at `:300`), `:316-339` (`case_feasibility`, `Unknown` at `:336`).
- **Contract:** `ProvedImpossible` may be returned only for a position the replay proved unreachable.
- **Trace:** all three return `Unknown` unless `prepare_states` succeeded. `prepare_states_checked` stores `PreparedStateCache::Ready` only after its worklist empties (`D/replay/engine.rs:3043-3288`); every bound, depth limit or unreconstructable step returns `None` and leaves `Failed`. Inside the loop a successor is skipped only when `case_flow::select` proves the case false (`:3221-3233`). A position with no `before` state in a ready snapshot is therefore unreachable, so `ProvedImpossible` is sound. `case_feasibility` returning `Unknown` for the same position is less precise, not unsound.
- **Outcome: intended.** The difference costs precision only.
