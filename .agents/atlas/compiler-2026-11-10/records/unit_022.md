# Unit 22 notes

Checkout: `<checkout>/` (main at `457adf4a11f`). All paths below are relative to `omega-rust/omega/compilation/` unless noted.

Files covered (51, 21,759 lines):

Stage 08 (`pipeline/08_selected-form-encoding-to-resolved-layout/src/`): `ordinary.rs`, `validation.rs`.
Stage 09 (`pipeline/09_resolved-layout-to-resolved-layout/src/`): `lib.rs`, `x86_branch_relaxation.rs`.
Products (`products/src/`): `checked_compilation.rs`, `checked_compilation/report.rs`, `compilation_products.rs`, `compilation_products/package_path_tests.rs`, `compile_outcomes.rs`, `compile_request.rs`, `evaluated_build.rs`, `independent_component.rs`, `lib.rs`, `package_build.rs`, `package_build/retention_identity.rs`, `prepared_build.rs`, `prepared_source.rs`, `publication.rs`, `terminal_publication.rs`.
Provider admission (`realization/provider-admission/src/`): `admission_source.rs`, `lib.rs`, `lowering_facts.rs`, `provider_admission.rs`, and under `provider_admission/`: `access_lifecycles.rs`, `adapters.rs`, `callback_entries.rs`, `compiler_builtins.rs`, `compiler_builtins/catalog.rs`, `compiler_builtins/retained.rs`, `model.rs`, `settlements.rs`, `settlements/source_imports.rs`, `source_evaluated_bindings.rs`, `static_interrupts.rs`, `static_interrupts/arrival_helpers.rs`, `static_interrupts/scopes.rs`, `terminal_authority_permission_policy.rs`, `terminal_authority_policy.rs`, `terminal_authority_policy/classification.rs`, `terminal_authority_policy/external_mechanisms.rs`, `terminal_authority_policy/host_cohorts.rs`, `terminal_authority_policy/windows_imports.rs`, `terminal_authority_review.rs`, `terminal_authority_review/context.rs`, `terminal_authority_review/reachability.rs`, `terminal_authority_review/reviewer.rs`, `terminal_authority_review/windows_handle_activations.rs`, `terminal_authority_review/windows_handle_aliases.rs`, `terminal_authority_review/windows_handle_arguments.rs`, `terminal_authority_review/windows_handle_calls.rs`, `terminal_authority_review/windows_handle_conditions.rs`.

All 21,759 lines were read in full (the line-count sum was checked with `awk -F'\t' '{s+=$4}' unit_022.tsv` = 21759). Greps are over `omega-rust` with the Grep tool.

---

## Part A. Stage 08: selected-form encoding to resolved layout

### A1. `pipeline/08_.../src/ordinary.rs` (1028 lines)

**Purpose.** Lays out ordinary functions: picks one program-wide layout policy, orders blocks so a conditional branch's fall-through successor sits immediately after it, computes block and instruction byte offsets (widening AArch64 conditional branches that do not reach), and turns each pre-layout row into a resolved row with bytes, branch evidence and fixup evidence (read, header comment lines 1-3).

**Shape.**
- `ConditionalBranchDescriptor` (`ordinary.rs:33`): predicate, terminator row, `when_taken`, `when_fallthrough`.
- `LayoutPlan` (`:40`): `BTreeMap` block offsets/sizes, `BTreeMap` instruction offsets, `function_size`, `widened_branches: BTreeSet<SelectedInstructionId>`.
- `select` (`:48`) chooses `SelectedFunctionLayoutPolicy`; `derive_order` (`:85`); `layout` (`:155`, entry point); `derive_plan` (`:236`, fixpoint over widening); `derive_once` (`:282`); `instruction_size` (`:342`); `resolve_row` (`:440`); `validate_image_address_fixup` (`:532`), `validate_internal_fixup` (`:559`), `validate_foreign_fixup` (`:599`), `validate_unresolved_field` (`:649`), `validate_callback_materializations` (`:678`); `conditional_branch_descriptor` (`:779`); `resolve_branch` (`:835`).
- Test module only instantiates `order_suite!` against `derive_order` (`:1025-1027`).

**Inputs/outputs.** Reads `SelectedInstructionPlan`, `BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>` (pre-layout rows), `BTreeMap<.., &PostAllocationMachineInstruction>`, `ValidatedPhysicalRegisterModel`, `SelectedFormEncodedRowStore`, and `isa_x86_64` / `isa_aarch64` encoders. Produces `ResolvedSelectedFunctionLayout` (blocks, rows, branch evidence, cloned fixups). No mutation of inputs.

**How it decides.**
- Policy choice (`:51-82`) is keyed on block structure: `is_single_entry`, whether the first block's terminator is `ConditionalBranch`, or `conditional_branch_less_pair_pattern!`, or `is_admitted_canonical_shape` (helpers live in stage-08 `lib.rs`, outside this unit).
- Order (`:85-153`) is keyed on block id (positions sorted by id, binary search) and on the fall-through successor of each conditional branch; a block with two predecessors by fall-through, a self fall-through, or a fall-through into the entry is rejected (`:126`, `:132`).
- Widening (`:236-280`) is keyed on a byte displacement (taken-block offset minus instruction offset) outside `isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH` (`:271`); candidates are rows of three kinds with `RetainedV1` + `DeferredControl{RequiresResolvedBranchLayout}` (`:315-330`). Fixpoint: grow set until unchanged (`:275`).
- Row resolution (`:462-529`) is keyed on the pair (`machine_disposition`, `state`); any other pair is `unexpected` (`:528`).
- Fixup checks are keyed on exact placeholder bytes (`[0,0,0,0]` for x86 rel32, `0x9400_0000` for AArch64 BL, `:569-579`, `:610-620`), `fixup.addend == 0`, `fixup.callee == instruction.kind.machine_callee()` (`:566`, `:580-583`), byte offsets inside the row (`:663-674`), and callback materialization groups being re-encodable to the same bytes by `encode_private_callback_materialization` (`:731`, `:761`).
- Branch resolution (`:835-1023`): machine alternative family must equal the family derived from the predicate (`:920`), fall-through block offset must equal branch end (`:937`), the decoded effects and byte length must equal what the machine row declared (`:889`, `:997-1003`).

**Cases by shape.**
- `select` has exactly four policies and otherwise errors: `SingleEntryBlockV1`, `EntryThenZeroFallthroughThenNonzeroV1`, `EntryThenNotLessFallthroughThenLessV1`, `PerFunctionCanonicalShapeV1`, else `UnsupportedFunctionShape` (`:56-82`). The first two non-canonical policies require `single_entry_count == 0` and every function's first block to have that terminator (`:58-75`).
- `SingleEntryBlockV1` requires exactly one block ending in `Return` (`:110-118`).
- Widening only for `ConditionalBranchNonZero | ConditionalBranchU64LessThan | ConditionalBranchI64LessThan` on AArch64 (`:316-322`, `:244`); x86-64 never widens and returns after one pass (`:244-246`).
- Sizes: jump is hard-coded 5 bytes x86 and 4 bytes AArch64 (`:413-417`); conditional is 6 x86 / 4 AArch64, 8 when widened (`:418-421`, `:428-433`).
- Callback materialization: x86 allows `Register`, `RecordField`, `OutgoingStack`; AArch64 refuses `OutgoingStack` (`:725`, `:757-759`).
- `instruction_size` accepts three AArch64-only `machine_disposition`s (`Aarch64ElidedCompareI64ZeroV1`, `Aarch64FusedBranchNonZeroToCbnzV1`, `Aarch64ElidedSameViewCopyI64V1`, `:349-374`) but `resolve_row` has no arm for them and reaches `_ => unexpected` (`:528`). Grep of the three names over `omega-rust`: 13 occurrences in 5 files (`machine-code/src/encoding.rs` 3, `encoding/identity.rs` 3, `backend/machine-emission/src/exit_contract.rs` 3, `ordinary.rs` 3, `07_.../row_encoding.rs` 1). So only `ordinary.rs:349-374` handles them in stage 08; whether another stage-08 path consumes them (the error type is `OptimizedResolvedSelectedFormLayoutError`) is suspected, not read.
- `select` error names `selected.functions[single_entry_count]` (`:79-81`), i.e. the function at index equal to the count of single-entry functions, not necessarily the offending function (inferred from reading).

**Same job elsewhere.**
- `derive_order` is implemented twice: `ordinary.rs:85` and `validation.rs:256` (grep `fn derive_order\b`: those two only). The second is documented as an independent re-derivation (`validation.rs:2`).
- `resolve_branch` (`ordinary.rs:835-1023`) and `validate_branch` (`validation.rs:428-618`) mirror each other (encode versus validate).
- Signed byte delta helper is copied three times: `byte_displacement` (`ordinary.rs:332`), `checked_delta` (`validation.rs:620`), `checked_delta` (`09/x86_branch_relaxation.rs:714`).
- Offset assignment over blocks/rows is done in `derive_once` (`ordinary.rs:282-313`) and again in `assign_dense_offsets` (`09/x86_branch_relaxation.rs:648-665`).

**Unused / one-caller.** `branch_size`, `unexpected`, `byte_displacement`, `instructions`, `conditional_branch_descriptor`, `conditional_branch_family`, `derive_plan`, `select` are `pub(crate)` and are imported by `validation.rs:23-26` (read). Not separately grepped for external callers (crate-private).

