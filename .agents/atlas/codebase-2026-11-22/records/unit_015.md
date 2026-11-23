# Record unit_015 (area 10-native-stages-04-10)

All paths relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/`. Stage 06 = `06_selected-instructions-to-selected-instructions/src/selected_rewrites/`; stage 07 = `07_selected-instructions-to-register-homes/src/allocation/`; stage 08 = `08_register-homes-to-post-allocation-machine/src/machine/`. Everything below was read unless marked otherwise. "S06/S07/S08" = stage 06/07/08.

NOT READ: `runtime_spill_and_split.rs` lines 541-5493 (see Coverage). Entries for that file cover lines 1-540 only, plus a grep outline of its module layout.

---

## S06 `rewrites.rs` (2818 lines, read in full)

1. **Purpose.** One file holding the whole stage-06 rewrite harness. It names the catalog of pre-allocation rewrite rules, runs them to a fixed point over the selected-instruction CFG, and writes a hash-bound receipt for the run. Individual rewrites (copy removal, address fold, compare folds, load forwarding) live in sibling files and are only re-exported here (`rewrites.rs:42,458-464,2632-2633,2716-2791`).
2. **Shape.**
   - `catalog` module (`:43`): `SELECTED_STAGE_RULE_CATALOG` (3 slices: SelectedLowering, PreAllocation, AllocationRecovery, `:95`), `selected_stage_rule_rows` (`:110`).
   - `condition_state` (`:126`): flag-reachability helpers: `materialized_bits` (`:160`), `constant_operands` (`:214`), `reaching_events` (`:386`, a worklist over a backward cone).
   - `place_storage` (`:465`): `local_slot_is_place_storage` (`:485`), `constant_index` (`:597`), `extent_reached_by` (`:576`).
   - `pre_allocation::catalog` (`:650`): `PRE_ALLOCATION_RULE_CATALOG: [_; 10]` (`:695`).
   - `pre_allocation::execution` (`:785`): `CandidateFrontier` (`:1012`), `run_passes` (`:1771`), `execute_pre_allocation_optimizations` (`:1899`), `validate_pre_allocation_optimization_custody` (`:1936`), `pre_allocation_custody_receipt` (`:1989`), `pre_allocation_completion_identity` (`:2073`).
   - `pre_allocation::model` (`:2156`): `ValidatedPreAllocationTransformation` (10 variants, `:2208`), `StagedPreAllocationOptimizationRun` (`:2351`), `OptimizedPreAllocationCustodyError` (`:2522`).
   - `window_hazards::unrostered_reach` (`:2646`), `required_stage_phase` (`:2799`).
3. **Inputs/outputs.** Input is a `StagedOptimizedAllocationLegality` (selected plan + liveness + ranges + legality). Output is a `StagedPreAllocationOptimizationRun`: a list of `StagedOptimizedPreAllocationStep`s (each carries the transformed plan and freshly re-analysed liveness/ranges/legality, `:1610-1640`), a terminal attempt record, and a custody receipt.
4. **How it decides.**
   - A rule's candidate sites are chosen by a predicate on `SelectedInstructionKind` (enum variant): `is_copy_i64` (`:1205`), `is_extension_candidate` (`:1218`), `is_fold_consumer` (`:1230`), `is_boolean_reader` (`:1242`), `is_branch_reader` (`:1253`), `is_compare_publisher` (`:1262`), `is_call_consumer` (`:1311`), `is_load_candidate` (`:1209`). Keyed on instruction kind, not on spelling.
   - Whether a site is rewritten is decided by the sibling `remove_*`/`fold_*` functions (not in this unit). Here the harness only classifies results: `retryable_candidate_error!` (`:1156`) treats `WorkBudgetExceeded | IdentityOverflow | ReplayMismatch` as hard errors; every other `Err` is treated as "declined" (`:1199`, `:1694`, `:1759`).
   - Progress is keyed on a numeric `measure()` (`:1315`) that must strictly decrease after every applied rewrite (`:1840`), else `PreAllocationMeasureMismatch`.
   - Frontier staleness is keyed on a per-function revision counter (`:1022,1066-1075`).
   - `unrostered_reach` classifies by instruction-kind list: Private / Store / `_ => Unaccounted` (`:2646-2712`).
5. **Cases by shape.**
   - `is_fold_consumer` only admits `Store` of `byte_size` 1, 2, 4 or 8 (`:1235-1237`).
   - `copied_call_operand_candidate` only looks at `CopyI64` producers of Use operands (`:1733`).
   - `committed()` accepts a commit that removed at most one body instruction: `.filter(|removed| *removed <= 1)` (`:1092-1095`).
   - `resolved_sequence_position` handles byte-sequence and indexed-primitive/structural roles, else `None` (`:536-573`).
   - `constant_operands` handles only `CompareI64`, `CompareI64Immediate`, `CompareI64Zero` (`:218-248`).
6. **Same job elsewhere.**
   - Receipt/identity sealing here (`pre_allocation_completion_identity`, `:2073`) parallels `post_allocation_manifest::reconstruction::expected_record` (`assignment.rs:383`) and S07 `receipt::build` (`home_assignment.rs:2546`); each hashes its own receipt fields separately (read).
   - `transformation_accessors!` (`:2186`) and the `project!`/`forward!` macros (`:2248,2290`) repeat the same ten-variant list; the variant list is also repeated in `encode_transformations!` tags 1-10 (`:2121-2124`) and in `PostAllocationSelectedTransformation` tag numbering (`assignment.rs:394-447`, grepped by reading).
   - `measure()`'s per-kind loop duplicates the predicates used by the catalog admission (`:1324-1342` vs catalog rows).
7. **Unused/one-caller.**
   - `declared_kind_matchers!` / `declared_pair_lookup!` (`:10,24`): Grep `declared_kind_matchers!|declared_pair_lookup!|matches_consumer|matches_producer` over `source-rust/` = 26 occurrences in 5 files, 2 of them in this file (the two macro definitions), so used by sibling files (4, 6, 8 per file; per-name split not counted).
   - `unrostered_reach` / `UnrosteredReach`: 10 occurrences in 2 files (5 here, 5 in `address_and_load_rewrites.rs`). One external consumer file.
8. **Stated limits.** None in this file with "unsupported" wording. `unexecuted_rewrites.rs` is the stated-limits holder (below).
9. **Fail-open risks.**
   - `Err(_) => Ok(CandidateAttempt::clean(1, 1, charged))` (`:1199`, `:1694`, `:1759`): any non-retryable error from a rewrite validator means "decline, keep original". This keeps (does not apply) the rewrite; a wrong program cannot get in through it, it only skips an optimisation. Classification: traced (declines are conservative).
   - `unrostered_reach` `_ => Unaccounted` (`:2711`) is the safe default. None seen as accept.
   - `condition_state::reaching_events` returns `Err(Use)` if `Unknown` reaches (`:444-448`). Conservative.
   - `local_slot_is_place_storage` returns `false` for unlisted slot kinds (`:509-515`). Conservative.

---

## S06 `selected_optimization.rs` (334 lines, read in full)

1. **Purpose.** The stage-06 entry function `optimize_selected_instructions` (`:191`). It builds liveness and ranges, runs at most one of the two executable phases (SelectedLowering or PreAllocation), and hands stage 07 a `SelectedAllocationInput`.
2. **Shape.** `SelectedInstructionOptimizationOutput` (`:32`, fields `body`, `allocation`, `run: Option<CompletedRun>`), `CompletedRun` (`:74`), `SelectedOptimizationReplay` (`:83`) with `validate` (`:100`), `RewrittenProgram::restage` (`:274`), `executed_slice_phases` (`:312`), `unexecutable_catalog_composition` (`:323`).
3. **I/O.** In: `instruction_selection::StagedOptimizedSelectedInstructions`. Out: output struct with allocation input and optional replay run.
4. **How it decides.** Which phase runs is keyed on which `OptimizationExecutionPhase`s have non-empty selections (`:211,221-228`). Two executable phases selected together returns `UnsupportedComposition` (`:226`). A non-empty AllocationRecovery selection alongside executed phases is also `UnsupportedComposition` (`:218,323-334`).
5. **Cases by shape.** Exactly zero or one executable phase; `_ => return Err(UnsupportedComposition)` (`:243`). Allocator availability fixed to `FramelessLeafCallerSavedViews` for this stage (`:231`).
6. **Same job elsewhere.** `validate` replays by re-running the producer (comment `:96-99`): "establishes determinism and custody, not independent correctness". Same non-independent replay pattern in S07 `publication_validation::validate` (read).
7. **Unused.** `into_allocation_input` / `retain_replay_for_validation` are called from S07 `register_allocation.rs:53-55` (read, 2 call sites).
8. **Stated limits.** `:9-14`: "The default compile trusts the run it just produced and does not replay it". `:96-99` as above.
9. **Fail-open risks.** `retain_replay_for_validation` returns `None` for an identity run (`:68`), so no replay is possible when no phase ran. This is stated (`:62`: "An identity run has no replay"). Classification: stated.

---

## S06 `unexecuted_rewrites.rs` (15 lines, read in full)

1. **Purpose.** A comment-only roster of rewrite families with no catalog row; the only code is `pub(crate) mod peepholes {}` (`:15`).
2. **Shape/I/O.** Empty module. Comment lists `arm_relocation`, `boundary_boolean`, `boundary_branch`, `commuting_relocation`, `interchange`, `confluence_relocation`, `inflow_relocation`, `scheduled_relocation`, `relocation` (owned by `EXACT-MACHINE-SIMPLIFICATIONS`), `dead_store`, `store_motion` (`ALIAS-AWARE-MEMORY`), `peepholes` (`DECLARATIVE-PEEPHOLES`) (`:3-12`).
7. **Unused.** `peepholes`: Grep over `source-rust/` = 2 occurrences, both in this file (comment and the empty module). The empty module has no users.
8. **Stated limits.** "do not yet have an active catalog row" (`:3`).
9. **Fail-open.** None seen.

---

## S07 `assignment.rs` (2681 lines, read in full)

1. **Purpose.** Staging for register-home assignment: baseline custody records, the three allocation procedures (plain, fixed-view-copy, pressure-rematerialization), the runtime-spill/split recovery loop, the post-allocation manifest, and a union-find of "same home" register groups.
2. **Shape.**
   - `baseline` (`:91`): `stage_register_homes_with_assignment` (`:106`), `StagedOptimizedRegisterHomes` (`:239`), `StagedOptimizedRegisterHomesAfterFixedViewCopies` (`:313`).
   - `post_allocation_manifest` (`:364`): `reconstruction::expected_record` (`:383`), `project_post_allocation_optimization_manifest` (`:555`).
   - `recovery` (`:587`): `assign_with_runtime_recovery` (`:610`), `fixed_view_allocation` (`:651`), `pressure_rematerialization_allocation` (`:693`).
   - `runtime_spill` (`:728`): `recovery::recover_over` (`:1671`), `FailedDomain` (`:979`), `BatchFunctionIndex` (`:895`), `FunctionRoster` (`:1385`), `RuntimeSpillSource` (`:2392`), `RuntimeSpillStepRewrite` (`:2487`), `RuntimeSpillAllocationError` (`:2543`).
   - `same_home::SameHomeComponents` (`:2561`).
3. **I/O.** Reads `StagedOptimizedAllocationLegality` (or reanalysis/pressure stages). Produces `StagedAllocation` variants (see `output.rs`) with homes plus manifest.
4. **How it decides.**
   - Which error triggers recovery is keyed on the error enum variant via `RegisterHomeError::admits_runtime_recovery` (`home_assignment.rs:2741`): only `NoCompatibleHome`, `NoCommonCandidate`, `UnresolvedEntryTransitions` (read).
   - Victim choice in `recover_over`: failure → `FailedDomain::of_indexed` (`:1001`) → `select_roster_candidate` (`:1456`), keyed on virtual register ids, `direct_relief` count, then `live_points_of` (sum of fragment lengths, `:1304`), then roster position. Order of attempts: split (`admissible_split_indexed`, `:1505`), before-fixed-use split (`:1784`), rematerialize (`:1879`), spill with crossing status (`:1890`), plain spill (`:1947`).
   - `admissible_split_indexed` pre-filters registers lacking a fixed definition via `has_fixed_definition` (`:1540-1544`) when the target plan's target equals the environment's and a `copy_i64` constraint exists (`:1522-1525`).
   - `inadmissible` error filter: `UnsupportedValue | UnsupportedUse | UnsupportedControlFlow` → try next candidate (`:2027-2034`); other errors abort.
   - `is_recovery_candidate` (`:875`): origin in `InstructionResult | BlockParameter | StructuralObservation | AbiTransport` or entry register.
   - Transformation-ledger uniqueness is keyed on a (tag, identity bytes) pair with tags 1-17 (`:394-448`); a duplicate gives `NonCanonicalTransformationLedger`.
5. **Cases by shape.**
   - `fixed_view_allocation`: probe special case only for `SharedEntryAfterCompareBeforeBranchV1` (`:655`); a capacity decline there skips straight to runtime spill (`:657-668`).
   - Spill spans: `RuntimeSpillSpanPolicy::UnitWriteCrossing` tried only when `pending_pressure.is_empty()` (`:1889-1896`).
   - `reuse_for` only reuses a crossing probe if its receipt and transformed plan equal the bounded spill (`:853-855`).
6. **Same job elsewhere.**
   - `SameHomeComponents::from_ranges` (`:2572`) and `tied_components` in `home_assignment.rs:171` both group registers by tied pairs and edge transfers; `home_assignment.rs:229` calls `SameHomeComponents::from_ranges`, but it also re-derives group membership (`grouped`, `:231-250`). Read, both sites cited.
   - `FailedDomain::in_function` (`:1111`) and `in_function_indexed` (`:1041`) compute the same pressure sets, one from a prebuilt `BatchFunctionIndex`, one by scanning `ranges.interference`. Both are live; `in_function` is used for non-indexed failures (`:1031`) and tests.
   - `candidate_position` (`:1356`) is `#[cfg(test)]` and duplicates `select_roster_candidate` ranking for tests.
