# Back half: the sealed portable program and Omega stages 00 to 09

Checkout `worker-23` (a few commits after `457adf4a11f`). Read-only; nothing built or run.
Tags: **[opened]** = I read the code at the cited lines in this checkout. **[record]** = reported by an atlas
digest/record and not re-opened by me (records are right about 82% of tested claims; "unused" and "same job"
are least reliable). "suspected", "not traced", "open" are the readers' words, kept as written.

Path shorthand (under `omega-rust/`): `S00/`..`S09/` = `omega/compilation/pipeline/<NN>_…/src/`;
`NC/` = `omega/compilation/native-compiler/src/`; `ME/` = `omega/compilation/backend/machine-emission/src/`;
`OU/` = `omega/compilation/representations/abstract-operations/optimization-unit/src/`;
`SEM/` = `…/optimization-unit/semantics/src/` (`UV` = `unit_validation.rs`, `CAND` = `candidates.rs`);
`codec/` = `psi/compilation/representations/terminal-psi/codec/src/`;
`P08/` = `psi/compilation/pipeline/08_lowered-psi-to-terminal-psi/src/`;
`X86` = `omega/compilation/backend/instruction-set-architectures/isa-x86_64/src/selected_form_encoding.rs`;
`A64` = the same file under `isa-aarch64`.

The default driver is `NC/pipeline.rs:75-208` **[opened]**: `lower_to_target` (00→01→02) and `emit_target`
(03→04→05→06→frame→07→08→09→`assemble_fixed_frame_realization`→fragment emission). Every boundary below cites
the hand-off line in that driver.

---

## Boundary P: Psi 08 → Omega 00 (the sealed portable program)

1. **Handed over.** `finalize_terminal_program` returns
   `TerminalProgramProduct::Portable(CanonicalTerminalArtifact)` built by `CanonicalTerminalArtifact::from_parts`
   (`P08/lib.rs:104-126`) **[opened]**. The artifact holds semantic bytes (`PSITERM\0` module), the Psi
   optimization execution record, optional debug bytes, a manifest, and an `Arc<TerminalModule>`
   (`codec/canonical_artifact.rs:38-45`) **[opened]**. Omega 00 consumes `&CanonicalTerminalArtifact` in
   `NativeScopePreparation::prepare` (`S00/artifact_admission.rs:170-223`) and `::lower` (`:225`) **[opened]**,
   producing `VerifiedNativeArtifactInput`; the driver hands that to stage 01 as
   `input.into_optimization_input()` (`NC/pipeline.rs:86`) **[opened]**.
2. **Trusted / re-checked.** The in-process route encodes the module with `encode_module` →
   `encode_execution_module`, which runs only `scalar_qualification_wire::validate` and `validate_canonical_order`
   (`codec/semantic_module.rs:4071-4075`) **[opened]**. Psi 08 additionally runs
   `terminal_semantics::activation_storage::validate_module` (`P08/lib.rs:113-117`) **[opened]**. The full receiving
   validator (`decode_module`: structural foundation, identity formations, dynamic descriptor admission, re-encode
   byte compare, `codec/semantic_module.rs:4093-4120`) **[opened]** runs only when bytes are decoded through
   `CanonicalTerminalArtifact::from_bytes` (`codec/canonical_artifact.rs:139-184`) **[opened]**. Production
   `from_bytes` callers found by grep: compiler-storage record read (`psi/compilation/psi-compiler/terminal-production/src/custody.rs:74`),
   component description (`omega/compilation/backend/artifacts/component-description/src/component_description.rs:1173, 1425`),
   interpreter embedding (`psi/interpreter/terminal-interpreter/src/embedding.rs:2369, 2926`), build-time
   evaluation accounting, CLI inspect **[opened grep]**. Whether a default `omega build` passes its artifact through
   a record read before Omega 00 was **not traced**. Omega 00 then trusts the module's obligations as discharged
   (see the obligation trace below) and re-derives target selections, layouts, native scope and scalar demand from
   it (`S00/artifact_admission.rs:176-214`) **[opened]**.
3. **Check on the form and when.** `decode_module` at decode time (foreign artifacts, stored records, interpreter
   embedding). Not on the in-process `from_parts` route **[opened]**. `pipeline.md:74-84` says received artifacts are
   reused "under host authorization" without proof replay **[opened]**.