**Stated limits.** None with "unsupported" wording beyond the `UnsupportedFunctionShape` error name.

**Fail-open risks.**
- `validate_callback_materializations` (`:678-777`): returns `Ok` for an empty `callbacks` slice only if `opcode_row_offset == 0` (`:773`); not fail-open.
- None seen in `ordinary.rs` (all unmatched cases go to `unexpected`).

### A2. `pipeline/08_.../src/validation.rs` (996 lines)

**Purpose.** Independently re-derives the policy, block order, offsets and widening for a candidate `StagedOptimizedResolvedSelectedFormLayout` and compares it row by row (header comment `:1-3`). Contains about 370 lines of tests (`:624-996`) for AArch64 widening.

**Shape.** `validate` (`:36`, entry); `validate_roots` (`:74`); `validate_identity` (`:94`, recomputes `resolved_machine_layout_identity`); `validate_ordinary` (`:110`); `validate_function` (`:148`); `collect_pre_layout_rows` (`:235`); private `derive_order` (`:256`, BTreeMap-based); `fallthrough` (`:314`); `validate_row` (`:319`); `validate_branch` (`:428`); `checked_delta` (`:620`).

**Inputs/outputs.** Reads `CurrentSelectedProgram`, `PostAllocationMachinePlan`, physical model, `SelectedFormEncoding` (pre-layout), the artifact. Returns `Result<(), Error>`; no mutation.

**How it decides.**
- Root identities: `pre_layout.selected() != selected.identity`, `pre_layout.machine() != machine.identity`, target equality and architecture (`:45-53`); and `pre_layout.post_allocation_machine_optimization().is_some()` is a `RootMismatch` (`:50`), with `validate_roots` passing `None` for optimization (`:58`).
- Row compare is keyed on `(machine_disposition, state)` exactly as production (`:340-424`); everything else is `ArtifactMismatch` (`:423`).
- Branch rows: re-run the target decoder (`validate_*_selected_*_form`) on the candidate bytes and compare decoded effects and register reads (`:462-488`, `:538-592`) and the whole evidence struct (`:614`).

**Cases by shape.** Same four policies via `select`; AArch64 widening size 8 (`:523-527`).

**Same job elsewhere.** See A1 (order, branch resolution, delta).

**Fail-open / weaker checks (traced by reading).**
- For rows with unresolved internal/foreign/image fixups, validation compares the candidate's fixup to the pre-layout row's fixup and checks the callee/boundary/reference match (`:352-406`); it does not call the production `validate_internal_fixup` / `validate_foreign_fixup` / `validate_callback_materializations` (`ordinary.rs:559-777`), so placeholder bytes, byte offsets and callback materialization shape in the pre-layout row are not re-checked here. The pre-layout store is an input to `validate`, not something it checks (traced: `validate` only compares identities, `:45-53`). Status: traced to the extent that production checks them in `resolve_row` only. A forged pre-layout store with bad placeholders and a candidate copying it would pass this validator; I did not trace who validates the pre-layout encoding.
- Tests at `:916-995` cover AArch64 only (grep not run; read).

---

## Part B. Stage 09: resolved layout to resolved layout

### B1. `pipeline/09_.../src/lib.rs` (268 lines)

**Purpose.** Stage entry. Consults a one-row catalog (`FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG`, `lib.rs:46-50`) to see whether the optimization `X86RelaxConditionalBranchesToRel8V1` is selected; if so runs the relaxation sweep, otherwise forwards the baseline layout (header `:1-3`).

**Shape.** `ResolvedLayoutOptimization` (`:81`; fields `current: Arc<ResolvedMachineLayout>`, `selections`, `budget`, `relaxation: Option<StagedOptimizedX86BranchRelaxation>`); `execute_resolved_layout_optimization` (`:111`); `validate_resolved_layout_optimization` (`:139`); `x86_rel8_selected` (`:168`); errors `FunctionRelativeLayoutCatalogError` (`:53`), `ResolvedLayoutOptimizationError` (`:65`).

**How it decides.** `x86_rel8_selected` matches the phase's selection slice by shape: `[]` -> false; `[X86RelaxConditionalBranchesToRel8V1]` -> check architecture equals catalog payload `X86_64` else `UnsupportedTarget`; `[other]` -> `UnsupportedSelection`; more -> `UnsupportedComposition` (`:175-193`). Keyed on `Optimization` enum value and `Architecture`.

**Cases by shape.** Exactly zero or one selected optimization in this phase; the code refuses two (`UnsupportedComposition`, `:192`).

**Unused / one-caller.** Grep `execute_resolved_layout_optimization|validate_resolved_layout_optimization` over `omega-rust`: 13 occurrences in 5 files (stage-09 `lib.rs` 6, `backend/machine-emission/src/function_realization.rs` 2, `exit_contract.rs` 2, `native-compiler/src/pipeline_boundary.rs` 1, `pipeline.rs` 2). Used.

**Fail-open risks.**
- `validate_resolved_layout_optimization` (`:139-166`) checks: selections equal; when disabled, `artifact.layout() == baseline`; when enabled, calls `validate_optimized_x86_branch_relaxation` and then `artifact.budget == relaxation.budget()` and `artifact.layout() == relaxation.layout()` (`:156-161`). The comment says validation does so "without re-running the fixed-point search" (`:6`, stated). The relaxation validator (B2) does not tie the relaxed layout to the baseline or recompute identity. A tampered artifact whose `current` and `relaxation.layout` agree with each other and carry a plausible roots set would pass. Status: stated for "no re-run"; the consequence is traced by reading `x86_branch_relaxation.rs:217-234`.

### B2. `pipeline/09_.../src/x86_branch_relaxation.rs` (1574 lines; code `1-725`, tests `727-1574`)

**Purpose.** x86-64 `Jcc rel32` (6 bytes) to `Jcc rel8` (2 bytes) relaxation. Each sweep inspects every conditional branch, selects those whose prospective short displacement fits `i8`, commits all of them in one batch, reflows offsets and displacements, and repeats until a sweep selects nothing (`:442-530`). Every rule evaluation, candidate, validation step, commit and iteration is charged against `OptimizationWorkBudget` (`charge`, `:700`).

**Shape.**
- `StagedOptimizedX86BranchRelaxation` (`:110`): `source`, `selected`, `machine`, `pre_layout`, `target`, `policy`, `budget`, `usage`, `output`, `output_revision`, `identity`, `attempts: Vec<X86BranchRelaxationAttempt>`, `actions: Vec<X86BranchRelaxationAction>`, `layout: Arc<ResolvedMachineLayout>`.
- `stage_optimized_x86_branch_relaxation` (`:207`), `validate_optimized_x86_branch_relaxation` (`:217`), `compute_trace` (`:442`), `inspect_production_branch` (`:322`), `prospective_short_displacement` (`:356`), `commit_short_form` (`:532`), `reflow_production_functions` (`:580`), `assign_dense_offsets` (`:648`), `rewrite_branch_offsets` (`:667`), `finish_artifact` (`:280`).
- Predicate tables `ENCODE`/`VALIDATE`: `[[fn; 2]; 3]` indexed by `predicate.rank()` and `near as usize` (`:45-61`, `:417`, `:433`).
- `X86BranchRelaxationWorkAxis` (`:74`) and `OptimizedX86BranchRelaxationError` (`:83`).

**Inputs/outputs.** Reads a baseline `ResolvedMachineLayout`, physical model, budget. Produces a cloned layout with rewritten rows, recomputed identity (`program.recomputed_identity()`, `:289`), an attempts list and an actions list.

**How it decides.**
- Branch is "near" iff `row.bytes.len() == 6`, "short" iff `== 2`, anything else `MalformedBranch` (`:379-385`, `:330`).
- Every inspected row is first validated by the target decoder for its predicate lane (`validate_lane`, `:331-338`).
- Selection key: `i8::try_from(prospective displacement)` (`:343`). The prospective displacement subtracts 4 from the target offset when the target is after the branch (`:369-375`) and measures from branch offset + 2 (`:376`). It ignores other branches shrinking in the same batch (so batch relaxation is conservative per sweep; later sweeps pick up more, test `:1537-1542`).
- After commit, total bytes must have dropped by exactly `count * 4` or `NonDecreasingByteMeasure` (`:509-516`).
- Reflow re-encodes every jump (`encode_x86_64_selected_jump_form`, requires `row.bytes.len() == 5`, `:599-611`) and re-encodes every conditional branch with its new displacement, requiring unchanged length and equal decoded effects (`:625-641`).
- `selected_alternative` (`:387-404`): short variant of non-zero is the key `variant` unchanged from the row (`NonZeroV1 => alternative`), while for U64/I64 less-than it hard-codes `variant` 0 near / 1 short.

**Cases by shape.** Only conditional rows with `ResolvedBranchEvidence::Conditional` participate (`:461-467`). Jumps are never relaxed, only re-encoded at fixed 5 bytes (`:607`). Only x86-64 (`ensure_x86_target`, `:243-253`). Fixed widths `SHORT_WIDTH=2`, `NEAR_WIDTH=6` (`:37-38`).