7. **Unused/one-caller.**
   - `RuntimeSpillAllocationError::RecoveryNotRequired` (`:2553`): Grep `RecoveryNotRequired` not run (not counted). Mark "suspected unused".
   - `FailedDomain::relief` / `direct_relief` / `spill_priority` are `#[cfg(test)]` (`:1152,1271,1326`).
   - `StagedOptimizedRegisterHomes` etc. are `pub(crate)`; the three `recover_after_*` entry points are used from `recovery` (read).
8. **Stated limits.** `:7-8`: "no procedure branches on it" (about Omega 04 history). `:588-593`: "Each procedure falls back to runtime spilling when register pressure remains".
9. **Fail-open risks.**
   - `candidate` loop: `Err(error) if inadmissible(&error) => continue` (`:1955`) restarts the loop without removing the victim a second time (roster already `remove_register`d at `:1876`); no accept path. Classification: traced; it shrinks the roster, so it terminates.
   - `loop { ... let Some(...) = ... else { break; } }` falls out of the loop to `Err(RuntimeSpillAllocationError::Homes(failure))` (`:2024`). It refuses.
   - `FailedDomain::of_indexed` returns `None` for other error variants (`:1021`), and the loop then `break`s to the error. Refuses.
   - `reuse_for` returns `None` when mismatched (`:853`), forcing recompute. Safe.
   - `PostAllocationSpillStatus` is `RealizedInSelectedProgram` iff any `RuntimeSpill` transformation appears (`:514-523`); otherwise `NotRequiredForValidatedHomePlan`. A manifest label, derived not checked.

