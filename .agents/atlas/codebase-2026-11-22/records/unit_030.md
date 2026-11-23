# Record: unit_030 (area 12-native-representations)

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/representations/` (written `R/`). Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Everything below was read; counts come from the Grep tool in count/content mode over `source-rust/` unless stated.

## 1. optimization-records (crate `optimization-core`)

### R/optimization-records/src/manifest/tests/pass.rs (1-93)
- Purpose: two unit tests for `OptimizationPassManifestRecord::new`: a pass record binds an ordered rule list, decisions and work usage and round-trips on the wire; bad rule lists and decisions naming an unlisted rule are refused.
- Shape: tests `pass_record_binds_rule_order_decisions_and_usage` (`:13`), `duplicate_and_unscheduled_manifest_identities_reject` (`:53`). Read.
- How it decides: refusal keys are the error variants `RuleSetIdentityMismatch` (reordered rules, `:48`), `DuplicateRuleIdentity` (`:65`), `DecisionNamesUnscheduledRule` (`:91`). The constructor itself is not in this unit.
- Same job elsewhere / unused / stated limits / fail-open: none seen (test file).

### R/optimization-records/src/manifest/tests/work_usage.rs (1-45)
- Purpose: one test that `OptimizationWorkUsage::within(budget)` is false when any one of the five counters exceeds its budget and true at the exact budget (`:7-45`).
- Fail-open: none seen. It pins the five axes (rule_evaluations, candidates, validation_steps, commits, iterations).

### R/optimization-records/src/manifest/work_usage.rs (1-51)
- Purpose: the five-counter record of work a pass actually used, with the check against an `OptimizationWorkBudget` and a 40-byte codec.
- Shape: `OptimizationWorkUsage` (`:9`, five `pub u64` fields), `within` (`:18`, const, five `<=` compares against the budget getters), `encode` (`:26`) -> `[u8; 40]` via `record_encoding::encode_work_counters`, `decode` (`:36`) returning `OptimizationManifestDecodeError::WrongLength{expected:40}` on any other length.
- How it decides: `within` is purely numeric; keyed on value (counter <= budget).
- Unused / one-caller: grep `OptimizationWorkUsage` over `source-rust` = 215 lines in 37 files (stages 03, 06, 07, 08, 12, the `machine-code`, `frame-admission`, `register-homes` crates and `published-objects`). Widely used.
- Fail-open: none seen (decode refuses non-40 lengths).

### R/optimization-records/src/record_encoding.rs (1-32)
- Purpose: shared byte helpers for the records in this crate: a count-width switch and the 40-byte work-counter codec.
- Shape: `CountFrame` (`:2`) with `Wire` -> `push_count_u32`, `Canonical` -> `push_count_u64` (`:9-12`); `encode_work_counters` (`:16`); `decode_work_counters` (`:24`, `try_into` a `[u8;40]`, `None` otherwise).
- Fail-open: none seen. `decode_work_counters` returns `None` on any wrong length, which the caller turns into an error.

## 2. post-allocation-machine/body-facts (crate `physical-program`)

### R/post-allocation-machine/body-facts/src/lib.rs (1-132)
- Purpose: obligations that travel with the machine form after the operation graphs are dropped: publication facts, closed private-function references and installation resources, then the same facts after frame binding (`BoundBodyContracts`).
- Shape: `NativeBodyFacts` (`:14`: `semantic: BodyPublication<SemanticEmissionFacts>`, `private_references: Vec<MachineFunctionIdentity>`, `installations: Arc<PlannedInstallationResources>`), `retain_selected_body_facts` (`:25`), `into_framed_body_facts` (`:37`), `EmissionFacts` (`:55`), `PhysicalEmissionFacts` (`:62`, a dozen fields, several are `Result<_, String|&'static str>` carried as data), `BoundBodyContracts` (`:82`) with `needs_installed_branch_checks` (`:88`) and `encoding_subject` (`:95`).
- Inputs/outputs: re-exports `BodyPublication`, `RetainedTargetOperations` from `verified_optimization_input::publication::target_replay` (`:7-9`). Produces a `BoundEncodingSubject` (`:125`).
- How it decides: `into_framed_body_facts` compares `self.private_references != actual_references` (a vector equality on `MachineFunctionIdentity`) and returns the string error "physical selection changed the closed native private-body references" (`:47-49`). `encoding_subject` returns one string error if any of seven identity/tuple comparisons differ (`:107-123`): installation admission match, frame match via `calls.physical_identity`, `(psi, machine)` vs `(roots.psi, roots.entry)`, `(calls.psi, calls.target)`, stack `(psi, target)`, foreign-call plan `(psi, target, physical_identity)` only when the foreign result is `Ok`, and semantic `(program.psi, program.entry)`. All keyed on identities.
- Fail-open: `encoding_subject` skips the foreign-call comparison when `facts.physical.foreign_calls` is `Err` (`:116`, `is_ok_and`) - traced: an `Err` foreign-call result is carried as data and not compared here; whether another site refuses it is not read. `needs_installed_branch_checks` returns false when there is no installed entry execution (`:89-91`, `is_some_and`) - open (no comment).
- Unused / one-caller: grep `NativeBodyFacts|BoundBodyContracts` over `source-rust` outside `body-facts/` = 20 lines in 10 files (stages 05, 06, 07, 08, 09, 10, 14 and `tests/physical_publication/mod.rs`). Used.
- Stated limits: header "There are no blocks, executable operation sequences, source artifacts or callbacks" (`:3-4`).

## 3. post-allocation-machine

### R/post-allocation-machine/src/adapters.rs (1-515)
- Purpose: a closed list of physical adapter operations (frame, memory, constants, x86 fixed templates, 32-bit scalar ops) and, for each, the registers and machine state it reads, writes, uses and clobbers.
- Shape: `AdapterInstructions{architecture, operations}` (`:7`) with `new`, `push`, `frame`, `effects` (`:215`, folds `StateEffectsEvidence::compose`); `AdapterOperation` (`:13`, 20 variants); `Scalar32Operation` (`:88`, 7); `X86AdapterTemplate` (`:121`, 18 fixed templates); `AdapterOperation::effects` (`:231`); private `Effects` accumulator (`:164`); `needs_address_register` (`:225`).
- Inputs/outputs: reads `calling_conventions::{MachineRegister, MachineState, StateEffectsEvidence}` and `target::Architecture`; produces a `StateEffectsEvidence`.
- How it decides: effects are a hand-written `match` on the variant and on `architecture`, e.g. AArch64 needs a scratch register when `!offset.is_multiple_of(size) || offset > 4095*size` (`:225-228`); `FrameAddress` on AArch64 reads its destination when `(offset>>12) > 4095 || (offset >= 4096 && offset & 4095 != 0)` (`:282-286`); `Constant` reads its destination on AArch64 when `value >> 16 != 0` (`:298-304`, comment: "each MOVK reads its destination"). Keyed on value ranges and architecture.
- Cases by shape: x86-only variants (`X86PushPop`, `X86AdjustStack`, ...) are valid under any `architecture`; `effects` does not check the architecture for them (`:337-427`). `X86PushPop` indexes a 16-entry table by `usize::from(register)` (`:339-356`), a `u8` above 15 would panic (inferred).
- Same job elsewhere: stage 11 `adapters.rs` (the grep of `AdapterInstructions|AdapterOperation` lists `pipeline/11_bound-framed-machine-to-selected-form-encoding/src/adapters.rs`, 7 lines) and `physical-recipes` (`native_entry_bridge.rs`, `native_foreign_bridge.rs`, `windows_internet.rs`) - suspected consumers, not read. Grep count: 65 lines across 10 files, 9 of them in this file.
- Stated limits: "Each fixed template is a declared, operand-free instruction sequence" (`:118`).
- Fail-open: none seen. Unknown states are not represented (closed enums).