**Same job elsewhere.** Offset assignment (see A1), `checked_delta` (A1). `ENCODE`/`VALIDATE` lane tables parallel the `match predicate` arms in `ordinary.rs:946-968` and `validation.rs:540-565`.

**Unused / one-caller.**
- `X86BranchRelaxationWorkAxis` is `pub` (`:74`) and is a field of the exported `OptimizedX86BranchRelaxationError::BudgetExceeded` (`:85`), but `lib.rs:34-38` does not re-export it. Grep: 7 occurrences, all in `x86_branch_relaxation.rs`.
- `StagedOptimizedX86BranchRelaxation` accessor methods (`:128-190`) not individually grepped.

**Fail-open risks.**
- `validate_optimized_x86_branch_relaxation` (`:217-234`) checks target, source/selected/machine/pre_layout identities against the baseline, policy constant, and `usage.within(budget)`. It does not check that `layout`, `attempts`, `actions`, `identity` or `output` correspond to the baseline (no recompute, no identity compare). What a wrong program could get through: an artifact carrying a layout that is not the relaxation of the baseline but has the right root identities and an in-budget usage. Status: stated (the header of `lib.rs:6` says validation does not re-run the search) but the missing identity comparison is not stated; traced by reading the function body only.
- `reflow_production_functions` re-encodes jumps only when the row's `branch` is `ResolvedBranchEvidence::Jump` (`:592-616`); rows with no branch evidence are skipped (`:588-590`), consistent with non-branch rows being position independent.

---

## Part C. Products crate (`products/src/`)

### C1. `checked_compilation.rs` (309) and `checked_compilation/report.rs` (283)

**Purpose.** `CheckedCompilation` retains each stage product as the unit its producer returned (execution, Build, admission, provider settlement, source custody, timings, terminal publication) and exposes read methods; `report.rs` admits the requested stop (check / terminal / native) and builds the report settlement before the stage sequence consumes the checked record.

**Shape.**
- `RetainedCompilationChecks` (`checked_compilation.rs:38`), `CheckedCompilationInputs` (`:46`), `CheckedCompilation` (`:64`), `PartialEq` over `execution` and `sources` only (`:73-77`).
- `admit_trust` (`:161`), `package_build_handoff` (`:181`), `terminal_native_proposal_request` (`:225`).
- `report.rs`: `ProductReportRequest` (`:38`), `ProductReportPlan` (`:62`), `CheckedProductReport` (`:25`), `CompileReportSettlement` (`:97`), `ProductReportPlan::prepare` (`:109`), `finish_terminal/_runtime/_native/finish` (`:205-278`).

**How it decides.**
- `admit_trust` returns `Ok` immediately when the Build declares no root grants (`:162-165`).
- `terminal_native_proposal_request` requires `native_provider_sources` to be `Ok` and the selected-plan provenance to be aligned with declarations by index and plan equality (`:236-258`).
- `prepare` (`report.rs:109-190`): rollback validity by requested kind; Check returns a check-only report after `admit_build_requirements`; otherwise takes `completed_outputs()`; artifact-only Build produces a build-outputs report; else builds a one-use settlement whose `expected_kind` is `TerminalArtifact` or `RetainedNativeArtifact` (`:158-175`). `finish` rejects a report whose kind differs from `expected_kind` (`:259-264`).

**Cases by shape.** Three requested kinds only (`report.rs:38-42`).

**Stated limits.** `checked_compilation.rs:13-16`: "Measurements, a retained judgment, and invocation-local derived code are not a separate checked semantic identity".

**Fail-open risks.** `admit_trust` early `Ok` when `grants.is_empty()` (`:163`): stated by design by the function's use of `root_grant_selectors`; a program with no root grants skips `reconstruct_trust_obligations` entirely. Status: open (no comment at the site; not traced).

### C2. `compilation_products.rs` (730) and `compilation_products/package_path_tests.rs` (221)

**Purpose.** `CompileReport` wraps root path, source count, build outputs and timings around one `CompileProduct` enum whose variants carry only the payload that product kind can hold (`:127-136`); also `PendingNativeProduct` (`:35`), `RetainedNativeProgram` (`:61`), `CompileOutputKind` (`:77`).

**Shape.** Variants: `CheckOnly`, `BuildArtifacts`, `TerminalArtifact`, `RetainedNativeArtifact`, `NativeExecutable`, `RetainedDynamicElfArtifact`, `DynamicElfExecutable`, `ObjectContainer`. Transitions `record_native_publication` (`:274`), `record_dynamic_elf_publication` (`:296`), `record_terminal_publication` (`:320`), `record_build_output_publication` (`:255`), `record_component_publications` (`:507`). `checked_native_package_path` (`:700-710`) surfaces the `.app` root only if the package receipt's name and identifier match the retained application metadata and `package.binds_executable(&executable)`. Test file `package_path_tests.rs` mutates the package receipt (identifier, member order, root, byte count, container digest) and flat receipt (path, container, size) and asserts the package path is withheld.

**How it decides.** Publication transitions are keyed on the variant (`RetainedNativeArtifact` required) and `wrote_output == false` (`:279-284`). `record_component_publications` accepts a retained ELF dynamic product, or a retained native product whose `object_format == MachO` or whose target equals `windows_x64` (`:511-525`); other targets error.

**Cases by shape.** `record_component_publications` handles exactly ELF-dynamic, Mach-O, Windows x86-64; message "component files require an admitted ELF, Mach-O, or Windows x86-64 process publication contract" (`:521-524`).

**Same job elsewhere.** `record_native_publication` and `record_dynamic_elf_publication` are twins (`:274-318`).

**Unused / one-caller (grep).** `CompileReport::checked(` call sites: 0 outside its own definition and the `Self::checked` in `check_only` (`:338`); `CompileReport::check_only(` has 1 outside call (`report.rs:128`). The `lib.rs` comment says report construction belongs to `CompileReportSettlement` (`lib.rs:22-24`); `checked` is `pub` (`:347`) with no callers found by `CompileReport::checked(` grep.

**Fail-open risks.** `record_build_output_publication` (`:255-272`): error only if the product is not `BuildArtifacts` **and** `wrote_output` is false; so any product with `wrote_output == true` passes (stated by the error message "requires a checked, published primary product"). Status: stated.

### C3. `compile_outcomes.rs` (174)

**Purpose.** `CompileFailure`, `CompilerRequestOutcome` (`Complete`/`Reject`/`Incomplete(ExceededProvision)`), per-target `CompileTargetOutcome`, ordered `CompileOutcomes`.

**How it decides.** `of_refusal` (`:48-60`): the refusal is `Incomplete` only if the first error carries `exceeded_provision` and every later error does too; any ordinary error makes it `Reject`; if there is no error at all, the first `and_then` yields `None` and returns `Reject` (`:52-54`). Stated in the header (`:2-4`). `halt_value`: 0/1/2 (`:62-68`). `into_single_report` requires exactly one outcome (`:161-173`).

**Fail-open risks.** none seen.

### C4. `compile_request.rs` (575)

**Purpose.** Compilation requests and their validation before any frontend or Build work: `CompileOptions`, `CheckedCompileRequest`, `CheckedExecutionRequest`, `NativeProductRequest`, `RequestedCompileProduct`, `CompileRequest`, `ValidatedCompileRequest`, `ValidatedTargetCompilation`.

**How it decides.** `CompileRequest::validate_for_execution` (`:370-502`): needs at least one destination; installation assumptions require a native product (`:380-386`); with more than one destination each must name a target (`:391-396`); a native request with no target name takes `TargetProfile::host_if_supported()` else errors listing catalogued profiles (`:397-419`); resolves profile by name; rollback validity per product (`:432-438`); duplicate profile detection (`:439-444`); package inputs vs shared sources (`:445-467`); assumptions partitioned by target (`:482-493`); targets sorted by `TargetProfile::ALL` position (`:494-500`). `validate_optimization_rollback` (`:167-195`): `Check` refuses every requested-disabled optimization; `TerminalArtifact` refuses all but phases `CheckedTrees` and `Psi`; `NativeArtifact` refuses none.

**Cases by shape.** Three requested products; destinations keyed on profile.

**Same job elsewhere.** `validate_installation_assumption_targets` and `partition_installation_assumptions` (`:127-157`) both check the profile membership; grep shows 6 occurrences in 3 files (`orchestration/src/build.rs` 1, `products/src/compile_request.rs` 4, `package-manager/src/operations/accepted_compilation.rs` 1).

**Fail-open risks.** `validate_for_execution` with a duplicate profile pushes a diagnostic but still pushes the target (`:439-444`, `:468`); the final `diagnostics.is_empty()` check (`:475`) turns it into an `Err`. Not fail-open (traced).

### C5. `evaluated_build.rs` (339)

**Purpose.** `EvaluatedBuild`, `BuildConfig` (settings, grants, provider selections, native bindings, assumed bindings, representation selections, wire-compat demands, root bindings, behavior exclusions, service audits, accepted component assumptions), `ComputedBuildConfig`, `BuildAdmission`, independent component discovery.