---

## S07 `fixed_view_copy_recovery.rs` (1461 lines, read in full)

1. **Purpose.** When a value changes physical register view at a fixed-register use (for example the value is in `rbx` but a call wants `rdi`), insert an explicit `CopyI64` before the use and rewrite the use to read the copy.
2. **Shape.** `fixed_view_copy::compute::compute_terminal_fixed_view_copies` (`:622`), `preflight::{validate_roots, copy_row, work_usage, next_instruction_id, next_register_id, find_leaf_block, find_site_block}` (`:70-287`), `site::build_site_copies` (`:388`), `source_exit::build_source_exit_copies` (`:483`), `emission::copy_emissions` (`:731`), `evidence::compute::derive` (`:817`), `work::apply_fixed_view_copy` (`:1148`), `materialize_fixed_view_copies` (`:1300`), `FixedViewCopyError` (`:1379`).
3. **I/O.** In: selected program, ranges, legality, fixed intervals, split requirements, segment homes, register env pieces, `FixedViewCopyPolicy`, budget. Out: `MaterializedFixedViewCopies { plan, receipt }`; the plan contains a cloned selected plan with copies inserted and consumers rebound.
4. **How it decides.**
   - Which uses get copies: derived from `FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1` boundaries (`:926-962`), keyed on segment ids and allocation-domain inequality (`:941-945`).
   - Grouping boundaries into one shared copy: `copy_emissions` groups by `(function, virtual_register, source_segment, source_domain, from_view, to_view)` (`:736-744`) and shares only if all members have the same incoming connector `source` and `terminator` (`:757-766`).
   - Source admission: register id equality, class equality, optional `entry_fixed_view` equality when leaf-local (`:319-324`); scalar type must be 64-bit integer (`is_i64`, `:47-52`, `:344`).
5. **Cases by shape.**
   - `admitted_source` origin match: `EntryParameter` always; `InstructionResult | BlockParameter | ScalarAbiAddress` only when not leaf-local; else `UnsupportedSourceRegister` (`:329-343`).
   - `find_leaf_block` requires the destination to be a `Return` terminator in a non-entry block (`:211-225`).
   - Site must be an `Operand` with `Use` access, else `UnsupportedTransitionSite` (`:402-413`).
   - Four policies: `LeafLocalBeforeFixedUseV1`, `ImmediateBeforeFixedUseV1`, `SharedEntryAfterCompareBeforeBranchV1`, `SharedSourceExitBeforeFixedUseV1` (`:140-149`); the shared-entry policy requires a shared exit connector (`:498-502`).
6. **Same job elsewhere.** `next_instruction_id` (`:159`), `next_register_id` (`:186`) duplicate `runtime_spill::next_instruction_id`/`next_register_id` (`assignment.rs`, `runtime_spill_and_split.rs:3354,3362` per grep outline, not read) and `runtime_split::next_instruction/next_register` (`:5222,5228`). Suspected duplication; the runtime_spill bodies were not read.
7. **Unused.** `FixedViewCopyError::{LegacySourceEvidence, UnsupportedPolicy, UnsupportedSegmentBoundarySet, CopyMismatch, TransformedPlanMismatch, ReceiptMismatch}` (`:1389-1427`): grep not run; `Revalidation`-style variants in other error enums likewise. Mark "suspected unused".
8. **Stated limits.** `:7-9`: "Nothing replays the transformed CFG ... records no second check."
9. **Fail-open risks.** `copy_emissions` falls back to one-copy-per-member when connectors differ (`:777-781`); this is conservative. `apply_fixed_view_copy` with `require_use_access = false` (called from `apply_copy`, `:38-45`) does not verify access kind in that path; the callers (`build_site_copies`, `build_source_exit_copies`) have already required `Use` access in the match, so a non-Use site never reaches it. Classification: traced.

---

## S07 `home_assignment.rs` (2754 lines, read in full)

