# Atlas packet unit_007: native stages 01 to 03 (area 09)

**Packet complete.** None of the 28 correctness observations in this packet (23 receipts) shows the native front end accepting a wrong program or producing a wrong result. 26 are intended behavior: each skip or default either narrows a set that a later check judges in full, changes only a diagnostic message, or refuses. One observation (Tarjan's edge skip and recursion depth) stays open, with the next check named. One (external-decision replay) is an unwired feature. No code changed in this packet.

Evidence baseline: `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. The survey read `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Since then, three merged changes touched these stages: [#5163](https://github.com/CathedralOS/Omega/pull/5163), [#5219](https://github.com/CathedralOS/Omega/pull/5219) and [#5371](https://github.com/CathedralOS/Omega/pull/5371). Only `plan_realization.rs`, `realized_supply.rs`, `provider_rows.rs`, `target_supply.rs`, `source_provider_choices.rs` and `pass_manager.rs` changed, and no claim below is affected beyond its line numbers.

Scope: [record unit_007](../../records/unit_007.md), all 34 ranges of [units/unit_007.tsv](../../units/unit_007.tsv), and the [area 09 digest](../../digests/09-native-stages-00-03.md) lines that cite `unit_007`. The only supplemental range for unit 7 (`second_pass_ranges.tsv` line 30, gap 20, `15_objects-to-image/image-elf/src/file_assembly.rs`) belongs to area 11 and its gap packet, so this packet skips it. Claims taken only from `unit_006` or `unit_008` belong to those packets.

Method: the owner read the current source at every cited site and traced each value to its consumer or validator. "Test" names an existing test that pins the behavior. Where none exists, the receipt says so. This review ran no compiler build or test; every outcome comes from reading source. There was no exploit development or vulnerability reproduction.

Paths: `S1/` = `source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/src/`, `S2/` = `.../pipeline/02_target-bound-program-to-abstract-operations/`, `S3/` = `.../pipeline/03_abstract-operations-to-abstract-operations/src/abstract_optimization/`. `D09:n` is line *n* of the area 09 digest. `R7:n` is line *n* of `records/unit_007.md`. Current line numbers are at the baseline.

## Dispositions already recorded (preserved)

These dispositions in [the Psi and native ledger](../07-13-psi-and-native.md) stand unchanged and are not repeated here:

| Claim | Ledger disposition |
| --- | --- |
| D09:188–189, `plan_provider_execution_inputs` unused; R7:62 | Same planning implementation |
| D09:191, pass identity literals; R7:308–310 | Shared pass names (#5371) |
| D09:198, missing provider labels (`unit_006`) | Refusal preserved |
| D09:202–203, `NormalizedNativeSupplies` and `UnsupportedOptimizationCombination`; R7:123 | Public API, no in-tree construction |
| D09:206, leaf-matches-boundary predicate written twice; R7:133 | Shared matching rule (#5371) |

The digest's own corrections also stand: D09:178 and D09:195 on the stage 02 callback-filter wording, and D09:194 on the opcode-based edge builder (any edge it misses makes the comparison against the Terminal graph fail).

## Receipts

### Stage 01: provider supply and entry

**1. A Build binding for an unreached requirement is skipped.** D09:98, D09:127; R7:64.
- Source: `S1/provider_execution_inputs.rs:142-153`.
- Contract: [foreign bindings](../../../../../wiki/spec/build/foreign_bindings.md) (line 156) requires a malformed binding to be refused even when no entry reaches it. The code meets that: every binding is unwrapped first (`binding.map_err(refuse)?`, :143), and a binding for another target is refused (:144-146). Only then does the loop skip a binding whose requirement this program does not reach. Bindings name their requirement through typed selectors, not text, so a misspelled requirement cannot reach this point.
- Test: `source-rust/application/tests/build_native_bindings.rs`, `a_dependency_binding_nothing_reaches_is_not_judged` and its twin `a_dependency_binding_the_program_reaches_is_judged`.
- Outcome: **intended**.

**2. Calling applications with an uninterned requirement or a non-method are dropped.** D09:99, D09:128; R7:136(a).
- Source: `S1/target_supply/plan_realization.rs:105-123`.
- Trace: `attach_calling` receives the program's whole `request.calling_applications` list (:544, :571) and attaches each application to the one plan schema that declares its requirement. For any single schema, most applications belong to other schemas, so the skip is that per-schema filter. `RealizedTargetSupply::from_checked` then checks every application in the full list for target and commitment (`S1/target_supply/realized_supply.rs:362-370`). `validate_calling_applications` (`build/providers/plans/service_schemas.rs:174-200`) refuses an attached row that is foreign or repeated.
- Outcome: **intended**.

**3. Unreached external bindings other than intrinsic or syscall `continue`; bindings for another target are filtered out.** D09:99; R7:136(b,c).
- Source: `S1/target_supply/plan_realization.rs:403-406` (a reached binding of another kind is refused) and `:287-290`.
- Trace: a foreign-target row is refused later by `realized_supply.rs:371-377`. The comment at `:300-301` states the unreached case.
- Outcome: **intended**. The earlier classes (stated, traced) are confirmed.

**4. `Foreign(Err(_)) => {}` accepts a leaf whose syscall has no Build receipt.** D09:97, D09:193, D09:179; R7:147, R7:364.
- Source: `S1/target_supply/realized_supply.rs:360`. The error is created at `plan_realization.rs:384-397`.
- Trace: the digest traced only leaves that serve a direct demand. This review traced the remaining case, a leaf for a boundary that has a selected provider row. `native_execution/provider_bindings.rs:98-116` leaves that boundary out of `direct_demands`, so the leaf never enters `direct` (:118-146) and is never nominated. `VerifiedDirectProviderSelection::check` adds plans for nominated leaves only (`with_direct_boundary_plans(direct)`, `direct_boundary_bindings` `check`, `:47-66`). So an erroring leaf is either refused (`binding.clone()?`, `provider_bindings.rs:131`) or not executed.
- Test: no test pins the refusal message "selected native syscall needs its exact evaluated Build receipt". A focused fixture would be a Linux syscall leaf with no Build receipt, once reached and once unreached.
- Outcome: **intended** (refusal preserved). The coverage gap remains.

**5. `unwrap_or_default()` gives an empty realization name.** D09:100; R7:125.
- Source: `S1/target_supply/installation_reach.rs:101-104`.
- Trace: `InstallationReachRow::realization_name` is read only in the text of the unresolved-requirement diagnostic (`verification/provider-correspondence/src/installation_reach.rs:74-78`). The requirement, plan and reach fields that decide the result are each checked for text and refuse when empty (:73-82, :86-95, :106-117).
- Outcome: **intended**. It affects message wording only, like the missing provider labels at D09:198.

**6. A missing type name becomes `""` before the entry signature compare.** D09:107; R7:53.
- Source: `S1/program_entry/visible_parameters.rs:54-61`.
- Trace: `service_surface::declared_type_signature` (`psi/interfaces/service-surface/commitment.rs:24-37`) returns an error for an empty result or parameter type, and `?` propagates it. An empty name is refused; it cannot match.
- Outcome: **intended** (refusal preserved).

**7. Package schema admitted without a binding** (R7:42), **folded artifact re-authorized in process** (D09:106; R7:97), **unmatched boundaries produce no leaf** (D09:132; R7:75), **deferred entry nomination error** (R7:111).
- Source: `S1/program_entry/published.rs:192-200`, `S1/published_application.rs:290-296`, `S1/provider_rows.rs:202-232`, `S1/selected_execution.rs:179-183`.
- Trace: the code still matches the stated or traced classes in the record. A missing leaf becomes `MissingSupplies`. Every consumer of the entry calls `entry_nomination()`.
- Outcome: **intended**. The stated and traced classes are preserved.

### Stage 02: admission and lowering

**8. A settlement whose requirement matches no module boundary stays in the admission request; `unwrap_or_default()` on boundary text.** D09:110, D09:195; R7:165(a,c).
- Source: `S2/admit_providers.rs:176-208`.
- Trace: this filter only removes a settlement whose matching boundaries were all left out of the lowered plan. Every settlement it keeps goes through `settle_boundary` (`verification/provider-admission/src/settlements.rs:70-120`). That function requires the exact selected plan and a plan row for the requirement, and refuses otherwise. An empty boundary text can only stop a match, which keeps the settlement and sends it to that full check.
- Outcome: **intended**. Keeping a settlement is not accepting it.

**9. A callback whose operation is not a module `BoundaryCall` stays.** D09:110; R7:165(b).
- Source: `S2/admit_providers.rs:248-260`.
- Trace: the filter can scope only callbacks that are registered at a boundary call. Every other callback is kept and admitted in full by `admit_native_providers` (`provider-admission/src/admission.rs:146-340`), together with its installed-interrupt and occurrence checks (:263-296).
- Outcome: **intended**.

**10. Arithmetic and relations check only the result type, not operand types.** D09:111, D09:199; R7:201, R7:365.
- Source: `S2/operations/scalars.rs:92-104`, `:348-378`.
- Contract: the lowered-form checker checks every scalar operation's operand and result shape through `goal_free_scalar_leaf_semantics` (`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs:1745-1748`). For a `BinaryInteger` leaf, `validate_operand_shape` requires equal integer operands, or comparable non-address operands for a Boolean result (`psi/semantics/src/scalars/scalar_leaf_semantics.rs:302-313`). Native stages trust an authorized artifact's checked judgments (`psi/artifact/authorization/authorization.rs:22-31`). Under AGENTS "verify at trust boundaries", default compilation does not re-verify them.
- Outcome: **intended**.

**11. `expect_scalar()` panics instead of refusing a malformed result.** R7:190(b).
- Source: `S2/operations.rs:298`, `:415-416`, `S2/operations/scalars.rs:92`.
- Trace: codec foundation validation requires a scalar result for every operation not listed explicitly (`psi/artifact/wire/src/foundation_validation.rs:12943-12947`, "scalar operation declares a Unit result"). It also requires one for `Call` (:12878-12882). So decoded artifacts cannot reach the panic.
- Outcome: **intended** (an unreachable invariant).

**12. Only machines with an authored declaration are checked against the scalar closure.** D09:112, D09:135; R7:226(a).
- Source: `S2/selected_input.rs:994-1003`.
- Trace: closure membership is keyed by checked declaration (`psi/execution-inputs/scalar-applications/completion.rs:50-68`), so a machine without one has no membership to test. Every scalar application the emitted reach demands must still have a completed value (`selected_input.rs:1059-1065`). The demand comes from `scope.reach().scalar_application_demand()` (:265).
- Outcome: **intended**.

**13. With no address model, the address-model comparison is skipped.** D09:112, D09:135; R7:226(b).
- Source: `S2/selected_input.rs:1021-1023`.
- Trace: every producer derives the model from `TargetSemantics::close_address_model(profile)`, which is `None` only for a profile without an address model. A completion formed without a model refuses every `AddressModel` range condition ("scalar range condition needs an admitted address model", `scalar-applications/observations.rs:706-709`). The carrier layout is compared unconditionally (:1010-1019).
- Outcome: **intended**. A closure with no model cannot have decided anything that depends on one.

**14. `scope: None` lowers the whole module.** D09:113; R7:190(a), R7:226(c).
- Source: `S2/operations.rs:1913`, `S2/selected_input.rs:268-283`.
- Outcome: **intended** (traced class preserved). Production passes `Some(scope)` (`S2/selected_execution.rs:17-27`, `:56-66`), and `None` is used only in test support.

### Stage 03: optimizer

**15. Tarjan skips edges to absent nodes, and `visit` recurses once per node on the current depth-first path.** D09:116, D09:140, D09:207; R7:302.
- Source: `S3/mod.rs:175-244` (skip at :199-201).
- Trace: the block graphs key every block of the function (`S3/control_flow_analyses.rs:196-201`, `:418-422`), so the skip cannot fire for them. The call graph (`:88-157`) keys the unit's functions, while `call_machine_references` (`representations/abstract-operations/optimization-unit/src/lib.rs:228-260`) also names dispatch-table machines. This review found no validator that ties every call reference to a function of the unit. The recursion depth grows with the longest simple path in a function's block graph or in the call graph.
- Next check: confirm that stage 02 lowers every machine reachable through a call or dispatch reference into `plan.functions`. Then measure the stack depth of `strongly_connected_components` on a generated straight-line function with tens of thousands of blocks.
- Outcome: **still open**. Not reproduced.

**16. `merge_boundary_ownership_witness` returns an empty witness when no ownership facts exist.** R7:269.
- Source: `S3/control_flow_cleanup.rs:650-691` (:663-668).
- Trace: it returns the empty witness only when the function has no structural parameters, entry claims or declared places, so there is no ownership to restore. Otherwise a missing fact returns `None`, and the rule proposes nothing.
- Outcome: **intended**.

**17. `shared_jump_fusion` builds substitutions without the dominance check `merge_substitutions` uses.** D09:68; R7:266.
- Source: `S3/control_flow_cleanup.rs:1130-1173`.
- Trace: fusion applies only when the target is a single terminal `Return*`, `ReturnStructural` or `Crash` node that is not the entry block (:1130-1142). The substituted values are the jump's own arguments, which are available at the jump that the copied node replaces, so they dominate the new use by construction. Block merge needs its dominance check because a target parameter can be used anywhere in the merged region. The pass manager revalidates every candidate (`S3/pass_manager.rs:643`).
- Outcome: **intended**.

**18. Countdown propose takes the first rank binding; replay requires exactly one.** D09:64; R7:277.
- Source: `S3/countdown_analyses.rs:84-88`, `:225-231`, `:263-273`.
- Trace: when propose and replay disagree, `validate::accept` returns `SnapshotMismatch`. The difference can only refuse.
- Outcome: **intended**.

**19. Components without a countdown projection stay uncertified, not refused.** D09:136; R7:341.
- Source: `S3/ranked_cycles_validation.rs:612-639`.
- Trace: an uncertified component gets no countdown analysis and no rewrite that depends on one. The certificates derived from the unit must equal the Terminal projection (:631-639).
- Outcome: **intended**. It loses optimization only.

**20. `compatible_policy_cse_phi_rows` drops redundant operations in the entry block; the other two row sets do not.** D09:137; R7:291, R7:366.
- Source: `S3/global_value_numbering.rs:2398-2421`.
- Trace: dropping a redundant row only withholds a candidate. Every candidate that remains is revalidated.
- Outcome: **intended**. This is a missed optimization at most, not a correctness defect.

**21. Self-remainder and self-subtract rely only on the accepted obligation fact.** D09:122; R7:324.
- Source: `S3/proof_check_elision.rs:708-773`; independent validator `optimization-unit-semantics/src/candidates.rs:6667-6690`, `:6700-6712`.
- Trace: `x % x = 0` holds whenever the accepted obligation proves the divisor nonzero, under every overflow policy. `x - x = 0` holds for `Exact`. Zero fits every closed integer width (`IntegerType::admits`, `foundation/semantic-records/src/proposition.rs:596-611`), and the spec defines widths from 1 to 128, so zero fits a parameter width too. Self-divide needs its explicit check because `1` does not fit `i1` (test `self_division_requires_a_positive_one_inhabitant`). The validator re-derives the same law and requires the matching accepted fact.
- Outcome: **intended**.

**22. A missing pass manifest; a pass name that changed in `rules.rs`.** D09:121; R7:313.
- Source: `S3/pass_manager.rs:46-48`, `:475`.
- Outcome: **intended** (traced class preserved). The name half is the D09:191 ledger disposition (#5371).

**23. External-decision replay cannot be reached from `run_registries`.** D09:146, D09:190; R7:311.
- Source: `S3/pass_manager.rs:385-399` passes `None` to `run_registries_inner`.
- Trace: optimization still records external decisions. Only replaying a supplied log is unwired, so no result is wrong.
- Outcome: **feature gap**. Wiring it up or removing it is an API decision.

## Outside this packet's correctness scope

The record's duplicate-logic notes (D09:57, :59–61, :63, :67) and stage-numbering comments (D09:169) describe maintenance cost, not wrong results. They remain in the digest for code-quality work. The record's "unsure" item about `optimization_graph` edges (R7:367) is resolved by D09:194: any edge the builder misses makes the comparison against the Terminal graph fail.

## Validation

- This review is source reading only. It ran no build, test or `omega` invocation, as the packet instructions require ("no global compiler rebuild"). Every receipt names the existing test that pins its behavior, or says that none does (receipts 4 and 15).
- Files the survey read that changed since `4f3c255d`: `git diff --stat 4f3c255d 24ad2700` over the three stage directories lists 6 files, 72 insertions and 46 deletions, from #5163, #5219 and #5371.