**How it decides.** `BuildConfig::provider_deployment` (`:51-82`): "deployed" iff root bindings or provider selections non-empty or any default selection has an activation with a package identity; root slot owner is parsed by `rsplit_once("::")` on a source-spelling slot string (`:66`) and mapped by `TargetProfile::from_root_slot_owner`. `same_source_choices` (`:84-126`) destructures every field (exhaustive) and compares; native binding evaluation compared by `terminal_code` only (`:104-118`). `with_execution_observation` (`:245-281`) refuses if native formations/requests/terminal code changed. `BuildAdmission::artifact_only` (`:206`).

**Fail-open risks.** `same_source_choices` compares native binding evaluations only by `terminal_code`, not by other evaluation content (`:104-118`); stated in `with_execution_observation` error text only. Status: open (not traced to consumers).

### C6. `independent_component.rs` (237)

**Purpose.** `IndependentComponentProduct` couples a component description with its retained Terminal artifact and permission policy; `SelectedIndependentComponent` joins a verified component to the receiving provider plan; `OMGCMP01` retention codec.

**Shape.** `encode_retained` (`:141`), `recover_retained` (`:163`), `Reader` (`:207`). Constants `MAGIC = b"OMGCMP01"`, `MAX_BYTES = 128 MiB` (`:27-28`).

**How it decides.** `new` requires `description.expected_subject() == retained.artifact().view().identity()` (`:110`). `recover_retained` checks magic and package digest (`:168`), trailing bytes (`:173`), and refuses non-empty installation assumptions (`:196-200`). `PartialEq` ignores `native_derivation_inputs` (`:94-100`).

**Unused / one-caller (grep).** `encode_retained|recover_retained`: 18 occurrences in 6 files (e.g. `compiler/src/checked/component_cache.rs` 3, `abstract-operations/optimization-unit/src/identity.rs` 8 — that file may use unrelated names; not read).

### C7. `lib.rs` (99), `package_build.rs` (213), `package_build/retention_identity.rs` (745)

**Purpose.** `lib.rs` is the module/re-export list with two doc blocks. `PreparedPackageBuild` (`package_build.rs:19`) seals a completed Build continuation to inputs and a `PackageBuildHandoff`. `retention_identity.rs` computes `native_product_input_identity` (`:25`), a 32-byte digest under domain `omega-native-product-build-outputs-v7\0` over the computed Build config, dependency handoffs and admission, using content-addressed source coordinates instead of arena/SourceId numbers (header `:1-7`).

**How it decides.**
- `same_product_source` (`package_build.rs:64-98`) compares a long list of fields (root path, rollback, build sources, `same_source_choices`, request lists, generated sources, project declaration, application, assembly inputs, semantic-role discovery, accepted semantic bindings, `same_completion_input`).
- The digest destructures `ComputedBuildConfig`, `BuildConfig`, `EvaluatedBuildSettings`, `BuildAdmission` exhaustively; it deliberately skips `native_formations`, `evaluation_usage`, `observation_summary`, `toolchain_provider_defaults` (`retention_identity.rs:95-101`).
- Symbols hashed by hermetic normalized identity plus machine callable identity text (`:685-707`); occurrences hashed as source coordinate (package identity, origin, scope, stratum, relative path, content digest, span, `:639-683`).
- `symbol_kind_tag` is an exhaustive map of 32 `SymbolKind` variants (`:710-745`).

**Same job elsewhere.** `same_product_source` field list duplicates part of `BuildConfig::same_source_choices` plus request lists, conceptually the same equality as `retention_identity.rs::computed` digest; they are separately maintained.

**Unused / one-caller (grep).** `native_product_input_identity|same_product_source|has_source_admission`: 6 occurrences in 5 files (`compiler/src/checked/product_batch.rs` 1, `package_compilation.rs` 1, `component_cache.rs` 1, `package_build.rs` 2, `retention_identity.rs` 1).

**Fail-open risks.**
- `with_independent_component_descriptions` (`package_build.rs:185-193`) replaces `self.inputs` after `seal` without recomputing `build_handoff`. Whether the handoff depends on component descriptions was not traced. Status: open.
- `symbol` hashes `symbol.is_valid()` false as a single bool and returns `Ok` (`retention_identity.rs:685-689`): invalid symbols hash identically. Status: open.

### C8. `prepared_build.rs` (145), `prepared_source.rs` (266)

**Purpose.** Retained frontend/Build continuations. `CheckedProductSource::shares_terminal_publication` and `adopt_derived_code` (`prepared_build.rs:112-131`) let siblings share terminal publication when they share the program `Arc`, Psi optimizations, `same_source_choices`, provider defaults and sources. `ProductSourceIdentity` holds a `Weak<CheckedTrees>` and compares by pointer (`:135-145`). `prepared_source.rs`: `SourcePreparation` (catalogue mutex + frontends) with `fork`/`adopt` guarded by `(catalogue.len(), retained_count)` snapshot (`:32-52`); `DeferredProductCompilation` and `PreparedCheckedSource` cache typed frontiers keyed by `ProductSourceInputs` / `SourceFrontierInputs` (assembly + toolchain defaults) with a linear `find` over a `Vec` (`:99-101`, `:216-218`).

**How it decides.** Retention only happens when the `Arc` strong count > 1 (`:118`, `:211`), i.e. someone else holds the retention. `adopt` returns `Ok(false)` when counts drifted (`:46-48`).

**Fail-open risks.** `adopt_derived_code` guards the equivalence only with `debug_assert!` (`prepared_build.rs:127`); in release it adopts regardless. Status: open (callers not traced).

### C9. `publication.rs` (254), `terminal_publication.rs` (119)

**Purpose.** `publish_compilation` (`publication.rs:26`) installs the retained product (native executable, macOS `.app`, dynamic ELF, `<stem>.psi`) plus components and build outputs; refuses re-publication (`:30-32`) and non-publishable kinds (`:37`). `publish_flat_native_files` (`:171-254`) stages component images/descriptions, `omega-components.tsv`, the root file, and (COFF only) the Windows internet provider DLL, then installs atomically (`install_staged_files`). `ProductTerminalPublication` memoizes the Terminal program (or its refusal) in a `OnceLock`, refusing a different Psi optimization projection (`terminal_publication.rs:52-81`).

**How it decides.** macOS package iff `ObjectFormat::MachO` and application intent `Gui` (`publication.rs:88-92`); components with a package are refused (`:98-100`). Component/target equality (`:184`), export-index tab/newline refusal (`:202-204`), provider-DLL name collision refusal (`:230-237`).

**Same job elsewhere.** `publish_flat_native_files` is called by both the flat-native path (`:129`) and dynamic-ELF path (`:163`).

**Unused / one-caller (grep).** `publish_compilation`: 17 occurrences across 11 files.

**Fail-open risks.** none seen.

---

## Part D. Provider admission crate (`realization/provider-admission/src/`)

### D1. `lib.rs` (49), `lowering_facts.rs` (132), `admission_source.rs` (231)

**Purpose.** Re-exports; sealed cohorts and facts read by target lowering (`AdmittedLoweringFacts`, `AdmittedNativeCallbackCohort`, `AdmittedInstalledInterruptCohort`, `LoweringEntryCall`); and the facts admission reads (`AdmissionSource` `:30`, `ComponentCallFacts`, `AdmittedProgramEntryCall`), plus `AdmittedInstalledEntryExecution` (`:144`), which re-checks that an installed entry still matches its current cohort.

**How it decides.** `check_current` (`admission_source.rs:169-222`): requires current psi equals entry psi, operation equality, x86-64, a fatal-entries/interrupt-delivery consistency rule, bootstrap assumption present with matching psi and installation (`:174-184`); entry unique by `placement_index` (`:185-190`); a unique current scope with the same operation (`:192-199`) whose service, boundary, arguments, table/context type, validator, work, completion and acknowledgement application equal the scope's (`:200-208`).

**Cases by shape.** x86-64 only (`:176`).

**Unused / one-caller (grep).** `AdmittedInstalledEntryExecution`: 23 occurrences across 10 files. `selected_calling_boundary`: 13 occurrences across 8 files. Used.

**Fail-open risks.** One-sided comparisons in `check_current` (`:209-217`): `received.fatal_entries().any(not in scope.fatal_entries())` (received may have fewer), `received.context_anchor().is_some_and(!= scope's)` (if received has no anchor the check passes even if scope has one), and `!received.installation_assumptions().is_empty() && != scope's` (an empty received set passes). What gets through: a current cohort that drops a fatal entry, the anchor or the assumptions but keeps everything else. Status: open (not traced; the constructor `for_entry` is used with the same cohort in some callers, `callback_thunks.rs` etc., not read).

### D2. `provider_admission.rs` (492)

**Purpose.** Top-level orchestration `admit_native_providers` (`:177-359`), plus `AdmittedNativeCallableAuthority` (`:92`, constructor private), `AdmittedNativeProviders` (`:124`), `admit_checked_provider_installation` (`:361`), `require_reached_providers` (`:421`).

**Shape / sequence (read, `:177-359`).** program provider context check -> callback entries (`PreparedEntries::new`) -> checked-provider installation -> helper associations -> installed interrupts -> interrupt settlements -> `require_reached_providers` -> access lifecycles -> provider executions (`settle_provider_executions`) -> compiler builtins -> per-settlement checks -> empty releases -> effective policy (explicit rows + filesystem cohort rows) -> closure review -> program provider context -> assembled result.