1. **Purpose.** The solver that gives every virtual register (or tied group) a physical register view, without spills. It is a most-constrained-first greedy colouring over interference, early-clobber and write-unit conflicts, with copy-affinity preferences.
2. **Shape.** `compute::domain::build_domains` (`:88`), `AllocationDomain` (`:47`), `compute::placement::compute_function` (`:655`), `compute_function_bitmask` (`:1102`), `place_packed` (`:1168`), `preferred_view` (`:796`), `preferred_view_mask` (`:1268`), `PackedSelectionTree` (`:1057`), `prepared_affinities::PreparedAffinities` (`:1560`), `prepared_conflicts::PreparedConflicts` (`:1666`), `FunctionHomeReuse` (`:2293`), `compute_terminal_register_homes` (`:2381`), `physical_requirement::validate` (`:2496`), `receipt::build` (`:2546`), `assign_register_homes_reusing` (`:2635`), `RegisterHomeError` (`:2685`).
3. **I/O.** In: `ValidatedAllocationLegality`, `ValidatedLiveRanges`, `ValidatedTargetRegisterEnvironment`, optional reuse cache. Out: `AssignedRegisterHomes { plan: Arc<RegisterHomePlan>, receipt }`.
4. **How it decides.**
   - Candidate views for a register: intersection of per-span candidate lists and early-clobber points (`common_candidates_repr`, `:439`; `intersect_candidates_repr`, `:503`). Empty with all-nonempty inputs gives `NoCompatibleHome`; otherwise `NoCommonCandidate` (`:480-498`).
   - Selection order key: `(viable count, Reverse(degree), first_point, leader vreg, domain_index)` (`SelectionKey`, `:980`), packed into `u64` or `u128` (`:1032,1049`).
   - View choice: avoid "stranding" a neighbour (`strands_neighbor`, `:939`), then prefer views matching already-assigned affinity partners, then partner votes, then fewest "stolen" (`:842-873` for the vector path, `:1268-1437` for the mask path). Final fallback is the lowest view id (`pool.first()` / `trailing_zeros`).
   - Tied/edge-transfer groups: `tied_components` (`:171`) rejects groups whose members interfere (`TiedRegistersInterfere`, `:306`), converting to `UnsupportedEdgeTransfer` when an edge transfer parameter is involved (`:296-305`).
   - Conflict relation between two views is keyed on physical unit overlap: `footprints_overlap`, `definition_overwrites_use` (write_units vs units) (`:2105-2108,2269`).
5. **Cases by shape.**
   - Bitmask fast path requires all views < 128 and all unit ids < 128 (`:1938-2041`); else a `Vec`-based path.
   - Packed `u64` keys require `domains.len() <= u16::MAX`, strictly increasing leaders, `first_point < 2^24`, degrees ≤ `u16::MAX` (`:1118-1122`); else `u128` keys.
   - `physical_matrix` is `view_count * view_count` bytes (`:2023`), so memory is quadratic in the number of views.
6. **Same job elsewhere.** Vector path (`preferred_view`, `partner_outlooks`, `:796-937`) and mask path (`preferred_view_mask`, `narrow_by_partner_votes_mask`, `:1268-1549`) implement the same preference ranking twice; grep `compute_function_bitmask|preferred_view\b|preferred_view_mask` in S07 = 4 occurrences, all in this file.
7. **Unused.** `RegisterHomeError::IntervalOverflow` (`:2703`): not grepped; "suspected unused" (no construction seen in the 2754 lines read). `uniform_physical_conflicts` etc. used in the mask path.
8. **Stated limits.** `:1-12`: "Nothing replays the resulting `RegisterHomePlan` ... is not evidence that a second check accepted the homes."
9. **Fail-open risks.**
   - `homes[...].expect("active register has assigned home")` (`:789`): every register with spans must have been assigned; if a domain failed under the `pressure` collector the function returns the first pressure error at `:770-772`/`:1262-1264` before `assemble_homes`, so no unassigned home is expected. Classification: traced.
   - `has_active_partner` etc. choose heuristically; no correctness consequence.
   - `FunctionHomeReuse::compute` reuses a prior result if legality and ranges are equal by `Arc::ptr_eq` or by value (`:2322-2328`). A stale hit needs equal inputs. Classification: traced.
   - `validate` accepts a register with empty spans if it appears in no relation (`:2510-2534`): such a register gets no home and is skipped by `assemble_homes` (`:785`). A register with no live points that is actually referenced in `ranges` relations errors `NoLivePoints`. Classification: traced.

---

## S07 `mod.rs` (249 lines, read in full)

1. **Purpose.** Crate-internal module root and doc comment for Omega 05; macros `receipt_accessor_rows!` (`:44`), `copy_receipt!` (`:56`), `stage_error_display!` (`:74`); the sealed trait `ValidatedSelectedAnalysis` (`:104-192`); re-exports.
4. **How it decides.** The sealed-trait list (`:118-192`) restricts which transformed-program types may be rewritten again: `SelectedProgram`, `OwnedSelectedProgram`, `SelectedProgramRef`, `ValidatedRuntimeSpill/Split/Rematerialization`, `ValidatedPressureRematerialization`, `ValidatedFixedOccurrences`, `MaterializedFixedViewCopies`.
6. **Same job elsewhere.** `copy_receipt!`/`receipt_accessor_rows!` also appear in S06 (`rewrites.rs:89,672,2329`). The macros are redefined per crate (suspected; S06 macros come from `selected_instruction_analyses` via `#[macro_use]`, not read).
7. **Unused.** `ValidatedSelectedAnalysis` is `pub` and used by `assignment.rs`, `rewrites.rs`, `pressure_rematerialization_recovery.rs` (read, at least 4 files).
8. **Stated limits.** Doc `:19-25`: "every post-rewrite graph is re-analyzed through fresh liveness, live-range, and legality checks before its homes are accepted." Compare `output.rs:3-4` ("without replaying them"): the claim at `mod.rs:23` is about re-analysis in the recovery loop (`assignment.rs:1838,1902,1986`), not about a re-check at publication.
9. **Fail-open.** None seen.

---

## S07 `output.rs` (406 lines, read in full)

1. **Purpose.** Projects whichever allocation procedure ran (`StagedAllocation`, `:316`) into one borrowed `AllocationOutput` (`:335`) and admits it as an owned `RetainedAllocation` (`retained::RetainedAllocation::admit`, `:236`).
2. **Shape.** `CurrentAllocation::capture` (`:83`), `ProjectAllocation` trait (`:310`), `AllocationEvidence` enum (4 variants, `:393`), `AllocationAdmissionError` (`:401`).
3. **I/O.** `admit` computes callee-saved requirements once (`:249-254`) and machine effects (`:87-88`), then wraps everything in `Arc<CurrentAllocation>`.
4. **How it decides.** Nothing selective; it only fails when `stage_allocated_callee_saved_requirements` or `analyze_machine_effects` fail.
5. **Cases by shape.** Exactly four `StagedAllocation` variants (`:316-321`).
6. **Same job elsewhere.** `allocation_output!` macro (`:9`) is used for three of the four; `RuntimeSpillAllocation::project_allocation` (`:272`) builds the struct by hand.
7. **Unused.** `RetainedAllocation::validate_publication` (`:228`) is the only route to `publication_validation::validate`; its callers outside this unit were not searched ("suspected" validator-only).
8. **Stated limits.** `:3-4`: "The handoff retains the producer's manifest and decisions without replaying them."
9. **Fail-open.** None seen.