4. **Independence.** `decode_module` re-encodes with the producer's own `encode_raw` and compares bytes
   (`codec/semantic_module.rs:4115-4118`) **[opened]** — the canonicality check is a call into the producer's
   encoder. The foundation validator is a separate 11,882-line hand-written validator (`codec/foundation_validation.rs`)
   **[record, area_12]**; `attachment_storage_types` in it is a line-for-line copy of `UV:17908-17947`
   **[record, area_05 check 22]**.
5. **Identity.** `TerminalPsiIdentity` = vocabulary marker + fingerprint of the semantic bytes
   (`codec/canonical_artifact.rs:85-91`) **[opened]**. Inside: dense ids (`MachineId`, `OperationId`, `ObligationId`,
   `PlaceId`…) and identity-text tables compared by text per owning table (`TP/identity.rs:22-39` etc.)
   **[record, area_12]**. Omega 00 joins on identity text with exactly-one rules:
   `S00/callback_settlement.rs:102-106, 273, 291`, `S00/entry_settlement.rs:209-217` (`split_once("::")`),
   `S00/native_execution_scope.rs:409-451, 606-638` (14 text sites in stage 00) **[record, area_04]**; absent text
   becomes `""` at `S00/callback_settlement.rs:520-523` **[record, open]**.
6. **Lost / skipped.** Stage 00 silent drops, 12 sites, e.g. integer compare with no realization row skipped
   (`S00/settlement.rs:130-137`, digest-checked), reachable boundary with no provider stays in scope without a body
   (`S00/native_execution_scope.rs:409-419`, digest-checked), operand types of FMA/bitwise/shift unchecked
   (`S00/lowering_scalars.rs:264-306`, open) **[record, area_04]**.

### Obligation trace (Omega 00 → end)

- **Where facts come from.** `OU/construction.rs:345-404` `collect_fact` emits
  `OptimizationFact::OperationObligationReference { obligation, support }` for (a) every operation that
  `operation_obligation` (`:406-490`) lists — range-obligated primitive stores/locals, structural scalar field
  stores, atomic resident obligations, byte-view field predicates, byte-sequence concatenation capacity,
  subslice/write/store/read obligations, element-view subslice and region obligations, element-view read/write,
  `IntegerExactCast`, obligated `IntegerShift`, obligated `IntegerArithmetic`; wildcard `_ => None` (`:488`) —
  (b) every runtime index of any operation (`:352-364`), (c) scalar record-field range obligations (`:366-381`)
  **[opened]**.
