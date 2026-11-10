# Area 4 digest: Omega pipeline stages 00 to 09

Built from 12 records: `unit_014` to `unit_022` and the second-pass records `unit_020_gap_006`, `_007`, `_008`. Code at commit `457adf4a11f`. Nothing here is a recommendation.

Path shorthand (all under `omega-rust/omega/compilation/`):
`S00/` … `S09/` = `pipeline/<NN>_…/src/`; `PR/` = `products/src/`; `PA/` = `realization/provider-admission/src/` (file names are unique in that crate, so the `provider_admission/…` subfolders are left out).
In stage 03: `LRC` = `legalization_reference_custody.rs`, `LSG` = `legalization_scalar_graph.rs`, `LSrc` = `legalization_source.rs`, `LT` = `legalization_target.rs`, `SSG` = `selection_scalar_graph.rs`, `SST` = `selection_structural.rs`, `SIN` = `selection_inputs.rs`.
Three crates have a `rewrites.rs` or `validation.rs`; the stage prefix says which.
"open", "stated", "traced", "suspected", "inferred" are the readers' words and are kept as they wrote them.

## What this area does

This area is the back half of the compiler. It takes a finished, checked, target-neutral program (a Terminal Psi artifact) and turns it into laid-out machine code rows, one stage at a time: Terminal Psi to "abstract operations" (00), optimise those (01), lower them to target operations with concrete storage homes and calling plans (02), legalise and select machine instructions (03), rewrite the selected instructions (04), give every virtual register a physical home, spilling or splitting if needed (05), pick the exact machine alternative per instruction (06), encode bytes that do not depend on layout (07), order blocks and resolve branches (08), and optionally shrink x86 branches (09). Two side crates sit beside the stages: `products` carries requests, checked records and the final report/publication, and `provider-admission` decides which outside providers (imports, system calls, compiler builtins, interrupt handlers) the program is allowed to use and with what authority. Almost every stage produces a "validated" wrapper with a receipt (a hash of identities), and many stages have a second function that is meant to re-check the output.

**Stage 00 (`00_terminal-psi-to-abstract-operations`, 14,157 lines).** In: a canonical Terminal artifact, a target profile, selected provider plans, calling applications and the ProgramEntry sidecar. Out: an `AbstractOperationPlan` (one flat operation vector per reachable machine), the retained "native realization proposal" (joined provider, callback, comparison and entry evidence) and its binary codec `OMGNTI20` (S00/retained_terminal_codec.rs:1449). Reachability from the entry decides what is lowered (S00/native_execution_scope.rs:214-362).

**Stage 01 (`01_abstract-operations-to-abstract-operations`, 20,944 lines).** In: the verified optimisation input and the selected Psi optimisations. Out: a `ValidatedOptimizedAbstractPlan`. It holds analyses (control flow, effects, constants, liveness, ranges), rule proposers for nine catalogued passes (S01/rules.rs:221), a pass manager that runs rules to a fixed point under a budget (S01/pass_manager.rs:539-712), and loop-invariant scalar motion with its own loop (S01/pass_manager.rs:714-886). Rewriting itself is applied in another crate (`optimization_unit`), per unit 14.

**Stage 02 (`02_abstract-operations-to-target-operations`, 22,644 lines).** In: the optimised abstract plan, boundary settlements, installation evidence, callbacks. Out: a `TargetOperationPlan` of block-structured functions. Each function is lowered block by block in dominator order while a `LiveDefinitions` record tracks where every scalar, structural value, view and stored reference lives, repeated until reference storage at block entries stops changing (S02/control_flow.rs:2828-3052).

**Stage 03 (`03_target-operations-to-selected-instructions`, 68,321 lines, the largest).** Three file groups. Legalization (`legalization*.rs`, about 27,800 lines): `legalize_target_operations` re-projects every node into `Legalized*` rows (S03/legalization.rs:405; LSrc:3072), and a separate standalone audit rebuilds every target row from the abstract side and compares (S03/legalization.rs:466-518; LT; LRC; most of LSG). Selection (`selection*.rs`, about 36,700 lines): emits `SelectedInstruction`s per function (SSG:7094-7632), with special emitters for access lifecycles, static interrupts, views and bytes, call ABI checks and edge-transfer bridge blocks. `structural_inputs.rs` (2,809 lines) is the byte-geometry library both groups use.

**Stage 04 (`04_selected-instructions-to-selected-instructions`, 13,442 lines).** In: a sealed selected program plus liveness. Out: the same program after either the literal-fold phase (23 catalogue rows, about 86 pair rules, S04/literal_fold_rewrites.rs:1037-1164) or the pre-allocation phase (ten rewrite families, S04/rewrites.rs:693-774), or unchanged. Only one executed phase at a time (S04/selected_optimization.rs:135-137).

**Stage 05 (`05_selected-instructions-to-register-homes`, 17,453 lines).** In: the stage-04 output. Out: a `RetainedAllocation` with one physical register view per virtual register. `home_assignment.rs` is a greedy colouring; when it fails for pressure, `assignment.rs` loops over split, rematerialise and spill rewrites until homes exist (S05/assignment.rs:1681-2024).

**Stage 06 (`06_register-homes-to-post-allocation-machine`, 2,032 lines).** In: allocated facts. Out: a `PostAllocationMachinePlan` where each instruction has exactly one machine alternative chosen from operand register aliasing (S06/plan.rs:601-672), plus one optional rule (xor-zero, S06/optimization.rs:205-213).

**Stage 07 (`07_post-allocation-machine-to-selected-form-encoding`, 4,468 lines).** In: selected and machine plans. Out: per-row bytes that do not depend on layout, with branches left "deferred" (S07/row_encoding.rs:220-265), and a validator (S07/validation.rs:74).

**Stage 08 (`08_selected-form-encoding-to-resolved-layout`, 2,567 lines).** In: pre-layout rows. Out: block order, offsets, resolved branches and fixup evidence (S08/ordinary.rs:155), and a validator that re-derives order and branches (S08/validation.rs:36).

**Stage 09 (`09_resolved-layout-to-resolved-layout`, 1,842 lines).** In: a resolved layout. Out: the same, or one where x86-64 `Jcc rel32` are shrunk to `rel8` (S09/x86_branch_relaxation.rs:442-530).

**`products` (4,709 lines).** In: compile requests and stage products. Out: validated requests, the retained `CheckedCompilation`, `CompileReport`/`CompileProduct`, native-product input digests, and published files.

