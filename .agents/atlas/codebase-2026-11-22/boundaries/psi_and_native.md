Built at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86` from `crate-map/07-psi-semantics.md` to `crate-map/13-native-targets-driver.md`, `digests/07-psi-semantics.md` to `digests/13-native-targets-driver.md`, and the records those digests cite (`unit_006` to `unit_050` with their `_gap_` records); no record file was opened directly, so every "record" citation below goes through a digest or crate map. Code was opened where a claim says "code".

# Boundary note: published Psi through native stage 15

Path prefixes: `PN/` = `source-rust/subsystems/compiler/psi-to-native/`; `P/` = `source-rust/subsystems/compiler/psi/`; `SP/` = `source-rust/subsystems/compiler/source-to-psi/`; `CODEC/` = `P/artifact/codec/src/`; `W/` = `P/artifact/wire/src/`; `AUTH/` = `P/artifact/authorization/`; `DRV/` = `PN/driver/`; `R/` = `PN/representations/`; `VOI/` = `R/abstract-operations/verified-optimization-input/src/`; `LO/` = `R/target-operations/legalized-operations/src/`; `EA/` = `R/machine-code/encoding-admission/src/`; `PAM/` = `R/post-allocation-machine/src/`; `TBP/` = `R/target-bound-program/`; `PO/` = `PN/verification/published-objects/src/`; `SV` = `PN/verification/stage_validators.rs`. Stage directories under `PN/pipeline/`: `S00/` = `00_psi-to-admitted-native-inputs/`; `S01/` = `01_admitted-native-inputs-to-target-bound-program/src/`; `S02/` = `02_target-bound-program-to-abstract-operations/`; `S03/` = `03_abstract-operations-to-abstract-operations/src/abstract_optimization/`; `S04/` = `04_abstract-operations-to-target-operations/src/`; `S05/` = `05_target-operations-to-selected-instructions/src/`; `S06/` = `06_selected-instructions-to-selected-instructions/src/selected_rewrites/`; `S07/` = `07_selected-instructions-to-register-homes/src/allocation/`; `S08/` = `08_register-homes-to-post-allocation-machine/src/machine/`; `S09/` = `09_post-allocation-machine-to-framed-machine/src/`; `EP/` = `09_post-allocation-machine-to-framed-machine/entry-planning/src/`; `S10/` = `10_framed-machine-to-bound-framed-machine/src/`; `S11/` = `11_bound-framed-machine-to-selected-form-encoding/src/`; `S12/` = `12_selected-form-encoding-to-resolved-layout/src/`; `S13/` = `13_resolved-layout-to-placed-functions/src/`; `S14/` = `14_placed-functions-to-objects/src/`; `EMI/` = `15_objects-to-image/image-emission/src/` with `FF` = `EMI/function_fragments.rs`; `NA/` = `15_objects-to-image/native-artifact/src/`; `IMG/` = `15_objects-to-image/image/src/`.

Words used below. **Psi** is the portable compiled program; inside the compiler its record is `TerminalModule`. A **handoff** is one stage's output becoming the next stage's input. A check is **independent** when code other than the producer's own derivation decides it (a separate validator, or a different derivation); it is **the same code re-run** when the check calls the producer's own helpers again; a **consistency check** only compares identities or hashes the value already carries. An **audit** is a check that runs only when a compile asked for stage validators (`--every-validator` or a validator name), because only then does the driver keep the original stage inputs. The driver calls the body stages 03 to 14 in order (`DRV/compile_body.rs:20-276`, code); the stages do not call each other except through their output types (record: crate map 09 §1).

The readers' words "suspected", "inferred" and "not traced" keep their meaning. "Inferred" with no record name is my own inference from the code cited beside it.

## Handoff 1: source stage 07 (or saved bytes) to native stage 00

**What is handed over.** The native compiler receives an `AuthorizedTerminalArtifact` (fields `artifact: Arc<CanonicalTerminalArtifact>` and `producer_identity: Arc<str>`, `AUTH/authorization.rs:50-53`, code) plus an `IssuedProviderDemand` (`S00/provider-demand/demand.rs:44-47`, code). The artifact holds a `ClosedTerminalModule` (the module, its canonical `PSITERM\0` bytes, the closed type catalog and the `TerminalPsiIdentity`, `CODEC/closed_module.rs:13-19`, code) and the manifest, optimization record, optional debug map and optional publication section (`CODEC/canonical_artifact.rs:122-129`, code). On the source route, `prepare` in `SP/representations/checked-program/checked_program.rs:233-250` seals the checked module with `ClosedTerminalModule::new` and `:203-205` wraps it with `CanonicalTerminalArtifact::from_closed_parts` (code). The compiler entry then records producer trust for itself with `HostProducerTrust::accept("current compiler invocation")` (`source-rust/subsystems/compiler/compiler.rs:103-108`, code). The demand is built from the same artifact the authorization holds: `source_demand` passes `authorization.shared_artifact()` to `IssuedProviderDemand::from_publication` (`DRV/inputs/published_application.rs:106-126`, code), and the saved route passes it to `from_saved_program`, which attaches the host installation to that same `Arc` (`DRV/inputs/published_application.rs:132-137`; `S00/provider-demand/demand.rs:139-141`, code).

**What stage 00 trusts.** That the artifact holds a program the lowered-form checker accepted. The codec says closure does not show this: "Closure does not show that a checker accepted the program or that its producer can be trusted" (`CODEC/closed_module.rs:1-3`, code). The trust decision is a host capability, compared by `Arc` pointer, and says nothing about the bytes (`AUTH/authorization.rs:12-39`, code).

**What is checked, where, and by whom.**
- Saved bytes: `CanonicalTerminalArtifact::from_bytes` checks magic, envelope marker, section lengths and trailing bytes, decodes each section and re-encodes and compares (`CODEC/canonical_artifact.rs:241-317`, code). The semantic section goes through `decode_closed_parts`: format marker 290, body decode, trailing bytes, scalar qualifications, canonical order, `validate_storage_foundation`, `validate_structural_foundation`, `terminal_semantics::structural_qualifications::validate`, identity-formation shape, dynamic descriptor results, then re-encode and byte compare (`W/semantic_module.rs:4264-4303`, code). The foundation validators live in a separate file of the wire crate (`foundation_validation.rs`, about 13,800 lines) written as decode-time checks, not as the encoder (record: digest 08 §1). The structural-qualification step is the same function the stage 06 checker calls (`formation.rs:87`), so it repeats the checker's judgment rather than checking it independently (record: digest 08 §7 citing unit_049; the comment at `W/semantic_module.rs:4287-4288` says the same, code). Kernel replay of identity formations is not run: the comment says decoding "does not re-decide a carried judgment" (`W/semantic_module.rs:4291-4293`, code), and `verify_identity_formations` and the other kernel replay verifiers have no caller outside the wire crate (record: digest 08 §7, checked there). Evidence seals in the publication section are checked by `EvidenceSealValidator::validate_all` (`CODEC/canonical_artifact.rs:292-298`, code); seals "grant no truth" (record: digest 08 §8). The certificate reader accepts fewer proof-row tags (0-27) than the writer emits (0-32) (record: digest 08 §10 item 1).
- In-memory source route: `ClosedTerminalModule::new` calls `encode_closed_execution_module`, which runs only the scalar-qualification check and the canonical-order check before encoding (`W/semantic_module.rs:4222-4230`, code). The decode-time foundation validators do not run on this route (inferred from the two code paths above; not traced whether a later step decodes the bytes again). The encoder has some refusals of its own (invocation scope, some structural shapes) (record: digest 08 §9 item 7).
- Stage 00 itself: `admit` takes the artifact the demand names and calls `require_artifact`, which compares manifest identities (`S00/admit.rs:13-26`; `AUTH/authorization.rs:77-90`, code). On both driver routes the demand's artifact is the authorization's own `Arc` (see above), so this comparison compares an artifact with itself there (inferred; it can refuse only for a caller that builds the demand from another artifact, such as the stage 00 test at `S00/admit.rs:46` named in crate map 09 §2.1). `TargetProviderInstallation::attach` refuses a different manifest identity or profile (record: digest 09 §1, `installation.rs:99`, `:102`).
- Independence: the saved-bytes route has a separate decode-time validator (independent of the encoder, not of the checker for the structural-qualification step). The in-memory route has only the encoder's own canonical-order and scalar-qualification checks.

**Other ways information moves.** Source declaration custody travels inside the artifact's publication section and is read back through `calling_declaration_owner` (`CODEC/canonical_artifact.rs:167-189`, code), not from source text. The saved route closes providers again from the module with `close_providers` and builds diagnostic labels from identity text (`S00/provider-demand/demand.rs:150-164`, code).

## Handoff 2: stage 00 to stage 01

**What is handed over.** `AdmittedProviderDemand { demand, authorization }` (`S00/admit.rs:8-11`, code), produced by `admit` (`S00/admit.rs:13-26`, code). The target demand carries `profile`, `logical` (closed logical providers), `has_bound_applications`, `bodies`, `labels`, `origins` and `review` (`S00/provider-demand/demand.rs:17-31`, code). Stage 01's `bind_published_application` consumes it (`S01/published_application.rs:311-342`, code) and returns `(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy)`; `RetainedTerminalArtifact` holds the authorization and a `TerminalNativeRealizationProposal` (`VOI/native_proposal.rs:624-628`, code).

**What stage 01 trusts.** The logical provider closure and bodies as stage 00 or the source publisher computed them; `select_published_providers` only splits the parts and plans from them (`S01/provider_selection.rs:43-57`, `:88-110`, code).

**What is checked.** `bind_published_application` refuses a demand that is not target-scoped (`S01/published_application.rs:321-325`, code). `RetainedTerminalArtifact::project` validates the program-entry settlement against the artifact (`VOI/native_proposal.rs:205-220`, code), and `NativeScopeSource::admit` compares the entry's artifact identity with the authorization's and the physical adapter's selection digest (`TBP/native_execution_scope.rs:66-117`, code). These are consistency checks in the producer's own crates. When stage 01 folds the target into a new artifact, it issues a new producer-trust decision itself, `HostProducerTrust::accept("in-process target folding")`, relying on its own `effective_artifact` check (`S01/published_application.rs:290-297`, code). A requirement with no identity text gets no label; the label only words the `MissingSupplies` message (record: digest 09 §5, §10 item 11). A Build binding naming an unreached or misspelled requirement is skipped (record: digest 09 §5, `S01/provider_execution_inputs.rs:149-154`, open).

**Other ways information moves.** Stage 01 runs in two separate calls. `bind_published_application` returns the retained artifact to the driver, and the driver later calls `provider_realization::native_execution::prepare_program` with that artifact's proposal (`DRV/inputs/mod.rs:23-37`, code). Stage 01 also runs authored calling policies in the Terminal interpreter and decodes their results by the last `::` segment of case names (record: digest 09 §1, §2, `S01/calling_policy_plans/build_time_decoding.rs:498`).

## Handoff 3: stage 01 to stage 02

**What is handed over.** `PreparedNativeExecution { checked: ScopedProviderSelection, providers: ScopedProviderBindings }` (`S01/native_execution.rs:14-17`, code), from `prepare_program`, which binds the scope source, runs `derive_native_scope` and `provider_bindings::bind` (`S01/native_execution.rs:93-100`, code). The driver also passes the proposal, private entry RAM recipes it admitted with `psi_entry_memory::admit_private_entry_ram`, and scalar applications it completed (`DRV/inputs/mod.rs:39-68`; `DRV/mod.rs:95-106`, code).

**What stage 02 trusts.** That the reachable machine set is complete: stage 02 lowers only `scope.reach().machines()` (`S02/selected_input.rs:314-316`, code). `retain_boundary` returns `Ok(())` for a reached boundary with no candidate or no selected adapter row (record: digest 09 §5, §10 item 5; partly traced, partly open).

**What is checked.** `from_scoped_providers` compares the coverage demands with the scope's demands and runs `EvidenceSealValidator::validate_all` on every published seal (`S01/native_execution.rs:20-48`, code); that is the same validator the codec already ran in `from_bytes` (`CODEC/canonical_artifact.rs:294-298`, code), so it is the same code re-run (also noted in record: digest 09 §3, unit_006). `NativeScopePreparation::prepare` resolves the entry against the target context, calls `scope.require_source_input`, and refuses a changed entry call or callback list (`S02/selected_input.rs:230-293`, code). These are consistency checks. Private entry RAM is checked by the separate `psi-entry-memory` crate (`DRV/inputs/mod.rs:51-60`, code; crate map 08 §2.21). The scalar closure must name the same artifact, target, layouts and custody, but only machines with an authored declaration are checked against it (record: digest 09 §2, §5, `S02/selected_input.rs:985-1003`, open).

**Other ways information moves.** Private entry RAM and scalar applications reach stage 02 from the driver, beside stage 01's output (`DRV/mod.rs:95-106`, code).

## Handoff 4: stage 02 to stage 03

**What is handed over.** `VerifiedNativeArtifactInput { optimization_input, access_view_establishments, private_entry_ram }` (`S02/selected_input.rs:185-190`, code), built by `NativeScopePreparation::lower`: `lower_selected_module`, `VerifiedPsiOptimizationInput::lowered`, `retain_native_selections`, `attach_evidence` (`S02/selected_input.rs:295-412`, code). `VerifiedPsiOptimizationInput` holds the `AbstractOperationPlan`, the authorized artifact as `context`, scope, scalar applications, entry call, callbacks, interrupts, assumed bindings and evidence (`VOI/lib.rs:63-79`, code). The driver passes it through `admit_prepared_native_realization` (`DRV/mod.rs:162-169`, code), then `into_optimization_input()` to stage 03 (`DRV/compile_body.rs:30-35`, code).

**What stage 03 trusts.** That the abstract plan lowers the Psi bodies faithfully. Stage 02 arithmetic checks only the result type; integer relations check no operand types (record: digest 09 §10 item 12, matched).

**What is checked.**
- `VerifiedPsiOptimizationInput::lowered` refuses a plan whose `psi` differs from the authorized artifact and runs `validate_cleanup_source`, which reads the source Psi module and requires every `InvokeOwned` cleanup action, keyed by machine, edge and ordinal, to match the lowered plan's (`VOI/lib.rs:93-168`, code). This is a separate crate reading the Psi module directly, so it is independent of the lowering code, but it covers only cleanup actions.
- Stage 03 rebuilds an optimization unit from the plan (`S03/verified_unit.rs:84-103`, code). When at least one pass is selected, `VerifiedPsiOptimizationSession::new` runs `optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines` (`S03/pass_manager.rs:1335-1364`, code), a separate checker crate (crate map 12 §2.14). With no pass selected, `from_transformed` runs only `derive_context`: runtime-evidence context and ranked-cycle admission (`S03/mod.rs:158-166`; `S03/pass_manager.rs:1366-1376`; `S03/validation.rs:25-49`, code). Whether the unit checker compares the plan with the Psi module, rather than the unit with the plan: not traced.
- An obligation with no evidence seal is accepted with fingerprint `[0; 32]`; kernel acceptance is "taken on faith" (record: digest 09 §5, §10 item 17, stated).

**Other ways information moves.** The authorized Psi module rides inside `VerifiedPsiOptimizationInput` and stage 03 reads it (`S03/verified_unit.rs:90-94`, code). `OMEGA_PROOF_MEASUREMENTS` prints a census to stderr without changing verdicts (`S03/verified_unit.rs:95-100`, code).

## Handoff 5: stage 03 to stage 04

**What is handed over.** `ValidatedOptimizedAbstractPlan { plan, replay_input, replay_unit, evidence, validation, pre_physical_manifest }` (`S03/publication.rs:232-240`, code), from `publish_optimization_run` (`S03/publication.rs:196-230`, code). `into_lowering_input` builds `OptimizedAbstractProgram { plan, unit, selections, budget_per_pass, identity_bundle, projection, projection_psi, final_unit, pre_physical_manifest }` and wraps it as `OptimizedAbstractBody` (`S03/publication.rs:293-304`, code). Stage 04's `lower_native_body` splits it into the optimized program and `AdmittedLoweringFacts` (`S04/target_lowering/mod.rs:343-348`, code).

**What stage 04 trusts.** The optimized plan and the unit's accepted obligation facts and ownership-frontier facts, which it passes to `prepare_execution_plan` as `PreparationEvidence` (`S04/target_lowering/mod.rs:448-455`, code).

**What is checked.** `require_lowering_context` refuses an unconsumed target-operations phase, lowering facts that belong to another plan or target, and a mismatched runtime-evidence context (`S04/target_lowering/mod.rs:465-480`, code). With an empty selection, stage 03 refuses a published plan that differs from its input plan (`S03/mod.rs:141-145`, code). Each rewrite candidate is checked by `validate_psi_rewrite_candidate` in the separate unit-checker crate (record: crate map 12 §3 step 3). For loop-invariant motion, propose and validate run the same planner and "agree by construction" (record: digest 09 §9, unit_008); the countdown analyses are a replay that differs from the proposer (record: digest 09 §3).

**Other ways information moves.** Provider settlements and the provider installation reach stage 04 from stage 02's admission through the driver, in `OptimizedTargetLoweringRequest`, not through stage 03's output (`DRV/image/plan_callables.rs:83-89`, `:165-171`; `S04/target_lowering/mod.rs:337-341`, code). Stage 04 reads the Psi module and the closed module carried in `AdmittedLoweringFacts` to close types (`S04/target_lowering/mod.rs:420-431`, code).

## Handoff 6: stage 04 to stage 05

**What is handed over.** `NativeBodyPlan { selection, facts, private_references }` (`S04/target_lowering/mod.rs:404-408`, code). `selection` is a `PreparedInstructionSelection` holding `Arc<PreparedExecutionPlan>` and the optimization, projection and manifest identities (`S04/target_lowering/mod.rs:371-378`, code). `PreparedExecutionPlan { plan, receipt, execution }` (`LO/lib.rs:108-115`, code) is sealed by `PreparedExecutionPlan::seal` (`LO/lib.rs:118-148`, code) at the end of `prepare_execution_plan` (`S04/preparation/legalization.rs:651-701`, code). `facts` is a `BodyPublication` of semantic emission facts from `emission_facts::project` (`S04/target_lowering/mod.rs:379-403`, code).

**What stage 05 trusts.** That the sealed plan's rows are a correct legalization. Selection "reads the prepared plan and the target's register environment" and nothing earlier (`S05/instruction_selection/selection.rs:1994-1996`, code).

**What is checked.** `seal` refuses a plan with no execution facts or with execution roots that differ from the plan's (`LO/lib.rs:122-134`, code). Stage 05 refuses a plan whose Psi or entry differ from the publication facts (`S05/select_instructions.rs:20-27`, code), binds installations against the plan (`:28-35`, code), and `BoundExecutionPlan::bind` compares Psi, target and entry (record: digest 10 §2, `S05/input.rs:39-53`). The independent re-derivation is `verify_execution_plan` (`S04/preparation/legalization.rs:793-888`, code): its doc says "Ordinary compilation never calls this; it runs on an explicit validator request and in tests" (`:802-803`, code). It lives in the producer's crate; it derives expected functions from the abstract plan and unit and compares them with rosters built by the producer's own `derive_target_function_rosters` (`S04/preparation/legalization.rs:776-789`, code), and it is the only non-test caller of `is_intact` (grep: `legalization.rs:873` and a test, code). It is reached through `verify_retained_target_operations` (`PO/lib.rs:344-386`, code) for the `TargetOperationCorrespondence` validator (`SV:61-64`, code). The lowering and the audit encode the same rules twice, so a lowering change not mirrored shows only under audit (record: digest 10 §3, unit_010). Most audit refusals are a cause-less `custody()` (record: digest 10 §8).

**Other ways information moves.** The semantic emission facts made here travel beside the program through stages 05 to 14 and are read again in stage 15 (`S05/select_instructions.rs:46-52`; `EP/imports.rs:98-115`; `FF:2785-2937`, code). With `retain_original`, the original target operations are kept for the audit (`S04/target_lowering/mod.rs:380-396`, code).

## Handoff 7: stage 05 to stage 06

**What is handed over.** `StagedOptimizedSelectedInstructions { body, register_environment, selections, budget_per_pass, selected, custody }` (`S05/select_instructions.rs:45-58`, code). `selected` is `ValidatedSelectedInstructions { plan, receipt }` (`S05/instruction_selection/selection.rs:1484-1488`, code); `select_bound_instructions` builds the plan and a receipt and runs no separate validation pass (`S05/instruction_selection/selection.rs:1997-2008`, code).

**What stage 06 trusts.** The selected program as built. The name "Validated" here means "built and given a receipt" (inferred from `selection.rs:1997-2008`).

**What is checked.** Only the type gate: `optimize_selected_instructions` takes the stage 05 type, not a bare input (`S06/selected_optimization.rs:184-193`, code). Stage 06 then builds liveness and live ranges from the program (`S06/selected_optimization.rs:206-209`, code); whether those analyses refuse a malformed program: not traced. Stage 05 refuses functions with non-dense register ids (record: digest 10 §10 item 7, `SG:7813`).

**Other ways information moves.** `NativeBodyFacts` (stage 04 facts, private references, planned installation resources) is carried in an `Arc` beside the program (`S05/select_instructions.rs:46-52`; `S06/selected_optimization.rs:194`, code).

## Handoff 8: stage 06 to stage 07

**What is handed over.** `SelectedInstructionOptimizationOutput { body, allocation: SelectedAllocationInput, run: Option<CompletedRun> }` (`S06/selected_optimization.rs:31-36`, code), from `optimize_selected_instructions` (`:191-258`, code).

**What stage 07 trusts.** The rewritten program and the analyses stage 06 validated for it. "The default compile trusts the run it just produced and does not replay it" (`S06/selected_optimization.rs:9-14`, code).

**What is checked.** Type gate only (`S07/register_allocation.rs:35-43`, code). The completed run is kept only when the body retains stage inputs (`S07/register_allocation.rs:51-56`, code). `SelectedOptimizationReplay::validate` re-executes the producer and "establishes determinism and custody, not independent correctness" (`S06/selected_optimization.rs:96-135`, code); it runs for the `SelectedInstructionRewriteReplay` validator (`SV:68-70`; `PO/lib.rs:334-342`, code). This is the same code re-run.

**Other ways information moves.** The stage 06 rewrite history reaches allocation only as the program's `SelectedOptimizationRecord`, kept as evidence (`S07/register_allocation.rs:12-17`, code).

## Handoff 9: stage 07 to stage 08

**What is handed over.** `RetainedAllocation { body, current: Arc<CurrentAllocation>, requirements, recovery }` (`S07/output.rs:192-198`, code), admitted by `RetainedAllocation::admit` after the procedure ran (`S07/output.rs:236-264`, code). Stage 08 reads it as `AllocatedProgramFacts` (`S08/mod.rs:134-145`, code).

**What stage 08 trusts.** That the register homes are correct. "Nothing replays the resulting `RegisterHomePlan` ... the default compile trusts this solver" and its receipt "is not evidence that a second check accepted the homes" (`S07/home_assignment.rs:1-12`, code).

**What is checked.** `validate_roots` compares the identity roots of the selected plan, effects, ranges, legality, homes, manifest, preservation, register environment, physical model and constraint catalog (`S08/plan.rs:255-331`, code); it does not check interference or liveness of the homes. `build_functions` accepts a homes row at another index when the machine id matches (`S08/plan.rs:373-377`, code; open in record: digest 10 §5). `validate_publication` runs "only on an explicit replay request" (`S07/output.rs:227-232`, code), from `PO/lib.rs:448` (code). Nothing replays stage 07's homes or fixed-view copies (record: digest 10 §8).

**Other ways information moves.** `NativeBodyFacts` passes through unchanged (`S08/mod.rs:177`, code).

## Handoff 10: stage 08 to stage 09

**What is handed over.** `StagedOptimizedPostAllocationMachinePlan { body, current: PostAllocationMachineProgram, effects, machine: ValidatedPostAllocationMachinePlan, optimization, custody }` (`S08/mod.rs:204-209`, `:176-183`, code). `current.identity` is recomputed after construction (`S08/mod.rs:156-173`, code).

**What stage 09 trusts.** The chosen machine alternatives and the result of the one post-allocation rule. That rule reuses a liveness table built for the first candidate and guards later candidates only with `debug_assert_eq!` (record: digest 10 §10 item 14, matched).

**What is checked.** Type gate (`S09/lib.rs:47-62`, code). Stage 09 checks its own frame output, not its input: `stage_non_authoritative_callee_save_storage` and `stage_target_frame_layout` derive a plan and pass it to validators in the separate `frame-admission` crate (`S09/frame_layout.rs:13-27`, `:117-137`, code). The machine program's identity integrity is checked one stage later by stage 11 (handoff 12).

**Other ways information moves.** When stage inputs are retained, stage 09 clones the stage 08 plan and the driver keeps it, with the allocation, for the fixed-frame audit (`S09/lib.rs:63-66`; `DRV/compile_body.rs:142-158`, code).

## Handoff 11: stage 09 to stage 10, through entry-planning

**What is handed over.** `FramedNativeBody { frame: PhysicalFrameProgram, facts }` (`S09/lib.rs:31-34`, code), where `PhysicalFrameProgram` holds `instructions`, `storage`, `frame` and `protocols` (`S09/lib.rs:105-110`, code). `EP/imports.rs:42-116` `plan_function_body` (code) turns it into `PreparedFunctionBody { program: PlannedFunctionBody { frame, private_references, imports, windows_internet }, facts: BodyPublication<EmissionFacts { semantic, physical }>, installations }` (`EP/imports.rs:13-17`, `:105-115`, code). The physical facts are captured by `frame_planning::physical_facts::for_emission` (`EP/imports.rs:100-104`, code) "before encoding drops the instruction columns" (record: crate map 10 §2.7).

**What stage 10 trusts.** The frame and the captured physical facts.

**What is checked.** `plan_function_body` refuses a private-function reference outside this program's namespace (`EP/imports.rs:79-86`, code), and `into_framed_body_facts` compares private-reference vectors (`EP/imports.rs:98-99`, code; record: digest 12 §1, `BF/lib.rs:37-49`). `bind_function_body` matches each installer guard with the retained handler measurement at the same placement and runs `BoundInstallationAuthority::verify` from the `installation-admission` verification crate (`S10/lib.rs:108-129`, `:142-187`, code). The physical facts are re-derived only in `verify_physical_origins`, during the `FixedFrameCorrespondence` audit, by calling the same stage 09 `capture` functions again (`PO/lib.rs:246-318`, code), so that check is the same code re-run.

**Other ways information moves.** `EmissionFacts.physical` travels to stage 15, where `prepare_object_for_image` reads the foreign-call plans from it (`FF:2931-2937`, code).

## Handoff 12: stage 10 to stage 11

**What is handed over.** `BoundFunctionBody` (`S10/lib.rs:135-140`, code), issued by `BoundFunctionBody::from_binding` with the `BINDER` capability, carrying the planned program, the bound installation authority and `BoundEncodingRequirements { subject, publication, retain_original, check_branches }` (`S10/lib.rs:155-171`, code). Only stage 10 imports `BINDER` (record: digest 12 §1, checked there).

**What stage 11 trusts.** The machine program and frame as bound.

**What is checked.** On every encode, stage 11 calls `validate_current_selected_form_encoding` (`S11/lib.rs:85`, code), which first runs `validate_current_program_integrity` on the input program (`EA/lib.rs:117-125`, code). That function, in the separate `post-allocation-machine` crate, recomputes the machine identity and compares register-environment, preservation and publication roots (`PAM/program_validation.rs:109-150`, code); its doc says it "does not claim correspondence with an earlier program" (`:109-111`, code). `compute` refuses a physical-model mismatch and walks function, block and instruction rosters by position with an id check (`S11/lib.rs:170-207`, code). This is a consistency check by separate code; it does not show the program is correct.

**Other ways information moves.** `BoundEncodingRequirements::capture_original` keeps the original frame instructions when an audit was requested (`S11/lib.rs:70-71`, code).

## Handoff 13: stage 11 to stage 12

**What is handed over.** `StagedOptimizedSelectedFormEncoding { current: EncodedMachineProgram, contracts }` (`S11/lib.rs:112-116`, code). `EncodedMachineProgram` carries bindings, exit requirements, fragment origins, layout requirements, frame demands, storage, frame and protocol identities, context, frame, protocol and the `SelectedFormEncoding` (`S11/lib.rs:91-109`, code).

**What stage 12 trusts.** The encoded bytes and row sizes.

**What is checked.** Stage 11 checks its own output before handing it over: `validation::validate` in `encoding-admission` checks each row through the ISA validators (`EA/lib.rs:117-125`, code). For x86-64 the validator decodes and requires byte equality with the canonical encoding; for AArch64 the encoder and validator share the same expected-stream builder, so they are not independent oracles (record: digest 11 §1, §3, §9, unit_017). Stage 12 `compute` refuses mismatched selected and machine roots, architecture, function count or a post-allocation optimization, and checks that rows appear once in order (`S12/lib.rs:100-156`, code). `validate_baseline_resolved_layout` runs only when an audit was requested (`S12/lib.rs:56-64`, code).

**Other ways information moves.** None found.

## Handoff 14: stage 12 to stage 13

**What is handed over.** `LayoutResolution { layout: ResolvedFunctionLayout, baseline_for_audit }` (`S12/lib.rs:39-43`, code). `settle` builds `SettledFunctionLayout { current: OptimizedMachineLayout, relaxation }` and returns `ResolvedFunctionLayout { contracts, layout }` (`S12/settled.rs:104-157`, code).

**What stage 13 trusts.** The relaxed offsets and branch widths. The exit contract "does not replay layout stages or the record it just produced" (record: digest 11 §8, `S13/exit_contract.rs:13-17`).

**What is checked.** The relaxation transition and the current layout are validated only when an audit was requested (`S12/settled.rs:130-137`, `:153-155`, code). Stage 13 refuses a layout that `into_placement` cannot split (`S13/lib.rs:51-53`, code). The driver refuses retained audit inputs that do not belong to the relaxed layout (`DRV/compile_body.rs:221-245`, code). Each stage 13 step has its own checker in the same file (record: digest 11 §1, unit_019).

**Other ways information moves.** The baseline layout is kept for the audit (`S12/lib.rs:58-62`; `DRV/compile_body.rs:221-231`, code).

## Handoff 15: stage 13 to stage 14

**What is handed over.** `FunctionPlacement { text, frame_application }` (`S13/text_placement.rs:79-82`, code), where `text` is `StagedOptimizedFixedFrameTextSection { contracts, encoded, body, text_section, native_layouts, manifest, custody }` (`S13/text_placement.rs:145-155`, code), built by `stage_optimized_fixed_frame_text_section` (`S13/text_placement.rs:54-75`, code). `native_layouts` is stored as a `Result` and reported only when the image asks (`S13/text_placement.rs:152`, `:185-191`, code).

**What stage 14 trusts.** The text section and its fixups: "During normal compilation we trust the freshly constructed text section and encoded container without re-decoding" (`S14/fragment_container.rs:6-10`, code).

**What is checked.** `publish_function_object` refuses an import target that differs from the text section's and Windows internet exports on another target, and refuses audit inputs that disagree with each other (`S14/fragment_container.rs:83-111`, code). `validate_optimized_fixed_frame_text_section` is called only from `validate_optimized_relocation_free_object_container` (`S14/fragment_container.rs:167-172`, code; grep), which image-emission calls from `validate_original` (`EMI/function_fragments/validation.rs:341-348`, code) only when a fragment replay was retained (`EMI/function_fragments/validation.rs:301-306`, `:309-320`, code). The fragment-emission replay does not compare the normalized foreign-call fixup (record: digest 11 §10 item 5, matched).

**Other ways information moves.** The body contracts (`BodyPublication`) pass through stage 13 without being read; a `compile_fail` doctest keeps stage 13 from reading them (`S13/text_placement.rs:162-170`, code).

## Handoff 16: stage 14 to stage 15 (object to object artifact)

**What is handed over.** `PublishedFunctionObject { publication, sections, replay }` (`S14/fragment_container.rs:141-147`, code), with `ObjectPublication { contracts, object, text, native_layouts, body, custody }` (`S14/fragment_container.rs:112-119`, code). The `OMGTRO` container is encoded but not decoded back on the ordinary path (`S14/fragment_container.rs:99-100`, code). `emit_fragment_object` passes it to `image_emission::prepare_object_for_image` (`NA/fragment_object_publication.rs:171-177`, code), which builds an `ObjectArtifact` (`FF:2769-2971`, code).

**What stage 15 trusts.** The sections and relocations as published, and the stage 04 and stage 09 facts carried with them: `prepare_object_for_image` derives functions, attributions, port effects and settlements from the carried emission input (`FF:2785-2964`, code).

**What is checked.** `prepare_object_for_image` refuses missing fragments and metadata as it reads them (`FF:2799-2914`, code). `publish_fragment_object` checks program-entry custody, the physical adapter on UEFI, receiver binding and runtime provisions (`NA/fragment_object_publication.rs:71-161`, code). The full object replay `validate_available` (`EMI/function_fragments/validation.rs:301-307`, code) runs from `validate_direct_executable_image` (`EMI/image.rs:1758-1770`, code), which is called from `RetainedNativeArtifact::validate` (`NA/native_artifact.rs:744-760`, code). `validate` runs from `NativeArtifact::from_replayed_parts` (`NA/native_artifact.rs:709-718`, code), whose only callers are tests under `PN/tests/physical_publication/` (grep over `source-rust`, code). The ordinary path uses `from_emitted_parts`, which does not call `validate` (`NA/native_artifact.rs:377-399`, `:680-687`, code). Whether the interpreter host's retained-image crate calls `RetainedNativeArtifact::validate` on a received artifact: not traced.

**Other ways information moves.** Stage 15 reads the stage 04 semantic facts and stage 09 physical facts that rode beside the program since those stages (`FF:2785-2937`, code). Under audit, image-emission reads stage 02's working program through `retained_working_program` (record: crate map 11 §2.12).

## Handoff 17: inside stage 15 (object artifact to final image, container bytes and native artifact)

**What is handed over.** `emit_direct_executable_image_signed` builds `FinalImage` with `image::build_final_image(FinalImageInput { target, import_program, object, relocations, text_bytes, data_bytes })` (`EMI/image.rs:1705-1728`; `IMG/builder.rs:151-170`, code), hands it to the ELF, Mach-O or PE writer by object format (`EMI/image.rs:1730-1737`, code), and returns `ExecutableImage` (`EMI/image.rs:1741-1755`, code). `ProgramImageInput::emit` then calls `NativeArtifact::from_emitted_parts` (`NA/native_artifact.rs:377-399`, code), which derives physical evidence with `PhysicalEvidencePass::Publication` and seals the artifact (`NA/native_artifact.rs:680-731`, code).

**What the writers and the artifact trust.** "Writers resolve fixups and place emitted regions. Independent replay belongs to retained-artifact receiving checks and backend tests" (`EMI/image.rs:1738-1739`, code). A relocation with an invalid symbol handle is turned into `Handle::invalid()` (`IMG/builder.rs:103-114`, code) and later refused by `apply_relocations` as an unknown symbol (record: digest 11 §5, stated).

**What is checked.** `from_emitted_parts` refuses an image whose foreign calls differ from the object's (`NA/native_artifact.rs:681-683`, code). The `Publication` evidence pass skips every byte check (record: digest 11 §5, `NA/physical/replay.rs:20-28`, stated). Shapes the derivation cannot handle become recorded gaps, not refusals (record: digest 11 §5, open). The Mach-O writer replays container mapping and loader fixups inside emission (record: digest 11 §1, `MACHO/lib.rs:878-905`). The dynamic ELF chain derives and then re-derives each stage in a `validate_*` function, but its last stage builds the output without re-checking bytes; the byte check is `#[cfg(test)]` (record: digest 11 §1, §10 item 4). For the static ELF and PE writers the records name no replay (record: digest 11 §8, which lists stage 11 and Mach-O as the only producers that replay). The receiver pass (`PhysicalEvidencePass::Receiver`) runs inside `RetainedNativeArtifact` evidence derivation (`NA/native_artifact.rs:648-668`, code), on the `from_replayed_parts` route described in handoff 16.