- **What Omega 00 does with them.** `build_verified_psi_optimization_unit` (`S00/optimization.rs:79-91`) →
  `source_checked_obligation_facts` (`:93-279`) **[opened]**: for every `OperationObligationReference` in every
  function, and every runtime-index segment on edge bindings and cleanup rosters (`:242-276`), it pushes one
  `AcceptedObligationFact::new(unit.psi, [0; 32], machine, owner, obligation, proposition)` (`:171-178`,
  `:265-272`). The second argument is `proof_bundle_fingerprint` (`OU/proof.rs:109-118, 207-214`) **[opened]**:
  it is always 32 zero bytes. `proposition` is empty except for a `WholeRepresentation` element view, where it is
  the canonical key of a count goal computed from layouts (`:128-170`). For range stores/locals/atomics a
  `SourceCheckedScalarReceiving { value, restriction }` is attached when the restriction computes (`:179-239`); when
  it does not compute (`??`/`?` inside `find_map`, `:198, 210-223`) the fact is still pushed without it **[opened]**
  (matches the record's "open" item). No evidence term, certificate or proof question is read or attached.
  Nothing in the function consults whether the front half discharged the obligation.
- **What `AcceptedObligationFact` is.** A struct `{ identity, psi, proof_bundle_fingerprint, machine, owner:
  PsiProvenance, obligation, proposition: Vec<u8>, scalar_receiving }` (`OU/proof.rs:109-118`) whose `identity`
  is a hash of its own fields (`:215-223`, `:250-260`) **[opened]**. `attach_accepted_obligation_facts`
  (`OU/proof.rs:519-542`) checks only: not already attached, `psi` matches, self-hash matches, sort and no
  duplicate `(machine, owner, obligation)`; then recomputes the unit identity **[opened]**.
- **Consumers in stage 01 and later** (grep of `accepted_obligation_fact|AcceptedObligationFact|accepted_fact` under
  `pipeline/`: S01 `rules.rs`, `proof_check_elision.rs`, `sccp.rs`, `global_value_numbering.rs`, `value_ranges.rs`;
  S03 `legalization*.rs`, `selection_*.rs`; S04 `literal_fold_rewrites.rs`) **[opened grep]**:
  - S01 lookup `accepted_obligations::accepted_obligation_fact` (`S01/rules.rs:129-161`) takes the first
    `OperationObligationReference` for the operation, then finds the fact by `(machine, Operation(op), obligation)`
    **[opened]**.
  - S01 proof-check elision gates each rewrite on that lookup (`S01/proof_check_elision.rs:67-71, 152-158`)
    **[opened]**. Rules that remove a runtime check only valid under the obligation: integer self-divide `x/x → 1`
    for any policy (`:634-668`, constant `IntegerType::one`), self-remainder `x%x → 0` (`:670-…`), zero-dividend
    `0/x → 0` and `0%x → 0` for exact, wrapping and saturating (`:559-596`), exact negative-one shift right
    (`:238-257`) **[opened]**. Each deletes a division (and its divide-by-zero behaviour) or a shift (and its count
    obligation). The stage-01 candidate validator re-checks the same fact in the same table:
    `CAND:2835-2856` requires the reference fact and `input.accepted_obligation_fact(..).identity == witness`
    **[opened]**.
  - S01 SCCP uses the fact as a witness for proof-certified folds (`S01/sccp.rs:345-364`) **[opened]**; whether
    `arithmetic.evaluate` refuses overflow/zero divisor itself was not opened.
  - S01 GVN uses it as the CSE payload (`S01/global_value_numbering.rs:2151, 2185-2212`) **[opened grep]**.
  - S01 range rule (`S01/value_ranges.rs:170-193`) requires a matching `ProofQuestion` with
    `canonical_certificate` and `proof_bundle_fingerprint == accepted.proof_bundle_fingerprint` **[opened]**. In
    `omega-rust/omega`, `proof_questions` is only ever initialised to `Vec::new()` (`OU/construction.rs:73`,
    `OU/identity.rs:2268`); `ProofQuestionIndexError` exists (`OU/proof.rs:545`) but no attach function was found
    **[opened grep]**. So this rule can find no question on the production path as far as grep shows.
  - S03 legalization `accepted_fact` (`S03/legalization.rs:234-244`) takes the first fact matching
    `(machine, owner, obligation)` and errors (`custody`) when none **[opened]**; used for exact shifts
    (`S03/legalization_source.rs:656-689`) and exact arithmetic (`:691-727`) plus byte/element-view and subslice
    rows (`:343-382, 1142-1467, 2412-2430`) **[opened for 656-727; others by grep]**.
  - S03 selection carries the identity into instruction kinds: `ExactBinary` → `ExactAddI64/SubtractI64/
    MultiplyI64/DivideU64/DivideI64/RemainderU64/RemainderI64 { obligation, accepted_fact }`
    (`S03/selection_scalar_graph.rs:4154-4301`), with no guard instruction emitted before or after (`:4265-4300`)
    **[opened]**. Wrapping/saturating division likewise (`:3825-3939`, `:4402-4524`) **[opened]**. Byte-sequence
    read emits `Load8Indexed` after an `ExactAddI64` offset, with the obligation and fact recorded only in a
    memory-access role, no `index < length` compare (`S03/selection_views_and_bytes.rs:1200-1266, 1318-1351`)
    **[opened]**. Element-view `RepresentationBytes` emits address, `ExactSubtractI64`, `ExactDivideU64` with no
    region bound compare and requires the fact (`:1910-1963`); `WholeRepresentation` requires the fact and scales
    the count with `WrappingMultiplyI64` (`:1879-1909`) **[opened]**.
  - S04 literal folds forward `{obligation, accepted_fact}` unchanged into immediate forms
    (`S04/literal_fold_rewrites.rs:3898-3927`) **[opened]**.
  - Encoders: x86-64 `ExactAddI64` is a bare `lea` (`X86:13945-13947`), `ExactDivideU64` a bare `div`
    (`:13948-13954`), `WrappingDivideI64` special-cases `-1` but has no zero test (`:13964-13979`) **[opened]**.
    AArch64 `ExactAddI64` decodes to one `add`, `ExactDivideU64` to one `udiv` (`A64:2556-2565, 2606-2612`)
    **[opened]**. The `accepted_fact` field appears in the x86-64 encoder file only in tests (`X86:21217-23102`)
    **[opened grep]**.
- **Runtime checks removed downstream because of an accepted fact** (the fact is required to select the form):
  overflow checks on exact add/subtract/multiply (no `jo`/flag test; plain `lea`/`add`), divide-by-zero handling on
  exact/wrapping/saturating division other than the hardware fault x86 `div` raises, byte-sequence index bounds
  (`Load8Indexed` without compare), element-view region bounds, exact shift count range (legalized as
  `ExactShiftLeft/Right` with the fact; the encoder body for these was not opened), plus the stage-01 algebraic
  deletions above. The design doc states the interpreter still checks (`wiki/drafts/designs/check_on_lowered_form.md:226-230`)
  **[opened doc]**; I did not open the interpreter.

### Boundary 00 → 01 (inside `optimize_abstract_operations`)

1. **Handed over.** `VerifiedPsiOptimizationInput` (`S00/optimization.rs:282-294`) → `optimize_abstract_operations`
   (`S01/lib.rs:135-160`) builds `VerifiedPsiOptimizationUnit` via `build_verified_psi_optimization_unit`
   (`S01/lib.rs:147`) **[opened]**. Plan: `AbstractOperationPlan`; unit: `PsiOptimizationUnit` with facts.
2. **Trusted / re-checked.** With at least one selected step, `VerifiedPsiOptimizationSession::new` reconstructs
   ownership frontier facts and runs `validate_psi_optimization_unit_with_admitted_cycle_machines`
   (`S01/pass_manager.rs:1294-1322`) **[opened]**. With no steps, `from_transformed` runs only
   `validate_transformed_psi_context` (`:1324-1334`) **[opened]**. Accepted facts are trusted: UV checks psi, self-hash
   and order (`UV:9697-9704`) and machine membership (`UV:1167-1171`) **[opened grep]**; `proof_bundle_fingerprint`
   has 0 hits in `unit_validation.rs` **[opened grep]**.
3. **When.** Default compile, when steps are selected **[opened]**.
4. **Independence.** UV calls the constructor's own `optimization_unit::operation_definition` / `operation_uses`
   (`UV:2649-2675`), so a variant missed by the constructor's wildcards is missed on both sides
   **[record, area_05 check 7]**. For accepted facts the checks are self-hash and order only **[opened]**.
5. **Identity.** Dense ids; unit identity = content hash recomputed on attach (`OU/proof.rs:540`) **[opened]**.
6. **Lost.** `operation_obligation` wildcard (`OU/construction.rs:488`) gives no fact for an unlisted
   obligation-carrying operation **[opened]**; OU constructor wildcards at `:608, 741, 910` **[record, open]**.

### Boundary 01 → 02

1. **Handed over.** `ValidatedOptimizedAbstractPlan` (`S01/lib.rs:140`, `publication.rs`) →
   `optimized.into_lowering_parts()` → `lower_optimized_to_target_operations(program, lowering_facts, request)`
   (`NC/pipeline.rs:94-95`; definition `S02/lib.rs:417`) **[opened]**.
2. **Trusted / re-checked.** Each rewrite candidate is validated in default compile by
   `validate_psi_rewrite_candidate` (`S01/pass_manager.rs:603`) **[opened]**, which replays on a clone and calls
   `revalidate_rewrite_output` (`CAND:10662-10690`) **[opened grep]**. An empty selection must not change the plan
   (`S01/lib.rs:154-158`) **[opened]**. Unit-level tables (`pruned_machines`, `accepted_obligation_facts`) "protected
   only by revalidation and the identity recompute" (`CAND:10686-10692`) **[record, open]**.
3. **When.** Default compile **[opened]**.
4. **Independence.** Candidate validators re-derive the rewrite; several share rules with the proposer:
   `call_result_constant` "producer and validator share the rule" (`CAND:9824-9851`, inferred); classifier and
   measure written twice (`S01/rules.rs:978` vs `S01/pass_manager.rs:149-176`) **[record]**. Fact checks read the
   stage-00 table (`CAND:2835-2856`) **[opened]**.
5. **Identity.** Dense ids; pass identities compared as byte strings (`S01/pass_manager.rs:224-260`,
   digest-checked) **[record]**.
6. **Lost.** CFG ignores `StructuralCase`/`InvokeNoNormal`/`ExternalCompletion` edges and liveness ignores
   `StructuralCase` successors (`S01/control_flow_analyses.rs:303-323`, `S01/semantic_analyses.rs:2241-2249`,
   digest-checked, reachability open) **[record]**.

### Boundary 02 → 03

1. **Handed over.** `ValidatedOptimizedTargetOperations` → `stage_optimized_instruction_selection(
   TargetSelectionInput { target, source: optimized.plan(), unit: optimized.unit(), origins, … })`
   (`NC/pipeline.rs:105-127`) **[opened]**. Stage 03 reads the target plan and the stage-01 unit (including
   accepted facts) side by side.
2. **Trusted / re-checked.** `legalize_target_operations` (`S03/legalization.rs:405-432`) re-projects rows and runs
   the installed-interrupt check, `validate_boundary_origins`, `derive_source_function_rosters` only; it does not
   run the audit; `seal` verifies nothing (`:440-463`) **[record, digest-checked]**. Optimised nodes are paired with
   target rows by position with all `CleanupCall` rows trailing (`LSrc:3004-3023`) **[record]**.
3. **Check and when.** `verify_target_operation_plan` (`S03/legalization.rs:480`) is reached only from
   `image_emission::replay::verify_retained_target_operations`
   (`omega/compilation/backend/images/image-emission/src/function_fragments.rs:3859-3870`) **[opened]**, whose
   callers are `omega/compilation/compiler/tests/corpus_runner.rs:1060, 1070` **[opened grep]**: corpus/test only.
4. **Independence.** The audit "rebuilds every target row from the abstract side and compares" **[record]**, but
   shares tables written twice with stage 02, e.g. `access_covers` identical text in `S02/references.rs:888-896`
   and `LRC:4582-4590` (digest-opened) **[record]**.
5. **Identity.** `(PlaceId, path)` custody keys; exactly-one joins `SIN:93-96`; `accepted_fact` first match, no
   duplicate rejection (`S03/legalization.rs:240-243`) **[opened]**.
6. **Lost.** 15 stage-03 silent drops (e.g. `LRC:6637` `_ => {}`; `SIN:647-649`) **[record]**; "an exact
   add/sub/mul/div whose operands have a different integer type than `scalar_type`" unless an obligation fact or
   earlier stage guarantees it (`LSG:8451-8459`, open) **[record]**.

### Boundary 03 → 04

1. **Handed over.** `ValidatedSelectedInstructions` → `optimize_selected_instructions(SelectedOptimizationInput {
   program, custody, register_environment, selections, budget_per_pass })` (`NC/pipeline.rs:128-138`) **[opened]**.
2. **Trusted / re-checked.** One executed phase only; anything else `UnsupportedComposition`
   (`S04/selected_optimization.rs:135-155`); each rewrite admits, clones, edits one instruction, seals
   **[record]**. Plan identity reuse with `prefix_verified` trusts the caller, `debug_assert!` only
   (`SI/functions.rs:201-231`, stated) **[record]**.
3. **Check and when.** Analyses' "compute, then seal" pairs: the seal only hashes and counts (`SIA/lib.rs:234`,
   `SIA/live_ranges.rs:1555`, `SIA/liveness.rs:2087`, `SIA/machine_effects.rs:877`) **[record, area_05]**; runs
   inline in default compile **[record]**.