**How it decides.**
- For every settlement it refuses an opaque foreign provider (not a checked component) when the boundary `requires_checked_non_observation` (`:245-260`) and refuses any non-component provider where `requires_checked_result` (`:261-272`).
- Effective policy: cohort rows are added to the explicit rows only if no explicit row has the same mechanism (`:283-291`).
- `require_reached_providers`: for each boundary reachable from the entry: `EmptyRunRelease` skipped; static interrupt operation must have an admitted mechanism equal to the expected `StaticInterrupt(kind)` (`:446-463`); boundaries with a placement operation skipped (`:464-471`); otherwise needs a selected requirement row or an assumed binding whose `required` equals the requirement text (`:472-485`).

**Cases by shape.** Fixed special classes: empty-run release, static interrupt, placement operation, other.

**Same job elsewhere.** Collecting settlements/mechanisms per source (interrupt, lifecycle, provider, builtin, empty release) repeats the same `(Vec<AdmittedBoundarySettlement>, Vec<AdmittedTerminalMechanism>)` pairing in `static_interrupts::settle`, `access_lifecycles::settle_native_lifecycles`, `access_lifecycles::settle_empty_releases`, `compiler_builtins::settle_compiler_builtins`, `settlements::settle_provider_executions`.

**Fail-open risks.**
- `require_reached_providers` (`:437-440`) uses `filter_map` with `?`: a reached boundary id with no `plan.boundary_machines` entry returns `None` (skipped), and `plan.identities.text(declaration.identity)?` (`:472`) skips a boundary whose identity text is absent. What gets through: a reachable boundary without a selected provider whose declaration or identity text is missing from the plan. Status: open (no comment; not traced to whether other code already rejects such plans; `ReviewContext::new` rejects an absent identity at `context.rs:69` but runs later and over `plan.boundary_machines` only).
- `interrupt_mechanisms.binary_search_by_key(boundary, ..)` (`:454-457`) assumes `interrupt_mechanisms` is sorted by boundary; `static_interrupts::settle` builds it from sorted `executions` (`static_interrupts.rs:411-444`), consistent (read).

### D3. `access_lifecycles.rs` (604)

**Purpose.** Settles native access lifecycles (`settle_native_lifecycles` `:34`) and empty-run releases (`settle_empty_releases` `:169`). A lifecycle call must have its profile supplied by the exact selected-provider call that produced it, observing the same unchanged extent (`bind_profile`, `:357-474`).

**How it decides.** Occurrence keyed on `OperationKind::BoundaryCall` whose boundary is a `CompilerPrimitive` lifecycle plan and whose psi operation is in the lowered function's operations (`:83-100`); `require_image_contract` (`:239-296`) - Terminal record shape, no borrowed storage, `require_runtime_value` (acyclic, no erased fields, no zero-size structural children), and on `Initialize` stored integer width equal to the carrier width; `selected_profile_call` (`:476-492`) needs exactly one installed call at the caller/producer site naming the requirement boundary; extent identity compared by `descriptor_results` payload targets (`:442-471`).

**Cases by shape.** `require_runtime_value` handles `Record`, `Sum`, `Mixed`, `FixedArray` with non-literal-zero length, and `_ => {}` for everything else (`:310-327`). Lifecycle `View` goes to `validate_access_view_content`.

**Stated limits (quote).** "native resident content requires retained loan forwarding for reference fields" (`:257`); "native lifecycle non-runtime fields require an authored custody agreement" (`:340`, `:348`); "native lifecycle narrowing requires checked concrete-value fitting evidence" (`:289`).

**Fail-open risks.** `_ => {}` in `require_runtime_value` (`:326`) accepts any structural shape other than the four named (e.g. scalar-like declarations). Status: open (not traced to which `StructuralTypeShape` variants exist beyond these).

### D4. `adapters.rs` (222), `callback_entries.rs` (326)

**Purpose.** `retain_selected_provider_adapters` (`adapters.rs:21`) projects checked-adapter rows into `SelectedProviderAdapter`s, refusing repeats and empty custody; `AdmittedProgramProviderContext` (`:127`) and `selected_synchronous_call_graph` (`:179`). `callback_entries.rs`: `project_native_callback_arguments` (`:22`) and `PreparedEntries` (`:98`) + `admit` (`:218`), the inbound handler entry ABI admission.

**How it decides.** `admit` (`callback_entries.rs:218-325`) requires entry-admitted callback, matching application index, registrar, psi, placement index, thunk psi; the body's declaration identity equals the callback body and the formals equal `thunk.parameter_formals()`; selected Calling application by (ABI identity, owner) with target and recomputed commitment equal (`:200-216`); the inbound plan equals the occurrence's `boundary_entry_plan`; parameters and origins match one by one (`:274-300`); formal ordinals must be unique (`:302-307`).

**Cases by shape.** `project_native_callback_arguments` returns `Ok(None)` for the whole slice as soon as one placement lacks `private_materialization` (`callback_entries.rs:39-41`). `admit` refuses `dispatch_only_parameter_count != 0` and any non-`SemanticFormal` origin (`:266`, `:285-288`).

**Same job elsewhere.** `PreparedEntries::new` (`:123-153`) and `arrival_helpers::flat_rows` (`arrival_helpers.rs:592-627`) both build an `IdTable<MachineId, Option<&TerminalMachine>>` from sorted machines by span from first to last id.

**Fail-open risks.** `PreparedEntries::new` returns an empty prepared set when `request.callback_occurrences.is_empty()` (`:114-122`); `admit_entries` then fails for any proposal because `occurrence()` finds nothing (`:189-198`), so not fail-open (traced).

### D5. `compiler_builtins.rs` (697), `compiler_builtins/catalog.rs` (328), `compiler_builtins/retained.rs` (547)

**Purpose.** Admit native compiler builtins. `retained.rs` derives proposals binding each demanded boundary to its one selected intrinsic row, plan and Terminal boundary (`derive_selected_intrinsic_settlement_proposals` `:141`), validates external provider owners (`:287`), and admits Terminal reference intrinsics (`admit_terminal_intrinsics` `:426`). `catalog.rs` converts `CompilerIntrinsicExecutionIdentity` to the local `CompilerBuiltinExecution` catalog via an exhaustive map (`SettledCompilerBuiltin::from_identity` `:24`). `compiler_builtins.rs` admits each proposal: `settle_compiler_builtins` (`:57-297`).

**How it decides.**
- `settle_compiler_builtins` first refuses any proposal whose settlement is `NoClosedCatalogIdentity` or `NoNativeBoundaryRealization` (`:67-90`); then x86 scalar FMA features (`admit_x86_fma_features` `:299`); then per proposal: the boundary's completion and `unconditional_termination` must equal the selected schema's method (`boundary_contract_matches_selected_schema` `:345-369`), the completion must equal the one the builtin implies (`compiler_builtin_completion` `:371-386`), the receiving policy must classify the mechanism (`:127-136`), then a per-execution `match`.
- Float arithmetic: `float_arithmetic_signature` (`:402-442`) compares the exact boundary signature to the operation/format.
- `validate_access_schema` (`:452-583`) compares the selected schema's single method against strings built from the Terminal identity text (`format!("{borrow}({accessor})")`, `"read"/"take"/"write"`, `primitive_identity` `named(name(u32))`, `:459-545`); i.e. keyed on source-spelling identity strings.
- `require_access_backing_origins` (`:604-697`) requires every reachable occurrence's origin to be captured RAM or a checked resident formation, and at least one occurrence.
- `catalog.rs`: `TaskRegisterActivate/Read`, `SegmentReload/Read`, `PrimitiveIntegerComparison` return `None` (`:178-181`, `:325`); `BuiltinFunction` non-float variants fall into `_ => return None` (`:266`).

**Cases by shape (quotes).**
- Hosted builtins by target: `HostedExitProcessI32|HostedWriteByteI32|HostedReadByte` each gated by `supports_target(request.target())` else error "local target catalog cannot realize ..." (`:230-262`).
- `ProgramImageRetainedSuffixStart`: only `target::NativeTarget::uefi_x64()` with an address-result boundary (`:172-186`).
- `AccessorScalarAccess`: `operation == Take` refused as "unsupported destructive take" (`:216`, `:224-226`).
- `StaticInterrupt`, `AccessLifecycle`, `EmptyRunRelease` refused through proposals (`:138-148`).
- `catalog.rs` `format_bearing_builtin` has an explicit arm per `Float*F32/F64` variant (`:82-163`); `compiler_builtin_execution` maps `NamedFloatConversion` by `(float(source), float(target), domain)` with `Exact` for int->float, `Exact|Saturating|Trapping` for float->int (`:297-323`); format change only when `from != format` (`:298`).
- `retained.rs::derive_selected_intrinsic_settlement_proposals`: skips everything when `demanded_boundaries.is_empty()` (`:166`).

**Same job elsewhere.** `boundary_contract_matches_selected_schema` is used at `compiler_builtins.rs:113`, `settlements.rs:200` and `retained.rs:471`. `SettledCompilerBuiltin::from_identity` call sites (grep): `static_interrupts.rs:428`, `retained.rs:262`, `retained.rs:535` (test), `access_lifecycles.rs:143`, `access_lifecycles.rs:222` = 5. The `catalog.rs` header says it is the sole mapping; consistent.