**Other ways information moves.** The native artifact keeps the canonical Psi artifact and reads its module again at stage 15 to derive evidence and check coverage (`NA/native_artifact.rs:653-667`, `:761-769`, code). Requirement identity text missing from the identity table reads as `""` in several native-artifact checks (record: digest 11 §5, open).

## Handoffs where nothing independent checks the value in a default compile

- Handoff 1, in-memory source route: only the encoder's own canonical-order and scalar-qualification checks run; the decode-time foundation validators run only on saved bytes, and stage 00's artifact comparison compares the artifact with itself on both driver routes.
- Handoff 2 (stage 00 to 01): identity consistency checks only; stage 01 issues its own producer trust for a folded artifact.
- Handoff 3 (stage 01 to 02): identity checks and a re-run of the same evidence-seal validator.
- Handoff 6 (stage 04 to 05): the re-derivation runs only on a validator request and shares the producer's roster helper.
- Handoff 7 (stage 05 to 06): type gate only.
- Handoff 8 (stage 06 to 07): the replay re-runs the producer, on request only.
- Handoff 9 (stage 07 to 08): register homes are not replayed; stage 08 compares identity roots only.
- Handoff 10 (stage 08 to 09): no check of the machine program at this handoff.
- Handoff 11 (stage 09 to 10): physical facts are re-captured only on request, by the same capture functions.
- Handoff 12 (stage 10 to 11): separate code checks identity integrity only, not correctness against the allocation.
- Handoffs 14 and 15 (stage 12 to 13, 13 to 14): layout, relaxation and text-section checks run only on audit.
- Handoff 16 (stage 14 to 15): the object replay is reached only through `from_replayed_parts`, which only tests call.
- Handoff 17 (inside stage 15): the publication evidence pass skips byte checks; the static ELF and PE writers have no replay named in the records.

Handoffs with some independent check in a default compile: handoff 1 on saved bytes (decode-time validator), handoff 4 (cleanup-source check in `verified-optimization-input`, and the unit checker when a pass is selected), handoff 5 (per-rewrite unit checker), handoff 13 (encoding validators; independent for x86-64, shared builder for AArch64).