4. **Independence.** Hash/count receipts; `seal_pre_allocation_machine_effects` always `Ok` **[record]**.
5. **Identity.** Dense virtual-register/instruction ids; rules keyed by `MachineSemanticKind` pair
   (`S04/branch_and_compare_rewrites.rs:187-191`) **[record]**.
6. **Lost.** Zero-extension removed after `Float32ToBits`/`LoadPacked` "without any proof" of zero upper bits
   (`S04/copy_and_extension_rewrites.rs:1475-1477`, open); refusal reasons collapse to "declined"
   (`S04/rewrites.rs:1197, 1692, 1757`) **[record]**.

### Boundary 04 → 05

1. **Handed over.** `selected.into_selected_output()` → `stage_register_allocation` (`NC/pipeline.rs:139-143`)
   **[opened]**; out `RetainedAllocation` (`S05/output.rs`) **[record]**.
2. **Trusted / re-checked.** Nothing published without a successful `probe_all` (`S05/assignment.rs:2002-2010`,
   traced) **[record]**.
3. **Check and when.** Headers promising an independent replay that the readers did not find:
   `S05/home_assignment.rs:1-6` (receipt built straight from the plan, `:2634-2639`), `S05/fixed_view_copy_recovery.rs:1-6`,
   `S05/preservation.rs:528-573` (replay re-runs the same `DirectTraversal`, `:487`),
   `S05/runtime_spill_and_split.rs:492-502` (bare `sealed!`) **[record]**; grep for
   `fn (validate_register_homes|replay_register_homes|validate_homes)` = 0 and `ReplayMismatch` never constructed
   (digest re-ran) **[record]**.