**Stated limits.** `catalog.rs:15-18` doctest (`compile_fail`) names `terminal_psi_to_abstract_operations::SettledCompilerBuiltin`, a path from another crate; not checked.

**Fail-open risks.**
- `retained.rs:210-217`: when a demanded requirement matches zero intrinsic rows (`matches.is_empty()`) the code `continue`s with no diagnostic; only `> 1` matches produce an error. What gets through: a demanded boundary with no intrinsic row is not an error here (it may be served by a non-intrinsic row). Status: open (no comment at the site; the header at `:4-9` says a row it cannot close is retained with its reason, which describes rows that exist).
- `catalog.rs:68-165` / `:257-267` `_ => return None`: unknown `BuiltinFunction` yields `None`, which becomes `NoNativeBoundaryRealization`, which `settle_compiler_builtins` rejects (traced, `compiler_builtins.rs:80-84`). Not fail-open.

### D6. `model.rs` (386)

**Purpose.** Request and admitted carriers: `NativeBoundaryRealization` (`:21`), `NativeProviderSettlement`, `NativeCompilerBuiltinSettlement`, `ProviderAdmissionRequest` (`:73`, 16 borrowed slices/fields), `NativeCallbackArgumentProposal`, `InstalledInterruptNativeEntryProposal`, `AdmittedInstalledInterruptEntry/Scope/Helper`, `AdmittedFatalInterruptEntry`, `AdmittedInterruptContextAnchor`, `AdmittedCallbackEntry`, `AdmittedBoundarySettlement`, `AdmittedBoundaryExecution`. Admitted types have `pub(super)` fields or `pub(super)` constructors.

**Cases by shape.** `AdmittedInstalledInterruptScope.arguments: [StructuralArgument; 6]` (`:155`): six-operand installation shape fixed in the type.

**Fail-open risks.** none seen (data only).

### D7. `settlements.rs` (604), `settlements/source_imports.rs` (442), `source_evaluated_bindings.rs` (502)

**Purpose.** `settlements.rs`: for each provider settlement derive a boundary realization: `rejoin_normalized_foreign_call` (import/table, `:384`), `rejoin_component_call` (`:328`), `rejoin_system_call` (`:501`); `settle_boundary` (`:90`) joins evidence to the selected plan and emits one admitted settlement per matching boundary. `source_imports.rs` derives the terminal mechanism identity for demanded import/table/syscall leaves and host-cohort rows (`derive_source_evaluated_mechanisms` `:32`). `source_evaluated_bindings.rs` prepares source-evaluated import and syscall executions (`prepare_source_evaluated_bindings` `:74`).

**How it decides.**
- Plan lookup is by exact evidence (`plan_by_exact_evidence(report_identity, exact_plan)`, `settlements.rs:312-325`); requirement rows by `identities.text(row.requirement_identity) == requirement` string equality (`:110-113`, `:405-409`, `:520-523`).
- Component call: ~12-way identity/commitment/target equality (`:345-366`).
- System call: Linux only via `CallingPolicy::native_syscall_for_target`; number non-negative; external row equal in binding, target, trait name, method, provider; call plan equals `evaluate_call_plan(policy, &signature)` (`:513-593`).
- Source-evaluated mints: `source_evaluated_bindings.rs:203-220` computes `provider_execution_report_identity`, `..._fingerprint`, `boundary_contract_report_fingerprint` itself by FNV-1a over the requirement text, plan report identity and locator digest (`fnv1a`, `:332-339`), plus the same-stack receipt id (`:221-227`) and admits `bytes` from `row.admitted_same_stack_bytes`, defaulting 256 KiB `SAME_STACK_BYTES` when 0 (`:124-133`, `:249`).

**Cases by shape.**
- `source_imports.rs:72-176`: match on `ProviderBinding`: `VtableField|TableFunction`, `Syscall`, `Import`, `_ => continue` (`:175`). For `Import`: callbacks slice `[]`, `[first, rest..]` with all equal registrar plan and context, else error "no unique matching registrar plan" (`:145-168`).
- `exact_demanded_import_plans` keeps only `Import|VtableField|TableFunction` rows, 0 matches `=> {}` (`source_evaluated_bindings.rs:368-381`).
- `rejoin_normalized_foreign_call`: tables (`VtableField`/`TableFunction`) matched with retained kind and equal field, else must be `Import`/`Import` (`settlements.rs:432-485`).

**Same job elsewhere.** "exactly one selected row for requirement" appears at `settlements.rs:405-415`, `:520-528`, and the count at `source_imports.rs:295-318`, `source_evaluated_bindings.rs:288-303`. "Exactly one retained external row" appears at `settlements.rs:416-425`, `:536-544`, `source_imports.rs:320-335`. `authenticate_callback_application` (`source_imports.rs:376-442`) and the registrar check in `static_interrupts.rs:257-271` both call `authenticated_callback_placements` and compare to the proposal's placements.

**Stated limits.** `source_imports.rs:5-9`: states that it "does not rejoin settlement coverage or re-compare targets, locators, and syscall numbers" because those were done earlier.

**Fail-open risks.**
- `external_calling_application` (`settlements.rs:271-310`): the comparison of the application's commitment/fingerprint to the plan schema runs only `if declared` (`:290-308`), where `declared` means the schema lists a calling application with the same `abi.requirement_identity`. If the schema lists none, the application taken from the retained external row is accepted without a schema comparison. Status: open (no comment; I did not trace whether the retained row is itself schema-derived).
- `source_imports.rs:55-57`: `let Some(...) = rows.selected_external else { continue }` skips any demanded boundary with no external selected row; `selected_count > 1` check happens after the skip (`:66`). A boundary that has two selected non-external rows is not reported here. Status: open.
- `mint_source_evaluated_imports` (`source_evaluated_bindings.rs:177-272`) creates the execution evidence that settlement later accepts (`ProviderExecutionEvidence` implemented by `SourceEvaluatedBindingExecution`, `:144-168`); evidence is self-minted from the plan, not independently attested. Stated in header (`:4-10`, "execution records or rejoin a complete supplied roster"). Status: stated.
- `prepare_system_calls` (`:274-330`): requirement count check across all plans (`:288-303`).

### D8. `static_interrupts.rs` (446), `static_interrupts/arrival_helpers.rs` (1025), `static_interrupts/scopes.rs` (590)

**Purpose.** Admit installed static interrupt handlers. `require_helper_associations` and `arrival_helpers::validate` check that every arrival helper (`Context`, `BeginCompletion`, `FinishCompletion`) is invoked with the arrival's unchanged acknowledgement token and that the controller body returns normally between begin and finish. `admit` binds each proposed handler entry to its checked installation occurrence, private placement and registrar table materialization. `scopes::prepare` builds `AdmittedInstalledInterruptScope`s and enforces `ConfigurationLease`. `settle` settles only operations with an admitted scope or helper.

**Shape.** `ArrivalHelperUse` (`arrival_helpers.rs:30`), `Token` (`:38`), `Invocation` (`:48`), `Associations` (`:60`), `Calls` (`:778`), `ExecutionOrder` (`:484`); `check_closure` (`:237`); `ConfigurationLease` (`scopes.rs:476`); `changes_configuration` (`:581`).

**How it decides.**
- Helper tokens flow through value-identity analysis (`ValueIdentityAnalysis::require_at_invocation`), keyed on value identity, not names. Order of begin/finish uses block dominance for cross-block and operation index within a block (`ExecutionOrder::precedes`, `:547-558`); a missing position returns `false` (fail-closed).
- Specific indices: operand 5 is the context (`declaration.structural_parameters[5]`, `:160`); `selected_body(module, operation, 3)` is the completion, 1 validator, 2 work (`scopes.rs:204-206`; `arrival_helpers.rs:161`) — keyed on `static_machine_ordinal`.
- Scope: structural access pattern fixed: `[0]` and `[1]` must be `MutableBorrow`, `[2]` and `[5]` `SharedBorrow` (`scopes.rs:195-198`).
- `ConfigurationLease::require` refuses `ActivateTaskRegister`, `ReloadSegmentDescriptors`, `GlobalDescriptorTable{Load}`, `RegisterAccess` where `changes_configuration` (GDT load, task register activate, segment reload, system write, MSR write) (`scopes.rs:503-519`, `:581-590`), `Install` reached from the closure (`:534-536`), and any boundary with no static-interrupt kind whose `published_service_ceiling` contains the machine-control service (`:542-549`). `require_masked` refuses only `RegisterAccess` with `InterruptControl(Enable)` (`:565-572`).
- Fatal (non-timer) handler: must be unable to return normally (`normal_completion_possible`), stack `Interrupted` or `Dedicated`, entry control `InterruptReturn` (`scopes.rs:246-267`).
- Bootstrap assumption required whenever the installation is reached; interrupt-delivery assumption required when fatal entries exist (`scopes.rs:325-332`).

**Cases by shape.** Six structural operands (`static_interrupts/arrival_helpers.rs:154`, `scopes.rs:174`); x86-64 only for gate geometry (`scopes.rs:441-443`); `Install => unreachable!()` inside the helper match (`arrival_helpers.rs:372`, after an earlier refusal at `:298-302`).