---

## S07 `preservation.rs` (172 lines, read in full)

1. **Purpose.** Computes which callee-saved registers the allocated function bodies write, by intersecting write units, implicit defs and clobbers with the ABI's callee-saved list.
2. **Shape.** `compute::derive` (`:41`), `stage_allocated_callee_saved_requirements` (`:124`), `ValidatedAllocatedCalleeSavedRequirements` (`:145`).
3. **I/O.** In: `AllocationOutput`, policy enum, budget. Out: `AllocatedCalleeSavedRequirementPlan` sealed with an identity.
4. **How it decides.** Only policy `AllocatedSelectedWritesIntersectAbiPreservationV1` is accepted (`:46-50`). Root check compares four identities (`:55-61`). Budget check is `usage.within(ceiling)` (`:78`).
5. **Cases by shape.** One policy; `selected.len() != homes.len()` → `FunctionRosterMismatch` (`:17`).
6. **Same job elsewhere.** The traversal itself is in `selected_instruction_analyses::register_writes` (not read).
7. **Unused.** Called from `output.rs:249` (1 site in this unit).
8. **Stated limits.** None.
9. **Fail-open.** None seen. Header says "independently replay" (`:2`), but no replay function exists in this file (only `compute::derive` and `custody::seal`). Noted as a comment/code mismatch.

---

## S07 `pressure_rematerialization_recovery.rs` (841 lines, read in full)

1. **Purpose.** Under register pressure, re-emit a `MaterializeI64` constant just before its future uses instead of keeping the value in a register.
2. **Shape.** `compute::compute_terminal_pressure_rematerialization` (`:596`), `candidate_action::derive` (`:180`), `application::apply` (`:33`), `function_plans::build_functions` (`:350`), `materialize_constraint::select` (`:413`), `rematerialize_selected_active_resident` (`:701`), `PressureRematerializationError` (`:806`).
3. **I/O.** In: selected program, ranges, legality, spill choices, recovery classifications, allocator availability, constraint catalog, policy, budget. Out: `ValidatedPressureRematerialization` with a transformed plan.
4. **How it decides.** Victim must be `RecoveryVictimRole::ActiveResident` and classification `ImmediateU64RematerializationCandidate` (`:188-208`). Arity keyed on policy: single-future-use policy requires `future_uses.len() == 1`; multi-use requires `>= 2` (`:209-212`). All future uses must be in the candidate's block and after its point (`:215-216`). The victim's register fields, and the original `MaterializeI64` instruction (kind, constraint, provenance, operands), must match exactly (`:228-264`).
5. **Cases by shape.** One victim per function (`function.classification` is `Option`, `:383-393`). At least one function must have an action, else `NoAction` (`:518-520`). Every future use must be a single Use operand with no fixed view (`:293-298`).
6. **Same job elsewhere.** Dense-id validation `selected_structure::validate_dense` (`:465`) is similar to `fixed_view_copy::preflight::next_instruction_id` (`fixed_view_copy_recovery.rs:159`) (both read).
7. **Unused.** `PressureRematerializationError::RootMismatch` (`:807`): not grepped; "suspected unused".
8. **Stated limits.** None. Single-victim-per-function is structural.
9. **Fail-open.** None seen; all mismatches return errors.

---

## S07 `publication_validation.rs` (254 lines, read in full)

1. **Purpose.** Explicit (non-default) check that a retained allocation's manifest history and recovery selection agree with what the build selected.
2. **Shape.** `validate` (`:20`), `validate_history` (`:36`), `validate_recovery_selection` (`:86`); 3 unit tests (`:159,179,212`).
4. **How it decides.** Compares transformation ledgers by value; `RuntimeSpill` evidence requires the manifest identity to equal the evidence identity and the ledger to start with the history (`:57-65`). Completion identities must be `Some` exactly when the corresponding phase has selections (`:74-77`). Recovery rule must equal the AllocationRecovery selection slice (`:91-96`).
5. **Cases by shape.** Four evidence variants (`:45-69`).
6. **Same job elsewhere.** Parallels `SelectedOptimizationReplay::validate` (`selected_optimization.rs:100`).
7. **Unused.** `AllocationPublicationValidationError` is `pub use`d in `mod.rs:98`; callers not searched.
8. **Stated limits.** `:3-4`: "The default handoff only retains these facts."
9. **Fail-open.** Default compile does not call this (stated, `:3`).

---

## S07 `reanalysis.rs` (167 lines, read in full)

1. **Purpose.** Re-run liveness, live ranges and legality over the program after fixed-view copies were inserted.
2. **Shape.** `compute_selected_reanalysis` (`:18`), `stage_optimized_selected_reanalysis` (`:96`), `StagedOptimizedSelectedReanalysis` (`:117`), `OptimizedSelectedReanalysisError` (`:152`).
4. **How it decides.** Only one gate: `require_no_transitions` (entry-transition count must be 0, `:76-85`). Called twice (`:45` and `:100`) on the same value.
7. **Unused.** Error variants `UpstreamTransformation`, `LivenessRevalidation`, `LiveRangeRevalidation`, `AllocationLegalityRevalidation`, `ReceiptMismatch` (`:153-161`): not grepped; "suspected unused".
8/9. None seen.

---

## S07 `register_allocation.rs` (107 lines) and `lib.rs` (8 lines), read in full

1. **Purpose.** The stage-07 entry: `stage_register_allocation` (`register_allocation.rs:42`) picks the allocation procedure and admits the result. `lib.rs` re-exports it.
4. **How it decides.** The procedure is keyed on the selected `AllocationRecoveryRule` catalog entry: none → `assign_with_runtime_recovery`; `FixedViewCopies(policy)` → `fixed_view_allocation`; `PressureRematerialization{..}` → `pressure_rematerialization_allocation` (`:73-88`). Whether replay is retained is keyed on `body.retains_stage_inputs()` (`:52`).
5. **Cases by shape.** Exactly three procedures.
7. **Unused.** `RegisterAllocationError` variants all constructed in `assignment.rs recovery` (read).
8. **Stated limits.** `:12-13`: Omega 04 history "does not enter this decision".
9. **Fail-open.** None seen.

---

## S07 `rewrites.rs` (1781 lines, read in full)