4. **Independence.** Receipts are hashes; `RH/lib.rs:31-37`: a wire register-home plan "is a candidate record, not
   proof" (stated) **[record]**.
5. **Identity.** Dense ids; bitmask paths need ids < 128 (`S05/home_assignment.rs:363, 1931-1938`) **[record]**.
6. **Lost.** `Err(_) => {}` after rematerialisation (`S05/assignment.rs:1886, 1940`); unchecked `u32` subtraction
   (`:970, 1320`) **[record]**.

### Boundary 05 → 06

1. **Handed over.** `allocation.facts()`, `current.register_environment()`, selections, budget →
   `stage_optimized_post_allocation_machine_plan` (`NC/pipeline.rs:144-154`; `S06/lib.rs:132`) **[opened]**.
2. **Trusted / re-checked.** Sorted home index, first match wins (`S06/plan.rs:320-340`), unlike stage 05's which
   rejects duplicates (`S05/preservation.rs:39-78`) **[record]**; one liveness table reused across candidates,
   guarded by `debug_assert_eq!` (`S06/optimization.rs:335-344`, digest-checked) **[record]**.
3. **Check and when.** Address field `_ => None` checked later in stage 07 (`S06/plan.rs:578-583`, traced) **[record]**.
4. **Independence.** Not established.
5. **Identity.** `PostAllocationMachineIdentity`; PAM wire decode is re-hash only, "reauthenticated physical data is
   still only a proposal" (`PAM/codec/tests.rs:846-855`) **[record]**.