**Same job elsewhere.** `flat_rows`/`membership` (`arrival_helpers.rs:592-639`) vs `PreparedEntries::new` (D4). `Calls::new` call-edge extraction (`:788-933`) vs `HandleCallGraph::new` in `windows_handle_calls.rs:136-333` (both enumerate module invocations, boundary invocations, dynamic calls via `dynamic_callees`, and cleanup rosters).

**Stated limits.** `static_interrupts.rs:11-17`: "A recognized declaration alone never supplies a native implementation, and these rows grant neither root memory nor permission to publish IDTR".

**Fail-open risks.**
- `ConfigurationLease::require_masked` (`scopes.rs:556-577`) only recognizes `RegisterAccess` operations with `InterruptControl::Enable`. Other operation kinds that could re-enable interrupts are not examined. Status: open (not traced; what other op kinds could do so was not read).
- `scopes::prepare` skips a registration entirely when it has no admitted entries and no helper (`:114-116`); `settle` then fails only if the installation is reached (`static_interrupts.rs:389-394`). A registration not reached and not entry-bearing is skipped (stated by design in `static_interrupts.rs:11-14`). Status: traced.
- `AdmittedInstalledEntryExecution::check_current` one-sided comparisons: see D1.

### D9. `terminal_authority_permission_policy.rs` (245), `terminal_authority_policy.rs` (346), and `terminal_authority_policy/{classification, external_mechanisms, host_cohorts, windows_imports}.rs` (372, 288, 251, 316)

**Purpose.** Policy that maps terminal mechanisms to authority dispositions, and the receiving permission policy.
- `TerminalAuthorityPermissionPolicy` (`terminal_authority_permission_policy.rs:32`): sorted `(service_schema, requirement)` rows, deny-by-absence; build refuses empty/control-char requirements and duplicates (`:89-138`); `validate_retained_package_terminal_authority_permissions` requires one-for-one match with the accepted package policy (`:167-220`); `validate_package_terminal_authority_permission_custody` rejects repeated coordinates (`:222-245`).
- `TerminalAuthorityPolicy` (`terminal_authority_policy.rs:83`): `classify` for compiler intrinsics uses a closed inventory (`classification.rs`) or the parameterized caller-RAM scalar family; other mechanisms (`NormalizedForeign`, `Table`, `Syscall`, `CheckedPhysical`) need an explicit row (`:107-130`). `terminal_authority_policy_with_rows` (`:155-245`) refuses reserved compiler-intrinsic rows, zero contracts, non-Linux syscall targets, port writes not exactly `[PortIo]`, duplicates.
- `classification.rs`: `CLOSED_POLICY_ROW_COUNT = 494 + BuiltinFunction::COUNT` (`:30`), `closed_policy_mechanisms` builds the list and `assert_eq!`s its length (`:127`); `classify_compiler_intrinsic` (`:131-181`) and an exhaustive `classify_builtin_function` match with no wildcard (`:235-346`).
- `external_mechanisms.rs`: canonical identities for foreign imports (revalidates the entry plan, rejects if validation rewrites it, `:30-108`) and `conservative_syscall_terminal_mechanism` (`:110-237`, Linux x86-64/ARM64 only, refuses boundary `requires` and parameter qualifications).
- `host_cohorts.rs`: filesystem/time/console/DarwinClock cohorts keyed on `method.name` strings (`:44-90`, `:137-147`, `:187-194`); Darwin clock keyed on trait name `"DarwinClock"` plus a rebuilt Mach-O locator for libSystem symbols (`:207-251`).
- `windows_imports.rs`: reviewed list of 16 `(dll, export, classes)` rows keyed on the normalized locator identity digest (`:38-121`); `mechanism_row` requires target `WindowsX64`, non-zero implementation contract and `AdmittedPlan` argument contract (`:123-139`).

**How it decides.** Name-keyed: `settled_filesystem_cohort(name)` (fixed list of ~60 spellings, `:61-89`), `settled_time_host_cohort` (6 names, `:137-147`), `settled_console_cohort` (6 names, `:187-194`). `cohort_mechanism_row` (`terminal_authority_policy.rs:247-264`) sends Windows foreign mechanisms to `windows_imports` (locator keyed) and everything else to the name-keyed cohort tables. So for non-Windows foreign mechanisms the permitted classes follow the Requirement method **name** (source spelling), not the imported symbol; only Darwin clock additionally pins the symbol (`host_cohorts.rs:242-251`). Stated in header: "Requirement names only locate rows inside the canonical checked schema" (`host_cohorts.rs:5-6`).

**Cases by shape.** Hard-coded cohort name lists (above); ordinary-release filesystem names `close|find_close|close_handle` yield no mechanism row (`:85-87`, `:127-130`). Windows reviewed-import list is fixed at 16 entries (`windows_imports.rs:42-100`); `DuplicateHandle`, `CloseHandle`, CRT `_read/_write` explicitly unclassified (comments `:11-17`; tests `:175-185`, `:272-298`).

**Same job elsewhere.** Console classification exists twice: by method name (`host_cohorts.rs:187-205`) and by intrinsic identity (`classification.rs:147-155`). Filesystem facet lists in `host_cohorts.rs` vs Windows import classes (`windows_imports.rs:41-69`) overlap in meaning.

**Unused / one-caller (grep).** `filesystem_host_permission_rows|time_host_permission_rows`: 8 occurrences in 4 files (package-manager `review/candidate/semantic_bindings.rs` 3; definition/re-export files 5). `validate_package_terminal_authority_permission_custody|validate_retained_package_terminal_authority_permissions`: 11 occurrences in 6 files. `settled_filesystem_cohort|settled_time_host_cohort|settled_console_cohort`: all uses inside `host_cohorts.rs` (the header says it is the one place for cohort membership, `terminal_authority_policy.rs:15-22`; consistent).

**Stated limits.** `host_cohorts.rs:9-11`: ordinary-release filesystem requirements refuse unconstrained mechanism rows "until an occurrence-specific release contract is established". `windows_imports.rs:11-19`: DuplicateHandle/CloseHandle/CRT reads and writes "stay unclassified".

**Fail-open risks.**
- Classification of non-Windows foreign mechanisms by method name (above): a foreign import whose requirement method is named e.g. `read` and whose implementation is any symbol receives the filesystem content-read facet row (`host_cohorts.rs:127-135` via `terminal_authority_policy.rs:260-262`). What gets through: a mislabeled authority class for an import whose real behavior is broader than its method name. Status: open (the header says names "only locate rows inside the canonical checked schema", but the mechanism is the leaf's computed identity; I did not trace the schema's relation to the symbol).
- `terminal_authority_policy_with_rows` does not check `Table` kinds beyond zero contracts (`:185-195`). Not fail-open.

### D10. `terminal_authority_review.rs` (243), `terminal_authority_review/context.rs` (355), `reachability.rs` (332), `reviewer.rs` (403)

**Purpose.** Walks every authority edge reachable from the program entry and records one `TerminalAuthorityClosureLeaf` per reached boundary or checked operation into a `TerminalAuthorityClosureReviewReceipt` (`review_terminal_authority_closure` `terminal_authority_review.rs:52-98`). `ReviewContext` indexes boundaries, admitted mechanisms and component authority; `AuthorityGraph` pairs `SynchronousCallGraph` with physical observations (`PortWrite`, register access, admitted `Install` scopes); `Reviewer` expands each boundary once.

**How it decides.**
- `physical_authority_use` (`terminal_authority_review.rs:114-243`) is an exhaustive `match` over `AbstractOperation` (header `:13-18`): `PortWrite`, `ActivateTaskRegister`, `ReadTaskRegister`, `ReloadSegmentDescriptors`, `ReadSegmentSelector`, `GlobalDescriptorTable`, `RegisterAccess` yield checked physical/register use, every other variant `None`.
- `reachable_authority_edges` (`reachability.rs:126-300`): refuses machines whose assembly target requirement does not accept the profile (`:146-162`); `PortWrite` only on x86-64 (`:214-221`); register access only where `target_applicability` matches the profile's architecture (`:246-266`); service presence must equal the catalog's `requires_service_authority` (`:269-274`); a register operation whose service is `None` is `continue`d with no leaf (`:275-277`); `Install` needs an admitted scope with matching boundary (`:180-187`) and adds validator and work bodies to the work list (`:205`).
- `Reviewer::expand_boundary` (`reviewer.rs:71-287`): lifecycle/empty-release boundaries must have the exact admitted intrinsic and an **empty** disposition (`:96-126`); static interrupt helpers likewise, `Install` needs an admitted scope (`:127-170`); component boundaries re-admit the component's own leaves (`:179-203`); `CheckedAdapter` rows recurse into the adapter candidate's closure (`:205-227`); `CompilerIntrinsic|Import|Syscall|VtableField|TableFunction` take the exact mechanism (role must match binding, `context.rs:310-354`) and optionally replace the policy with checked-handle rows (`:236-252`); `VtableSlot` is refused (`:280-284`).
- `admit_leaf` (`reviewer.rs:350-402`): classification failure is an error only when a permission policy was supplied; permission row lookup keyed on `(schema digest, requirement text)`.
- `ReviewContext::selected_requirement` rejects a plan whose `target` string differs from the profile's name (`context.rs:254-260`); `missing_requirement` lists up to 12 same-arity rows, message length ≤ 16384 (`:264-308`).