1. **Purpose.** Staging for allocation-recovery rewrites: active-resident rematerialization staging, operation-local fixed-occurrence materialization (`materialize_selected_fixed_occurrences`, `:454`), fixed precolored segment homes and fixed-view copies staging, and `settlement_gaps` helpers.
2. **Shape.** `rematerialization::compute` (`:10`), `fixed_occurrences` (`:406`; `FixedOccurrencesError` `:428`, `rewrite_function` `:600`, `materialize` `:787`, `local_occurrence_registers` `:921`, `admit_copy` `:1097`), `fixed_view::fixed_precolored_segment_homes` (`:1263`), `fixed_view::fixed_view_copies` (`:1502`), `settlement_gaps` (`:1700`).
3. **I/O.** Mostly wiring: each `stage_*` function calls analyses from `selected_instruction_analyses`, wraps the result with a custody receipt.
4. **How it decides.**
   - Rematerialization staging requires exact policy triple (`SingleBlockFarthestEndThenHighestVregV1`, `SelectedVictimImmediateU64EligibilityV1`, `...BeforeFirstOfMultipleFutureFlexibleUsesV1`), else `UnsupportedPolicy` (`:44-51`).
   - Fixed-occurrence copies: a register is instruction-local (no copy needed) when it has ≤3 references, one constrained occurrence, same block, and no escape through successor transports (`:1028-1072`). Which registers are fixed is derived from `fixed_view` or tied operands (`fixed_component`, `:866`).
   - `admit_copy` accepts scalar types `Callable | Boolean | IeeeFloat` or integers of 8/16/32/64 bits, else `UnsupportedCarrier` (`:1101-1110`); copy row must be exactly `Use`,`Def`, same class, flexible, with no implicit effects (`:1118-1123`).
   - `capacity_decline` (`:1478`): `SegmentPressure` or work-`BudgetExceeded` in any of three analyses.
5. **Cases by shape.**
   - Only one rematerialization policy combination is accepted (`:44-49`).
   - `origin_at` handles `InstructionResult | EntryParameter | BlockParameter` and `StructuralObservation | AbiTransport`; else `UnsupportedCarrier` (`:1137-1157`).
   - `transport_kind` lists 11 instruction kinds that count as register-to-register transports (`:1080-1095`).
   - Settlement remap uses `ClaimCompletion` → `boundaries` table, all others → `positions` table (`:1728-1731`).
   - `references.len() > 3` is skipped (`:1028`); `roster[index].2.len() < 4` caps stored references (`:1000`). Arbitrary-looking numeric limits, in code, not in comments.
6. **Same job elsewhere.** Settlement index shifting appears three times: `settlement_gaps::remap` (`:1718`), manual `+1` loop in `pressure_rematerialization_recovery.rs:98-107,119-128`, and `fixed_view_copy_recovery.rs:1254-1261` (all read). `rewrites.rs:1700-1735` is the helper; the other two do not use it.
7. **Unused.** `settlement_gaps::{gap_tables, push_position, remap}` are used by `runtime_spill_and_split.rs:363` (1 site read; others not read).
8. **Stated limits.** None.
9. **Fail-open.**
   - `bounded_settlements` and `constrained_operand_bound == 0` return early with no copies (`:611-637`) after checking that tied operands name an existing operand. A program with no fixed views passes unchanged; the next stages (legality) judge it. Classification: traced (no rewrite needed).
   - `instruction_local` registers get no copy (`:813-815`): a wrong "local" classification would leave a fixed-view conflict for the later solver to refuse. Classification: open (relies on `home_assignment` to refuse).

---

## S07 `runtime_spill_and_split.rs` (5493 lines; lines 1-540 read, 541-5493 NOT READ)

1. **Purpose.** Runtime register-pressure rewrites: rematerialize a constant (`runtime_rematerialization`, `:11`), spill to a stack slot with stores and reloads (`runtime_spill`, `:540`), and split live ranges with copies (`runtime_split`, `:4892`) (header comment `:1-6`).
2. **Shape (read part).** `runtime_rematerialization::admission::admit` (`:43`), `rematerialize_selected_runtime_value` (`:365`), `RuntimeRematerializationError` (`:524`). Grep outline of the unread part: `rewrite::spill_selected_runtime_value` (`:985`), `spill_selected_runtime_value_with_crossing_status` (`:1021`), `slot::assign` (`:1720`), `snapshot_copy` (`:2700`), `unit_effects::preserve_live_units` (`:3012`), `derive_runtime_spill` (`:3486`, about 1230 lines), `runtime_split::admission::admit` (`:4921`), `before_fixed_use` (`:5244`), `split_selected_runtime_value` (`:5360`). I did not read these bodies.
4. **How it decides (read part).** `admit` accepts a victim only if: origin is `InstructionResult`; scalar is Callable/Boolean/IeeeFloat or a non-address integer of 8/16/32/64 bits (`:66-71`); no `entry_fixed_view`; `definition_site` is a parameter or node (`:72-80`); the defining instruction is exactly a `MaterializeI64` matching the target's `materialize_i64` constraint (`:129-150`); every use is a plain Use (not fixed, tied, early-clobber; same class; not tied-to by another operand) (`:163-186,235-254`); no use appears in a successor transport binding (`:187-217`); all use blocks are dominated by the definition (`require_dominated_uses`, `:277`).
5. **Cases by shape.** Only `MaterializeI64` definitions; anything else `UnsupportedValue` (`:129-131`). Any non-Use access to the victim → `UnsupportedUse` (`:184,254`).
6. **Same job elsewhere.** The rematerialization candidate checks parallel `pressure_rematerialization_recovery::candidate_action::derive` (`:180-335`); two separate implementations of the "immediate u64 rematerialize" idea (one chosen by classification, one by direct admission). Read both.
7. **Unused.** Not assessed for the unread region.
8. **Stated limits.** None in read part.
9. **Fail-open (read part).** `check_materialize` on `Err` still requires `instruction_block(definition)` to exist, otherwise `UnsupportedValue`, but returns the original `ConstraintMismatch` (`:106-115`); it never accepts. None seen.

---

## S08 `machine/mod.rs` (268 lines, read in full)

1. **Purpose.** Stage-08 entry `stage_optimized_post_allocation_machine_plan` (`:134`): builds the baseline machine plan, runs post-allocation optimisation, seals the plan, assembles `PostAllocationMachineProgram`, and records a custody receipt.
2. **Shape.** `StagedOptimizedPostAllocationMachinePlan` (`:205`), `StagedOptimizedPostAllocationMachineCustodyReceipt` (`:233`), `OptimizedPostAllocationMachinePipelineError` (`:260`), `post_allocation_machine_custody` (`:186`).
3. **I/O.** In: `&RetainedAllocation` (facts). Out: the staged plan with `current: PostAllocationMachineProgram` whose `identity` is recomputed (`:173`).
4. **How it decides.** Nothing selective. It rewrites only via `execute_post_allocation_machine_optimizations`.
8. **Stated limits.** Doc comment `:34-37`: "We never inspect prior rewrite history to choose a different construction route".
9. **Fail-open.** `identity: [0; 32]` initial value then recomputed (`:171-173`), not a risk.