### R/post-allocation-machine/src/codec.rs (1-441)
- Purpose: encode and decode the `OMGPMX\0\0` version-28 wire artifact of a `PostAllocationMachinePlan` and check its content identity.
- Shape: `MAGIC` `:36`, `VERSION = 28` `:37`; `PostAllocationMachineDecodeError` (`:57`: Truncated, WrongMagic, UnsupportedVersion, InvalidField, InvalidIdentity, TrailingBytes); `encode_terminal_post_allocation_machine_plan` (`:77`); `decode_terminal_post_allocation_machine_plan` (`:90`); `decode_content` (`:172`), `decode_instruction` (`:263`), `decode_operand` (`:401`), `decode_invocation_site` (`:39`).
- Inputs/outputs: uses the `effect_codec::Cursor` from `selected_instructions::...::effects::program::encoding` (`:23`). Produces a `PostAllocationMachinePlan`.
- How it decides: version equality (`:98`), header identity vs `post_allocation_machine_identity(&plan)` after decoding (`:108`), trailing bytes (`:105`). Closed tags: choice rule must be byte 0 (`:188-191`); the address-operation tag is matched against 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16 (`:273-386`) and any other tag is `InvalidField`; `Store` byte size must be 1,2,4 or 8 (`:290`); `FrameStorageSlotId::ActivationBase` is accepted only with tag 3 and offset 0 (`:374-378`). Keyed on tag bytes.
- Cases by shape: seventeen fixed address-operation shapes; a new `PhysicalAddressOperation` variant must be added in four places (enum `instructions.rs:35`, `codec.rs:273`, `identity.rs:120`, plus consumers).
- Same job elsewhere: `identity.rs:99-272` encodes the same fields in the same order (same job in the reverse direction). `register-homes/src/recovery/fixed_view_copy_codec.rs` encodes the same `VirtualRegisterOrigin`, `RegisterConstraintKey` and scalar types with its own tag tables (suspected duplication of tag tables, not compared byte for byte).
- Stated limits: header comment "Decoding rejects unknown versions, invalid closed-tag bytes, trailing bytes, or any mismatch" (`:7-9`).
- Fail-open: the plan's alternative and unit lists are decoded by `effect_codec::decode_alternative` / `decode_units` from another crate; a decoded plan with any alternative is accepted (test `reauthenticated_physical_data_is_still_only_a_proposal`, `codec_tests.rs:847-855`, stated: decode proves only byte identity). Traced for callers: `validate_current_program` (below) is the correspondence check.

### R/post-allocation-machine/src/codec_tests.rs (1-987)
- Purpose: tests for the codec: pinned identity and wire digests for four empty-boundary plans (`:43-79`), round trips for each `PhysicalAddressOperation`, tag corruption at fixed byte offsets (`:479-530`), every-prefix truncation (`:836-844`), header/root authentication (`:533-548`), scratch-buffer size bound (`:82-103`), cleanup invocation site (`:975-987`).
- Shape: tests only; hard-coded byte offsets `HEADER_IDENTITY_OFFSET`..`WRITE_SEMANTICS_OFFSET` (`:32-40`) tie the tests to the exact wire layout.
- Stated limits: test comment `reauthenticated_physical_data_is_still_only_a_proposal` (`:847`).
- Fail-open: none seen.

### R/post-allocation-machine/src/control_flow.rs (1-27)
- Purpose: the function and block records of the physical program.
- Shape: `PostAllocationMachineFunction{machine, local_storage_slots, outgoing_arguments, blocks}` (`:10`), `instructions()` (`:18`), `PostAllocationMachineBlock{block, instructions}` (`:24`, comment: "Ordinary selected instructions followed by the selected terminator").
- Fail-open: none seen.

### R/post-allocation-machine/src/costs.rs (1-260)
- Purpose: a descriptive (non-authoritative) cost model carrying instruction size knowledge per native target, with a domain-separated identity.
- Shape: `target_cost_model(target)` (`:16`), `TargetCostModel` (`:113`), `TargetCostModelIdentity` (`:39`), `NonAuthoritativeMachineSizeCost` (`:52`, `ExactBytes(u16)` / `EncoderResolved{minimum_bytes,maximum_bytes}`), `NonAuthoritativeLatencyCost` (`:87`, one variant `Unavailable`), `observe` (`:144`). Three tests (`:174-259`).
- How it decides: `observe` is a one-to-one map from `MachineSizeKnowledge`/`MachineLatencyKnowledge`; identity hashes version byte 1 plus `target.wire_identity_bytes()` (`:21-31`). Test pins that `NativeTarget` with a 4-byte pointer differs from linux_x64 (`:195-211`).
- Unused / one-caller: grep `target_cost_model|TargetCostModel` over `source-rust` with `costs.rs` excluded (content mode) returned hits only in `post-allocation-machine/src/lib.rs:14` (doc comment) and `:45` (the re-export `pub use costs::{TargetCostModelIdentity, target_cost_model}`). Every other hit is `optimization_core::TargetCostModelIdentity`, a different type with the same name (`optimization-records/src/identities/decisions.rs:10`; the stage 03 `pass_manager.rs:1309` builds its own identity from the bytes `omega.psi-baseline-structural-cost-model.v1`). So `target_cost_model()`, `TargetCostModel`, `observe` and the two `NonAuthoritative*` cost types have zero call sites outside their own file and the re-export. Two distinct types named `TargetCostModelIdentity` exist (`costs.rs:39` and optimization-records).
- Stated limits: "non-authoritative", "without promoting encoder-resolved bounds into exact byte facts or using cost observations for semantic admission" (`:5-8`). Latency is always `Unavailable`.
- Fail-open: none seen.

### R/post-allocation-machine/src/evidence.rs (1-109)
- Purpose: records of which optimization selections and phase roots a physical program carries into publication.
- Shape: `PostAllocationMachinePhaseReceipt` (`:10`, `complete_selection`, `phase_selections`, `result` identities), `PostAllocationMachineOptimizationCustody` (`:43`, optimization, artifact identity, selections, action_count, baseline_bytes, selected_bytes) with `from_parts` constructors and getters.
- Unused / one-caller: `PostAllocationMachineOptimizationCustody` 21 grep lines in 8 files (pipeline 11, 13, machine-code `encoding.rs`, `layout.rs`, `encoding_identity.rs`, encoding-admission `layout/validation.rs`); the receipt is used in `program.rs`, `program_validation.rs`.
- Fail-open: none seen (plain data; `from_parts` does not validate).

### R/post-allocation-machine/src/identity.rs (1-279)
- Purpose: SHA-256 content identity of a plan, streamed in chunks shared with the wire encoder so no whole-plan buffer is built.
- Shape: `PostAllocationMachineIdentity` (`:11`), `post_allocation_machine_identity` (`:23`, domain `omega.terminal-postallocation-machine.v27\0`), `write_terminal_post_allocation_machine_content` (`:41`), `encode_instruction` (`:99`), `encode_units` (`:274`).
- How it decides: n/a (hash). Note the domain says v27 while the wire version is 28 (`codec.rs:37`) and `lib.rs:13` also says v27; both stated in the files.
- Same job elsewhere: mirror of `codec.rs` decode (same field order).
- Fail-open: none seen.

### R/post-allocation-machine/src/instructions.rs (1-101)
- Purpose: the physical instruction record and the symbolic addressing payloads.
- Shape: `MachineAlternativeChoiceRule` (`:10`, one variant `UniqueApplicableInCatalogOrderV1`, comment: "Target catalogs partition physical-home configurations so exactly one alternative applies"), `PostAllocationMachineInstruction` (`:15`, instruction id, `MachineAlternative`, operands, `address: Option<PhysicalAddressOperation>`, six unit lists), `operand_footprint` (`:29`, linear `find` on operand ordinal), `PhysicalAddressOperation` (`:35`, 16 variants).
- Stated limits: the choice-rule comment above (the uniqueness is a claim about target catalogs, enforced elsewhere; grep not done).
- Fail-open: none seen.

### R/post-allocation-machine/src/lib.rs (1-112)
- Purpose: crate root; declares modules, re-exports, and `PostAllocationMachinePlan` (`:66`) with `function`, `blocks`, `instructions`, `encode`, `decode`.
- Shape: the plan binds twelve upstream identities (selected, effects, ranges, legality, homes, post_allocation_manifest, target, register_environment, physical_register_model, register_constraints, machine_effect_catalog, choice_rule) plus `functions` (`:66-81`).
- How it decides: `function(machine)` is a linear scan (`:84-91`).
- Fail-open: none seen. `pub mod adapters;` is declared after the test-free tail (`:112`).