6. **Lost.** None recorded.

### Boundary 06 → 07

1. **Handed over.** `selected.current_program()`, `machine.machine().plan()`, `registers`, frame plan →
   `stage_optimized_layout_independent_selected_form_encoding` (`NC/pipeline.rs:164-173`; `S07/lib.rs:100`)
   **[opened]**.
2. **Trusted / re-checked.** Unlisted kinds route to `Ordinary` (`S07/row_encoding.rs:263`, stated) **[record]**.
3. **Check and when.** `validate_optimized_layout_independent_selected_form_encoding` (`S07/lib.rs:112`) is called
   from `ME/function_realization.rs:444` inside `validate_fixed_frame_function_relative_realization` (`:433`)
   **[opened]**. That chain is reached from `ME/fragment_emission.rs:486` → `:72-75` → `ME/frame_application.rs:86-89`
   → `ME/text_placement.rs:65-68` → `object-file/src/fragment_container.rs:53-56` → image-emission
   `function_fragments.rs:24430` / `:4108` / `object.rs:908` → `image.rs:224, 1894` →
   `validate_direct_executable_image` → `NativeArtifact::validate` (`native-artifact/src/native_artifact.rs:521-536`)
   **[opened]**. `NativeArtifact::validate` is called from `from_replayed_parts` (`:496-506`) and
   `inspect_native_code_layouts` (`:616-621`). The default constructor `from_emitted_parts` (`:471-478`, used at
   `:201`) seals without `validate` **[opened]**; grep finds no caller of `NativeArtifact::from_replayed_parts`
   **[opened grep]**; `inspect_native_code_layouts` is called by
   `omega/interpreter/terminal-runtime/src/execution/native_code_layouts.rs:35` **[opened grep]**. So I found the
   stage-07 validator on no default-compile path; it runs in tests and under that interpreter inspection.