---

## S08 `machine/optimization.rs` (929 lines, read in full)

1. **Purpose.** Post-allocation rule execution. At present exactly one rule: `X86SelectXorZeroI64MaterializationV1` replaces `mov r64, 0` with the declared `xor r32,r32` form when the extra clobbered units (flags) are dead afterwards.
2. **Shape.** `execute_post_allocation_machine_optimizations` (`:44`), `executor` (`:205`), `rewrite_declared_forms` (`:277`), `UnitLivenessTable` (`:434`), `extra_units` (`:379`), `apply_form` (`:398`), 9 tests (`:572-929`).
3. **I/O.** Mutates `PostAllocationMachinePlan` in place; returns a `PostAllocationMachineOptimizationRun` with per-rule actions and work usage.
4. **How it decides.**
   - Candidates keyed on `SelectedInstructionKind::MaterializeI64` with value 0 (`materializes_exact_zero`, `:222-230`).
   - Rule lookup by `Optimization` enum: `executor` matches one variant, `_ => None` (`:205-213`) and a selected rule without an executor is `NoExecutor` (`:62`).
   - Rule applies only if `plan.target.architecture == required_architecture` (`:63-69`).
   - Rewrite permitted only if the declared alternative (found by `applicability == OptimizationSelected{optimization}`, `:369-377`) adds extra units that no later instruction on any path reads before all are rewritten (`UnitLivenessTable`, `:446-518`). Keyed on physical register unit ids.
5. **Cases by shape.** One rule, one architecture. Liveness is block-level with a fixed-point loop capped at `blocks.len() + 1` iterations (`:461`).
6. **Same job elsewhere.** Block successor tables are built here (`successor_table`, `:232`) and also in S06 (`adjacency`, `rewrites.rs:320`); different representations (read both).
7. **Unused.** `PostAllocationMachineRuleExecution::candidates` / accessors are used by tests; external callers not searched.
8. **Stated limits.** `:19-22` "can only shorten unit liveness; a single `UnitLivenessTable` ... remains conservatively sound".
9. **Fail-open.**
   - `live_after`: `self.live_in.get(*successor).copied().unwrap_or(true)` treats a missing block as live, i.e. refuses the rewrite (`:512`). Safe.
   - `block_effect` / `compute_block_and_suffix_effects` classify `Reads` before `Kills` per instruction (`:529-533`). An instruction that both reads and fully rewrites the set counts as a read. Safe.
   - In `UnitLivenessTable::compute`, `live_in[block]` only moves false→true; `Kills` blocks never become live; loop bounded at `len+1` passes (`:461-482`). Whether `len+1` passes always reach the fixed point is not proven in the code. Classification: open.
   - `table.get_or_insert_with(...)` computes the table for the first candidate's `extra` unit set and reuses it for later candidates, guarded only by `debug_assert_eq!(*units, extra)` (`:336-343`). In release builds a different `extra` for a later candidate would be checked against a table built for the first candidate's units. Today there is one rule and one form, so `extra` is constant. Classification: open (depends on there being one declared form per rule and function).
   - `usage.rule_evaluations` etc. use `saturating_add` (`:70,321-322,345,358`): overflow saturates instead of erroring; budget check then compares (`:83`). Classification: traced (saturation only makes usage look larger or equal at max).

---

## S08 `machine/plan.rs` (873 lines, read in full)

1. **Purpose.** Builds the baseline post-allocation machine plan: resolves every operand to its assigned physical register view and chooses the one applicable machine alternative per instruction.
2. **Shape.** `compute_post_allocation_machine_plan` (`:223`), `validate_roots` (`:255`), `build_functions` (`:355`), `build_instruction` (`:451`), `choose_alternative` (`:614`), `is_applicable` (`:633`), `post_allocation_receipt` (`:180`), `PostAllocationMachineError` (`:109`), 3 tests (`:784,812,842`).
3. **I/O.** In: `AllocatedProgramFacts` via `PlanSources`. Out: `PostAllocationMachinePlan` plus sealed `ValidatedPostAllocationMachinePlan`.
4. **How it decides.**
   - Root agreement of ~25 identity/target fields (`validate_roots`, `:255-331`).
   - Home lookup keyed on virtual register id by binary search over a sorted copy (`PreparedHomes`, `:333-353`); function lookup by index and machine id, falling back to `homes.function(machine)` (`:373-380`).
   - Operand check: class equality of home, view and operand; `fixed_view` must equal the home view (`:481-492`).
   - Alternative choice: exactly one alternative's `applicability` predicate must hold: aliasing between views (`physical.model().aliases`) (`:633-686`). `OptimizationSelected` is always `false` here (`:684`). Zero → `NoApplicableAlternative`, more than one → `AmbiguousApplicableAlternatives` (`:626-630`).
   - Address form chosen by matching `SelectedInstructionKind` variants (`:528-590`), `_ => None`.
5. **Cases by shape.** `Address` is attached for 17 listed kinds, with `base_operand: 0` hard-coded for the load/store/offset kinds and `index_operand: 1` for `Load8Indexed` (`:529-574`). All other kinds get `address: None`.
6. **Same job elsewhere.** `validate_roots` repeats identity-matching of the same kind as `fixed_view_copy::preflight::validate_roots` (`fixed_view_copy_recovery.rs:70`) and `reconstruction::expected_record` root checks (`assignment.rs:454-465`). Three separate root-agreement checks, each hand-written.
7. **Unused.** `PostAllocationMachineError::{PhysicalRegisterModelMismatch, ...}` all constructed in `validate_roots`. `PostAllocationMachineError::IdentityMismatch` (`:157`): not grepped; "suspected unused".
8. **Stated limits.** None.
9. **Fail-open.** `homes.get(...).filter(|home| home.machine == function.machine).or_else(|| homes.function(function.machine))` (`:373-377`) accepts a home entry at a different index if the machine id matches; keyed on machine id, so a mismatch of index alone is tolerated. Classification: open (no comment).

---

## S09 `entry-planning/src/body_cpu.rs` (331 lines, read in full)