### R/post-allocation-machine/src/operands.rs (1-33)
- Purpose: operand footprint and an instruction-qualified physical read.
- Shape: `QualifiedPhysicalRead` (`:13`, source instruction, operand, vreg, class, view, units), `PhysicalOperandFootprint` (`:23`).
- Unused / one-caller: `QualifiedPhysicalRead` grep = 4 lines in 3 files: definition (`operands.rs:13`), re-export (`lib.rs:51`), one use as `Box<post_allocation_machine::QualifiedPhysicalRead>` (`machine-code/src/encoding.rs:238`). One external use site.
- Stated limits: "A fused branch has no selected operands of its own" (`:3-6`).

### R/post-allocation-machine/src/program.rs (1-81)
- Purpose: the post-allocation program that later stages pass: contracts, identities, the machine plan, register environment, callee-saved requirements, publication facts, phase receipt, function columns, plus a recomputed identity.
- Shape: `PostAllocationMachineProgram` (`:18`, 11 pub fields), `PhysicalFunction = selected_instructions::SelectedFunction` (`:35`), `function` (`:38`), `physical` (`:44`), `recomputed_identity` (`:51`, domain `omega.physical-machine-program.v3\0`), `Deref` to the machine plan (`:75`).
- How it decides: identity is a hash of the listed identity bytes, function digests and counts; it does not hash the machine body again (comment `:48-50`: "this digest does not hash that body again" - replay checks it).
- Fail-open: none seen.

### R/post-allocation-machine/src/program_validation.rs (1-692)
- Purpose: replay checks of a physical program against its original admitted owners (`validate_current_program`) and a self-consistency check (`validate_current_program_integrity`).
- Shape: `PhysicalProgramValidationError` (`:15`, ten variants), `PhysicalProgramReplay` (`:30`), `validate_current_program` (`:39`), `validate_current_program_integrity` (`:112`), nine tests (`:438-691`).
- How it decides: all comparisons are identity/equality compares: selected plan identity recomputed, `program.machine.as_ref() != machine`, environment, preservation plan fields, publication record, phase receipt, per-function `contracts/entry_block/blocks` equality (`:98-105`). Integrity also checks per block that `block.instructions.len() + 1 == physical.instructions.len()` and instruction ids line up pairwise with the terminator (`:161-179`).
- Same job elsewhere: `encoding-admission` and `verification/published-objects` call it (grep `validate_current_program|PhysicalProgramReplay` = 27 lines in 4 files: 20 here, 2 `encoding-admission/src/layout/mod.rs`, 3 `encoding-admission/src/lib.rs`, 2 `verification/published-objects/src/lib.rs`).
- Fail-open: `validate_current_program` zips `program.functions` with `selected.plan.functions` after a length check (`:95-105`), traced: length equal so no row is skipped. `validate_current_program_integrity` zips functions of the program with `machine.functions` after a length check (`:151-154`) and blocks after a length check (`:161-163`); traced, no skip.

## 4. program-entry-contracts

### R/program-entry-contracts/entry_signature.rs (1-458)
- Purpose: the source-independent signature of a selected `ProgramEntry` slot: declaration text, machine/state names, parameter list with their Extent layouts, and a 32-byte identity.
- Shape: `SelectedProgramEntrySignature` (`:159`), `ProgramEntryVisibleParameterSignature` (`:118`), `ProgramEntryExtentValueLayout` (`:58`, fixed `[ProgramEntryExtentFieldLayout; 2]`), `ProgramEntryResultSignature` (`:26`, only `Unit`), `from_checked_declaration` (`:171`), `identity` (`:226`), `validate_capture` (`:313`), `declared_type_signature` (`:339`), `visible_parameter` (`:350`), `ProgramEntryExtentValueLayout::from_checked_record` (`:72`) / `validate_capture` (`:98`).
- How it decides: the Extent layout must be exactly `{base: addr@0, length: u64@8}` as a 16-byte, align-8 integer shape: "selected source Extent must be the exact {base: addr@0, length: u64@8} x86-64 record" (`:100-110`). Signature validation requires `target_slot == owner.program_entry_slot()`, non-empty machine/state name, a present callable identity, result `Unit` (`:314-318`); each parameter must have a present type identity, nonzero power-of-two alignment, `!is_const`, `!is_mutable`, a valid Extent layout whose shape equals its value shape (`:322-335`). The identity hashes the declaration, target slot strings, calling conventions, and the text of identities looked up through `identities.text(...).unwrap_or_default()` (`:262-267`, `:284-289`): the callable and each type enter the hash by their text, not by table index (test `:416-445`).
- Cases by shape: only `Unit` result; only the two roles `Image` and `InitialStorage`; Extent fixed at two fields; `push_shape` has `debug_assert!` that the shape class is Integer (`:403-406`), so non-integer shapes are not rejected in release builds (inferred: it only writes size and alignment).
- Same job elsewhere: `physical_contract.rs` and `storage_binding.rs` in this crate re-check the same `slot != slot.owner.program_entry_slot()` condition (`physical_contract.rs:36`, `storage_binding.rs:47`).
- Unused / one-caller: grep `ProgramEntryPhysicalContractPlan|SelectedProgramEntrySignature|SelectedProgramStorageEntryPlan` outside `program-entry-contracts/` = 46 lines in 9 files (stage 01 `program_entry*`, stage 09 `physical_entry.rs`, stage 15 `image-emission/src/entries.rs`, `verified-optimization-input/src/hosted_receiver.rs`, `target-bound-program/entry_settlement.rs`). `declared_type_signature` calls `service_surface::declared_type_signature`; its callers were not grepped.
- Stated limits: header "address-free Extent record graph" (`:1-6`); the hard-coded 16/8 layout is stated "x86-64 record".
- Fail-open: `identities.text(index).unwrap_or_default()` (`:265-267`, `:286-289`, `:342`): an index missing from the table hashes as the empty string. Traced: `validate_capture` requires `normalized_callable_identity` and each parameter type identity to be non-absent but does not check the index is in range of the table; a missing-in-table index would hash like `""` and two different such signatures would collide. Classification: open (no comment; the caller supplies the table).