4. **Independence.** Header says the replay "never calls the producer's route table or encoders" (`S07/lib.rs:45-47`)
   **[opened]**; for dynamic calls the validator calls `encode_*_selected_dynamic_call_template`
   (`S07/validation.rs:629-651`), the same functions the producer calls (`S07/row_encoding.rs:661, 670`)
   **[opened]**. Other rows are checked through ISA decoders per the header (not opened).
5. **Identity.** `SelectedFormEncodingIdentity`, row positions; deferred branch rows' `code`/`footprint` not
   examined (`S07/validation.rs:315-327`, open) **[record]**.
6. **Lost.** None recorded as silent drops.

### Boundary 07 → 08

1. **Handed over.** `encoding.program()` with selected and machine plans → `stage_optimized_resolved_selected_form_layout`
   (`NC/pipeline.rs:174-183`; `S08/lib.rs:84`) **[opened]**.
2. **Trusted / re-checked.** Pre-layout placeholders are not re-checked by the validator: "a forged pre-layout store
   with bad placeholders and a candidate copying it would pass this validator" (`S08/validation.rs:352-406`, traced)
   **[record]**.
3. **Check and when.** `validate_optimized_resolved_selected_form_layout` (`S08/lib.rs:94`) runs on the default
   path: `assemble_fixed_frame_realization` (`ME/function_realization.rs:373`, called `NC/pipeline.rs:196`) →
   `stage_whole_function_exit_contract_for_layout` (`ME/function_realization.rs:388`) → `validate_layout`
   (`ME/exit_contract.rs:153, 204-222`) **[opened]**.
4. **Independence.** Documented as an independent re-derivation (`S08/validation.rs:2`); block order and branch
   resolution written twice (`S08/ordinary.rs:85, 835-1023` vs `S08/validation.rs:256, 428-618`); signed-delta helper
   three times **[record]**.
5. **Identity.** `ResolvedSelectedFormLayoutIdentity`; layout identity omits `normalized_foreign_call_fixup`
   (`MC/layout.rs:326-332`, open) **[record]**.
6. **Lost.** Three AArch64 dispositions sized but with no `resolve_row` arm (`S08/ordinary.rs:349-366, 528`,
   digest-checked) **[record]**.

### Boundary 08 → 09

1. **Handed over.** `layout.shared_program()` → `execute_resolved_layout_optimization` (`NC/pipeline.rs:184-195`;
   `S09/lib.rs:111`) **[opened]**.
2. **Trusted / re-checked.** Validation "without re-running the fixed-point search" (`S09/lib.rs:6`) **[record]**.
3. **Check and when.** `validate_resolved_layout_optimization` (`S09/lib.rs:139`) on the default path via
   `ME/exit_contract.rs:221` **[opened]**.
4. **Independence.** It checks root identities and budget only (`S09/x86_branch_relaxation.rs:217-234`,
   digest-checked); "an artifact carrying a layout that is not the relaxation of the baseline but has the right root
   identities and an in-budget usage" passes **[record]**.
5. **Identity.** Root identities + usage.
6. **Lost.** None recorded.

---

## Answers

**(a) Taken on faith with no check anywhere in default compilation.**
- Psi 08 → Omega 00 obligations: every operation obligation becomes an accepted fact with a zero proof fingerprint
  and no evidence (`S00/optimization.rs:171-178, 265-272`) **[opened]**; downstream code requires only the fact's
  presence (`S01/rules.rs:129-161`, `S03/legalization.rs:234-244`, `CAND:2835-2856`) **[opened]**.