1. **Purpose.** Measures the CPU state an entry (interrupt handler) body and every callee it can reach touch, from the final selected form and the frame layout, before encoding.
2. **Shape.** `PlannedBodyCpuState` (`:14`), `require_body` (`:28`), `planned_call_effects` (`:53`), `plan` (`:72`).
3. **I/O.** In: `&PhysicalFrameProgram`, entry `MachineId`, `Preemption`. Out: `PlannedBodyCpuState` wrapping `BodyCpuEffects`.
4. **How it decides.**
   - Walks callees with a worklist keyed on `MachineId`: direct call targets (`call_target().machine()`, `:157-161`), plus callable targets from the prepared invocation summary (`callable_cpu_targets`, `:197-205`).
   - Per-row CPU state is keyed on `row.alternative.key.family` (an enum): `CodeSegmentReload`, `DataSegmentReload`, `SegmentSelectorRead`, control-register / descriptor-table / task-register / interrupt families, `ModelSpecificRead/Write` (`:243-316`); `_ => {}` for all other families (`:315`).
   - Opaque foreign calls: must have `normalized_foreign_calls` entry, no callbacks, and if `preemption == Masked` the binding must promise Masked (`:209-227`).
5. **Cases by shape.** `planned_call_effects` has exactly two architectures: `X86_64` and `Aarch64` (`:56-69`). MSR families require x86-64 (`:276-280`). MSR state is derived only when the selector operand is a selected constant (`selected_constant_operand`, `:301-306`), else the `Option` is `None` and passed to `x86_64_model_specific_register_state`.
6. **Same job elsewhere.** Not elsewhere in this unit.
7. **Unused.** `PlannedBodyCpuState::observations` / `into_effects`: not grepped.
8. **Stated limits.** `"interrupt provider callback closure has no retained state footprint"` (`:218`), `"interrupt indirect call lacks admitted provider state"` (`:214`) are `Error::Unsupported`.
9. **Fail-open risks.**
   - `_ => {}` for unlisted `MachineAlternativeFamily` values (`:315`): a state-changing family not in the list adds no `ControlState`/`SegmentState` entries beyond what `footprint.decoded` derives from the encoded unit effects. Classification: open (no comment saying why).
   - `callable_cpu_targets(...)? ` returning `None` skips adding callees (`:197-205`) while still recording the call's decoded footprint; what a `None` means is defined in `installation_admission` (not read). Classification: open.

---

## Coverage

Read in full (lines):
- S06 `rewrites.rs` 1-2818 (2818); `selected_optimization.rs` 1-334 (334); `unexecuted_rewrites.rs` 1-15 (15).
- S07 `assignment.rs` 1-2681 (2681); `fixed_view_copy_recovery.rs` 1-1461 (1461); `home_assignment.rs` 1-2754 (2754); `mod.rs` 1-249 (249); `output.rs` 1-406 (406); `preservation.rs` 1-172 (172); `pressure_rematerialization_recovery.rs` 1-841 (841); `publication_validation.rs` 1-254 (254); `reanalysis.rs` 1-167 (167); `register_allocation.rs` 1-107 (107); `rewrites.rs` 1-1781 (1781); `runtime_spill_and_split.rs` 1-540 (540); `lib.rs` 1-8 (8).
- S08 `lib.rs` 1-8 (8); `machine/mod.rs` 1-268 (268); `machine/optimization.rs` 1-929 (929); `machine/plan.rs` 1-873 (873).
- S09 `entry-planning/src/body_cpu.rs` 1-331 (331).
- Total read in full: 16,997 lines.

Skimmed: none. (For `runtime_spill_and_split.rs` I ran one Grep for module/function names, which gave an outline only; that is not reading.)

NOT READ: `07_selected-instructions-to-register-homes/src/allocation/runtime_spill_and_split.rs` lines 541-5493 (4,953 lines). This covers `runtime_spill` (including `derive_runtime_spill`, slot sharing, snapshot copy, unit effects) and all of `runtime_split`.

## Five things a maintainer most needs to know about this unit

1. S06 runs at most ONE executable phase (SelectedLowering or PreAllocation) per compile and rejects any mix, including a mix with AllocationRecovery selections (`selected_optimization.rs:218-228,323-334`). Pre-allocation rewrites apply one site at a time; each commit re-runs liveness, ranges and legality and must strictly lower the `measure()` number (`rewrites.rs:1832-1848`).
2. A rewrite that fails for any reason other than budget, overflow or replay mismatch is silently "declined", not reported (`rewrites.rs:1196-1199`, `:1691-1694`, `:1756-1759`). Declines are counted but their reasons are not recorded.
3. S07's default path does not replay or independently check the homes it produces: `home_assignment.rs:6-11`, `fixed_view_copy_recovery.rs:7-9`, `output.rs:3-4`. The only check is the explicit `validate_publication` (`output.rs:228`), which compares recorded identities, not correctness (`publication_validation.rs:3-4`).
4. Pressure recovery is a loop in `recover_over` (`assignment.rs:1671-2025`) that tries split, before-use split, rematerialize, then spill for each failing register, re-analysing the whole function set after each batch. It is triggered only by three `RegisterHomeError` variants (`home_assignment.rs:2741-2748`). Several shape limits are hard-coded: at most 3 references per register for "instruction-local" (`rewrites.rs:1028`), 128-view / 128-unit bitmask fast path (`home_assignment.rs:1938-2041`), `u16` domain-count packing (`home_assignment.rs:1118`).
5. S08 has exactly one post-allocation rule (x86 xor-zero) and baseline alternative choice requires exactly one applicable alternative (`plan.rs:626-630`); `UnitLivenessTable` is built once per function for the first candidate's extra-unit set and reused under a `debug_assert_eq!` only (`optimization.rs:335-343`).

## What you are unsure about

- Everything in `runtime_spill_and_split.rs` after line 540 (spill rewriting, slot sharing, snapshot-copy refusal rules, split admission). Claims about "next_instruction_id/next_register_id" duplication rely on a grep outline only (lines `:3354,3362,5222,5228`).
- Whether `RuntimeSpillAllocationError::RecoveryNotRequired`, `RegisterHomeError::IntervalOverflow`, `PostAllocationMachineError::IdentityMismatch`, `PressureRematerializationError::RootMismatch`, and several `*Revalidation`/`ReceiptMismatch` error variants are ever constructed. I did not grep them (marked "suspected unused").
- The per-name split of the 26 grep hits for the `declared_*` macros and `matches_consumer/producer`; I only know 2 hits are in `rewrites.rs`.
- Whether `len+1` passes in `UnitLivenessTable::compute` (`optimization.rs:461`) always reaches the fixed point for all CFGs.
- The meaning of `callable_cpu_targets(...) -> Ok(None)` (`body_cpu.rs:197-205`) and of `footprint.decoded(...)`, both defined in `installation_admission` (not read).
- `preservation.rs:2` says "independently replay", but I found no replay code in that file.