**Cases by shape.** Binding variants listed above. `admit_program_root` refuses any direct checked instruction when a permission policy is present: "direct checked instruction has no exact receiving-permission subject in this policy" (`reviewer.rs:293-295`).

**Same job elsewhere.** Reachable-body traversal duplicated with `terminal_installation::SynchronousCallGraph` (wrapped, not copied) and `arrival_helpers::Calls`. Plan/boundary dedup maps appear in `ReviewContext::new` (`context.rs:66-77`) and `static_interrupts::settle` (`:358-367`).

**Fail-open risks.**
- `Reviewer::admit_leaf` (`reviewer.rs:358-367`): when `permission_policy` is `None` and `physical_policy.classify` fails, the leaf is recorded with `exercised = None` rather than refused. Stated: `terminal_authority_review.rs:6-10` ("when `None`, exact mechanisms and available classifications are retained without claiming receiver admission") and `context.rs:38-41`. Status: stated.
- `reachability.rs:275-277`: register operations with no service are skipped with no leaf. Status: traced by the preceding check that `service.is_some()` equals the catalog's `requires_service_authority` (`:269-274`), so skipped ones are exactly those the catalog says need no service authority.

### D11. `windows_handle_activations.rs` (330), `windows_handle_aliases.rs` (803), `windows_handle_arguments.rs` (689), `windows_handle_calls.rs` (390), `windows_handle_conditions.rs` (198)

These belong to a Windows file-handle lifetime replay; sibling modules (`windows_handle_contracts`, `_inputs`, `_lifetimes`, `_origins`, `_paths`, `_replay`, `_scalar_table`, `_storage`, `_values`) are declared at `terminal_authority_review.rs:28-42` but are not in this unit.

**`windows_handle_activations.rs`.** Computes a "balanced path" relation over origin equations (call/return matching through the same invocation) with a worklist capped at 65,536 pairs (`:56`); outputs `ActivationPaths` (`:32`) with `balanced_sources`, `outer_contexts`, `inner_contexts`. **It is declared `#[cfg(test)]` (`terminal_authority_review.rs:26-27`).** Grep `windows_handle_activations|ActivationPaths` over `omega-rust`: 4 hits, all in `terminal_authority_review.rs:27` and the file itself (the struct, its impl, and one test call at `:308`). No non-test user. Fail-open: none seen (budget exhaustion returns `Err`).

**`windows_handle_conditions.rs`.** `ScalarConditions` (`:13`) keeps necessary scalar guards by `ScalarTerm`; joins intersect; arrival renames values simultaneously. Also **`#[cfg(test)]`** (`terminal_authority_review.rs:31-32`); grep `ScalarConditions|windows_handle_conditions` shows only the declaration and the file itself for this crate (other hits are the unrelated `terminal_psi::ScalarConditions`). `require` returns `true` without recording when the term is a live storage observation (`:45-47`); `proves` negates through `require` so a storage-observation term is never "proved" (`:57-60`, traced).

**`windows_handle_aliases.rs`.** `HandleWordAliases` (`:17`: scalar type, exact acquired instance, dependency set), `HandleStorageRange`, `CapturedHandleRegion`, `HandleAliases` (`:246`: scalar table, exact cells, bounded regions). Rules: an exact handle needs a 64-bit non-address integer carrier (`handle_word`, `:111-113`); narrowing drops exactness permanently (`represented_as`, `:56-64`); any write overlapping a cell retires its exactness but keeps dependencies (`invalidate_overlapping`, `:444-450`); capture/install move regions between homes with offset checks (`:471-527`); `stored_word` returns `untracked` when no cell exists (`:367-371`).

**`windows_handle_arguments.rs`.** Solves origin equations to finite sets of closed integer/boolean/`CurrentProcess` atoms with budgets `MAX_VALUES = 64`, `MAX_COMBINATIONS = 1024`, 1,048,576 steps (`:18-19`, `:220`); unseeded cycles become `Unknown` (`:265-283`). Per-API checks keyed on `WindowsHandleContract` and argument ordinals: `closed_argument_roots` (`:122-146`) lists CreateFile `[2,3,5,6]`, DuplicateHandle `[0,2,5,6]`, QueryFileMode `[3,4]`, Read/Write `[3]`, QueryFinalPath `[2]`; `acquisition` requires args 3 and 6 exactly 0 (`:367-368`), `duplicate` requires args 0 and 2 to be the `CurrentProcess` pseudo-handle, arg 5 signed-32 zero, arg 6 `== 2` (`DUPLICATE_SAME_ACCESS`, `:400-405`), `mode` args 3 = 4 and 4 = 16 (`:425-426`), `transfer` arg 3 = 0 (`:449`). Cases by shape: these fixed constants and ordinals are the "reviewed API" shapes. Fail-open: `actual` requires a non-empty `Finite` set else errors (`:301-307`), so unknown arguments are refused (traced).

**`windows_handle_calls.rs`.** `HandleCallGraph::new` (`:136-333`) builds executable caller edges from module invocations, boundary invocations, dynamic calls (full target roster from `dynamic_callees`), and nominal cleanup rosters; `boundary_target` (`:353-390`) chooses `CallTarget::Boundary` (component or unresolved foreign), `Machine` (program entry, target family member, checked adapter), or `Intrinsic`. Caps `IdTable::from_rows_with_gaps(.., 65_536)` (`:153`, `:177`, `:318`).

**Same job elsewhere.** `Calls::new` in `arrival_helpers.rs:788-933` (see D8).

**Unused / one-caller.** See D11 greps above.

**Fail-open risks.** In `windows_handle_aliases.rs`, `stored_word` of an address with no tracked cell yields an `untracked` word (`:367-371`), meaning "no tracked handle flows from here" (comment `:24-26` states that this supplies no value or permission and that the canonical replay owns initialization/access). Status: stated; the replay owner (`windows_handle_replay`, `_storage`) is outside this unit and not read.

---

## Coverage

- **Read in full:** all 51 files, 21,759 lines (stage 08: 1,028 + 996; stage 09: 268 + 1,574; products: 309 + 283 + 730 + 221 + 174 + 575 + 339 + 237 + 99 + 213 + 745 + 145 + 266 + 254 + 119; provider-admission: 231 + 49 + 132 + 492 + 604 + 222 + 326 + 697 + 328 + 547 + 386 + 604 + 442 + 502 + 446 + 1,025 + 590 + 245 + 346 + 372 + 288 + 251 + 316 + 243 + 355 + 332 + 403 + 330 + 803 + 689 + 390 + 198).
- **Skimmed:** none.
- **Not read:** none.

## Five things a maintainer most needs to know about this unit

1. Stage 08 has two independent implementations of block ordering and branch resolution (`ordinary.rs:85`, `:835` vs `validation.rs:256`, `:428`); the validator never re-runs the production fixup checks (`ordinary.rs:559-777`) and trusts the pre-layout rows for placeholder bytes and callback materializations (`validation.rs:352-406`).
2. Stage 09's validator does not recompute or identity-check the relaxed layout; it checks roots and `usage.within(budget)` only (`x86_branch_relaxation.rs:217-234`, `lib.rs:139-166`), as `lib.rs:6` says. Relaxation itself only shrinks conditional branches and only on x86-64.
3. Provider admission is a long chain of independent re-joins (`provider_admission.rs:177-359`); several joins are keyed on text (requirement identity strings, method names: `settlements.rs:110-113`, `host_cohorts.rs:61-89`, `compiler_builtins.rs:459-545`), and non-Windows foreign mechanisms are classified by method name (`terminal_authority_policy.rs:255-264`).
4. Places that skip rather than refuse: `require_reached_providers` `?` skips (`provider_admission.rs:437-440`, `:472`), `external_calling_application` `if declared` (`settlements.rs:290-308`), intrinsic derivation `matches.is_empty()` continue (`retained.rs:210-217`), one-sided `check_current` comparisons (`admission_source.rs:209-217`), and `require_masked` recognizing only one register-access form (`scopes.rs:565-569`).
5. Two Windows-handle modules in this unit exist only under `#[cfg(test)]` and have no non-test users (`windows_handle_activations.rs`, `windows_handle_conditions.rs`; `terminal_authority_review.rs:26-32`), while the production replay modules they pair with are outside this unit.

## What you are unsure about

- Whether any other stage-08 path consumes the three AArch64 elided/fused dispositions that `ordinary.rs:349-374` sizes but `resolve_row` rejects (`:528`); only counted occurrences, did not read `row_encoding.rs`, `exit_contract.rs` or the stage-08 `lib.rs`.
- Who validates the pre-layout encoding store that stage-08 validation trusts (`validation.rs:45-53`).
- Whether `CompileReport::checked` is called through any path other than `CompileReport::checked(` (zero hits for that spelling).
- The meaning of the sibling Windows-handle modules not in this unit (`windows_handle_contracts`, `_inputs`, `_lifetimes`, `_origins`, `_replay`, `_storage`, `_values`, `_scalar_table`, `_paths`), so "stated" classifications in D11 rest on comments only.
- Whether `PreparedPackageBuild::with_independent_component_descriptions` (`package_build.rs:185-193`) leaves a stale `build_handoff`; not traced.
- Whether the retained external binding row compared in `external_calling_application` (`settlements.rs:271-310`) is schema-derived, which decides if the `if declared` branch can accept a mismatched application.