**`provider-admission` (23,928 lines; 13,184 in this area's records, the rest in unit 023).** In: a `ProviderAdmissionRequest` (16 borrowed inputs, PA/model.rs:73). Out: admitted settlements, callback and interrupt cohorts, and a terminal-authority closure review receipt (PA/provider_admission.rs:177-359; PA/terminal_authority_review.rs:52-98).

## How it decides

**Identity (ids and digests).** The dominant key. Operations, values, places, blocks, edges, machines and structural types are matched by id with `==`, usually with an "exactly one match" rule: `(Some(x), None) = (it.next(), it.next())` (S00/callback_settlement.rs:273, 291, 297, 309, 350, 446, 599; SIN:93-96, 463-469, 1584-1600; S03/legalization_function_inputs.rs:93-132). Plans are found by 32-byte digest (S00/settlement.rs:140-152); provider plans by exact evidence (PA/settlements.rs:312-325); rewrite rules by `MachineSemanticKind` pair (S04/branch_and_compare_rewrites.rs:187-191, 832-836); allocation recovery by error variant (S05/assignment.rs:637, 1723; S05/home_assignment.rs:2734-2741).

**Place (place id plus projected path).** Reference custody is keyed on `(PlaceId, path)` in both stage 02 (S02/references.rs:639-640, 1949-1953) and stage 03 (`(place, path, formation)`, LRC:1974, 3822). Overlap is decided on exact path segments, with a "definite" test for duplicate call arguments (LRC:3735, used 5435-5439) and a "may" test for conflicts (LRC:3718). Stage 04 load forwarding keys stores on `(place, byte_offset, byte_count)` (S04/address_and_load_rewrites.rs:900-908).

**Value.** Scalar widths and signs are tested by value: "unsigned 64-bit non-address" for indexes and lengths, "unsigned 8-bit" for bytes (S02/control_flow_storage.rs:1199, 1328, 1617-1621, 2929; S03/selection_views_and_bytes.rs:53, 449-462). Literal folds key on the literal's numeric value against a bound (S04/literal_fold_rewrites.rs:3833; SIN:2949-3002 requires `0` or `1..=4095`). Optimiser passes accept a candidate only when `predicted_cost_delta < 0` (S01/pass_manager.rs:339) and a numeric measure strictly falls (S01/pass_manager.rs:692; S04/rewrites.rs:1838-1845).

**Position.** Stage 03 pairs optimised nodes with target rows by position, with all `CleanupCall` rows trailing (LSrc:3004-3023). Interrupt installation uses literal argument positions 0 and 5 (S02/control_flow_operations.rs:981, 986; LSrc:1594; LSG:3878-3882) and `static_machine_ordinal` 1, 2, 3 (S00/native_execution_scope.rs:30-33; PA/scopes.rs:204-206). Resource-profile layout is navigated by field position (S03/selection_access_lifecycles.rs:2232-2266). Placement byte offsets must be contiguous from 0 (SIN:1126-1154).

**Enum variant.** Every stage has one or more large `match` over the operation or instruction enum; the exhaustive ones (no `_` arm) are S00/lowering.rs:565-1260, S01/semantic_analyses.rs:291-375 and 901-1225, SSG:7358-7540, PA/terminal_authority_review.rs:114-243, PA/classification.rs:235-346.

**Source spelling (interned identity text, names, byte-string labels).** 33 distinct production sites cited by the records, in four crates. Test-only and example-only sites are listed separately and not counted. Stages 02 to 09 have none: units 16, 17, 18, 19, 20 and the three gap records each say they saw no `display_name`, label or string comparison in their files.

- Stage 00, 14 sites. Joins compare `module.identities.text(..)` strings:
  S00/callback_settlement.rs:102-106 (requirement text selects the provider row); :217-224 (optional owner text); :520-583 (boundary text, `unwrap_or_default()` gives `""`);
  S00/entry_settlement.rs:209-217 (requirement split by `split_once("::")` into owner and name, method chosen by text equality); :84-114 (parameter types compared by identity text); :772-777 (declaration text); :521-537 (four-text candidate match);
  S00/native_execution_scope.rs:409-451 (requirement, provider, declaration and machine text); :606-638 (settlement filter by text);
  S00/artifact_admission.rs:419-445 (family head and member text);
  S00/retained_terminal.rs:658-667; :1214-1223 (`semantic_declaration` and `callable` text); :1317-1351;
  S00/retained_terminal_vocabulary.rs:773-778 (producer identity text on decode).
- Stage 01, 2 sites: the convergence measure is chosen by comparing the pass identity to byte strings such as `b"omega.psi-pass.control-flow-cleanup.v13"` (S01/pass_manager.rs:224-260); registry admission compares pass and rule identity bytes (S01/rules.rs:600-610).
- `products`, 1 site: root slot owner parsed by `rsplit_once("::")` on a slot string (PR/evaluated_build.rs:66). (Target profiles are also looked up by requested name, PR/compile_request.rs:397-419; that is request input, not counted.)
- `provider-admission`, 16 sites: PA/provider_admission.rs:472-485 (assumed binding `required` equals requirement text); PA/settlements.rs:110-113, :405-409, :520-523 (requirement rows by text), :513-593 (trait name, method name); PA/compiler_builtins.rs:459-545 (access schema compared to strings built with `format!("{borrow}({accessor})")`, `"read"`/`"take"`/`"write"`); PA/host_cohorts.rs:61-89 (about 60 filesystem method names), :137-147 (6 time names), :187-194 (6 console names), :207-251 (trait name `"DarwinClock"`); PA/context.rs:254-260 (plan `target` string against profile name); PA/reviewer.rs:350-402 and PA/terminal_authority_permission_policy.rs:89-138 (permission rows keyed on schema digest plus requirement text); PA/source_imports.rs:295-318; PA/source_evaluated_bindings.rs:288-303 (and FNV-1a over requirement text at :203-220); PA/callback_entries.rs:200-216 (ABI identity plus owner).
- Not judgments, recorded for completeness: stage 02 attaches `format!("... {binding:?}")` debug text to errors (S02/control_flow.rs:2137, 2329); stage 02 and 03 refusals carry only the Rust call-site line (S02/model.rs:139-142, 320-325; LRC:4151).
- Test-only or example-only, 5 sites: S00/lib.rs:129-146 and 258-318 (scan the compiler's own source for `decode_module(` and `from_bytes(`); S05/register_allocation.rs:103-262 (asserts order of source strings via `include_str!` and `str::find`); S05/output.rs:317-360 (counts `Self::capture(` in its own source); S01/examples/psi_optimization_allocations.rs:225-229 (substring filter).

## The same job in more than one place

Jobs that cross units come first. "Opened" means I read both sites.

1. **"Does this access cover that one" table.** LRC:4582-4590 and S02/references.rs:888-896. Each record only suspected the other. Opened: the two bodies are identical text. More copies by grep, not read: S03/selection.rs:877 (different signature), `optimization-unit/semantics/src/unit_validation.rs:14940`, `psi/interpreter/terminal-interpreter/src/storage_references.rs:4451` (unit 17, "suspected same relation"). Local variants that differ: LSG:262-272 (Owned covers only Shared), LSG:3328-3340, LSG:4206-4217, LSG:6078-6092; three copies of an `allowed` match in S02/control_flow_calls.rs:206-213, 254-261, 588-595; S02/structural_layout.rs:91-97.
2. **"Is this place a completed view descriptor" (`completed_descriptor` / `descriptor_result`).** S02/scalar.rs:655, 688; LSG:2227, 2252; SIN:2878, 2917. Units 16 and 17/19 each suspected the others. Opened: the core test is the same in all three (nonlinear multiplicity, `view_descriptor_projected_qualifications`, no establishments, no claims, `is_descriptor`). They differ in how the producer is found, and `descriptor_result` in selection requires a `Call` row with non-empty `descriptor_origins` (SIN:2925-2936) where the other two require a `materialized_structural_result` match.
3. **Borrowed projection walkers.** `runtime_borrowed_projection` S02/structural_layout.rs:1076 and S03/structural_inputs.rs:2061; `runtime_leaf_copy_projection` S02/structural_layout.rs:1050 against `leaf_copy_projection` S03/structural_inputs.rs:1815. Units 16 and 20 each suspected the other. Opened the first pair: same job (root type and path to selected type, byte offset and runtime index elements) written separately over different layout tables; the stage-03 copy returns `Option` without a shape and adds a `borrowed_referent_presentation == Unchanged` filter. Second pair not opened.
4. **Scalar type to `ValueShape`.** S02/control_flow_storage.rs:2774, S02/function_signature.rs:654 and inline at 81-90, S02/scalar.rs:2163, S03/structural_inputs.rs:72, S03/selection_call_abi.rs:1964, LSG:8601. Opened four: S03/selection_call_abi.rs:1964 is a one-line wrapper of `ValueShape::for_scalar`, LSG:8601 forwards to it, S03/structural_inputs.rs:72 is its own table (8/16/32/64 only), S02/scalar.rs:2163 has a different signature and accepts any integer width unless `require_native_parameter`. So unit 20's "four definitions" is three implementations and one alias.
5. **Descriptor-input guard for a normalised foreign call.** S02/references.rs:1550-1578 and S03/legalization_descriptor_inputs.rs:94-154. Opened: same checks in the same order; stage 02 also excludes selected calls and reads the settlement, stage 03 looks up the target row and uses stored layouts. (Unit 16 cites the stage-02 site as `coordination.rs:1550-1578`; see Disagreements.)
6. **`access_call_custody`.** S02/references.rs:702 and LRC:7495 (unit 17, suspected). Opened signatures only: both return `Vec<TargetAccessCallBinding>`, one over `AbstractFunction` + `LiveDefinitions`, one over `PsiOptimizationFunction` + `Custody`. Bodies not compared. `fn leaf_type` likewise has four definitions by grep (LRC:4341, S02/references.rs:487, S00/lowering_storage.rs:856, `unit_validation.rs:14846`; unit 17, "suspected duplicates (not read)"; not opened by me).
7. **Normalised foreign call check.** S03/selection_call_abi.rs:819-1269 and LT:3077-3317 (unit 18 read both): callbacks, private ordinals, structural arguments, result shape, entry-plan revalidation, at two stages.
8. **Per-operation lists that must stay in step.** Three enumerations of scalar `AbstractOperation` variants in legalization: LSG:7438-7714, LSG:7763-8584, LSrc:1745-2496 (unit 17 read all three). Three hand-kept lists of "instruction kinds that read a place" in selection: SIN:547-649, SIN:2503-2541, SIN:2303-2386 (unit 19). Ten parallel enumerations of the ten pre-allocation families in S04/rewrites.rs (catalog 693-774, policy bits 2188-2207, enum 2297, identity enum 2362, `instruction()` 2321, `identity()` 2336, `From` 2375, tags 2119-2122, error enum 2618-2629, accessors 2275), plus the 17 hand-numbered tags in S05/assignment.rs:383-433 (gap_008).
9. **Three function-header validators.** LSG:1111-1256, LSG:1932-2023, LSG:3464-3566; the three-way choice between them appears twice (LSG:8698-8704, 9039-9045).
10. **"Root structural type of a place"**, six near-copies in stage 03: LRC:4353, LRC:2102, LSG:5571, LSG:5069, LSG:2079, LSG:3349-3370 (unit 17 read all). Stage 02 has its own set: S02/control_flow_storage.rs:2817, :290; S02/references.rs:497, 516, 203; S02/scalar.rs:786; S02/unit.rs:33.
11. **`byte_carrier_leaf_copy`** (a `FixedArray` of literal length with `u8` elements, `index + layout.size <= length`): S00/lowering_storage.rs:1366-1412, S02/control_flow_storage.rs:1972-2013, LSG:5284. Each reader read its own copy; not opened side by side.
12. **`scalar_arrays::shape`** (nested literal-length arrays of primitive scalars): S02/control_flow_operations.rs:1250, LSG:4859-4913, SIN:4657-4690. Three readers described the same rule; unit 15 marked the stage-03 side "suspected".
13. **Pick one calling application by ABI and commitment.** S00/callback_settlement.rs:211-238, S00/retained_terminal.rs:304-339, S00/entry_settlement.rs:229-252 (unit 14 read all; target filter differs).
14. **Successor enumeration from a block terminator.** Stage 01 has five walks with different terminator sets: S01/control_flow_analyses.rs:303-323, S01/semantic_analyses.rs:2241, :2361, :1836-1874, S01/ranked_cycles_validation.rs:731-763. Stage 02 has two more (S02/control_flow.rs:2715-2747 and `lower_terminator` 1392-1830).
15. **Dominators.** S01/control_flow_analyses.rs:214-274; S01/validation.rs:1099; S02/control_flow.rs:110 (bit-set, with the older solver kept as a test oracle 177-239); LT:717-798 (rebuilt on each of 17 `dominates` calls, LT:681); `terminal-psi/semantics/src/control_graph.rs:348` (suspected, not read).
16. **Strongly connected components.** S01/lib.rs:199-267; three further same-named functions in the Psi tree by grep (unit 14, "suspected same algorithm, not read").
17. **Pass loop.** S01/pass_manager.rs:714-886 repeats 539-712 step for step (unit 15 read both).
18. **Pass identity strings** written as constants in S01/rules.rs:8-19 and again as literals in S01/pass_manager.rs:224-243 (grep: 8 and 7 hits).
19. **Classifier and its measure.** `classify` at S01/rules.rs:978 against `dead_total_scalar_operation_count` S01/pass_manager.rs:149-176; S01/proof_check_elision.rs:287 against S01/pass_manager.rs:178.
20. **Propose and replay in the same file.** S01/countdown_analyses.rs has `compute`, `replay` and `validate` per layer, the replay being "a second implementation in the same file" (unit 14); S01/ranked_cycles_validation.rs:239 against :431 are two matchers of one loop shape, cross-checked on purpose (:631).
21. **Stage 04 rewrite skeleton.** Admit, clone the whole plan, edit one instruction, seal: S04/address_and_load_rewrites.rs:252, 1306; S04/copy_and_extension_rewrites.rs:644, 1265, 1642; S04/branch_and_compare_rewrites.rs:530, 1175, 1779, 2273. The two fold admits repeat one skeleton (S04/branch_and_compare_rewrites.rs:60-260 and 714-881) with one difference: the branch version filters non-flag implicit uses (799-805), the boolean version does not (157-160).
22. **"Delete and shift every later id".** S04/copy_and_extension_rewrites.rs:1058 and S04/literal_fold_rewrites.rs:1901 (gap_007 read both; shift by one against shift by `partition_point`).
23. **`isolated_declaration`** has two bodies with different meanings: `selected-instructions/analyses/src/rewriting.rs:156` (`barrier == None`) and S04/branch_and_compare_rewrites.rs:1130 (`barrier == ControlFlow`) (gap_006 read both). Same name, not the same predicate.
24. **Work-step formulas** written once in admission and again in `measured_steps`: S04/branch_and_compare_rewrites.rs:555-610, 1199-1251, 1690-1715 against 2292-2323.
25. **Stage 05 pairs.** `stage_selected_lowering_register_allocation` S05/assignment.rs:691-716 and `stage_pre_allocation_register_allocation` 718-743; the `transformed` module halves 2782-2838 and 2840-2896 ("line-for-line parallel", gap_008). Two colouring implementations, vector and bitmask: S05/home_assignment.rs:648-866 and 1095-1430. Production victim ranking S05/assignment.rs:1454 against a test-only ranking 1356 with a different key.
26. **"Next dense id" and "shift boundary settlements".** Three id helpers: S05/pressure_rematerialization_recovery.rs:489, S05/fixed_view_copy_recovery.rs:154-195, S05/rewrites.rs:601-604. Settlement shift inline at S05/pressure_rematerialization_recovery.rs:98-128, S05/fixed_view_copy_recovery.rs:1248-1255, S05/runtime_spill_and_split.rs:5323-5330 and 5406-5412, and as a helper at S05/rewrites.rs:1626-1662.
27. **Sorted home index.** S05/preservation.rs:39-78 (rejects duplicates) and S06/plan.rs:320-340 (first match wins) (unit 21 read both).
28. **Declared size rule.** S07/validation.rs:561-578 and S08/lib.rs:261-272 (unit 21 read both; same semantics, `u16` against `u64`).
29. **Block order and branch resolution.** S08/ordinary.rs:85 and S08/validation.rs:256; S08/ordinary.rs:835-1023 and S08/validation.rs:428-618 (documented as an independent re-derivation, S08/validation.rs:2). Signed delta helper three times: S08/ordinary.rs:332, S08/validation.rs:620, S09/x86_branch_relaxation.rs:714. Offset assignment twice: S08/ordinary.rs:282-313, S09/x86_branch_relaxation.rs:648-665.
30. **Stage 07 route lists.** Producer kind list S07/row_encoding.rs:227-265 and validator match S07/validation.rs:176-364 (macro patterns), kept in step by hand; x86-64 and AArch64 branches duplicated throughout S07/row_encoding.rs:419-515, 812-838, 896-1035.
31. **Layout constants with no shared name.** Descriptor 16 bytes with length at offset 8: LSG:3407, 3431, 5988, 6475; LSrc:2363-2392; S02/scalar.rs:89-105; S02/control_flow_calls.rs:483-488. Sum tag 4 bytes at offset 0: LSG:5157-5160; SIN:752-756; SSG:6394-6402. Task state 104 bytes and descriptor 10 bytes: S03/selection_interrupts.rs:427, 433, 1597, 1605; SST:2148, 2156, 2220, 2634. Inline copy limit 4096: S03/selection.rs:742.
32. **`unique_row`.** S00/lowering.rs:1273 and LSG:8676 (unit 14 suspected). Opened: same idiom, different results; the stage-00 one returns `None` on zero or several matches, the stage-03 one returns `Err` on several and `Ok(None)` on zero.
33. **Provider admission.** "Exactly one selected row for the requirement" at PA/settlements.rs:405-415, :520-528, PA/source_imports.rs:295-318, PA/source_evaluated_bindings.rs:288-303. Call-edge extraction twice: PA/arrival_helpers.rs:788-933 and PA/windows_handle_calls.rs:136-333. Console classification twice: by method name PA/host_cohorts.rs:187-205 and by intrinsic identity PA/classification.rs:147-155. The same resource-profile validity rule exists as Rust in `psi/foundation/access-plans/src/resources.rs:1236, 405, 427, 1396` and as emitted instructions in S03/selection_access_lifecycles.rs:4189, linked only by tests (unit 18).
34. **`products` equality written three ways.** `same_source_choices` PR/evaluated_build.rs:84-126, `same_product_source` PR/package_build.rs:64-98, and the digest PR/retention_identity.rs:25 ("separately maintained", unit 22).

## Fixed lists of shapes

Stage 00
- Installed interrupt helpers: `for ordinal in [1, 2, 3]` (S00/native_execution_scope.rs:30-33).
- ProgramEntry: two schemas only (S00/entry_settlement.rs:84-114); internal storage signature is exactly two 16-byte, align-8 records of two 8-byte integers (167-203); entry call binding only for `NativeTarget::uefi_x64()` (709). The same target test is at S02/control_flow.rs:476 and LT:5480 (grep: 3 hits; I re-ran it, 3).
- Direct boundary leaf assumed to have one row and one method: `leaf.plan.rows[0]`, `declaration.methods[0]` (S00/retained_terminal.rs:1245-1246).
- Wire tags: `BuiltinFunction` 105 numbered variants (S00/retained_terminal_vocabulary.rs:1479-1585), `TargetProfile` 9 (1710-1720).

Stage 01
- Convergence measure: seven named passes plus a default (S01/pass_manager.rs:244-260). Pass steps: two shapes (S01/rules.rs:1223). Pass catalogue: nine entries (S01/rules.rs:221).
- Control-flow graph knows four terminator kinds (S01/control_flow_analyses.rs:303-323); liveness knows two (S01/semantic_analyses.rs:2241-2249).
- Only one loop ranking shape: unsigned fixed-width `0 < n` guard with `n - 1` descent, one entry edge, one exit edge, exactly two constants (S01/ranked_cycles_validation.rs:246-341; S01/countdown_analyses.rs:65-70, 709).
- Loop-invariant motion: an if-else chain over about 20 shapes, each with a plain and a crash-continuation twin (S01/ranked_rewrites.rs:589-704; S01/validation.rs:100-262).
- Cleanup rules run only on functions with no block structural parameters (S01/control_flow_cleanup.rs:1576-1585). Specialisations skip cyclic machines (S01/rules.rs:663, 1054, 1159).
- SCCP: 39 rules (S01/sccp.rs:1006); proof-check elision: 12 (S01/proof_check_elision.rs:804); GVN: 16 (S01/global_value_numbering.rs:1670-1689).

Stage 02
- Block structural parameters: six carrier kinds (S02/control_flow.rs:2634-2647). Terminators: eight, `_ => Err(unsupported_control_flow)` (1392-1830).
- `records::establish`: `(Record, no case)`, `(Sum, case)`, `(Mixed, case)`, `(FixedArray literal length, none)`, else unsupported (S02/control_flow_storage.rs:3359-3407). Byte concatenation: exactly two operands (1135). Observations: a fixed operation list (2071-2493). Primitive storage: six kinds (2921-3299).
- Builtin realisations: a `match` at S02/unit.rs:1043-1295 that refuses `ProgramImageRetainedSuffixStart | NearestIeeeFloatArithmetic(_) | AccessorScalarAccess(_) | EmptyRunRelease | StaticInterrupt { .. } | AccessLifecycle { .. }` at this site (1247-1253). `HostedReadByte` result must be `[empty, byte]` (1136-1193). `LinuxWriteLine` ELF on x86-64 or AArch64 only (1087-1092). Scalar-argument boundary calls allowed for `HostedExitProcessI32 | HostedWriteByteI32 | NearestIeeeFloatArithmetic | AccessorScalarAccess` or a normalised foreign call (S02/lib.rs:213-227).
- Every FMA is refused: `UnsupportedIeeeFloatFmaRealization` (S02/coordination.rs:522-533).
- Reference leaf walk capped at 4096 leaves (S02/references.rs:2374, 2668).
- `unsupported_control_flow` is the catch-all: 71 uses in control_flow.rs, 348 across the stage's six files (unit 15 grep).

Stage 03
- Boundary call projection accepts one argument with `HostedWriteByteI32` (LSrc:257-272). Hosted read byte only on `[linux_x64, macos_x64, linux_arm64, macos_arm64]`, no Windows target (LSG:4785-4791), with `let [empty, byte] = cases.as_slice() else` (LSG:4711).
- Cleanup callee: at most one structural parameter (LSG:2944; SSG:2453 "call.arguments.len() > 1").
- Structural call argument: an ordered ladder (LSG:6202-6448) and a five-probe combination with four accepted arms (LSG:6056-6073).
- Literals 8/16/32/64-bit only (LSG:7731-7747); `integer_type` only u64, i64, u32, i32 (LSG:9104-9116); integer widths `8 | 16 | 32 | 64` with fixed carrier repeated about ten times in SSG (1233, 1329, 3667, 3693, 3918, 3987, 4038-4040, 4193, 7161).
- Audit: `IntegerWiden` into 16, 32 or 64 bits only (LT:3367); hosted scalar only `HostedWriteByteI32` and `HostedExitProcessI32` (LT:2992-3050).
- Selection: direct register results 1 or 2 fragments (SIN:1131); native code layout exactly 24 bytes (SIN:3058); `store_registers` 1 or 2 operands, `bulk_operands` 5 (S03/selection.rs:622, 765); `referent_used` about 35 arms then `_ => false` (S03/selection.rs:1559).
- Interrupts: x86-64 only (S03/selection_interrupts.rs:1825); six structural arguments (915; PA/model.rs:155); `ProviderSelected` stack always an error (1353, 1537); table size at most 4096 and a multiple of 16, TSS exactly 104 bytes, tags hard-coded 0, 1, 2 (S03/lib.rs:114-125, 211-213).
- Access lifecycles: profile layout fixed by position, 32 regions, 16 rules, 12 atomic permissions, 32 reach entries (S03/selection_access_lifecycles.rs:2232-2266); result sum exactly `[Ready, Rejected]` (4372-4381).
- Geometry: `project_storage` handles `Case`+`Field`, indexes on arrays and views, `Field` on records, `_ => return None` (S03/structural_inputs.rs:1939-2043).

Stage 04
- Literal fold: producer must be `MaterializeI64{Unsigned}` (S04/literal_fold_rewrites.rs:1221-1226); bounds 4095, `EncodingNonZero(4095)`, `Exactly(0 | 1 | u64::MAX)`; each saturating rule pinned by an `assert!`, for example "the clamped identity fold holds only for a non-u64 carrier's zero literal" (3070).
- Constant boolean: 15 rule rows (S04/branch_and_compare_rewrites.rs:449-465); constant branch: 9 rows, NonZero, U64<, I64< only, no `<=` (1097-1107).
- Address fold consumers: `Load8/16/32/64`, `AddressOffset`, `Store` of 1/2/4/8 bytes (S04/address_and_load_rewrites.rs:34-59); AArch64 offset `/scale <= 4095` (61-72).
- Copied call operand: three consumers `CallUnit`, `CallScalar`, `CallAggregate` (S04/copy_and_extension_rewrites.rs:398-412).
- `unrostered_reach`: about 50 kinds listed `Private`, five `Store`, `_ => Unaccounted` (S04/rewrites.rs:2742-2809).
- Stage admission: one executed phase; anything else `UnsupportedComposition` (S04/selected_optimization.rs:155).

Stage 05
- Two routable declared recovery rules, others `UnsupportedComposition` (S05/register_allocation.rs:48-56). Recovery admitted only for `NoCompatibleHome | NoCommonCandidate | UnresolvedEntryTransitions` (S05/home_assignment.rs:2734-2741).
- Each rewrite has its own victim list: floats allowed in spill, not in split (S05/runtime_spill_and_split.rs:4934); address-typed integers excluded in rematerialise and split, not in spill (3619-3622); spill slots 8 bytes align 8 (1733-1735); fixed-view copies 64-bit integers only (S05/fixed_view_copy_recovery.rs:42-47).
- Bitmask paths need view and unit ids below 128 (S05/home_assignment.rs:363, 1931-1938; S05/runtime_spill_and_split.rs:4173-4178).
- One rematerialisation action per function (S05/pressure_rematerialization_recovery.rs:979-982).

Stages 06 to 09
- One optimisation rule, `X86SelectXorZeroI64MaterializationV1` (S06/optimization.rs:205-213); address field set from 17 instruction kinds, `_ => None` (S06/plan.rs:515-577).
- Encoding routes: 16 address kinds, calls, foreign calls, image address, 4 branch kinds, else `Ordinary` (S07/row_encoding.rs:227-265); two architectures; hosted frame windows a three-row table (S07/frame_address.rs:284-291).
- Layout: four policies, else `UnsupportedFunctionShape` (S08/ordinary.rs:56-82); jump 5 bytes x86 and 4 AArch64, conditional 6 and 4, 8 when widened (413-433).
- Relaxation: zero or one selected optimisation (S09/lib.rs:175-193); x86-64 only; widths 2 and 6 (S09/x86_branch_relaxation.rs:37-38).

`products` and `provider-admission`
- Three requested product kinds (PR/checked_compilation/report.rs:38-42). Component publication: "component files require an admitted ELF, Mach-O, or Windows x86-64 process publication contract" (PR/compilation_products.rs:521-524).
- Windows reviewed-import list fixed at 16 rows (PA/windows_imports.rs:42-100); closed policy inventory `494 + BuiltinFunction::COUNT` (PA/classification.rs:30).
- Windows handle API ordinals and constants (PA/windows_handle_arguments.rs:122-146, 367-449); budgets 64 values, 1024 combinations (18-19).
- Linux only for system calls (PA/settlements.rs:513-593); x86-64 only for installed entries (PA/admission_source.rs:176).

## Accepts when it does not know

The three classes are kept apart. Where a record says what wrong program might get through, it is quoted or paraphrased closely; where it does not, nothing is added.

### Classed "open" (no reason at the site, not traced)

Stage 00 (unit 14)
- S00/settlement.rs:130-137: an integer `IntegerEqual` or `<=` operation with no realisation row is skipped; only a float compare is an error. `validate_roster` follows (226-238), not read.
- S00/callback_settlement.rs:586-588: no matching `Import` row for a registrar, nothing rewritten; "a registrar served by a non-import binding (e.g. syscall) keeps its row".
- S00/callback_settlement.rs:520-523 and S00/entry_settlement.rs:94-95: absent identity text becomes `""`; "an absent identity on one side could match an absent on the other".
- S00/entry_settlement.rs:684-686: a boundary operation matching no selected ProgramStorageEntry row ends in `Ok(None)` (699-701).
- S00/artifact_admission.rs:1015-1017: a scalar closure input with no address model skips the address-model comparison.
- S00/native_execution_scope.rs:409-411 and 412-419: a reachable boundary with no provider candidate, or none selected, "stays in the scope without a body".
- S00/native_execution_scope.rs:611-637, 644-647, 681-684: a settlement, builtin proposal or callback whose boundary or operation is not in the module is kept "in scope". :489-496: a boundary entry with no admitted member is skipped.
- S00/optimization.rs:198, 210-223: a store or atomic whose restriction cannot be computed yields an accepted obligation fact with no `scalar_receiving`. :169-178: a fixed zero 32-byte field, purpose not stated.
- S00/lowering.rs:1624-1625: cleanup `requirement_obligations` cleared and `cleanup_receiver` rewritten at runtime lowering. :189: dynamic calls carry empty `selected_evidence`. :1571: `ReturnStructural` with empty `trivial_affine_locals`, "not traced".
- S00/lowering_scalars.rs:264-266, 290-306, 562-567: operand types of fused multiply-add, bitwise operations and shifts are not checked; "wrong operand types would pass through this stage".
- S00/lowering_storage.rs:400-418: an operation kind not in the read-only list and not lending the root is treated as not writing.
- S00/retained_terminal.rs:932-934 ("a family source with a stale/absent head is ignored"), 948-950, 1224-1226, 713-716.

Stage 01 (units 14, 15)
- S01/control_flow_analyses.rs:303-323: `StructuralCase`, `InvokeNoNormal` and `ExternalCompletion` add no edges, so blocks reached only through case arms would be marked unreachable and absent from dominators; "whether any caller can see such functions is not traced".
- S01/semantic_analyses.rs:2241-2249: liveness omits `StructuralCase` successors; "a still-used scalar could be proposed for removal" by the dead-node proposer (S01/rules.rs:372-378). Reachability with a bad program: open.
- S01/control_flow_cleanup.rs:1141-1158: `shared_jump_fusion` builds substitutions without the dominance check that `merge_substitutions` has (231-261).
- S01/ranked_cycles_validation.rs:428, 619-624: a cycle with no recognised countdown shape is left without a certificate; "a cycle with a legit-but-different ranking is not an error here".
- S01/specialization.rs:478-508: a callee's constant on its single `Return` is used without checking that the return is reachable or that the call has crash continuations.
- S01/validation.rs:2557-2564: a node outside the loop whose operation is not enumerated and which references the picked place is accepted.
- S01/validation.rs:1086-1087: duplicate value definitions overwrite silently.

Stage 02 (units 15, 16)
- S02/control_flow_operations.rs:557-570: `PortWrite` passes `value` through with no source lookup.
- S02/control_flow_storage.rs:1320: any arrival parameter passes as a byte-write source here. :3569: reference leaves of a descriptor-member field are not relocated. :2613-2616: an IEEE float block parameter is skipped.
- S02/function_signature.rs:546-552: a failing `prepare_function_signature` becomes `Ok(None)`. :228, 232-252: a layout root whose recipe cannot be built is dropped.
- S02/references.rs:1818-1827: "a call whose incoming reference lives in a sum-case path can silently delete the destination's tracked provenance instead of failing". :2534-2540: the same skip for call results. :2235-2237: a place with no contract is treated as having no references to discard.
- S02/structural_layout.rs:184-189: an index segment whose prefix does not resolve is treated as an ordinary step.

Stage 03 (units 16 to 20)
- S03/legalization.rs:240-243: `accepted_fact` takes the first match and does not reject duplicates.
- S03/legalization_descriptor_inputs.rs:81-91: a selection error is discarded with `.ok()`.
- LRC:6004-6006, 6853-6855, 6881, 7209, 7286-7297: no custody work on positions or edges the flow analysis proved impossible; "an operation on a position the analysis wrongly proves unreachable gets no custody checking" (traced only to the producer, LRC:1100-1125).
- LRC:6637: `apply` falls through `_ => {}` for any operation without an arm.
- LRC:3495-3503: "a stored-call effect from an inactive-case source with a genuinely missing leaf would not be flagged". LRC:5598-5617, 5651-5653: "a call result reference path that has no recorded custody". LRC:5790-5796: `DiscardRoot` for a place with no contract is skipped and the result ignored (7040).
- LSG:228-230: a binding to a non-join parameter returns `true` from this function. LSG:2304-2309, 2317-2319: empty descriptor origin list. LSG:1849-1853: a `MutableBorrow` window request with an empty path is authenticated without checking backing roots.
- LSG:8451-8459 with LSrc:691-727: "an exact add/sub/mul/div whose operands have a different integer type than `scalar_type`", unless an obligation fact or an earlier stage guarantees it; the reader "did not find the guarantee in this unit".
- LSG:8948-8953, 8914-8921: `match_input` skips structural call arguments that cross a `Referent` and calls with a structural result.
- LSG:7376-7409 into LSrc:3053: `structural_contract` returning `None` is accepted (inferred).
- LSrc:1287-1469: byte and element view read and write projections do not compare against the target row.
- LT:810-812 with 446-448: an unreached block is dominated by everything; "a target plan whose unreachable block uses values defined in other blocks". Traced for selection (rejected at S03/selection.rs:166-168); open for the standalone audit.
- S03/optimized.rs:68-70: a required register parameter with no fixed view in the environment gets no constraint and no error.
- S03/selection.rs:1559: an instruction kind not in the `referent_used` list counts as not using the referent.
- S03/selection_call_abi.rs:1428-1430: "a function with no result and arbitrary parameter placements passes this entry predicate".
- SIN:647-649 ("a new instruction kind that reads a parameter place and is not added to the list ... would silently get no home"), SIN:2540, SIN:4856-4858, SIN:3276, SIN:30-32 and 49-51.
- S03/selection_interrupts.rs:404-475: memory footprint uses hard-coded 104 and 10.
- SSG:2972-2990, 6777-6789: a value not in `required_values` gets no transport. SSG:3628-3648: an integer constant is materialised with no range check against the carrier.
- SST:1044: "a `StoreStructuralField` whose `OwnedResult` root is not a home of this function would pass this admission check". SST:5522-5531: `ReleaseReference | EstablishTrivialAffineLocal` emit nothing and return `Ok(true)`. SST:227-230.
- S03/structural_inputs.rs:2385-2410: "an access with an offset past the end of a borrowed view descriptor frame's declared shape". :111-113, 131: empty reach returns `true`. :2437-2440: size bound skipped for an element-view root.

Stage 04 (unit 20 and gaps)
- S04/address_and_load_rewrites.rs:880-881: "a store to an outgoing-argument slot that overlaps the forwarded place". :1218-1228: an instruction with no memory-access row is accepted when classed `Private` (gap_008 read the classifier: default is `Unaccounted`, S04/rewrites.rs:2807).
- S04/copy_and_extension_rewrites.rs:1475-1477: a 32-bit zero extension is removed after `Float32ToBits` or `LoadPacked` "without any proof in the file that the upper 32 bits are zero". :1464: `as u64` truncation.
- S04/branch_and_compare_rewrites.rs:157-160: a materialisation with an extra non-flag implicit use reaches `resolved_unit_surface` unfiltered.
- S04/branch_and_compare_rewrites.rs:1617-1653: a flag unit live out of the function is not considered. The reader's sub-question (does `block_instructions` include the terminator) is answered below: it does.
- S04/branch_and_compare_rewrites.rs:1508-1560: the reader feared an empty `reaching_events` set would let a compare be removed. Checked below: an empty set is an error, so that path does not exist.
- S04/literal_fold_rewrites.rs:2891-2913: "if `WrappingRemainderI64` can take an unsigned 64-bit register, `x % 0xFFFF_FFFF_FFFF_FFFF` is not 0 for general `x`, so the fold would be wrong"; operand signedness not read.
- S04/literal_fold_rewrites.rs:2571-2617: `ConditionalBranchNonZero` counted as equality-only with no comment; live-out not seen. :3526-3541: implicit defs dropped when no `implicit_uses` lists the unit. :1717-1815, 1901-2023: only the listed tables are renumbered.
- S04/rewrites.rs:158-191: "a register that had one `MaterializeI64` and was also a block parameter bound by a successor would be reported as constant". :144-156: `literal_bits` and `immediate_bits` treat negatives differently.

Stage 05 (unit 21, gap_008)
- S05/assignment.rs:490-491: the manifest record trusts `ranges.plan().target` and `.selected`. :970, 1320: unchecked `u32` subtraction.
- S05/rewrites.rs:626-631, 654-682: "instruction-local" registers are left as is with no comment.
- S05/home_assignment.rs:1988-2010: the non-dense view-id branch checks only the class.
- S05/runtime_spill_and_split.rs:1651-1663: "if a load position is missing from `loads` but a store used a shared slot the reload would read the private slot".

Stages 06, 07
- S06/optimization.rs:304, 335-344: one liveness table per function built from the first candidate's extra-unit set and reused, guarded only by `debug_assert_eq!` (342).
- S07/validation.rs:315-327: `code` and `footprint` of a deferred row are not examined here.

`products` (unit 22)
- PR/checked_compilation.rs:162-165: no root grants, `reconstruct_trust_obligations` is skipped.
- PR/evaluated_build.rs:104-118: native binding evaluations compared by `terminal_code` only.
- PR/package_build.rs:185-193: inputs replaced after sealing without recomputing the handoff.
- PR/package_build/retention_identity.rs:685-689: invalid symbols hash identically.
- PR/prepared_build.rs:127: `adopt_derived_code` guards equivalence only with `debug_assert!`.

`provider-admission` (unit 22)
- PA/admission_source.rs:209-217: "a current cohort that drops a fatal entry, the anchor or the assumptions but keeps everything else".
- PA/provider_admission.rs:437-440, 472: "a reachable boundary without a selected provider whose declaration or identity text is missing from the plan".
- PA/access_lifecycles.rs:326: `_ => {}` accepts any structural shape other than the four named.
- PA/retained.rs:210-217: a demanded requirement matching zero intrinsic rows continues with no diagnostic.
- PA/settlements.rs:290-308: with no declared calling application in the schema, the application from the retained row is accepted without a schema comparison.
- PA/source_imports.rs:55-57, 66: "a boundary that has two selected non-external rows is not reported here".
- PA/scopes.rs:556-577: only `RegisterAccess` with `InterruptControl(Enable)` is recognised as re-enabling interrupts.
- PA/host_cohorts.rs:127-135 through PA/terminal_authority_policy.rs:260-262: "a mislabeled authority class for an import whose real behavior is broader than its method name".

### Classed "stated" (a comment or name at the site gives the reason)
- S01/lib.rs:183-189: empty step list uses `from_transformed`; comment 18-21: "an exact identity transform without reconstructing unused optimizer ownership analyses".
- S00/lowering_storage.rs:147-149: no coherence edge for pure stores and fences (stated by the predicates). S00/retained_terminal_vocabulary.rs:1096-1098: a digest memo decodes to default (a cache).
- S01/control_flow_cleanup.rs:657-662: merge accepted with no ownership facts when the function owns nothing (stated by the condition).
- S02/control_flow_storage.rs:355-369: "a borrowed arrival can supply only its live whole view descriptor".
- LRC:108-122: nothing is expired when liveness is unavailable; test name "unavailable_lookup_liveness_never_proves_a_coordinate_dead" (818).
- S07/row_encoding.rs:263: unlisted kinds go to `Ordinary`; every draft is checked afterwards (header 25-28).
- S09/lib.rs:139-166 and S09/x86_branch_relaxation.rs:217-234: validation "without re-running the fixed-point search" (S09/lib.rs:6). The reader adds that the missing identity comparison is not stated: "an artifact carrying a layout that is not the relaxation of the baseline but has the right root identities and an in-budget usage".
- PR/compilation_products.rs:255-272: any product with `wrote_output == true` passes ("requires a checked, published primary product").
- PA/source_evaluated_bindings.rs:177-272: execution evidence is minted from the plan, not independently attested (header 4-10).
- PA/reviewer.rs:358-367: with no permission policy a classification failure is recorded, not refused (PA/terminal_authority_review.rs:6-10).
- PA/windows_handle_aliases.rs:367-371: an untracked word means no tracked handle (comment 24-26).

### Classed "traced" (the reader followed callers)
- S00/callback_settlement.rs:106-108: an unselected registrar is not placed; accepted by design at S00/retained_terminal.rs:1309-1316; "whether an unselected registrar can reach native execution is not shown here". :109-114: adapters validated at S00/retained_terminal.rs:1317-1351.
- S00/entry_settlement.rs:362; S00/artifact_admission.rs:240-255 (lowering later errors at S00/lowering_scalars.rs:475-477).
- S01/pass_manager.rs:258-260 and :290: unknown pass or cycle gives a measure that then fails closed at :692. S01/countdown_analyses.rs:84-88 against 225-231: propose takes the first binding, replay requires one; overall rejected. S01/ranked_rewrites.rs:397: custody reconstruction skipped when there are no certificates.
- S02/control_flow.rs:2856-2877: merge errors swallowed inside the fixed-point loop, re-checked at 3046-3049 and 3053-3078. :2721: the cleanup guard lists all four variants, so it is always true (inferred). S02/coordination.rs:47-55; S02/lib.rs:640-642, 674; S02/references.rs:1758-1760; S02/scalar.rs:383, 1836 (cycle guard that cannot trigger).
- S03/legalization.rs:440-463: `seal` verifies nothing; only `legalize_target_operations` calls it. LT:2726-2728: accepts on `available: None`, no such constructor in the crate (0 grep hits). LRC:6982-6989. S03/selection_call_abi.rs:354-356, 1814-1816. SSG:7282-7287: a literal feeding a branch compare is skipped, safe because `folded_literal` requires no other use (SIN:2988-2999). SST:2257, 5570.
- S04/rewrites.rs:1197, 1692, 1757: any other family error is a decline, program unchanged. S04/literal_fold_rewrites.rs:3508-3524, 3846-3849, 2109-2149 (spill and recovery analyses do not gate folding).
- S05/assignment.rs:1179-1265, 1886, 1940, 1954: nothing is published without a successful `probe_all` (2002-2010). S05/home_assignment.rs:2315-2321, 2503-2527, 807-811. S05/output.rs:297.
- S06/plan.rs:578-583 (checked in stage 07, S07/row_encoding.rs:519, 583); :576.
- S08/validation.rs:352-406: the validator does not re-run the production fixup checks (S08/ordinary.rs:559-777); "a forged pre-layout store with bad placeholders and a candidate copying it would pass this validator; I did not trace who validates the pre-layout encoding".
- PA/callback_entries.rs:114-122; PA/catalog.rs:68-165, 257-267; PA/scopes.rs:114-116; PA/reachability.rs:275-277.

## Silent drops

Places where something that could not be planned or resolved leaves no row and no diagnostic. Counts are distinct sites from the records: 54 in all.

- Stage 00, 12: S00/callback_settlement.rs:106-108, 586-588; S00/settlement.rs:130-137; S00/native_execution_scope.rs:409-411, 412-419, 489-496; S00/retained_terminal.rs:713-716, 932-934, 948-950, 1224-1226; S00/optimization.rs:198-239; S00/lowering.rs:1625.
- Stage 01, 2: S01/ranked_cycles_validation.rs:619-624 (no certificate); S01/validation.rs:2959-2961 (frontier facts left unrewritten, inferred).
- Stage 02, 6: S02/function_signature.rs:228, 546-552; S02/references.rs:1818-1827 (then `commit` removes the descriptor, 1915-1932), 2534-2540, 2235-2237; S02/structural_layout.rs:184-189.
- Stage 03, 15: S03/legalization_descriptor_inputs.rs:81-91; LRC:6637, 3495-3503, 5598-5653, 5790-5796, 6982-6989; LSG:2304-2319; S03/optimized.rs:68-70; S03/selection.rs:1559; SIN:647-649, 2540; SSG:2972-2990, 6777-6789; SST:227-230, 5522-5531.
- Stage 04, 5: S04/rewrites.rs:1197, 1692, 1757 (every refusal reason collapsed to "declined"); S04/literal_fold_rewrites.rs:1261 (action dropped when the rewrite errors), 2267-2292 (no charge).
- Stage 05, 4: S05/assignment.rs:1886 (`Err(_)` after rematerialisation), 1940 (`Err(_) => {}`); S05/rewrites.rs:1405-1421 (pressure and budget mapped to decline); S05/rewrites.rs:654-682 (block skipped).
- Stages 06 to 09 and `products`: none recorded.
- `provider-admission`, 10: PA/provider_admission.rs:437-440, 472; PA/retained.rs:166, 210-217; PA/source_imports.rs:55-57, 175; PA/source_evaluated_bindings.rs:368-381; PA/callback_entries.rs:39-41 (`Ok(None)` for the whole slice); PA/scopes.rs:114-116; PA/reachability.rs:275-277.

Related, not a drop of a row: refusal reasons are lost. Stage 02 has one catch-all error with only a call-site line (S02/model.rs:320-325); stage 03 uses `custody()` almost everywhere (LRC:4151) and `FunctionProjectionMismatch` for all edge-transfer failures (S03/selection_edge_transfers.rs:1606-1608); S04 custody validators report `step: 0` whichever step differed (S04/literal_fold_rewrites.rs:525; S04/rewrites.rs:1963).

## Unused or unwired

With the readers' grep results; mine where I re-ran them.

- S00/artifact_admission.rs:1080 `lower_artifact`: no product caller; 1 example and 1 test-support file (grep of `lower_artifact(|lower_decoded_artifact(`). `AdmittedNativeArtifact` and `into_optimization_artifact`: the same two files.
- S01/pass_manager.rs:398: `run_registries` always passes `None` for external decisions, so the replay branches (433-439, 489-501, 624-627, 797-804) are "suspected dead from the outside" (the `_inner` name was not grepped).
- S01 analyses with no consumer: `PlaceAccessSummaries`, `PlaceAliases`, `ExecutableEdges` (unit 15 grepped `omega\` only). Re-run across all of `omega-rust`: `PlaceAccessSummaries` appears only in S01/analyses.rs and the enum definition; `PlaceAliases` only there and in its own producer file; `ExecutableEdges` also in S01/rules.rs:1146 and two invalidation/import lines in `optimization-unit/semantics/src/candidates.rs:355, 415`. No rule requires any of them. Empty impl blocks at S01/semantic_analyses.rs:1489, 1491 and S01/analyses.rs:48.
- S01/countdown_analyses.rs:1508-1512: `ValidatedCountdownInvariantConstantPlacementAnalysis` has no accessor (suspected: only `Ok`/`Err` is used).
- S02/lib.rs:41: `pub mod access_view_inputs {}` is empty with zero users (re-run: 1 hit for `access_view_inputs::` in the tree, an unrelated Psi path).
- S02/model.rs: 16 `LoweringError` variants with no `LoweringError::<Name>` construction found: `MultipleNativeCallbackArguments`, `MissingNativeCallbackArgument`, `ConditionalControlFlowRequiresBlockLowering`, `ConditionalConditionMustBeBoolean`, `ConditionalArmBindingTypeMismatch`, `HostedExitProcessUnsupportedTarget`, `UnitFunctionNotStraightLine`, `UnitOperationInScalarFunction`, `RelevantOpaqueStructuralField`, `FunctionHasNoReturn`, `OperationAfterReturn`, `FunctionResultMismatch`, `FunctionResultKindMismatch`, `UnsupportedRuntimeBooleanCondition`, `IeeeFloatFmaOperandMismatch`, `InstalledProviderClaimTransferMismatch` (the search did not include `Self::` or glob imports).
- S02/scalar.rs:383, 1836: the `active` cycle guard is never pushed to (0 matches for `active.push|active.pop`).
- LRC:4144 `window_regions`: only a self-call. LRC:6101 `shared_copy_type`: 1 call. LSG:3080: unused `_body` parameter. S03/selection_call_abi.rs:321: a branch that cannot run because :272 handles the same condition (inferred). S03/selection_access_lifecycles.rs:4176, 6602: test-only.
- S04/unexecuted_rewrites.rs:15: empty `pub(crate) mod peepholes {}`; the file is a prose roster of families that "do not yet have an active catalog row" (1-13); 4 grep lines, re-exports only.
- S04/rewrites.rs:59, 71, 104-107: `AllocationRecovery` rows are in the stage-04 catalogue but nothing in the file runs or resolves them.
- S04/literal_fold_rewrites.rs:4282, 4296: `LiteralFoldPlan::encode`/`decode`, 0 path-qualified callers. :967 `EmptySequence`: 1 hit, the definition. :4465 `LiteralFoldError::RootMismatch`: 0 hits.
- S04 single-variant enums whose `match` arms select nothing: S04/copy_and_extension_rewrites.rs:355-374; S04/branch_and_compare_rewrites.rs:295-302, 911-919.
- S05/assignment.rs:2632 `RecoveryNotRequired`: 1 hit, the declaration (re-run: 1). :1356, 1161, 1271, 1326: test-only ranking helpers.
- S05/reanalysis.rs:153-161: five error variants with 0 construction sites. S05/pressure_rematerialization_recovery.rs `RootMismatch`: 0. S05/fixed_view_copy_recovery.rs: `CopyMismatch`, `TransformedPlanMismatch`, `ReceiptMismatch`, `LegacySourceEvidence`, `UnsupportedPolicy`, `UnsupportedSegmentBoundarySet`: 0 each. S05/home_assignment.rs `IntervalOverflow`: 0.
- S05/runtime_spill_and_split.rs:530, 3224, 5460: `ReplayMismatch` in three error enums, 0 construction sites (re-run: the only 3 mentions in the crate are the declarations). `fn (validate_register_homes|replay_register_homes|validate_homes)`: 0 matches (re-run: 0).
- S09/x86_branch_relaxation.rs:74: `X86BranchRelaxationWorkAxis` is `pub` but not re-exported; 7 occurrences, all in the file.
- PR/compilation_products.rs:347: `CompileReport::checked` is `pub` with 0 outside callers (re-run: one `Self::checked(` at :338, nothing else).
- PA/windows_handle_activations.rs and PA/windows_handle_conditions.rs: declared `#[cfg(test)]` (PA/terminal_authority_review.rs:26-32), no non-test users (4 grep hits).

## Stated limits

- S04/unexecuted_rewrites.rs:1-13: rewrite families that "do not yet have an active catalog row", owned by `EXACT-MACHINE-SIMPLIFICATIONS`, `ALIAS-AWARE-MEMORY`, `DECLARATIVE-PEEPHOLES`.
- S02/references.rs:1692-1696, 1753: "stored-reference invocation origin has no physical metadata conversion"; "captured incoming contents have no physical metadata conversion"; "stored-reference literal origin has no physical metadata conversion"; "stored-reference access formation has no physical metadata conversion"; "dynamic call has no stored-reference effect summary".
- S02/coordination.rs:522-533 with S02/model.rs:47: every fused multiply-add is refused (error variant, no comment).
- S00/retained_terminal_codec.rs:1451-1452: "rebuild the component to retain them again".
- S01/specialization.rs:5-7: "Every specialization family excludes machines holding authenticated cyclic components".
- LRC:7155-7156: "Unknown targets still fail at the existing edge lookup below."
- LT:2270-2272, 2253, 2330: "representation view has an unsupported source window or access"; "view source has no matching element-region geometry"; "borrowed collection cannot supply a static inline window".
- SSG:7489: "installed table publication requires the complete guarded physical implementation". S03/selection_interrupts.rs:1354, 211: "gate has no selected stack domain"; "target cannot materialize a private entry address".
- S03/selection_access_lifecycles.rs:145, 6530, 6667: "initialization requires a fixed complete image"; "resident aggregate requires whole or repeated byte placement"; "resident scalar exceeds the native register carrier".
- SIN:5494, 4039: "representation-view pointer crossing lacks an exact source descriptor coordinate"; "thin scalar foreign pointer requires the same byte footprint in every backing".
- S04/literal_fold_rewrites.rs:2843, 2865, 2949, 3018: "the fault discharge holds only for the divisor literal one"; "the obligation discharge holds only for the dividend literal zero"; "the annihilator fold holds only for the literal zero"; "the identity fold holds only for the all-ones literal".
- S04/rewrites.rs:1365-1366: "Ordinary legality refusals decline the site; budget, identity and validator failures abort the run".
- S08/lib.rs:6: "before optional x86-64 short-branch relaxation runs in stage 09". S09/lib.rs:6: validation "without re-running the fixed-point search".
- PR/checked_compilation.rs:13-16: "Measurements, a retained judgment, and invocation-local derived code are not a separate checked semantic identity".
- PA/access_lifecycles.rs:257, 340, 289: "native resident content requires retained loan forwarding for reference fields"; "native lifecycle non-runtime fields require an authored custody agreement"; "native lifecycle narrowing requires checked concrete-value fitting evidence".
- PA/compiler_builtins.rs:216: "unsupported destructive take". PA/host_cohorts.rs:9-11: filesystem release requirements refuse rows "until an occurrence-specific release contract is established". PA/windows_imports.rs:11-19: DuplicateHandle, CloseHandle and CRT reads and writes "stay unclassified".
- PA/static_interrupts.rs:11-17: "A recognized declaration alone never supplies a native implementation, and these rows grant neither root memory nor permission to publish IDTR".
- PA/source_imports.rs:5-9: it "does not rejoin settlement coverage or re-compare targets, locators, and syscall numbers".
- Headers that promise an independent replay which the reader did not find in the file: S05/home_assignment.rs:1-6 and S05/lib.rs:34 (no replay function, receipt built straight from the plan, S05/home_assignment.rs:2634-2639); S05/fixed_view_copy_recovery.rs:1-6 (`seal_validation` only builds the receipt, 1095-1116); S05/preservation.rs:528-573 (the replay re-runs the same `DirectTraversal`, 487); S05/runtime_spill_and_split.rs:492-502, 3174, 5425 (seals are bare `sealed!` calls); S07/lib.rs:45-47 "never calls the producer's route table or encoders" against S07/validation.rs:629-651.

## Disagreements

1. **Wrong file in a citation.** Unit 16 cites the stage-02 descriptor-input guard as `coordination.rs:1550-1578` (twice). `coordination.rs` has 645 lines. The code is at S02/references.rs:1550-1578. The claim itself (same checks as S03/legalization_descriptor_inputs.rs:94-154) holds.
2. **Where `selected_boundary_calls::resolve` lives.** Unit 16 read S02/lib.rs:517-701, found only `index`, and wrote that `resolve` "must be defined elsewhere in the crate (not in my unit)". Unit 15 places it at S02/control_flow_calls.rs:2021. Both are right: there are two modules with that name, S02/lib.rs:517 and S02/control_flow_calls.rs:2011.
3. **Does legalisation run the audit?** Unit 17's summary describes three layers (LSG validates through `match_input`, LRC models custody, LSrc projects) as if all run together, and asks whether `validate_source_custody` running at LSrc:3077 and S03/legalization.rs:488 is redundant. Units 16 and 18 say `legalize_target_operations` does not call the audit. The code agrees with 16 and 18: `legalize_target_operations` (S03/legalization.rs:405-432) calls the installed-interrupt check, `validate_boundary_origins` and `derive_source_function_rosters` only; `match_input` is called only from `audit_target_operation_plan` (:509), which is called only from `verify_target_operation_plan` (:480), whose one external caller is `backend/images/image-emission/src/function_fragments.rs:3878` inside `mod replay` (:3851). So the two `validate_source_custody` calls are on separate paths, and the reference-custody walk (`block_entry_states`, one call at LT:1244) is reached only through that audit. Whether the image-emission replay runs in a default compile was not traced by any record or by me.
4. **Second pass against first pass, stage 04 macros.** Unit 20 wrote that `validated_transformed_receipt!`, `rewrite_receipt!` and `rewrite_receipt_identity!` are "defined in `rewrites.rs`, not read". gap_008 read `rewrites.rs` and found only `declared_kind_matchers!` and `declared_pair_lookup!` at crate level. The code agrees with gap_008: the three receipt macros are in `selected-instructions/analyses/src/lib.rs:267, 316, 353` and `sealed!` in `…/analyses/src/rewriting.rs:396`.
5. **An open concern the code closes.** gap_006 left open whether `reaching_events` can return an empty set and so let a compare be removed. S04/rewrites.rs:442-446 returns `Err(ConditionStateError::Use)` when the set is empty or contains the entry's unknown event. gap_008 read the function but did not say this.
6. **A shared unknown the code answers.** gap_006 and gap_007 both left open whether `block_instructions` includes the terminator (it decides whether flag readers in a block's own branch are seen). `selected-instructions/analyses/src/rewriting.rs:41-46` chains `terminator_instruction(&block.terminator)` after the body instructions. So terminator readers are seen; the "live out of the function by another route" part stays as the readers left it.
7. **`ExecutableEdges` mentions.** Unit 15 says the kind "appears only in `analyses.rs:74` and as an invalidation entry at `rules.rs:1146`". There are two more mentions in `optimization-unit/semantics/src/candidates.rs:355, 415`. The conclusion (no rule requires it) is unchanged.
8. **Count of `scalar_shape` definitions.** Unit 20 lists four definitions as a suspected four-way duplicate. Two of them are one-line forwards to the same function (see job 4).

No other contradictions were found between records. Unit 17's notes contain superseded sections; its "Final summary" was used.

## What I checked myself

All by opening the cited lines in the checkout or re-running a grep across `omega-rust`. 35 checks.

| # | Claim | Where | Result |
|---|---|---|---|
| 1 | `legalize_target_operations` does not call the audit | S03/legalization.rs:405-432, 466-518 | matches |
| 2 | `verify_target_operation_plan` has one external caller | function_fragments.rs:3878 (grep, 1 call) | matches |
| 3 | Unreached block is dominated by everything | LT:806-812 | matches |
| 4 | Entry predicate is true when there is no result | S03/selection_call_abi.rs:1428-1430 | matches |
| 5 | Relaxation validator checks roots and budget only | S09/x86_branch_relaxation.rs:217-234 | matches |
| 6 | Liveness follows only `Jump` and `Conditional` | S01/semantic_analyses.rs:2241-2249 | matches |
| 7 | CFG knows four terminator kinds, `_ => {}` | S01/control_flow_analyses.rs:303-323 | matches |
| 8 | Empty `access_view_inputs` module, no users | S02/lib.rs:41; grep | matches |
| 9 | `RecoveryNotRequired` never constructed | grep: 1 hit | matches |
| 10 | `ReplayMismatch` never constructed in stage 05 | grep: 3 declarations only | matches |
| 11 | No home replay function exists | grep: 0 | matches |
| 12 | `access_covers` in stage 02 and stage 03 is the same job (each side suspected) | LRC:4582-4590; S02/references.rs:888-896 | same job, identical text |
| 13 | `completed_descriptor`/`descriptor_result` three ways (suspected) | S02/scalar.rs:655-703; LSG:2227-2265; SIN:2878-2937 | partly: same core test, second halves differ |
| 14 | `runtime_borrowed_projection` two ways (each side suspected) | S02/structural_layout.rs:1076-1103; S03/structural_inputs.rs:2061-2088 | same job, separately written |
| 15 | `scalar_shape` four definitions (suspected) | S02/scalar.rs:2163; S03/structural_inputs.rs:72; S03/selection_call_abi.rs:1964; LSG:8601 | partly: three implementations, one alias |
| 16 | `unique_row` re-defined in stage 03 (suspected) | S00/lowering.rs:1273; LSG:8676 | same idiom, different result on none or many |
| 17 | `access_call_custody` twin (suspected) | S02/references.rs:702; LRC:7495 | signatures only: same output type, two stages |
| 18 | Guard closure at `coordination.rs:1550-1578` | file has 645 lines | does not match: it is S02/references.rs:1550-1578 |
| 19 | Guard pair is the same job | S02/references.rs:1550-1578; S03/legalization_descriptor_inputs.rs:108-153 | matches |
| 20 | `resolve` not in S02/lib.rs | two `mod selected_boundary_calls` (lib.rs:517, control_flow_calls.rs:2011) | matches both records |
| 21 | Does `block_instructions` include the terminator (open in two records) | rewriting.rs:41-46 | yes |
| 22 | Can `reaching_events` return empty (open) | S04/rewrites.rs:442-446 | no: empty is an error |
| 23 | Receipt macros defined in S04/rewrites.rs (first pass) | grep of `macro_rules!` | does not match: defined in `selected-instructions/analyses` |
| 24 | Three analyses have no consumer | grep across `omega-rust` | matches; two extra `ExecutableEdges` mentions, neither a consumer |
| 25 | Pass identities compared as byte-string literals | S01/pass_manager.rs:224-260 | matches |
| 26 | Stage 07 validator calls an encoder the producer also calls | S07/validation.rs:629-651; S07/row_encoding.rs:661, 670; header S07/lib.rs:45-47 | matches |
| 27 | Two Windows-handle modules are test-only | PA/terminal_authority_review.rs:26-32 | matches |
| 28 | Three AArch64 dispositions are sized but have no `resolve_row` arm | S08/ordinary.rs:349-366, 528 | matches |
| 29 | Liveness table reuse guarded only by `debug_assert_eq!` | S06/optimization.rs:335-344 | matches |
| 30 | Integer comparison with no row is skipped, float errors | S00/settlement.rs:130-137 | matches |
| 31 | Boundary with no candidates returns `Ok(())` | S00/native_execution_scope.rs:409-419 | matches |
| 32 | `uefi_x64()` in three stages | grep in `pipeline/`: 3 hits | matches |
| 33 | `adopt_derived_code` guarded only by `debug_assert!`; `admit_trust` early `Ok`; `CompileReport::checked` has no outside caller | PR/prepared_build.rs:126-131; PR/checked_compilation.rs:161-165; grep | matches (all three) |
| 34 | Reached-provider check skips with `?` in `filter_map` | PA/provider_admission.rs:436-440 | matches |
| 35 | One-sided `check_current`; schema comparison only `if declared` | PA/admission_source.rs:209-217; PA/settlements.rs:290-295 | matches (both) |

Corrections made to the digest from these checks: the stage-02 guard is cited at `references.rs`; the two open stage-04 entries that depended on `reaching_events` and `block_instructions` carry the answer; `scalar_shape` is recorded as three implementations; the audit path is stated as in Disagreement 3.

## Not covered

- Records: all 12 were read in full. None skipped.
- Source ranges: every `.rs` file in the ten pipeline crates and in `products` is listed in a unit table and has a record that says it was read in full (unit 020's four unread files are covered by gap_006, gap_007, gap_008). I compared the crate file lists against all unit tables: no file is missing.
- `provider-admission`: 12 of its 44 files (10,744 of 23,928 lines) belong to unit 023, which is not one of this area's records: `windows_handle_contracts.rs`, `windows_handle_inputs.rs`, `windows_handle_lifetimes.rs`, `windows_handle_origins.rs`, `windows_handle_paths.rs`, `windows_handle_replay.rs` (2,825), `windows_handle_scalar_table.rs`, `windows_handle_storage.rs` (2,115), `windows_handle_values.rs`, `execution.rs` (1,414), `provider_installation.rs`, `replay.rs`. Nothing in this digest describes them.
- Depth limits the readers stated: unit 18 read `validate_operation` (LT:5321-6866) and the borrowed-argument chain (S03/selection_call_abi.rs:29-476) "for structure, not verified arm by arm"; unit 21 read `PreparedConflicts::new`, bitmask placement, `slot::assign`, `shareable` and `derive_runtime_spill` in order "but its correctness was not independently checked"; unit 14 summarised wire-tag lists by count.
- Crates outside this area that these stages lean on and no record here read: `optimization_unit` and `optimization_unit_semantics` (where stage-01 candidates are applied and validated), `selected-instructions` and its `analyses` (machine semantics, `sealed!`, receipts), `calling_conventions`, `terminal_semantics`, the ISA encoders and decoders, `machine_code`.
- Pairs I listed as the same job without opening both sides: `byte_carrier_leaf_copy` (three stages), `scalar_arrays::shape` (three sites), `leaf_copy_projection` against `runtime_leaf_copy_projection`, `leaf_type` (four sites), and the out-of-area `access_covers` copies.