### R/program-entry-contracts/physical_contract.rs (1-202)
- Purpose: the validated target slot, arrival types and calling plan for a physical (process-entry) contract, retained together.
- Shape: `ProgramEntryPhysicalContractPlan` (`:7`), `new` (`:22`), getters, one test (`:158`).
- How it decides: `new` refuses unless: a `physical_arrival_requirement` exists; `target_slot == owner.program_entry_slot()`; requirement non-empty; `physical_contract_package == Some(target_package)`; digest's package matches (`:33-44`); the slot's `physical_calling_convention == Some(arrival.calling_convention())` (`:46`); parameter count equals `arrival.parameter_count()`; identities non-empty; `calling_plan_report_fingerprint != 0` (`:59`); the boundary plan's policy and parameter count match and a result exists (`:65-68`). Keyed on package identity, counts and non-empty strings.
- Fail-open: none seen. A zero fingerprint is refused (`:59`), but any non-zero `u64` fingerprint is accepted without recomputation (open: the fingerprint is the caller's claim; no comment).

### R/program-entry-contracts/program_entry.rs (1-31)
- Purpose: crate root: re-exports, trait `HostedReceiverCustody` (`:17`), `PhysicalEntrySelection` (`:26`).
- Unused / one-caller: grep `PhysicalEntrySelection|HostedReceiverCustody` = 13 lines in 5 files (2 here).

### R/program-entry-contracts/storage_binding.rs (1-200)
- Purpose: binds the target's `ProgramStorageEntry::enter` slot to its boundary schema and optional physical contract.
- Shape: `SelectedProgramStorageEntryPlan` (`:24`), `from_target_slot` (`:33`), `with_physical_contract` (`:117`), `ProgramStorageEntryRootRole` (`:18`), `ProgramStorageEntryDiagnostic` (`:13`). One test (`:157`).
- How it decides: `declares_exact_continuation` by `slot.schema` (`:38-46`): ProgramStorageApplication requires `ImageAndInitialStorage`; HostedApplication requires `None`. Requirement text must equal `"ProgramStorageEntry::enter"` built with `format!` (`:49-51`), the schema's trait text must equal the slot's `boundary_schema` (`:65`), and exactly one method must match the requirement identity (`:78-93`) with owner `"ProgramStorageEntry"` and name `"enter"` compared by string (`:94-95`). Source-spelling keys: the constants at `:9-10` and the `format!`.
- Fail-open: none seen.

## 5. selected-instructions

### R/selected-instructions/allocation_input.rs (1-78)
- Purpose: the hand-off from stage 06 to stage 07: the selected program with either its live ranges or its allocation legality.
- Shape: `SelectedAllocationInput` (`:15`), private enum `AllocationAnalyses::{LiveRanges, Legality}` (`:20`), `with_live_ranges` (`:26`), `with_legality` (`:32`), `selected`, `selections`, `record`, `into_legality` (`:64`).
- Inputs: types from `crate::` (`AllocatorAvailabilityRequirement`, `StagedOptimizedLiveRanges`, `StagedOptimizedAllocationLegality`, `stage_optimized_allocation_legality`, `SelectedOptimizationRecord`).
- Placement note: the file sits in the `selected-instructions` directory but imports `crate::{AllocatorAvailabilityRequirement, StagedOptimizedLiveRanges, StagedOptimizedAllocationLegality, stage_optimized_allocation_legality, SelectedOptimizationRecord, OptimizedAllocationLegalityCustodyError}` (`:4-8`). Grep `allocation_input` over `R/` returns only `R/README.md:3` and no `mod allocation_input` / `#[path]` line. I did not read the crate that declares it; suspected that a stage 06 crate includes it by path (grep `SelectedAllocationInput` over `psi-to-native` = 26 lines in 8 files, in stage 06 `analyses/src/lib.rs`, `selected_rewrites/*`, stage 07 `allocation/*`, and this file).
- How it decides: `into_legality` uses `required.unwrap_or(AllEnvironmentAllocatableViews)` (`:69-70`): with no required view set it assumes all allocatable views. Comment: "or all allocatable views in the environment when no recovery rule is selected" (`:59-61`) - stated.
- Fail-open: `None` required availability widens to all views (`:69-70`), stated in the doc comment (`:59-63`).

## 6. register-homes (crate `register-homes`, wire artifacts framed by `Envelope`)

Wire-codec call sites, common to all files in this section: grep of `(FixedViewCopyPlan|LiteralFoldPlan|PressureRematerializationPlan|RecoveryClassificationPlan|SpillChoicePlan|RegisterHomePlan|PostAllocationOptimizationManifest|AllocatorAvailabilityPlan)::decode` over `source-rust` excluding `register-homes/` returned 13 lines, all in `post-allocation-machine/src/codec_tests.rs` (these are `PostAllocationMachinePlan::decode` test lines; the pattern matched that name by substring). Inside `register-homes/` the same pattern returns 108 lines in 7 files: definitions plus `codec_tests.rs` (67), `post_allocation_manifest.rs` (12, mostly its tests), `recovery.rs` (11, tests). Grep `\.encode\(\)` over `pipeline/07_selected-instructions-to-register-homes` returned 0. So no production call of any wire `encode`/`decode` of these artifacts was found in stage 07, and none of `decode` outside register-homes and post-allocation-machine tests. The artifacts' wire forms are exercised by tests; whether a CLI/verification path reads them was not grepped beyond these two searches.

### R/register-homes/src/constraints.rs (1-690)
- Purpose: data records (and identities) for what allocation may choose and where fixed register views force splits: allocator availability, per-virtual-register legality, fixed/precolored point intervals, and fixed/precolored split requirements.
- Shape: `AllocatorAvailabilityPlan` (`:44`, with `encode`/`decode` framed `OMGAVA\0\0` v1 `:31`), `AllocatorAvailabilityPolicy` (`:36`, `AllEnvironmentAllocatableViewsV1` | `ExplicitUnconstrainedViewAllowlistV1{views}`), `AllocationLegalityPlan` (`:175`, per-function `Arc<FunctionAllocationLegality>`), `FixedPrecoloredIntervalPlan` (`:325`), `FixedPrecoloredSplitRequirementPlan` (`:400`), `FixedPrecoloredSourceSegmentOpening` (`:449`, three variants), identity functions `allocator_availability_identity` (`:128`), `allocation_legality_identity_from_digests` (`:232`, domain `...legality.v8`), `allocation_legality_function_digest` (`:253`), `fixed_precolored_interval_plan_identity` (`:355`, `...point-intervals.v2`), `fixed_precolored_split_requirement_plan_identity` (`:461`, `...split-requirements.v2`). Aggregation modules `allocator_availability`, `allocation_legality`, `fixed_precolored_intervals`, `fixed_precolored_split_requirements` re-export names (`:569-601`). Two tests (`:628-689`).
- Inputs/outputs: reads `selected_instructions` ids and points, `register_model` ids, `optimization_core` budgets; produces identity bytes and the availability wire form. Only `AllocatorAvailabilityPlan` has `encode`/`decode` here.
- How it decides: decode checks magic, version, policy tag (0/1, else `UnknownPolicy`), trailing bytes and recomputed identity (`:59-105`). `allocation_legality_identity_from_digests` asserts `digests.len() == plan.functions.len()` with `assert_eq!` (`:236-240`), a panic, not an error.
- Cases by shape: `FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1` is the only policy (`:320`); `FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1` the only split policy (`:395`).
- Same job elsewhere: the encode of `LiveRangeEdgeConnector` in `encode_connector` (`:555`) maps a missing `psi_edge` to 0 (`connector.psi_edge.map_or(0, ...)`), the same 0-as-absent convention as `selected_blocks.rs:191-196` (`encode_successor`).
- Unused / one-caller: not grepped per type.
- Stated limits: "Incompatible fixed views remain explicit transition or split requirements rather than being silently reconciled" (`:5-7`).
- Fail-open: none seen. Sorted-ness of `classes`/`candidates` ("Class-ID-sorted", "View-ID-sorted" comments `:48`, `:212`) is stated in comments but no check was seen in this file or in `decode` (open: a decoded plan with unsorted rows passes if its identity matches).

### R/register-homes/src/lib.rs (1-979)
- Purpose: crate root: shared codec helpers (`Envelope`, `decode_list`, `domain_identity`, `cursor::optional`), the `RegisterHomePlan` wire form (`OMGRAH\0\0` v7), the allocated-program views, fixed-precolored segment homes, and callee-saved requirement records with identity.
- Shape: macros `decode_error_display!`, `decode_error!`, `identity_newtype!` (`:49-90`); `codec_bytes::Envelope` (`:153`, `frame`, `open`), `decode_list` (`:195`, `Vec::with_capacity(count.min(cursor.remaining()))`), `AllocatedProgram` (`:301`, two `Arc`s), `AllocatedProgramRef`, `AllocatedProgramFacts` (`:322`), `RegisterHomePlan` (`:393`) with `encode`/`decode` (`:408`,`:414`), `register_home_identity` (`:339`, `omega.terminal-register-homes.v7\0`), `FixedPrecoloredSegmentHomePlan` (`:499`), `CalleeSavedModificationWitness` (`:583`, three variants), `AllocatedCalleeSavedRequirementPlan` (`:649`), `AllocatedCalleeSavedRequirements` (`:677`, plan + sealed identity), `allocated_callee_saved_requirement_identity` (`:692`), `mod pressure_rematerialization` and `pub mod literal_fold` at the file end (`:972-979`). Tests (`:771-970`).
- How it decides: decode refuses wrong magic, version != 7, machine id 0 (`InvalidMachineId`), trailing bytes, identity mismatch (`:414-459`). Test `retired_split_roster_wire_rejects_before_identity_comparison` (`:813`) pins that a version-6 buffer is `UnsupportedVersion(6)`.
- Same job elsewhere: `Envelope::frame/open` (`:163-192`) duplicates `wire::codec_bytes::encode_header/decode_header` used by `literal_fold.rs:194,202` and `pressure_rematerialization.rs:45,53` (same crate, two framing styles; `OMGLFD` and `OMGREM` use the `wire` helpers, the other artifacts use `Envelope`). `codec_bytes::encode_target` (`:105`) re-implements target encoding that `fixed_view_copy_codec.rs:405-429` wraps and `post_allocation_manifest.rs:422` decodes via `NativeTarget::from_wire_identity_bytes`; three target encodings in the crate.
- Unused / one-caller: `pub mod register_homes { pub use crate::*; }` (`:767-769`) re-exports the crate under its own name (not grepped for users). `AllocatedProgramRef`, `AllocatedProgramFacts` not grepped.
- Stated limits: header "Every artifact envelope in this crate verifies its magic header, exact wire version, and 32-byte domain-separated SHA-256 identity" (`:42-45`). The header says `FixedViewCopyPlan` is version 116 (`:20-22`, `OMGFCV\0\0` version 116) but `recovery/fixed_view_copy_codec.rs:63` sets `VERSION = 118` and `recovery/codec_tests.rs:549` asserts 118: stale doc number.
- Fail-open: `decode_list` allocates `count.min(cursor.remaining())`, so a huge claimed count cannot over-allocate, and element decoding fails on truncation: none seen.

### R/register-homes/src/literal_fold.rs (1-464)
- Purpose: the record of literal-folding rewrites chosen after allocation analysis: for each function, a list of actions replacing a literal-defining instruction and its consumer by an immediate-operand form; wire form `OMGLFD\0\0` v13.
- Shape: `LiteralFoldPolicy` (`:19`, a `u32` bit set with 23 rule bits `:24-46`, constants `*_V1`), `LiteralFoldPlan` (`:171`), `LiteralFoldAction` (`:292`), `encode`/`decode` (`:190`,`:200`), `literal_fold_identity` (`:357`, domain `omega.terminal-literal-fold.v8\0`), one test (`:417`).
- How it decides: `LiteralFoldPolicy::from_canonical_bits` accepts any non-zero bit combination within `KNOWN_BITS` (`:159-167`) and `contains` is false for an empty `other` (`:145-147`). Decoder keys: constraint family byte 0-4 else `UnknownConstraintFamily` (`:329-336`); option tag 0/1.
- Cases by shape: 23 fixed rule bits (exact add/subtract/divide, compare, extension, wrapping/saturating zero/one/minus-one forms); a new rule needs a new bit and a new `KNOWN_BITS` entry (`:47-69`).
- Same job elsewhere: `decode_constraint_key` (`:326`) duplicates `pressure_rematerialization.rs:298` and `recovery/fixed_view_copy_codec.rs:509` (three copies, identical tag table 0-4).
- Unused / one-caller: grep `LiteralFoldPlan|LiteralFoldPolicy` over `source-rust` = 71 lines in 4 files, 56 in stage 06 `literal_fold_rewrites.rs`.
- Fail-open: a decoded `LiteralFoldAction` is not checked against any selected plan (stated by design: a different crate replays it); `result: Option<VirtualRegisterId>` accepted as `None` or `Some`.

### R/register-homes/src/post_allocation_manifest.rs (1-767)
- Purpose: the post-allocation optimization manifest: which selected-plan rewrites ran (append-only roster of 17 transformation kinds), the roots they bind, statistics, a text rendering, and wire form `OMGPAO\0\0` v16.
- Shape: `PostAllocationOptimizationManifest` (`:142`, 19 fields), `PostAllocationSelectedTransformation` (`:62`, 17 variants) with the row macro `selected_transformation_rows!` (`:82-139`) tying wire tag, identity bytes and rendered kind together; `recomputed_identity` (`:165`, `...manifest.v7\0`), `encode`/`decode` (`:172`,`:177`), `render_text` (`:268`), `PostAllocationOptimizationManifestError` (`:449`, six variants) and `...DecodeError` (`:461`).
- How it decides: decode requires stage tag 1, `spills` tag 1 or 2, `frame/emission/publication` each tag 1 (`Unavailable`), a known transformation tag 1..17, and a matching recomputed identity (`:177-266`). Keyed on tags and identity.
- Cases by shape: `PostAllocationManifestStage` has one variant (`RegisterHomes`); `PostAllocationUnavailableData` has one variant (`Unavailable`), so `frame`, `emission` and `publication` are always "unavailable" (`:337-339`).
- Unused / one-caller: grep `PostAllocationOptimizationManifestError\b` (word-bounded, content mode): constructed at stage 07 `assignment.rs:451` (`NonCanonicalTransformationLedger`), `:464` (`RootMismatch`), `:473` (`UnresolvedFixedViewTransitions`), `:535` (`StatisticsOverflow`). The variants `ContentMismatch` and `IdentityMismatch` (`:454-455`) have no construction site in those hits.
- Stated limits: "Unavailable" statuses for frame/emission/publication are the only values the codec accepts.
- Fail-open: none seen; unknown tags are refused.

### R/register-homes/src/pressure_rematerialization.rs (1-343)
- Purpose: record of one pressure-driven rematerialization per function (move an immediate's materialization to avoid a register spill); wire `OMGREM\0\0` v2.
- Shape: `PressureRematerializationPlan` (`:23`), `FunctionPressureRematerialization{machine, action: Option<...>}` (`:163`), `PressureRematerializationAction` (`:169`, victim, views, original/fresh materialize ids, `source_value`, `value: IntegerValue`, rewrites), `identity` submodule (`:191-274`, domain `omega.terminal-pressure-rematerialization.v2\0`), decode helpers (`:298-328`).
- How it decides: policy byte 0 or 1 else `UnknownPolicy` (`:73-77`); option byte 0/1; `source_value` id 0 refused (`InvalidValueId`).
- Cases by shape: both policy names say `ImmediateU64` (`recovery.rs:957-958`) but `value` is an `IntegerValue` carrying `u128`/`i128` (`:320-327`), so width is not restricted by the type; tag meaning here is Unsigned=0, Signed=1 (`:243-250`, `:321-325`) whereas `recovery.rs:420-421` and `fixed_view_copy_codec.rs:503-504` use Signed=0, Unsigned=1 for `IntegerValue`.
- Fail-open: none seen.

### R/register-homes/src/publication.rs (1-57)
- Purpose: carries the allocation manifest, selections and budget forward after allocation without its graphs.
- Shape: `AllocationPublicationFacts` (`:14`), `retain` (`:24`, SHA-256 over domain `omega.allocation-publication-facts.v1\0`, manifest identity, selections identity, budget), getters.
- How it decides: none; "Retention grants no authority" (`:5-6`, stated). `retain` does not recompute `record.identity` (it hashes the field as given).
- Unused / one-caller: used by `post-allocation-machine` (`program.rs:26`) and `program_validation.rs:139-140`.
- Fail-open: `retain` accepts any record; stated: "This does not revalidate output that owner just produced" (`:22-23`); the check is `program_validation.rs:140` (`publication.identity != publication.recomputed_identity()`), traced.

### R/register-homes/src/recovery.rs (1-1651)
- Purpose: records and codecs for three allocation-recovery artifacts: spill choice (`OMGSPC\0\0` v2), recovery classification (`OMGRCV\0\0` v8) and the `FixedViewCopyPlan` identity; also declares the submodules below. None grants allocation authority on decode alone (`:8-12`, stated).
- Shape: `SpillChoicePlan` (`:714`, policy `SingleBlockFarthestEndThenHighestVregV1`), `SpillChoice` (`:732`), `RecoveryClassificationPlan` (`:88`), `PressureRecoveryClassification` (`:110`), `RecoveryClassification` (`:132`: `ImmediateU64RematerializationCandidate` | `NoAdmittedRecovery{reason}`), `NoAdmittedRecoveryReason` (`:154`, 7 reasons), `FixedViewCopyPolicy` (`:947`, 4 variants), `FixedViewCopyPlan` (`:971`, holds `Arc<SelectedInstructionPlan>`), `FixedViewCopy` (`:986`), `FixedViewCopyDecodeError` (`:1010`, about 95 variants), `fixed_view_copy_identity` (`:1113`, domain `...fixed-view-copies.v10\0`), `recovery_classification_identity` (`:537`, `...v7`), `spill_choice_identity` (`:885`, `...v2`). Tests (`:1163-1650`).
- How it decides: classification decode keyed on tags (policy 0; role 0/1; classification 0/1; reason 0..6; origin tags 0,1,3,4,5,6,7,8,9; definition-site 0..2; provenance 0/1). `PressureRecoveryClassification.scalar_type` decode accepts tags 0..3 only (`:397-412`) while the encoder `encode_classification_scalar_type` also emits tag 4 (`IeeeFloat`) and tag 5 (`Callable`) (`:641-667`); a plan encoded with a float or callable scalar type fails to decode with `UnknownScalarType` (traced from the two match tables; not run).
- Cases by shape: only one victim per function per plan ("First pressure point only; None proves the bounded walk encountered no pressure", `:728`, stated); `RecoveryClassificationPolicy` and `SpillChoicePolicy` each have one variant.
- Same job elsewhere: `decode_classification_origin` (`:426-485`) and `decode_register` origin match (`fixed_view_copy_codec.rs:747-788`) decode the same nine-tag `VirtualRegisterOrigin` table twice; `decode_classification_scalar_type` (`:394`) vs `decode_scalar` (`fixed_view_copy_codec.rs:465`) use different tag tables for the same `ScalarType` (classification: 0 bool,1 signed,2 unsigned,3 address; copy codec: 0 bool,1 integer+carrier+sign,2 float,3 callable); `decode_classification_provenance` (`:506`) vs `decode_provenance`/`decode_work` (`fixed_view_copy_codec.rs:814-846`); `decode_classification_integer_value` (`:416`) vs `decode_integer` (`:499`). All in this crate.
- Unused / one-caller: `pub mod classification`, `spill_choice`, `fixed_view_copy` re-export modules (`:52-74`) not grepped. `RecoveryClassificationDecodeError` has 27 variants including `InvalidPlaceId`, `InvalidObligationId` etc.
- Stated limits: "None of these artifacts grant allocation, spill, rematerialization, stack-slot, or publication authority on decode alone" (`:8-9`).
- Fail-open: decoded classification/spill rows are not checked against the program they describe (stated at `:10-13`: independent validators replay them). `NoAdmittedRecoveryReason` unknown tags are refused.

### R/register-homes/src/recovery/codec_tests.rs (1-2063)
- Purpose: tests for `FixedViewCopyPlan` wire encode/decode: header, rejection precedence (trailing bytes, then payload digest, then transformed-identity, then outer identity), round trips of calls, memory roles, local slots, outgoing slots, catalogs.
- Shape: fixtures `shared_function`, `plan`, `structural_function`, `catalog`; `pub(super)` helpers `plan`, `function_with_operand`, `successor_parameter_function`, `transformed_identity_offset`, `selected_payload_offset` used by sibling test modules (`selected_blocks.rs:517`, `program_contracts.rs:281`).
- How it decides: pins version 118 (`:549`), rejects versions 0,104,VERSION-1,VERSION+1,u32::MAX (`:684`) and 115 before reading types (`:833-844`). Test names mention `artifact_v15` (`:1196`, `:1500`) though the version is 118.
- Stated limits: comment "Version 115 writes a copy-permission bool where the current payload writes declared multiplicity" (`:835`).
- Fail-open: none seen (test file).

### R/register-homes/src/recovery/execution_facts.rs (1-154)
- Purpose: decodes the `NativeExecutionFacts` embedded in a recovery artifact and re-seals it.
- Shape: `decode` (`:14`), `decode_summary` (`:53`), one test (`:68`).
- How it decides: requires the exact domain prefix (`NATIVE_EXECUTION_FACTS_DOMAIN`, `:15`); passes decoded roots to `NativeExecutionFacts::seal`, which returns `InvalidExecutionFacts` on mismatch (the test `:128-151` pins "callable entry summaries differ from the activated roots"). Row-count guard `decode_rows(cursor, 32, ...)` (`:33`).
- Fail-open: none seen. The seal itself is in `target-vocabulary` (not read).

### R/register-homes/src/recovery/fixed_view_copy_codec.rs (1-958)
- Purpose: the `OMGFCV\0\0` v118 envelope: encode a `FixedViewCopyPlan` and the full transformed `SelectedInstructionPlan` payload; decode verifies payload digest, canonical re-encoding, transformed identity and outer identity.
- Shape: `ENVELOPE_DOMAIN = omega-fixed-view-copy-envelope-v64\0` (`:65`), `FixedViewCopyPlan::encode`/`decode` (`:68`,`:74`), `envelope_identity` (`:100`), `encode_content` (`:138`), `decode_prefix`/`decode_content`, `encode_selected_plan`/`decode_selected_plan` (`:550`,`:561`), `encode_ordinary_plan`/`decode_ordinary_plan` (`:589`,`:609`), `encode_function`/`decode_function` (`:652`,`:682`), `encode_register`/`decode_register` (`:722`,`:741`), `encode_scalar`/`decode_scalar` (`:431`,`:465`), many small helpers (`decode_id`, `decode_option_u64`, `decode_bool`...). The submodule `Cursor` is re-exported from `crate::cursor`.
- How it decides: order of refusal in `decode` (`:74-97`): header (magic, version) first, then content, then `cursor.remaining() != 0` -> `TrailingBytes`, then `!transformed_payload_matches` -> `TransformedPayloadMismatch`, then recomputed `selected_instruction_plan_identity` vs the prefix claim -> `TransformedIdentityMismatch`, then envelope identity -> `IdentityMismatch`. `decode_ordinary_plan` also checks `facts.roots()` against the plan header (`:626-637`). `decode_function` requires `function.ids_are_positions()` (`:716`).
- Cases by shape: `encode_scalar` refuses a rigid (parameterized) integer width with `CodecError::IntegerWidthClosure` (`:441-443`), comment "a rigid width is not an executable integer descriptor". Carrier byte 1 (address) is accepted only with sign Unsigned (`:484`).
- Same job elsewhere: `decode_target` (`:409-429`) and `encode_target` (`:405`) duplicate `register-homes/src/lib.rs:105` and `post_allocation_manifest.rs:422` target codecs; `decode_definition_site` (`:372`) duplicates `recovery.rs:487` and `decode_classification_definition_site`.
- Stated limits: header: "Older artifacts with optional legacy evidence reject at the version header instead of selecting a compatibility dialect" (`:12-13`). The comment lines `:57-62` list schema changes (multiplicity replaced a copy-permission bit).
- Fail-open: `decode_function` builds `SelectedFunctionContracts` with `structural_block_parameters`, `outgoing_arguments`, `local_storage_slots`, `calls`, ... initially empty then fills them via `decode_contracts` (`:697-715`): traced, filled before return. A decoded plan is "unchecked" (`:14-15`, "before returning the unchecked plan"): consumers must validate.

### R/register-homes/src/recovery/program_contracts.rs (1-429)
- Purpose: encode/decode `SelectedProgramContracts` (target profile, function ABIs, publication facts, execution facts) for the recovery artifact, reconstructing sealed values through their owners.
- Shape: `encode` (`:20`), `decode` (`:44`), `decode_abi` (`:80`), `decode_rows` (`:159`, row-count bounded by `remaining / minimum`).
- How it decides: the target profile is found by comparing `profile.identity().as_str().as_bytes() == identity` over `TargetProfile::ALL` (`:54-57`): a string compare against the stored profile name; absent -> `InvalidForeignTargetProfile`. Entry conversion count must equal `call_plan.parameters.len()` (`:84-88`). Row minimum sizes are hard-coded numbers: 65 for ABI rows (`:63`), 24 for scalar rows (`:141`), 48 for structural parameters (`:156`).
- Cases by shape: `LegalizedFunctionAbi` carries at most one `scalar` and one `mixed_structural_scalar` option (`:105-127`).
- Fail-open: `SelectedProgramContracts::new` (not read) rejects unordered/repeated ABI rows (test `:343-361`) and an incompatible entry conversion (test `:363-402`), traced from tests only.

### R/register-homes/src/recovery/publication.rs (1-189)
- Purpose: decode `NativePublicationFacts` (per-function operation count, sites, byte literals, call targets, dynamic-dispatch and callable catalogs).
- Shape: `decode` (`:13`) with row minimums 36, 13, 24, 17 (`:21,25,38,49`); catalog sections decoded by `terminal_wire::decode_dynamic_dispatch_catalog` and `decode_callable_catalog`, then `NativeCodeCatalogs::seal` and `NativePublicationFacts::seal`.
- How it decides: exact domain prefix; site tag 0/1 else `InvalidProgramContracts("publication site has another kind")` (`:26-34`).
- Fail-open: none seen; three tests pin refusals for truncation and huge counts (`:138-188`).

### R/register-homes/src/recovery/selected_blocks.rs (1-1358)
- Purpose: encode/decode `SelectedBlock`, its origin, terminator and successors (including structural bindings and case edges) in the recovery payload.
- Shape: `encode_block` (`:36`), `decode_block` (`:78`), `encode_successor` (`:178`), `decode_successor` (`:299`), `decode_case` (`:447`); terminator tags 0..10 in `decode_block` (`:99-169`) while the encoding is delegated to `selected_instructions::terminator_encoding::encode_terminator` (`:69-74`).
- How it decides: `decode_successor` refuses lineage mismatch: `psi_edge.is_none() != matches!(role, OperationContinuation)` -> `InvalidSuccessorLineage` (`:311-313`). Structural transport shapes: `WholeValue` requires nonzero size, power-of-two alignment, `byte_size % alignment == 0`, `source_byte_offset % alignment == 0`, no offset overflow, and destination `StructuralBlockParameter` (`:383-394`); `Address` requires `byte_count != 0` and the same destination kind (`:412-419`); `ScalarReference` requires a valid `ScalarStorageCodec::decode(codec)` (`:355-357`) and `StructuralBlockParameter` destination. Keyed on tag bytes and values.
- Cases by shape: terminator tag table 0..10 is fixed (comment-free). Crash cause decode accepts bytes 1 (Trap) and 2 (Abort) only (`:115-118`).
- Same job elsewhere: terminator tags are both encoded in `selected_instructions` crate and decoded here (two-crate pair, grep for `encode_terminator` not done).
- Stated limits: header "`encode_terminator` is delegated ... so the wire tag layout is stated once in the crate that owns `SelectedTerminator`" (`:7-9`); decoding is not delegated.
- Fail-open: `decode_block` does not check the block's `id` against its position (that is `ids_are_positions()` in `fixed_view_copy_codec.rs:716`, traced).

### R/register-homes/src/recovery/selected_instructions.rs (1-1025)
- Purpose: encode/decode `SelectedInstruction`, its operand rows and `SelectedInstructionKind` (about 100 kinds) in the recovery payload.
- Shape: `encode_instruction` (`:32`), `decode_instruction` (`:52`), `encode_kind` (`:90`, a tag `match` then delegation to `selected_instructions::kind_encoding::encode_kind(bytes, tag, kind)` `:201`), `decode_kind` (`:204`, `match` on tag byte), `trapping_form`, `decode_call_target` (`:538`).
- How it decides: tag bytes. Shared tag families (saturating, trapping, atomic, float) are looked up through `selected_instructions` helpers (`saturating_kind(tag)`, `trapping_form(tag)`, `AtomicForm::from_machine_tag(tag)`) (`:228-239`, `:390-415`). `Store64`/`FrameAddress` with `ActivationBase` accepted only for tag 18 and offset 0 (`:301-305`, error `UnknownOption(4)`). `ImageAddress` reference tags 0..5 (`:323-345`).
- Cases by shape: the `encode_kind` match lists every variant by hand (`:91-200`); the decode match lists the same tags (`:207-528`); adding a kind requires both plus the delegated encoder. The doc comment cites tags "54, 55, 60, 61, 62 plus appended carriers" and "`122 + ordinal`" (`:6-9`).
- Same job elsewhere: `decode_call_target` (`:538`) vs `structural_calls.rs:302` `decode_native_call_target` (three variants vs two; different target types: `SelectedCallTarget` vs `NativeCallTarget`).
- Fail-open: tag `ExactDivideU64` etc. decode needs a payload of obligation id + 32-byte accepted fact; unknown tags refused (`:528`). None seen.

### R/register-homes/src/recovery/structural_calls.rs (1-2614)
- Purpose: encode/decode calls (`SelectedCallContract`), normalized foreign calls (`SelectedNormalizedForeignCall`), calling-convention plans, provider conformances, structural arguments and results for the recovery payload; decoding rebuilds sealed values by re-running their admission functions.
- Shape: `encode_call`/`decode_call` (`:187`,`:320`), `encode_normalized_foreign_call`/`decode_normalized_foreign_call` (`:676`,`:765`), `decode_call_binding` (`:911`), `decode_call_plan` (`:475`), `decode_placement` (`:551`), `decode_shape` (`:599`), `decode_machine_register` (`:634`), `decode_state_plan` (`:1182`), `decode_state_set` (`:1222`), `encode_provider`/`decode_provider` (`:1385`,`:1451`), `encode_result`/`decode_result` (`:1544`,`:1561`), layouts (`:1532`,`:1595`).
- How it decides: `decode_call_binding` chooses the mechanism by comparing the decoded target-name string to the literals `"@compiler-import"` (`:915`), `"@hosted"` (`:934`), the empty string (system call, `:938`), `"@table"` (`:984`) and `"@component"` (`:989`); any other string is taken as a target profile name for an import locator (`:1007`). Source spelling (a string sentinel in the target-name field) is the discriminant. Validation per mechanism: compiler import -> `has_valid_entry_mechanism()` only (`:929-932`); system call -> `has_valid_entry_mechanism()` plus `validate_boundary_entry_plan(...)` (`:975-980`); hosted import -> both (`:1030-1035`, error `InvalidSameStackContribution`); component -> `has_valid_entry_mechanism()` only (`:1096-1102`); plain import and table -> no entry-plan validation (`:1076-1090`, `:1092-1103`). The same-stack contribution is re-admitted via `admit_same_stack_contribution` and its recomputed report identity and commitment must equal the stored bytes (`:1047-1064`). Foreign locators are re-normalized with `normalize_foreign_locator` (`:1173`).
- Cases by shape: calling policies tags 1..6 (`:478-485`); register tags 0..18 with payload byte required 0 for tags <= 15 (`:639-643`); machine-state bit mask 9 bits (`MACHINE_STATE_SET_BITS`, `:77`); `descriptor_input` count type must be an integer (`:825-828`); `IntegerImmediate` source scalar must be an integer (`:1365-1368`). Result tags 0..3 (`:783-815`).
- Same job elsewhere: `decode_native_call_target` here and `decode_call_target` in `selected_instructions.rs:538`; `decode_machine_function_identity` (`:904`) relates to `function_identity::MachineFunctionIdentity::from_bytes` used in `selected_instructions.rs:328`.
- Unused / one-caller: `decode_locator` is `#[cfg(test)]` (`:1139`).
- Stated limits: header: decoding "reconstructs sealed values through their owning admission paths rather than trusting wire bytes" (`:9-13`).
- Fail-open: (1) `decode_result` sets `qualification_establishments: Vec::new()` (`:1577`) and `encode_result` never writes that field (`:1544-1559`): a `StructuralOperationResult` with non-empty establishments round-trips to an empty list. Traced; whether other stages require it is not read. Classification: open. (2) `decode_target_parameter` sets `projected_qualifications: Vec::new()` (`structural_declarations.rs:555`), filled only by `decode_abi_parameter` (`:565`): paths using `decode_parameter` keep it empty while `encode_target_parameter` (`:538`) does not write it. Open. (3) Mechanisms `Import` and `Table` are not validated beyond same-stack admission (see above): traced.

### R/register-homes/src/recovery/structural_contracts.rs (1-1593)
- Purpose: encode/decode the function-level contract rows: structural signature (types, referents, quotient/access catalogs, parameters, result, places, claims), local storage slots, outgoing slots, memory accesses, boundary settlements, call origins, completion claims, ownership events and cleanup actions.
- Shape: `encode_contracts`/`decode_contracts` (`:148`,`:197`), `decode_local_slot` (`:58`), `decode_invocation_site` (`:111`), `encode_signature`/`decode_signature` (`:991`,`:1073`), `encode_boundary_settlement`/`decode_boundary_settlement` (`:416`,`:539`), `encode_call_source`/`decode_call_source` (`:605`,`:668`), `decode_provider_execution` (`:722`), `decode_external_execution` (`:737`), `decode_cleanup` (`:953`), `decode_ownership` (`:933`), `decode_block_parameters` (`:356`).
- How it decides: memory-access role tags 0..21 (`:236-333`); hosted execution accepts only the exact built-in for that operation (`encode_hosted_execution`, `:505-521`: another built-in -> `MalformedStructuralFoundation`); `decode_block_parameters` requires strictly increasing block ids, non-empty parameter lists and wire space (`count > remaining / 62`, `remaining / 46`) (`:362-379`); `decode_signature` validates quotient representations through `terminal_semantics::quotients::validate_quotient_representations` (`:1088`) and rebuilds the catalog through `with_access_lifecycles`/`with_quotients` (`:1191-1202`). Reference-origin decode refuses a mix of place/carrier (`:1173-1175`, `UnknownOption(0)`).
- Cases by shape: call-source tags 1,2,4,5, tag 3 "belonged to retired target-family calls and remains unassigned" (`:672`, tested `:1215-1223`); cleanup tags 1,2,4,6 with 3 and 5 retired (`:956-989`, tested `:1568-1577`); ownership events 1..6.
- Same job elsewhere: `decode_invocation_site` here duplicates `post-allocation-machine/src/codec.rs:39` (grep `decode_invocation_site` = 14 lines in 6 files: codec.rs 3, codec_tests.rs 1, recovery/selected_instructions.rs 2, structural_calls.rs 2, structural_contracts.rs 3, `selected-instructions/src/effects/program/encoding.rs` 3). Their error mapping differs (`InvalidSemanticId`/`UnknownOption` here, `InvalidField` there).
- Stated limits: "Tag 3 belonged to retired target-family calls and remains unassigned" (`:672`).
- Fail-open: `encode_boundary_settlement` writes a constant byte 1 for the zero-size `realization` and discards it (`let _ = settlement.realization;`, `:477-478`); decode requires byte 1 (`:582-585`): fixed, none seen.

### R/register-homes/src/recovery/structural_declarations.rs (1-1566)
- Purpose: encode/decode structural type declarations, fields, cases, parameters, arguments, places, paths and qualification metadata for the recovery payload.
- Shape: `encode_type`/`decode_type` (`:66`,`:192`), `encode_fields`/`decode_fields` (`:403`,`:422`), cases, `encode_scalar_restriction`/`decode_scalar_restriction` (`:915`,`:974`), `decode_path` (`:712`), `decode_qualification_path` (`:721`), `decode_reference_origin_path` (`:737`), `decode_projection` (`:750`), `decode_place` (`:652`), `decode_target_argument` (`:614`), `encode_projected`/`decode_projected` (`:863`,`:878`).
- How it decides: `encode_type` refuses `TypeParameter` shapes and rigid array extents (`:87-91`, `:164-167`: "unknown generic storage cannot be encoded in an executable copy plan", "an array extent is still a rigid const input"). `decode_type` accepts shape tags 1,2,3,4,5,6,7,8,10,12,13,14,15,16,19 (`:218-315`), refuses others (`UnknownStructuralTypeShape`); tag 12 requires a bounded-integer or refined-scalar restriction whose scalar equals the declared one (`:297-303`); tag 16 requires `cell.is_supported()` (`:307-312`). Wildcard `AllElements` (tag 10) is accepted only by `decode_qualification_path` (`:728-730`), refused by `decode_path` and `decode_reference_origin_path` (`decode_projection` returns `UnknownStructuralPathSegment`, `:783-785`). `decode_authored_field_storage` fragment count must be nonzero and <= `remaining / 40` (`:354-357`).
- Cases by shape: record storage kinds Packed, AccessView, Accessor, Authored, TargetLayout (tags 2,14,15,10,13); array extent tags 0/1; `ByteSequenceCarrier` tags 1,2,3.
- Same job elsewhere: `decode_id`, `decode_option_u64` come from `fixed_view_copy_codec.rs:354,298`; `decode_multiplicity`/`decode_access` have `encode_*` twins here (no duplicates elsewhere seen).
- Fail-open: (1) `decode_type` always sets `requires: Vec::new()` (`:318`) and `encode_type` never writes `requires` (`:66-190`): a non-empty `requires` is dropped. Open. (2) `encode_target_argument` writes the runtime-index count as `u32::try_from(len).unwrap_or(u32::MAX)` (`:598-602`): a count above `u32::MAX` is silently saturated and then writes `len` rows; decode would read the wrong count. Open (unreachable at normal sizes). (3) `encode_projected(.., retain=false)` writes nothing and `decode_projected(.., false)` returns empty (`:863-891`); inside this unit every call passes `true` except `encode_parameter`/`encode_semantic_parameter` which forward a parameter (grep of `retain_projected_qualifications` in `recovery/` = 10 lines; all call sites in the files read pass `true`). Stated by parameter name, not commented.

## Coverage

- Read in full, in order: all 39 ranges of `units/unit_030.tsv`, line for line: 93 + 45 + 51 + 32 + 132 + 515 + 441 + 987 + 27 + 260 + 109 + 279 + 101 + 112 + 33 + 81 + 692 + 458 + 202 + 31 + 200 + 690 + 979 + 464 + 767 + 343 + 57 + 1651 + 2063 + 154 + 958 + 429 + 189 + 1358 + 1025 + 2614 + 1593 + 1566 + 78 = 21,859 lines (my own addition of the table's line counts in `unit_030.tsv`, not a command).
- Skimmed: none.
- Not read: none.
- Not read but cited as context: all crates outside this unit (`selected-instructions`, `target-vocabulary`, `terminal-wire`, `wire`, `calling-conventions`, `target`) and stage 06/07/08 sources, except for the grep lines quoted above.

## Five things a maintainer most needs to know about this unit

1. Most of this unit is wire codecs for artifacts that only tests were found to encode or decode. Grep of `X::decode` for the register-homes and post-allocation-machine plan types found 13 lines outside `register-homes/` (all in `post-allocation-machine/src/codec_tests.rs`), and `.encode()` appears 0 times in `pipeline/07_selected-instructions-to-register-homes`. The wire forms are version-pinned (`OMGPMX` 28, `OMGRAH` 7, `OMGPAO` 16, `OMGFCV` 118, `OMGLFD` 13, `OMGREM` 2, `OMGSPC` 2, `OMGRCV` 8, `OMGAVA` 1) and carry many tests; I did not find the production reader.
2. Decoding in this unit proves byte identity only. The code says so (`recovery.rs:8-12`, `codec_tests.rs:847`: "reauthenticated physical data is still only a proposal"). Correspondence checks live elsewhere: `post-allocation-machine/src/program_validation.rs` (27 grep lines for its entry points across 4 files) and stage 07 validators.
3. `FixedViewCopyPlan` embeds an entire `SelectedInstructionPlan` (`fixed_view_copy_codec.rs:550-607`), so the register-homes crate re-implements the encode/decode of instructions, blocks, calls, foreign calls, structural types and ABI contracts (about 11,000 lines across the `recovery/` files). Several tag tables are written twice (encode and decode) and some a third time elsewhere: `VirtualRegisterOrigin`, `RegisterConstraintKey`, `IntegerValue`, scalar types and target encoding each appear in two or three files with different tag layouts.
4. A few codec asymmetries and silent drops were seen: `decode_classification_scalar_type` (`recovery.rs:394`) lacks the float and callable tags that `encode_classification_scalar_type` (`:661-667`, `:641-644`) writes; `decode_result` drops `qualification_establishments` (`structural_calls.rs:1577`); `decode_type` drops `requires` (`structural_declarations.rs:318`); `decode_target_parameter` leaves `projected_qualifications` empty (`:555`).
5. `decode_call_binding` (`structural_calls.rs:911-1104`) selects the foreign-call mechanism by comparing a decoded name string to `"@compiler-import"`, `"@hosted"`, `""`, `"@table"`, `"@component"`, and only some mechanisms get `validate_boundary_entry_plan`. The costs module (`post-allocation-machine/src/costs.rs`) has no call site outside its own re-export.

## What you are unsure about

- Whether the wire `encode`/`decode` of the register-homes and post-allocation-machine artifacts has a production reader (for example `omega` CLI or `verification/published-objects`) under another spelling; I grepped `Type::decode` and `.encode()` in stage 07 only.
- Where `selected-instructions/allocation_input.rs` is included into a crate (no `mod` line found in `representations/` by grep; used by stages 06 and 07).
- Whether the `decode_classification_scalar_type` asymmetry is reachable: it requires a recovery classification whose victim has a float or callable scalar type; the policy name `SelectedVictimImmediateU64Eligibility` suggests those would be classified `UnsupportedScalarType` but the row still carries `scalar_type` (inferred, not traced to stage 06 `recovery_classification.rs`).
- `X86PushPop` register index range (`adapters.rs:339-356` indexes a 16-element array with `usize::from(u8)`): whether callers can pass a value above 15 was not traced (inferred panic).
- The 21,859 total above is my hand addition of the table's counts; I did not run a separate count command.