- Psi 08 → Omega 00 structure on the in-process route: `from_parts` skips the foundation validator
  (`codec/semantic_module.rs:4071-4075` vs `:4108-4113`) **[opened]**; whether a record read intervenes in a default
  build was not traced.
- 02 → 03: the target-plan audit runs only from `corpus_runner.rs:1060, 1070` **[opened grep]**.
- 06 → 07: the stage-07 validator is on no default path I found (`native_artifact.rs:471-478` vs `:496-506`)
  **[opened]**.
- 04 → 05 and 03 → 04: receipts that hash and count; promised replays not found **[record]**.

**(b) Truly independent.** None of the checks I opened is a full separate derivation with no shared code.
Closest: `decode_module`'s foundation validator (a separate hand-written validator, but its canonicality step calls
the producer's `encode_raw`, `codec/semantic_module.rs:4115`) **[opened]**; stage 08's validator (documented
re-derivation, record) with the pre-layout gap above; the stage-03 audit (separate rebuild, shares duplicated
tables, record). Stage 07's dynamic-call check calls the producer's encoder **[opened]**; stage 09's checks roots and
budget only **[record]**.

**(c) Shared-builder defects that pass producer and validator.**
- Accepted-fact table: produced by `source_checked_obligation_facts`, re-checked by `attach` (`OU/proof.rs:519-542`),
  UV (`UV:9697-9704`) and `CAND:2835-2856` against itself only **[opened]**.
- `OU/construction.rs` def/use extractors called by UV (`UV:2649-2675`) **[record]**; `operation_obligation`
  wildcard (`OU/construction.rs:488`) drops a fact on both sides **[opened]**.
- `encode_raw` for canonicality (`codec/semantic_module.rs:4115`) **[opened]**.
- Stage-07 dynamic-call templates (`S07/validation.rs:629-651` vs `S07/row_encoding.rs:661, 670`) **[opened]**.
- `S05/preservation.rs` replay re-runs `DirectTraversal` (`:487`) **[record]**.

## Claims opened by me (more than twelve)
`S00/optimization.rs:79-279`; `OU/proof.rs:109-260, 505-542`; `OU/construction.rs:345-490`;
`S01/proof_check_elision.rs:1-185, 555-670`; `S01/rules.rs:122-161`; `S01/value_ranges.rs:150-194` + grep of
`proof_questions`; `S01/sccp.rs:330-364`; `S01/lib.rs:130-197`; `S01/pass_manager.rs:1290-1334`;
`CAND:2830-2858`; `S03/legalization.rs:215-255`; `S03/legalization_source.rs:655-727`;
`S03/selection_scalar_graph.rs:3815-3939, 4150-4303, 4398-4524`; `S03/selection_views_and_bytes.rs:1200-1351,
1815-1964`; `S04/literal_fold_rewrites.rs:3895-3927`; `X86:3112-3161, 13940-13979`; `A64:2552-2612`;
`codec/canonical_artifact.rs:30-189, 365-389`; `codec/semantic_module.rs:4065-4120`; `P08/lib.rs:95-140`;
`custody.rs:55-94`; `NC/pipeline.rs:40-208`; `ME/function_realization.rs:373-479`; `ME/exit_contract.rs:1-12,
141-222`; `ME/fragment_emission.rs:59-83, 470-499`; `native_artifact.rs:471-545, 612-622`;
`S07/lib.rs:40-50`, `S07/validation.rs:620-655`, `S07/row_encoding.rs:648-675`; `UV` grep sites `1167-1171, 9697-9704`.

## Not established
- Whether a default `omega build` routes the Psi 08 artifact through `from_bytes` (record read) before Omega 00.
- Whether `IntegerArithmetic::evaluate` (SCCP) refuses overflow/zero divisor; encoder bodies of exact shifts.
- What the interpreter checks (only the design doc's statement).
- What `ValidatedOptimizedTargetOperations` checks at 02's output; stage 06 validator independence.
- `pipeline.md:91-93` says the default path does not run stage-output validators; the code runs the stage-08 and
  stage-09 validators on the default path (`ME/exit_contract.rs:153`) and UV/candidate validation in stage 01.
  Recorded as a fact, not reconciled.
