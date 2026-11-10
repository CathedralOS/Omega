# Unit 29 notes

Files covered (all under `omega-rust/omega/compilation/representations/selected-instructions/`):
analyses/src/allocation_legality.rs, allocator_availability.rs, fixed_precolored_intervals.rs, fixed_precolored_segment_homes.rs, fixed_precolored_split_requirements.rs, legality.rs, lib.rs, live_ranges.rs, liveness.rs, machine_effects.rs, recovery_classification.rs, rewriting.rs, spill_choice.rs, work_accounting.rs; src/atomic.rs, blocks.rs, calls.rs, constraints.rs, control_flow.rs, effects.rs, effects/catalog.rs, effects/catalog/identity.rs.

(Paths below are relative to `.../selected-instructions/`; `A/` = `analyses/src/`.)

## A/allocation_legality.rs (1301 lines, read in full)

**Purpose.** For each virtual register's live-range fragments, computes which physical register views (sub-views of a register) could legally hold it at each point, as spans of constant candidate lists. It honours fixed-view constraints, early clobbers, architectural-unit liveness (physical units already busy) and reserved units. Then it seals the result with a digest/receipt.

**Shape.** `pub mod compute` (A/allocation_legality.rs:17) holds private submodules `early_clobbers` (:19), `fixed_views` (:79), `function` (:198), `live_points` (:445), `view_candidates` (:570), plus `compute_terminal_allocation_legality` (:959) and `validate_roots` (:1045). `pub(crate) mod validate` (:1066) has `FunctionCounts` (:1073) and `admit` (:1127). Public entries: `analyze_allocation_legality` (:1187) and `analyze_allocation_legality_reusing` (:1209). `CandidateViews` (:591) caches candidate lists per class and per "cell" (interval between boundary points); `CellCandidates` enum Unknown/AllBaseViews/PerBlock (:584). `AllocationLegalityError` (:1259) has 9 variants.

**Inputs/outputs.** Reads `ValidatedLiveRanges`, `ValidatedAllocatorAvailability`, the physical register model, constraint catalog, reservation profile, selected constraint keys. Produces `AllocationLegalityPlan` (from `register_homes`) with per-function `FunctionAllocationLegality` (spans, early_clobber_points, entry_transitions per vreg), then `ValidatedAllocationLegality` with receipt counts (read).

**How it decides.**
- Fixed view at a point: `fixed_views::at_live_point` (:119) collects the `view` of every fixed constraint whose site is `Entry` at the fragment's first point, or an `Operand` whose `point` equals the current point; more than one distinct view is rejected as `IllegalFixedView` (`reject_ambiguous` :183). Keyed on point value and `(block, point)` for entry.
- `for_early_clobber` (:92) keys on `(position, instruction, def_operand)` identity.
- `fixed_candidate` (:889): the fixed view must exist, have id equal to its index and class equal to the register's class (else `UnknownFixedView`), and must not `conflicts` (:923) (reserved unit, or a unit active in that block at that point) else `IllegalFixedView`.
- Unconstrained candidates: base views are those in the class that are `allocatable`, in the availability list, and with no reserved unit (:629-648). A cell with no architectural unit active is `AllBaseViews` (:769-770); otherwise per-block list via a u64 bitmask when the class has <= 64 arch units (:669-685), else the slow `unconstrained` (:860).
- Empty candidate list is an error `NoCandidateViews` (:539, :59).
- Adjacent spans are merged when the candidate slice pointer/len or contents are equal (:547-554).

**Cases by shape.** Bitmask fast path only when `arch_units.len() <= 64` (:669); otherwise falls back to per-view scan (:805-815). Tied-operand: no code here (tied handled elsewhere in the unit).

**Reuse across runs.** `compute_reusing` (:332) reuses a previous function's legality for a vreg when the previous ranges' machine equal, no early clobbers either side (:344-345), the vreg's fragments end before `unchanged_point_limit` (:254) and fragments and fixed_constraints equal (:376-384). Plan-level reuse: whole function carried if `prior_ranges.plan().functions.get(i) == Some(live_ranges)` (:1006-1011), after a check that target, optimization_unit, work_schedule and identities agree (:987-998). `unchanged_point_limit` compares `block_domains` and architectural-unit rows to find the first changed point.

**Same job elsewhere.** The `validate_roots` environment check duplicates the shape of `roots::validate` in fixed_precolored_segment_homes.rs:744 and the identity comparisons in allocator_availability.rs:39-52 (read). `seal_plan!` is used in the other files instead of the manual `admit` here.

**Unused/one-caller.** `analyze_allocation_legality_reusing` (:1209), `analyze_allocation_legality` (:1187): see grep section at end of file (to be filled).

**Stated limits.** None seen in comments (the file has almost no comments beyond the header).

**Fail-open risks.**
- `conflicts` (:934-938): a view unit that is not found in `function.architectural_units` is treated as not active (`return false`). This is expected (units never used have no row). Traced: a missing row legitimately means "never live".
- `FunctionCounts`/`admit` (:1140-1143): when `carried[i]` is true the previous counts are trusted instead of recomputed, and digests carried (`carried_digests`, :1153). Wrong input only if a caller passes a `carried` slice that does not match; `admit` is `pub` inside `pub(crate) mod validate`, callers are `analyze_*` only (read).
- `reuse_state` closure (:340-351) treats early clobbers as disqualifying reuse (fail closed).
- `unchanged_point_limit` returns `LiveRangePoint(u32::MAX)` when block domains are equal and lengths equal (:265), meaning "everything unchanged" before the unit comparison lowers it. Traced from code: lowers by every differing unit row; extra units on either side lower the limit by their first fragment/action.
- No place that accepts on `None` without a stated reason found otherwise.

## A/allocator_availability.rs (196 lines, read in full)

**Purpose.** Turns a named policy (`AllocatorAvailabilityPolicy`) into a per-class list of "unconstrained" physical views the allocator may use, and seals it.

**Shape.** `compute::compute_terminal_allocator_availability` (:30), `admitted_views` (:77), `materialize_allocator_availability` (:122), `ValidatedAllocatorAvailability` (carrier, :168) with `unconstrained_views(class)` (:174), `AllocatorAvailabilityError` (:184).

**How it decides.** Environment-allocatable = `view.allocatable` and none of `units`/`write_units` reserved (:82-89). Policy `AllEnvironmentAllocatableViewsV1` keeps all such views (:91-97). `ExplicitUnconstrainedViewAllowlistV1 { views }` requires strictly increasing ids (:99), each view to exist (`physical.model().view`) and be environment-allocatable (:106). Root check compares architecture, physical identities, target and recomputed environment identity (:39-51).

**Cases by shape.** Exactly two policies (match at :90).

**Fail-open.** None seen. Note `unconstrained_views` returns `None` for unknown class and callers treat that as `UnknownClass` in allocation_legality.rs:398 (but `CandidateViews::new` uses `.unwrap_or(&[])` at :624-626, i.e. an unknown class yields empty availability, which then errors later only if a register of that class is processed) - traced by reading.

## A/fixed_precolored_intervals.rs (361 lines, read in full)

**Purpose.** Turns each fixed-register constraint on a virtual register into a one-point half-open interval `[p, p+1)` after checking the legality analysis already forces exactly that view there.

**Shape.** `compute::compute` (:23), `derive_function` (:79), `reject_early_clobber_fixed` (:143), `resolve_point` (:175), `require_view` (:208), `interval_key` (:238); `analyze_fixed_precolored_intervals` (:269); error enum (:323).

**How it decides.**
- Early clobber def with a fixed constraint is rejected (`UnsupportedEarlyClobberFixedConstraint`), matched on `(instruction, def_operand, def_virtual_register)` (:158-162).
- `resolve_point` (:183): Entry -> first fragment start; Operand -> its point; exactly one fragment must contain it (:198).
- `require_view` (:216-234): exactly one legality span must contain the point in that block and its candidates must be exactly `[view]`.
- Work accounting via `Work` (budgeted).

**Fail-open.** none seen (every mismatch is an Err).

## A/fixed_precolored_segment_homes.rs (1277 lines, read in full)

**Purpose.** Gives each "domain" (a set of source segments of one virtual register that must share a view) one physical view, using a greedy most-constrained-first coloring with affinity (copy) preferences and a look-ahead that avoids stranding neighbours.

**Shape.** `compute::compute` (:802); submodules `conflicts` (:13; `Conflicts` struct :35, `build` :79), `functions::derive` (:212), `placement::assign` (:255) with `select` (:342), `preferred_view` (:388), `residual_base` (:604), `strands_neighbor` (:642), `propagate_forced` (:661), `clique_bound_violated` (:697), `roots::validate` (:744), top-level `domains` (:860, `Domain` :884, `build` :910), `affinity_edges` (:1193), `assign_fixed_precolored_segment_homes` (:1111). Policy: only `MostConstrainedLowestCompatibleViewV1` (:827).

**How it decides.**
- Domain building keyed on segment `opening` enum: `SourceRangeStartV1` and `IncompatibleFixedUseDomainBoundaryV1` make a new domain; `IncomingSourceEdgeV1 { connector }` joins the domain that closed in `connector.source` block, intersecting candidates (:930-966). Missing predecessor -> `MissingIncomingDomain`.
- Two domains conflict when they overlap in block+point (`live_overlap` :130) AND their virtual registers interfere per `ranges.interference` binary search (:151-153); view pairs conflict if `footprints_overlap` (:108).
- Selection order: tuple `(viable_len, Reverse(degree), first_point, virtual_register, first_segment)` minimum (:363-375).
- `preferred_view` ranks `(guaranteed, votes, Reverse(stolen))` among views that do not strand a neighbour (:471).
- `SegmentPressure` error if no view remains (:287-294).
- `affinity_edges` (:1193) silently `continue`s when an affinity's registers, occurrence points or domains cannot be resolved (:1213-1234). That only drops a preference, not a legality condition.

**Cases by shape / limits.** `clique_bound_violated` (:697) is a heuristic pigeonhole bound; `residual_base` returns `None` on violation so that `keeping` stays empty and the whole `viable` is used (:412). This is look-ahead only; the final check is `SegmentPressure`.

**Data-structure note.** Uses `BTreeMap`/`BTreeSet` keyed on domain index and ids heavily (:14, :240, :265-267, :1088), contrary to the repo convention of dense Vec tables (AGENTS.md) - noted only.

**Fail-open.** `checked_view` (:156) does a linear scan `physical.model().views.iter().find(...)` and errors if absent: fail closed. In `placement::assign` (:303-308) a neighbor missing from `viables` is skipped (`continue`): domain already assigned, so correct. Traced.

## A/fixed_precolored_split_requirements.rs (922 lines, read in full)

**Purpose.** For each virtual register with fixed-view use constraints, decides where its source live range has to be cut into segments because the allowed-view sets at successive points no longer intersect; each cut must be backed by a declared `EntryFixedViewTransition`.

**Shape.** `compute::compute` (:663), `function::derive` (:209), `partition::register` (:302) / `fragment` (:407) / `push_segment` (:500), `topology::tree` (:552), `cuts::CutIndex` (:26) with `boundary` (:58), `require_transition` (:124), `use_transition` (:145), `finish` (:169); `site_key` (:885), `site_point` (:905), `early_clobber_uses` (:912).

**How it decides.**
- Tied registers (appear in `ranges.tied_pairs`) are rejected outright: `UnsupportedTiedRegister` (:325-332, `tied_registers` :275).
- Cut at span where intersection with the running shared set becomes empty (:439-441). A cut requires exactly one fixed interval starting at (block,start) (:67-85), a Use access (:102), and `point.candidates == [row.view]` (:112).
- Transitions: `use_transition` (:145) needs exactly one declared `from` view that is in the source set (:159-164), returns false otherwise. In `fragment` the boolean result is ignored (:455 `cuts.use_transition(&prior, site, destination_view);` no `?`), whereas the cross-block case at :358 uses `require_transition` and errors.
- `topology::tree` (:552) admits only: each target block has at most one incoming connector (:568), a connector's source must be an earlier fragment (:612), legality spans must exactly tile fragments (:591-606); everything else `UnsupportedCrossBlockRange`.
- `finish` (:169) checks every used transition is declared.

**Cases by shape.** "tree" topology only (fragment with at most one incoming edge connector, source earlier in fragment order); no tied registers. (read)

**Fail-open.** `CutIndex::finish` (:174-178): checks `used` entries against `transitions` which are the same index they were inserted from, so the check is tautological: every `used` key was derived from `self.transitions.get(&key)` and chosen from it (:152-165). Traced by reading: `finish` can never fail; the error `UnsupportedCrossBlockRange` there is unreachable. Also `use_transition` return value ignored at :455: a mid-fragment cut with no declared transition (or ambiguous: two matching from-views -> `false`) is accepted. Which program gets through: a fragment where the shared view set becomes empty at a fixed-use point and `entry_transitions` has no (or two) matching entry for that site; the cut is still recorded with `IncompatibleFixedUseDomainBoundaryV1`. Status: "open" (I did not trace whether `entry_transitions` is constructed to always contain every such transition; allocation_legality.rs:145 builds transitions for every Use fixed constraint from every other pinned view, which suggests it always does when `pinned.len() >= 2`).

## A/legality.rs (296 lines, read in full)

**Purpose.** Staging wrapper: takes `StagedOptimizedLiveRanges`, picks an allocator-availability policy, runs `analyze_allocation_legality` and bundles ranges + availability + legality + a custody receipt.

**Shape.** `stage_optimized_allocation_legality` (:206, all environment-allocatable views), `..._for_frameless_leaf` (:213, caller-saved only), `..._for_active_resident_immediate_u64_multi_use_rematerialization_v1` (:220, views named `rax`,`rcx` / `x0`,`x1`), `stage_optimized_allocation_legality_with_availability` (:228, pub(crate)); `StagedOptimizedAllocationLegality` (:236); custody receipt (:264); error enum (:286).

**How it decides.** Frameless-leaf policy: view is allocatable and every unit is in the preservation convention's `caller_saved` set (:121-128); convention fetched per architecture (X86_64, Aarch64 only, :100-110). Rematerialization policy picks views by display name strings `"rax","rcx"` / `"x0","x1"` via `view_named` (:167-170, :175). Keyed on source spelling (register names).

**Fail-open.** none seen (missing names -> error).

## A/lib.rs (1209 lines, read in full)

**Purpose.** Crate root of the analyses crate. Defines the macros every analysis file uses to build receipts and carriers, re-exports the analysis entry points, defines the `SelectedProgramView` trait, a small byte `Cursor`, the allocation-recovery rule catalog, the `SelectedOptimizationOutput` ledger types, `carried_digests`, and `UnsealedRecoveryFacts` (incremental re-analysis for spill/split recovery).

**Shape.**
- Macros (all `#[macro_export]`): `shared_analysis_identity_accessors` :45, `receipt_accessor_rows` :73, `copy_receipt` :86, `staged_producer_accessors` :113, `validated_analysis_accessors` :160, `work_defect_conversion` :175, `validated_plan_carrier` :186, `shared_plan_carrier` :207, `seal_plan` :234, `failed_display` :254, `validated_transformed_receipt` :267, `receipt_selected_identities` :296, `rewrite_receipt` :316, `rewrite_receipt_identity` :353.
- `ALLOCATION_RECOVERY_RULE_CATALOG` (:403): exactly two entries, `SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` and `ActiveResidentImmediateU64MultiUseRematerializationV1`, both `TargetIndependent`. `selected_allocation_recovery_rule` (:425): empty selection -> `None`; one selection in catalog -> `Some`; one not in catalog -> `UnsupportedSelection`; more than one -> `UnsupportedComposition` (:431-444). Case by shape: "zero or one recovery rule".
- `codec_bytes` (:452) and `cursor::Cursor` (:571): byte-slice reader with `take`, `u16/u32/u64/u128`, `length`.
- `SelectedProgramView` trait (:705) with impls for `SelectedProgram` (:759), `SelectedProgramRef` (:737), `OwnedSelectedProgram` (:800).
- `carried_digests` (:855): per-function digest reused from `previous_digests[i]` iff `carried[i]` is true and the previous slice has index i, else recomputed.
- `UnsealedRecoveryFacts` (:879) with `from_validated` (:887), `reanalyze_changed_functions` (:909), `seal` (:987).
- Two unit tests (:1091-1209) for receipt counts after partial carry.

**How it decides.**
- `reanalyze_changed_functions` marks function i "changed" unless `shares_function_storage` or `==` on the function (:921-929). Keyed on structural equality of the whole `SelectedFunction` (or storage pointer sharing).
- `seal` computes `carried[i]` (:1015-1048): function unchanged AND liveness/ranges/legality rows `Arc::ptr_eq` or `==` to the anchor's.

**Fail-open risks.**
- `seal` :1007-1014: for functions whose liveness was computed in the cheaper "for_ranges" form (`liveness_materialized[i] == false`), it tries `liveness::compute::compute_function` and on `Err` silently keeps the cheaper form (`let Ok(full) = ...` in an `if let` chain). A recovery fact set could then be sealed with a partially materialized liveness row. Status: "open" (I have not read what `compute_function_for_ranges` omits versus `compute_function`; see liveness.rs notes).
- `seal` returns infallible `admit_validated_liveness` and `live_ranges::validate::admit`: these only hash/count; no revalidation of the rows (`validate::admit` at live_ranges.rs:1555). Stated nowhere in a comment; by design "verify at trust boundaries" per AGENTS.md (inferred).
- `reanalyze_changed_functions` indexes `liveness_functions[index]` etc. directly (:937, :947, :976) so a function-count change panics rather than being accepted.

**Same job elsewhere.** `carried_digests` pattern is repeated manually in `allocation_legality::validate::admit` (A/allocation_legality.rs:1140-1143 counts) and `live_ranges::validate::receipt::admit` (:1565-1573 counts). `FunctionCounts` exists in both allocation_legality.rs:1073 and live_ranges.rs:1530 with different fields.

**Unused/one-caller (greps to be added in the end section).**

## A/live_ranges.rs (1849 lines, read in full)

**Purpose.** From a `SelectedFunction` and its `FunctionLiveness`, builds the `FunctionLiveRanges` fact row for each function: per-virtual-register fragments (contiguous point ranges within a block), occurrences, fixed-view constraints, edge connectors, tied def/use pairs, early-clobber constraints, copy affinities, edge register transfers, architectural-unit (physical register unit) live ranges and actions, and the virtual-register interference pairs. Points are two per liveness position: `before_point = 2*pos`, `after_point = 2*pos+1` (:959-980).

**Shape.** `compute` module (:13) with submodules `architectural_units` (:15), `constraints` (:198; `derive_early_clobbers` :204, `derive_tied_pairs` :389), `copy_affinities` (:579), `edge_transfers` (:627), `fragments` (:684; `block_domain` :693, `register_fragments` :720), `function::compute_function` (:1024), `interference` (:1201, interval-tree sweep), `compute_terminal_live_ranges` (:1447), `compute_terminal_live_ranges_reusing` (:1469). `validate` module (:1523): `FunctionCounts` (:1530), `receipt::admit` (:1555), `require_liveness_of` (:1609). Public: `analyze_live_ranges` (:1629), `analyze_live_ranges_reusing` (:1638), staging (`stage_optimized_live_ranges` :1710, `StagedOptimizedLiveRanges` :1723), `LiveRangeError` (:1799, 13 variants).

**How it decides.**
- `reject_unsupported` (:1185): any operand with access `UseDef` is rejected (`UnsupportedUseDef`). `operand_point` (:943) also errors for `UseDef`.
- Early clobbers (:204-369): one big boolean condition (:305-338) rejects with `UnsupportedEarlyClobber` unless the instruction has: the early-clobber operand is a `Def`; at least 2 operands and at least one `Use`; if there are several Defs then none tied; a tied source must be a `Use` with a lower operand index, a different vreg, same class, itself untied; all other operands are `Use`s (or other early-clobber Defs when multiple defs), untied; no operand repeats a vreg or operand id; the tie component has exactly one early definition; no unrelated operand uses a tied register. Keyed on operand index, vreg id, class, access.
- Tied pairs (:418-423): def operand index must be greater than use operand index, vregs differ, classes equal, use untied.
- Copy affinities (:584-625): only `SelectedInstructionKind::CopyI64` with exactly two operands; copies whose source vreg equals destination are skipped (:610). A block with a precomputed `block_copy_affinities` row uses that instead and a malformed copy there is `FunctionMismatch` (:593).
- Fragments (:720-868): reverse walk of each block; for instructions with no `virtual_defs`/`virtual_uses` lists, falls back to per-operand scan (:752-813); a vreg defined and also used in the same instruction (`also_used`) does not close its fragment at the def (:788). Register ids not present in `register_index` are silently skipped (:740-742, :773-775, :816-818, :837-839, :847-849).
- Duplicate register ids in `selected.virtual_registers`: only the first occurrence (canonical `pos == canonical_idx`) receives fragments; later duplicates get empty fragments but the same occurrences/edge connectors (:1145-1158). No error is raised.
- Interference (:1269): pairs of vregs with overlapping non-empty fragments in the same block (half-open overlap); architectural units are not part of it.

**Cases by shape.** Early clobber shape list above (a fixed shape list, refuses the rest). `register_fragments` fast path vs operand-scan path chosen by whether `virtual_defs`/`virtual_uses` are both empty.

**Reuse.** `compute_terminal_live_ranges_reusing` (:1469): `compatible` requires identities of previous selected/liveness/unit/schedule/target and function count (:1478-1487); a function row is carried when selected function is shared or `==` and liveness row `==` (:1495-1506).

**Fail-open risks.**
- Skipped register ids (above): an operand or liveness entry naming a virtual register id that is not in `selected.virtual_registers` produces no fragment and no error. A wrong program would get through only if liveness lists a vreg the selected function does not declare; liveness is derived from the same function, so status "open" (not traced).
- `has_duplicate_register_ids` case silently yields rows with empty fragments for duplicates (:1145-1149). "open".
- `compute_terminal_live_ranges` (:1451-1458) `zip`s selected functions with liveness functions, which would truncate on length mismatch; `require_liveness_of` (:1609) only compares selected identities. "traced": identity equality of the selected plan implies the liveness was built from it, so lengths agree unless the liveness plan was forged.
- `LiveRangeError` variants `VirtualRegisterMismatch`, `ArchitecturalUnitMismatch`, `InterferenceMismatch`, `TiedPairMismatch`, `EarlyClobberMismatch`, `NonCanonicalRows`, `LivenessRevalidation`: only `LivenessRevalidation` constructed in this file (:666); others not constructed here (grep in end section).

**Same job elsewhere.** The overlap test "same block and half-open overlap" appears in `fixed_precolored_segment_homes.rs:130-136` (`live_overlap`) and in the test oracle `interference::tests::expected` (:1331). Architectural-unit fragment building (:15-196) is a copy of the register fragment building (:720-868) specialised to units (inferred from reading).

**Stated limits.** Only error names (`UnsupportedUseDef`, `UnsupportedTiedOperand`, `UnsupportedEarlyClobber`).

## A/liveness.rs (2312 lines, read in full)

**Purpose.** Backward dataflow liveness over a `SelectedFunction`: which virtual registers and which physical register units are live into and out of every block and instruction, with block-parameter substitution along edges. Output is `FunctionLiveness` (per block: live-in/out, per-instruction uses/defs/live sets, successor edge live sets, an arena of `OperandPosition` rows).

**Shape.**
- `compute::control::successors` (:27): successor list of a terminator (ConditionalBranch two, less-pair two, Jump one, no-successor pattern none).
- `compute::flow` (:47): `FunctionFlow::compute` (:127) / `compute_units_filtered` (:134) / `compute_domains` (:142) worklist solver. Virtual sets are `Vec<u64>` bitsets; unit sets are a `u64` mask when every unit id < 64 and `has_high` is false, otherwise `BTreeSet` (:221-232, :297-308, :311-333).
- `compute_terminal_liveness` (:544), `compute_terminal_liveness_reusing` (:563), `compute_function` (:741, full), `compute_function_for_ranges` (:748, `materialize_instruction_live_sets = false`), `compute_function_units_filtered` (:622, pub, unit-only liveness for chosen blocks), `reject_unsupported_constraints` (:839), `materialize_block` (:1068).
- `edge_values` (:1227): `FunctionIndex` (:1558), `validate_transports` (:1677), `incoming_argument` (:1880), `validate_case_transport` (:1998), `case_payload::observed_source` (:1462), `has_edge_use` (:1652).
- `validate::receipt::admit_validated_liveness` (:2087): digest + receipt only.
- Public: `analyze_liveness` (:2116), `analyze_liveness_reusing` (:2123), `stage_optimized_liveness` (:2184), `LivenessError` (:2263, 11 variants).

**Inputs/outputs.** Reads `SelectedFunction` (blocks, terminators, virtual registers, calls, provenance) through `SelectedProgramView`; produces `LivenessPlan` with `Arc<FunctionLiveness>` rows.

**How it decides.**
- Edge parameter substitution: per edge, `(parameter, argument)` pairs from `bindings` with `SelectedValueTransport::Registers` and from `structural_case.payloads` with `Registers` transport; `Unused`/`Unmaterialized` ignored (:245-261). Live-out of the predecessor = target live-in minus parameters plus arguments of live parameters (:411-428).
- Register ids that are in a target's entry set but not in `valid_register_bits` (declared exactly once) fail with `FunctionMismatch` (:406, :418). `valid_register_bits` is only built when some block has an edge (:335).
- Early-clobber / tied / UseDef shape rejection in `reject_unsupported_constraints` (:839-1047): only blocks for which `block_has_constraint_operands(block_index)` is true are examined (:849). Rejects any `UseDef` operand; multi-early-clobber instructions must be all `Use` (non-early) or `Def` (early), untied, no repeated vreg or operand (:871-893); single early-clobber has the same shape rules as `live_ranges.rs` `derive_early_clobbers` (:908-950); tied defs must be Def with higher operand index than a Use, different vreg, same class, untied use (:990-996); tie components must have one early definition (:1007-1045).
- `validate_transports` (:1677) checks edge roles against block origins with an explicit allowed list (`Semantic`, `CaseDispatchContinuation`, `OperationContinuation`, `EdgeTransferContinuation`), destination origin rules (:1818-1843), single binding per parameter and one destination register per binding (:1845-1871). Returns `Ok(())` immediately for functions where no block has `Operation` origin, trap/invoke-no-normal terminator, or any successor (:1684-1694).
- `observed_source` (:1462) accepts a signed 8/16/32-bit `AbiTransport` register only if it is produced by exactly the expected SignExtendI8/16/32 instruction in an `EdgeTransfer` block fed by a `Load8/16/32` structural observation with matching place/offset and provenance edge.

**Cases by shape.**
- "Registers" vs "Unused" transport only; any other `SelectedValueTransport`/`SelectedCasePayloadTransport` variants are `None` in substitutions (:245-258). In `incoming_argument` a binding source whose origin is `StructuralParameter | SpillAddress | StructuralObservation | ScalarAbiAddress | InstructionScratch | AbiTransport` is rejected (:1978-1984).
- Case payload scalar normalisation: sign-extension width must be 8, 16 or 32 (:1488-1493); everything else returns `None` (rejected).
- Unit set representation: mask when all unit ids < 64 else BTreeSet (:226-230).

**Reuse.** `compute_terminal_liveness_reusing` (:563): same compatibility rule as live_ranges.rs:1478; function row carried when selected function storage shared or `==`.

**Same job elsewhere.**
- Early-clobber/tied rule set is implemented twice in this unit: liveness.rs:839-1047 (`reject_unsupported_constraints`) and live_ranges.rs:204-448 (`derive_early_clobbers`, `derive_tied_pairs`); `tied_component` is duplicated (liveness.rs:1049, live_ranges.rs:371). The two copies are not textually identical (e.g. liveness.rs requires `operands.len() > 1` at :923; live_ranges.rs :312 requires `operands.len() >= 2` - same; liveness checks multi-early through a different predicate than live_ranges.rs:316-331).
- Block-resolve logic: `flow::compute_domains` has its own dense/sparse block ordinal table (:151-201) while `FunctionIndex::block_index` (:1640) uses `ids_are_positions`; `edge_transfers::derive` in live_ranges.rs:646-656 uses `FunctionIndex`.

**Fail-open risks.**
- `block_virtual_transfer(...).unwrap_or((&[], &[]))` (:281-285): if a block has no cached virtual transfer row, its uses/kills are treated as empty, so nothing is live-in because of that block. Status: "open" (the cache is owned by `selected_instructions::blocks`, which I read later in this unit: see blocks.rs notes).
- `block_unit_masks(...)` None sets `can_use_unit_masks = false` (:306-308) and then `block_unit_transfer(...)` None leaves that block's transfer empty (:313-332 `if let Some`): no unit uses/kills. Same "open" status.
- `reject_unsupported_constraints` skipped for blocks where `block_has_constraint_operands` is false (:849): a block whose flag is wrongly false would skip the UseDef/early-clobber/tied checks. "open" (flag derived in selected_instructions blocks.rs, see there).
- `validate_transports` early `Ok(())` (:1693): functions with no Operation-origin block, no trap/invoke, and no successors are never checked; such a function has no edges, so edge transport has nothing to validate ("traced" by reading the predicate).
- `seal` path in lib.rs:1010: `compute_function_for_ranges` and `compute_function` call the same checks (:745, :752 both run `reject_unsupported_constraints`, `validate_transports`, `FunctionFlow::compute`), so `compute_function` can only differ in `materialize_block`; its `Err` (which lib.rs ignores) can only come from `incoming_argument` inside successor materialisation, which also runs in the cheap form (:1150-1161). "traced": ignoring that Err in lib.rs:1010 cannot hide a failure the cheaper form did not already hit.
- `LivenessError` variants `RootMismatch`, `BlockMismatch`, `InstructionMismatch`, `SuccessorMismatch`, `NonCanonicalSet`, `TransferMismatch`, `FixedConstraintMismatch` are not constructed in this file (to grep at end).

**Cheaper form note.** `compute_function_for_ranges` (:748) leaves `virtual_uses`, `virtual_defs`, `virtual_live_in`, `virtual_live_out`, `unit_live_in`, `unit_live_out` of every `InstructionLiveness` empty (:1124-1142); live_ranges.rs:752-813 has a matching branch that reads operands directly when both lists are empty. Unit uses/defs/clobbers are always populated.

## A/machine_effects.rs (1035 lines, read in full)

**Purpose.** For every instruction (including each block terminator) of a selected program, looks up the target's machine-effect declaration (memory, trap, barrier, call, cleanup, encoding alternatives) by (semantic kind, constraint key), checks the instruction's operands and implicit register-unit effects against the register constraint, and records the result as a `PreAllocationMachineEffectPlan`.

**Shape.** `analysis::analyze` (:20); `catalog::validated_catalog` (:59) with a process-wide `static CATALOGS: OnceLock<Mutex<Vec<(CatalogKey, Arc<ValidatedMachineEffectCatalog>)>>>` (:63) keyed on `(NativeTarget, constraint catalog identity, physical model identity)`; `build_validated_catalog` (:85) calls `isa_x86_64` / `isa_aarch64` catalog builders and validators; `facts::compute::compute_terminal_pre_allocation_machine_effects` (:142), `validate_catalog_roots` (:228), `terminal_selected_keys` (:246, copies about 100 key fields one by one), `compute_instruction` (:367), `exact_declaration` (:450), `machine_semantic_kind` (:473, one arm per `SelectedInstructionKind`, no wildcard), `machine_alternative_is_transparent` (:750), `foreign_call::constraint` (:776) and `declaration` (:823); `seal_pre_allocation_machine_effects` (:877); public `analyze_pre_allocation_machine_effects` (:903), `analyze_machine_effects` (:1030), `validated_machine_effect_catalog` (:1020), `MachineEffectError` (:965, 14 variants), `MachineEffectStageError` (:111).

**How it decides.**
- Declaration lookup: `BTreeMap<(MachineSemanticKind, RegisterConstraintKey), Option<&Declaration>>` where a duplicate key is stored as `None` -> `AmbiguousDeclaration` (:171-175, :459-463); missing -> `MissingDeclaration` (:464).
- `compute_instruction`: `Store` with `byte_size` not in 1/2/4/8 -> `ConstraintEffectMismatch` (:377-382); work provenance must settle only claimed sites (:383); operand count equals constraint's (:390); each operand's number, access, class, fixed view, tie and early-clobber flags and the register's id/class must equal the constraint's (:395-417); `implicit_uses/defs/clobbers` equal (:418-425).
- Dynamic-call and normalized-foreign-call instructions get their constraint and declaration built per instruction (`foreign_call::constraint` :776, `declaration` :823) keyed on `instruction.kind.call_target()` being `Dynamic`, or kind `NormalizedForeignCall`; all other kinds use the catalog row verbatim (`Cow::Borrowed`, :805, :850).
- Catalog roots: target, constraint identity, selected-keys equality (:234-243). Root of environment compared by recomputing `target_register_environment_identity` (:155-164).

**Cases by shape.** Exhaustive match in `machine_semantic_kind` (:473-747); adding an instruction kind forces a compile error here. Aarch64/X86_64 only (:89-102, :857-864).

**Fail-open risks.**
- `plan.identity` is a placeholder `[0;32]` overwritten after construction (:212-224); not a risk.
- `validated_catalog` cache lookup/insert ignore a poisoned lock (:71, :77): fall through to rebuild; fail closed.
- `ValidatedPreAllocationMachineEffects::new` is `pub` (:953) and takes any plan + receipt without checks; "open" whether outside callers use it (grep in end section).
- `MachineEffectError::{NonCanonicalFunction, NonCanonicalBlock, InstructionMismatch, IdentityMismatch}` are declared but not constructed in this file; `seal_pre_allocation_machine_effects` (:877) always returns `Ok` and does no verification (named "seal").

**Same job elsewhere.** `terminal_selected_keys` copy of `TargetRegisterEnvironmentConstraintKeys` into `SelectedConstraintKeys` (:246-365) mirrors the key listing in `selected_instructions` constraints (src/constraints.rs, see below) and in `register_environment` (outside unit, suspected).

## A/recovery_classification.rs (558 lines, read in full)

**Purpose.** For the one spill victim chosen in each function, decides whether the victim could be recovered by rematerializing a constant ("immediate u64 rematerialization") instead of spilling to a frame slot, and records the reason when not.

**Shape.** `classify_pressure_recovery` (:475); `compute_terminal_recovery_classifications` (:399); `function_classification::classify` (:25), `victim_role` (:90); `immediate_eligibility::classify` (:134), `unique_definition` (:232), `future_uses` (:261), `is_fixed_unsigned_u64` (:330); `work_usage::required` (:356); `validate_roots` (:440); `RecoveryClassificationError` (:544).

**Inputs/outputs.** Reads selected function, live ranges, allocation legality, `ValidatedSpillChoices`. Produces `RecoveryClassificationPlan` with, per function, `None` (no choice) or `PressureRecoveryClassification { block, point, victim, role, ..., classification }` where `classification` is `ImmediateU64RematerializationCandidate {...}` or `NoAdmittedRecovery { reason }`.

**How it decides (`immediate_eligibility::classify`, :134-230), in order:**
1. victim scalar type must be a non-address unsigned 64-bit integer, else `UnsupportedScalarType` (:142, :330).
2. origin: only `InstructionResult` passes; `EntryParameter`/`StructuralParameter` -> `EntryParameter`; `BlockParameter`, `SpillAddress`, `StructuralObservation`, `ScalarAbiAddress`, `InstructionScratch`, `AbiTransport` -> `UnsupportedRangeShape` (:145-162).
3. victim must have no edge use (`has_edge_use`), exactly one fragment, in the choice's block, no edge connectors (:163-169).
4. any fixed operand constraint at point >= choice point -> `FutureFixedUse` (:170-177).
5. defining instruction (unique, id equal to origin's) kind: nine proof-bearing exact/wrapping-divide kinds -> `ProofBearingDefinition`; `MaterializeI64` continues; any other kind -> `NonMaterializeDefinition` (:179-192). Matched on the instruction kind enum.
6. provenance of the definition must be exactly `values == [source_value]`, one operation, no edges, no obligations, nonempty work all at that operation, and the integer type must admit the constant, else it is a hard `VictimMismatch` error (:196-216).
7. future uses (Use occurrences at or after the choice point, in the choice's block, no fixed view) empty -> `NoFutureUse`.

**Cases by shape.** The list above (u64 constants materialised by `MaterializeI64` in a single-block range). Stated by the enum names; no comment.

**Fail-open risks.** None seen: every unsupported shape returns a `NoAdmittedRecovery` reason, which means "do not rematerialize" (fail closed). `RecoveryClassificationError::ClassificationMismatch` and `UsageMismatch` are not constructed here.

**Same job elsewhere.** `block_instructions` (:319) duplicates `liveness.rs:1215` `block_instructions` (iterator form) as a Vec-returning helper. `unique_definition` scan resembles `FunctionIndex::unique_instruction` (liveness.rs:1586).

## A/work_accounting.rs (179 lines, read in full)

**Purpose.** Counters for bounded work. `Work` accumulates five axes (rule_evaluations, candidates, validation_steps, commits, iterations) with checked adds and `finish(budget)` refuses if usage is outside the budget.

**Shape.** `WorkDefect` (:12: `Overflow`, `BudgetExceeded`), `count` (:22), `combine` (:26), `admit` (:43), `Work` (:55; `new`, five adders, `finish` :84). Tests :100-179.

**Users.** `Work` is pub(crate) and used by fixed_precolored_* and recovery_classification and (to be confirmed) spill_choice. See end section for counts.

**Fail-open.** None seen; overflow and budget are errors.

## A/rewriting.rs (566 lines, read in full)

**Purpose.** A toolbox of small helpers used by pre-allocation rewrites and pressure-recovery rewrites: block/edge mention queries, effect-declaration probes, building a `SelectedInstruction` from a constraint row, work-budget extent formulas, receipt macros, and plan lookup.

**Shape (all `pub`).** `block_edges` (:8): `member_position` :18, `function_at` :33, `block_instructions` :41, `block_instruction_ids` :48, `edge_surface` :51, `local_slot_mentions` :60, `successor_mentions` :64. `operand_probes` (:120): `effect_declaration` :129, `last_definition_before` :144, `isolated_declaration` :156. `selected_operands` (:164): `selected_operand` :167, `copy_shaped` :182. Top level: `RegisterAllocationRuleTargetApplicability` (:218, single variant `TargetIndependent`), `rewrite_scan` :222, `required_rewrite_budget` :226, `function_transports` :239, `function_scan` :243, `charge_steps` :249, `integer_copy_row` :261, `declares_pair` :285, `carrier_pair` :299, `ConePropagation`/`cone_propagation` :322-330, `plan_function` :369, macros `receipt!` :383 and `sealed!` :396, `instruction` :404, `register_occurs_once` :427, `rewrite_bytes` :492, `shifted_boundary_settlements` :508, `constant_fold_steps` :536.

**How it decides.**
- `isolated_declaration` (:156): a declaration is "isolated" iff memory/trap/barrier/call/cleanup are all `None`/`Never`/`None`. Keyed on enum values.
- `integer_copy_row` (:261): the `copy_i64` constraint row must be a Use(0)->Def(1) pair with the given classes and no implicit uses/defs/clobbers and no fixed views/ties/early clobber.
- `carrier_pair` (:299): same shape on a concrete instruction.
- `register_occurs_once` (:427): counts operand mentions plus structural-binding, case-payload and binding mentions; true iff exactly one. Note `structural_bindings` only count `WholeValue`/`ScalarReference`/`Address` with Register base (:454-457) and not local-slot mentions, whereas `successor_mentions` (:64) also counts `local_slot_mentions(destination, ...)`. The two do not count the same set (read).
- `shifted_boundary_settlements` (:508): settlements at positions greater than the removed index move down by one; `position > body` is an error.

**Cases by shape.** `RegisterAllocationRuleTargetApplicability` has exactly one variant. `declares_pair`/`carrier_pair`: exactly two operands.

**Same job elsewhere.** `block_instructions` here (:41) is duplicated at liveness.rs:1215 and recovery_classification.rs:319. `successor_mentions` vs `register_occurs_once` vs `edge_values::has_edge_use` (liveness.rs:1652) each scan a successor's transports for a register id with different coverage: has_edge_use looks at case payload `Registers.argument` and binding `Registers.argument` only (argument side, not parameter).

**Unused/one-caller.** Many `pub` items; greps in the end section.

**Fail-open.** `charge_steps` (:249) compares steps only to `budget.validation_steps()`; stated nowhere. `cone_propagation`, `constant_fold_steps` are pure arithmetic with overflow errors. `last_definition_before` (:144) returns `None` for no definition (callers decide). None seen otherwise.

## A/spill_choice.rs (613 lines, read in full)

**Purpose.** Linear-scan simulation over each function's virtual registers in start-point order, using the legality candidates, to find the first register that cannot be given a physical view; at that point it records which active resident or the incoming register should be the spill victim (farthest end, then highest vreg id).

**Shape.** `compute::compute_terminal_spill_choices` (:48), `reject_constraint_topologies` (:105), `validate_roots` (:116), `compute_function` (:143), `validate_local_shape` (:335), `interval_bounds` (:359), `common_candidates` (:374), `early_clobber_conflicts` (:401), `first_free` (:452), `interferes` (:482); `choose_spill_victims` (:510); `SpillChoiceError` (:565).

**How it decides.**
- Policy only `SingleBlockFarthestEndThenHighestVregV1` (:68).
- Each register is one interval `[first span start, last span end)` (:359-372). Candidates = intersection of all spans' candidate lists and all early-clobber point candidate lists (:378-398); empty -> `NoCommonCandidate`.
- `first_free` (:452) iterates candidates in ascending view id; a candidate is blocked by an active resident that interferes (pair lookup in `ranges.interference`, :471) and whose view footprint overlaps (`footprints_overlap`), or by `early_clobber_conflicts` against every previously assigned register (:408-430).
- On first failure: returns early with the choice (`return Ok(FunctionSpillChoices{choice: Some(..)})`, :315); later registers are not examined. Victim = max of `(exclusive_end, vreg)` among the incoming register and residents that would free a view (`reclaimed_view`).
- Rejects: any tied pairs in the function (:109); registers with `entry_transitions` (:187); early clobbers when pressure occurs (:219); pressure when the register or any active resident spans more than one block or a fragment other than the single interval (:225-271, `validate_local_shape` :343-355).

**Cases by shape.** "Single block, one fragment, no tied pairs, no early clobbers at the pressure point." Named by the policy and `UnsupportedPressureShape`.

**Fail-open risks.** `choice: None` is returned when every register finds a free view in this simplified model (:329); a real allocator could still fail where this model does not (e.g. fragments in several blocks use one interval-wide candidate set, which is stricter, not looser; the interference set is block-local overlap). "open" whether this can under-report pressure; not traced.
Unconstructed error variants here: `IntervalOverflow`, `ChoiceMismatch`, `UsageMismatch`.

**Same job elsewhere.** Greedy view selection with footprint-overlap/interference test also in fixed_precolored_segment_homes.rs (`conflicts::build` :79, `placement::assign` :255); both use `footprints_overlap` and `ranges.interference` binary search (`interferes` here :482 vs `registers_interfere` there :138).

## src/atomic.rs (325 lines, read in full)

**Purpose.** Vocabulary for indivisible machine atomic accesses: width (8/16/32/64 bits), operation (10 kinds), and the combined `AtomicForm` with dense ordinal, machine tag encoding, and operand-role arithmetic.

**Shape.** `AtomicWidth` :17 (`ALL`, `bytes`, `ordinal`), `AtomicOperation` :47 (`ALL`, `input_count` :89, `observes_resident` :97, `retries_until_commit(arch)` :105, `modifies_resident`, `is_bitwise_fetch`), `AtomicForm` :120 (`COUNT` = 40, `ALL` const-built :129, `ordinal` :149, `from_ordinal` :153, `machine_tag` :161, `from_machine_tag` :169, `machine_payload` :177, `from_once_payload` :185, `from_access` :196, `observed_operand` :224, `operand_count` :232, `outcome_operand` :238, `output_operands` :246).

**How it decides.** `from_access` (:196) maps a Terminal Psi `AtomicAccessEvent` to a form only if the integer is not an address type, `event.ordering_is_legal()`, and bits are 8/16/32/64; event kind maps one-to-one onto the operation. Machine tag: non-once forms use `194 + ordinal`, `CompareExchangeOnce` uses the fixed tag 240 plus a width payload (:161-183); `from_machine_tag` accepts only 194..230 (:170). Keyed on enum value.

**Cases by shape.** Comment at :4: "baseline forms provide sequentially consistent machine ordering; weaker source requests may use that stronger realization". Retry-until-commit rule hard-codes AArch64 for FetchAdd/FetchSub/Swap/CompareExchange (:105-112).

**Fail-open.** None seen (`from_access` returns `None` for unsupported).

## src/constraints.rs (50 lines, read in full)

`SelectedConstraintKeys { rows: TargetRegisterEnvironmentConstraintKeys, invoke_no_normal: Vec<RegisterConstraintKey> }` (:18) with `Deref/DerefMut` to `rows` (:23, :31); `SelectedFixedInputConstraint` (:38: machine, source_value, parameter_index, register, fixed_view); `SelectedSelectionConstraints` (:47). Data only, no logic. Mirrored by the field-by-field copy in analyses/machine_effects.rs:246.

## src/effects.rs (26 lines, read in full)

Module root: declares `pub mod catalog; pub mod program;` and re-exports about 40 names from `catalog` (:9-21) and 5 from `program` (:22-26). No logic. (`effects/program.rs` is not in this unit.)

## src/blocks.rs (1018 lines, read in full)

**Purpose.** Storage for a function's basic blocks as `Vec<Arc<BlockEntry>>` so that cloning a function after recovery edits shares unchanged blocks. Each entry lazily derives (`OnceLock`) a digest, retained encoding, a `BlockLookup` summary (bitsets of register ids, unit masks, instruction id bitset, copy affinities, transport counts) and a `BlockSlotSummary` (frame-slot events). Every mutable route clears the derived data.

**Shape.** `SelectedBlocks(Vec<Arc<BlockEntry>>)` :30; `BlockEntry` :32 (block + five `OnceLock`/Option caches); `BlockLookup` :110 (private); `BlockSlotSummary` :57 and `BlockSlotEvent` :42; `BlockEntry::lookup` :190 (one pass building all summaries, ~310 lines), `slot_summary` :504, `digest` :176; `detach` :702 (`Arc::make_mut` then clears caches); query methods on `SelectedBlocks` :716-966; `replace_block_contents` :800 (passes the old encoding with unchanged prefix/suffix counts); `identity_digests` :964.

**How it decides (lookup summaries).**
- Upward virtual uses: `Use` operands not already killed by an earlier `Def` in the block (:276-278, :347-349); kills: `Def` operands only, recorded after the instruction's uses (:319-323, :364-368). `UseDef` does not set a kill bit.
- `has_constraint_operands` is true when any operand is `UseDef`, `early_clobber` or has `tied_to` (:281-286, :352-354), terminator included. Thus liveness.rs:849 skipping blocks without the flag skips only blocks with none of those operand kinds (traced: closes the liveness.rs `reject_unsupported_constraints` filter question; `UseDef` is part of the flag).
- Unit masks: units with id < 64 in `u64` masks, others in `Vec`; `has_high_units_or_views` is true if any unit/view id >= 64 was seen (:436-439).
- `block_virtual_transfer` etc. return `None` only for an out-of-range block index (:882-885); the lookup is computed for every in-range block, so liveness.rs:281-285 `unwrap_or((&[], &[]))` and :306-332 do not fire for valid indexes (traced).
- `block_register_instruction_span` (:770): for blocks with fewer than 64 instructions returns the whole body `(0, len-1)` without checking the register's actual positions; for 64+ it uses recorded spans, where the high bit of `.1` marks "has an intermediate occurrence" (:298-305).
- `slot_summary` (:504): classifies each instruction into events keyed on `SelectedInstructionKind`: `Store64`/`FrameAddress` to a `FrameStorageSlotId::Local` slot at offset 0 record events, non-zero offsets or `HostedReadByte/HostedWriteByteI32` invalidate the slot, `Load64 { byte_offset: 0 }` records loads; anything else marks its operand registers as "non-load" (:514-596). Edge structural transports invalidate their destination slots (:638-669).

**Cases by shape.** Slot summary: only `Local` frame slots; offset 0 is the only forwardable form (`Load64 { byte_offset: 0 }`, `Store64` with `byte_offset == 0`).

**Asymmetries (read).**
- The instruction loop sets `fixed_definition_bits` for a non-Use operand with a fixed view (:314-316); the terminator loop (:346-363) records fixed views in the mask but never sets `fixed_definition_bits`, `materialize_definition_bits` or `register_instruction_spans`. So `has_fixed_definition` (:717) ignores terminator definitions. Status: "open" (callers are outside this unit; grep below).
- `copy_affinities`/`has_malformed_copy` consider body instructions only, not the terminator (:255-268).

**Same job elsewhere.** Copy-affinity derivation appears here (:255-268) and again in live_ranges.rs:601-619 as the fallback when `block_copy_affinities` returns `None` (which for in-range blocks never happens: :869-872). Block-id-to-index resolution: `SelectedFunction::block_index` (control_flow.rs:45), `liveness::flow` dense/sparse tables (liveness.rs:151-201), `FunctionIndex::block_index` (liveness.rs:1640).

**Fail-open.** `block_references_register` and `block_unit_*` return false/None for bad indexes (callers may treat false as "not referenced"); not traced.

## src/control_flow.rs (398 lines, read in full)

**Purpose.** The data types for a selected function: `SelectedFunction` (:23), `SelectedBlock` (:119), `SelectedBlockOrigin` (:127: Source, Operation, CaseDispatch, EdgeTransfer), `SelectedTerminator` (:173, 10 variants), `SelectedSuccessor` (:223), `SelectedSuccessorRole` (:165), structural/value transport enums (:257-305), plus helper functions and two pattern macros.

**Key functions.** `SelectedFunction::block_index` (:45) requires `blocks[id].id == id` (id is position); `virtual_register` (:53) same for registers; `ids_are_positions` (:59); `is_entry_register` (:95): `EntryParameter`/`StructuralParameter` always, or an `AbiTransport` with instruction id 0, byte offset 0, an entry fixed view, no definition site, and whose place equals the structural signature's result place. `terminator_instruction` (:324), `terminator_successors` (:370, allocates a `Vec`), `terminator_return_edge` (:332), macros `terminator_no_successor_pattern!` (:381) and `conditional_branch_less_pair_pattern!` (:393). `SelectedBlock::source_block` (:154) returns the EdgeTransfer's `target` for edge-transfer blocks.

**Same job elsewhere.** The successor list per terminator kind is written three times: liveness.rs:27-45 (`control::successors`, via the macros), control_flow.rs:341-378 (`terminator_successors_match!`), liveness.rs:1654-1665 (`has_edge_use`, via the macros). Block id/position assumption is repeated.

**Stated limits.** Comment at :225: `psi_edge` "Absent only for operation-internal control". Comment at :26: nominal_cleanup_subject is "Exact owner-designated cleanup subject".

**Fail-open.** None seen (data definitions).

## src/calls.rs (1488 lines, read in full)

**Purpose.** Four sub-modules: how call instructions get their physical register constraints (`constraints`), how the instruction run after a normalized foreign call is recognised (`normalized_foreign::return_transport`), the plain data types for frame slots, calls, memory accesses and boundary settlements (`ordinary`), and the dynamic-call selector-register instantiation (`dynamic`).

**Shape.**
- `constraints` (:14): `foreign_call_record` :33, `foreign_argument_registers` :55, `instantiate_foreign_call_constraint` :85, helpers `register_view` :254, `append_operand` :263.
- `normalized_foreign` (:282): `SelectedNormalizedForeignCall` :288; `return_transport::{ReturnTransport :307, resolve :314, validate_indirect_destination :500, result_buffer_address :562, follow_copies :590, copy_result :619, save_position :686, private_transport :725}`; `result_placement` :954.
- `ordinary` (:959): `OutgoingArgumentSlotId` :968, `LocalStorageSlotId` :992 (10 variants), `FrameStorageSlotId` :1122, `SelectedLocalStorageSlot` :1133, `SelectedOutgoingArgumentSlot` :1140, `SelectedCallContract` :1167, `SelectedMemoryAccessRole` :1176 (about 21 variants), `SelectedMemoryAccess` :1264, `SelectedBoundarySettlement(+Payload)` :1293-1300, identity encoders `encode_identity` :1061 and `canonical_bytes` :1331.
- `dynamic` (:1377): `call_record` :1389, `instantiate_constraint` :1403, `instantiate_declaration` :1442, `encoded_effects` :1469.

**How it decides.**
- `instantiate_foreign_call_constraint` returns `None` (refuses) unless: base row has no operands and the constraint family is `Call` for Import/Component/Table or `SystemCall` for syscalls; architecture matches; the boundary-entry plan re-validates to itself; callbacks count equals the plan's callback materializations; each callback's registrar plan, context, pointer shape, placement and destination agree (:98-148). Callback destination `Parameter` requires a single register or stack location; `Field` requires a single register location and field placements (:149-186). Scratch registers for memory destinations must not collide with parameter or indirect-result registers (:199-213).
- Argument registers are ordered with float-class placements after integer ones (`sort_by_key(... == ValueClass::Float)`, :72, stable sort) and callbacks that sit in a `Parameter` destination are skipped (:62-71). Keyed on placement class and position.
- `append_operand` (:263): for a `Def` operand removes the register's write units from `row.clobbers` (:276-278).
- `return_transport::resolve` (:314-498): the call must appear exactly once in the block; `ExternalCompletion` sites require unit result, no placement, no callbacks, the call as the last body instruction and a `HostedExitProcess` terminator with exact provenance (:328-354); otherwise the result is accepted in three exact shapes (indirect scalar via a boundary-result slot, scalar register result, structural result in registers or indirect), followed by only `CopyI64` copies that pass `copy_result` (:619-684), an optional `RestoreFloatingControl` for `same_stack_contribution`, and for an indirect scalar a `FrameAddress` + width-matched `Load8/16/32/64`. Anything else returns `None`.
- `save_position` (:686): exactly one `SaveFloatingControl` for the operation's boundary slot before the call, and every instruction between must carry that operation or be a "private transport" (:712-721). `private_transport` (:725) allows only `CopyI64`, `FrameAddress` of a `Spill` slot at offset 0 with an 8/8 slot, and `Load64 { byte_offset: 0 }` from such an address.
- Dynamic calls: selector register named by string, `"x16"` on Aarch64 and `"r11"` on X86_64 (`view_named`, :1407-1411); the selector use is inserted after the leading run of Use operands (:1420-1424). `instantiate_declaration` forces memory `ReadPointerV1`, trap `MayArchitecturalFaultV1`, alternative variant 1 and exact size 36 (Aarch64) or 24 (X86_64) bytes (:1454-1462).

**Cases by shape.** Return-transport shapes above; "private transport" three-kind list; callback destination two kinds; dynamic selector fixed registers and fixed encoded sizes (hard-coded byte counts at :1460-1461).

**Stated limits.** None as comments. `encoded_effects` takes `architecture` and discards it (`let _ = architecture;`, :1485).

**Fail-open risks.** None seen: every unmatched shape returns `None`, which callers (machine_effects.rs:794-811, 837-865) turn into `ConstraintEffectMismatch`. `foreign_call_record` (:33-53) checks the record by ordinal and by id uniqueness.

**Same job elsewhere.** `copy_result` (:619) and `private_transport`'s `CopyI64` arm (:740-762) and `rewriting::carrier_pair` / `integer_copy_row` (analyses/src/rewriting.rs:261-320) all test "two-operand `CopyI64` without fixed views/ties/early clobber"; written three times in different strengths. Frame slot identity encoding `LocalStorageSlotId::encode_identity` (:1061) vs the byte tag lists in effects/catalog/identity.rs (separate vocabularies).

## src/effects/catalog.rs (1685 lines, read in full)

**Purpose.** The data model of a target's machine-effect catalog: the inventory of semantic instruction kinds (`MachineSemanticKind`, built by a macro from `fixed` kinds, 5 `family` entries that expand per carrier, and 5 `dynamic` form families; the fixed count was not counted), their constraint-key mapping, alternative families and applicability rules, effect enums, the validated catalog and its structural validation.

**Shape.**
- `constraint_keys` (:8): `SelectedConstraintKeys::{ordinary_call_constraints :17, in_identity_order :33, for_semantic :163, declaration_keys :403}`.
- `validate_machine_effect_catalog` (:478); `MachineSemanticKind` generated by `machine_semantic_kinds!` (:528-694) with `FIXED` and `ALL` const arrays (:553, :697); `MachineAlternativeFamily` (:757); `From<MachineSemanticKind> for MachineAlternativeFamily` (:880, exhaustive); `MachineAlternativeKey` :1021; `MachineAlternativeApplicability` :1027 (7 variants); effect enums :1059-1126; `MachineEncodedEffects` :1134 with constructors :1147-1230; encoded memory/stack/trap/control enums :1234-1396; `MachineAlternative` :1399; `MachineEffectDeclaration` :1408 (`call` :1420, `single` :1475); `MachineEffectCatalog` :1505; `ValidatedMachineEffectCatalog` :1542; `MachineEffectCatalogValidationError` :1558 (13 variants); `canonical_machine_effect_catalog` :1582 (process-wide `static CANONICAL`, :1595); `validate_isa_machine_effect_catalog` :1624; macro `isa_machine_effect_catalog_plumbing!` :1645.

**How it decides.**
- `for_semantic` (:163-401) maps each `MachineSemanticKind` to one of the target's constraint keys. Many semantics share a key: `BitwiseAndI64/OrI64/XorI64` -> `subtract_i64` (:329-331); `BitwiseNotI64` and all zero/sign-extends -> `copy_i64` (:319-332); all five `MaterializeBoolean*` -> `materialize_boolean`; `ByteViewAddress`, `ExactAddI64`, `WrappingAddI64` -> `add_i64`; both ordered conditional branches -> `conditional_branch`; unsigned `SaturatingDivide/Remainder` reuse `divide_u64/remainder_u64`. `CallUnit`, `InvokeNoNormal`, `CallScalar`, `CallAggregate`, `ReturnAggregate`, `NormalizedForeignCall` return `None` (their keys come from rosters in `declaration_keys`, :403-452).
- Float arithmetic key choice (:226-257): rounding-direction ops use directed unary/binary rows by operand count; otherwise one-operand ops go to `ieee_float_conversion_control` when the operation is `IsNan/IsInfinite/IsFinite/IsNormal/IsSubnormal/Classify`, `FromInteger(U64)`, `TruncateToInteger{target: U64}` or `TruncateToInteger{domain: Trapping}`, else `ieee_float_unary`; two -> arithmetic; three -> ternary; other counts `None`.
- `validate_machine_effect_catalog` (:478-524): architecture equal, constraint identity equal, `invoke_no_normal` equals the ordinary-call roster, no duplicate key in `in_identity_order()`, declarations sorted by semantic, declaration list exactly equals `declaration_keys()` in order, every declaration's constraint exists and passes `validate_declaration` (validation.rs, outside unit).
- `validate_isa_machine_effect_catalog` (:1624): a catalog is accepted only if it equals the canonical one rebuilt by the ISA's builder (`validated.catalog() != &*canonical` -> `SemanticMismatch`, :1638).

**Cases by shape.** The `FIXED` list (:563-686) is an explicit roster; adding a semantic requires editing this list, `From`, `for_semantic`, `semantic_kind_tag`, and `machine_semantic_kind` in analyses/machine_effects.rs:473.

**Asymmetries / observations (read).**
- `in_identity_order` (:33-161) does not list `scan_byte_sequence`, `compare_byte_sequences`, `load_packed` or `store_packed`; these keys are therefore outside the duplicate-key check at :492 and outside the key list hashed at identity.rs:101-105. They still enter the identity through each declaration's own constraint key (identity.rs:110) and the declaration roster check (:502-513) (traced).
- `atomic_bitwise_update`, `ieee_float_directed_binary`, `ieee_float_directed_unary`, `ieee_float_conversion_control` are dropped from `in_identity_order` when equal to a sibling key (:65, :94, :99, :105) so aliasing is allowed for those only.
- Two process-wide mutable caches exist for catalogs: `CANONICAL` here (:1595, keyed on target + constraint identity; the `build` closure's identity is not part of the key) and `CATALOGS` in analyses/machine_effects.rs:63.

**Fail-open risks.**
- `validate_machine_effect_catalog` does not itself check alternatives; it delegates to `validation::validate_declaration` (outside unit). "open" for the unit.
- `canonical_machine_effect_catalog` cache (:1599-1613): poisoned lock -> rebuild; fail closed. A cached canonical catalog is returned for the same (target, constraint identity) even if `build` differs: wrong only if two different builders are used with the same key (suspected, not checked).

**Unused/one-caller.** `for_semantic` used in 4 files (grep above: catalog.rs 4, identity.rs tests, 03 selection x7, isa machine_effects x4). `MachineEffectDeclaration::single` / `call` etc. are used by the ISA crates (suspected; not grepped).

## src/effects/catalog/identity.rs (2335 lines, read in full)

**Purpose.** Canonical byte encoding and SHA-256 identity of a `MachineEffectCatalog`: constraint-key inventory, each declaration's semantic tag, effects, and alternatives; plus the pinned numeric tags per semantic kind / alternative family and tests that pin them.

**Shape.** `MachineIdentityBytes` trait (:18) with impls for `Vec<u8>`, `sha2::Sha256`, `DomainDigestEncoder` (:22-38); `machine_effect_catalog_identity` (:40, domain string `omega.terminal-machine-effect-catalog.v43`); `encode_machine_alternative_identity` (:185); `encode_machine_encoded_effects_identity` (:268); `effect_tag_arms!` macro (:575-710) used by `semantic_kind_tag` (:712) and `alternative_family_tag` (:726); `semantic_kind_payload` (:716), `alternative_family_payload` (:730); `saturating_family_tag` (:759), `trapping_family_tag` (:777, `122 + ordinal`); tests :781-2335 (about 1550 lines).

**How it decides (tag assignment).** Every enum is mapped to a hand-written byte or u16. Semantic and family tags are the same table (`effect_tag_arms!` is expanded for both enums, :713, :727). Dynamic forms share a fixed family tag plus a payload byte: float arithmetic 230, float result policy 231 (`FAMILY_TAG` asserted in tests :2234, :2274), atomics 194..229 (non-once) or 240 (once, payload = width), trapping 122+ordinal, saturating per `saturating_family_tag`.

**Tag alias found (read, grep).** `MachineEncodedMemoryEffect::ReadTableFieldV1 { pointer_operand, byte_offset }` is encoded with leading byte 19 (:282) and `MachineEncodedMemoryEffect::CatalogFenceV1` is also encoded with leading byte 19 (:440). Both variants are live (grep `ReadTableFieldV1`: 12 matches in 8 files incl. isa_x86_64/isa_aarch64 machine_effects.rs; `CatalogFenceV1|catalog_fence_v1`: 17 matches in 9 files). The pinned-bytes test `encoded_memory_effect_identity_bytes_are_pinned` (:1880-1968) covers 11 memory variants and does not include either of these two. What could collide: a `ReadTableFieldV1` effect and a `CatalogFenceV1` effect whose following bytes (the six operand/offset bytes versus the stack/trap/control bytes) line up; I did not construct a concrete collision. Status: "open".
Also unused numbers: memory tag 24 and 0 is `NoneV1`.

**Other facts.** Identity writes `u8::from(key.is_some())` for 38 optional keys (:47-87), then lengths of the five roster vectors, the `invoke_no_normal` key list, `in_identity_order()` keys, then all declarations (:88-181). `OptimizationSelected` applicability encodes `optimization as u8` (:239), so reordering the `Optimization` enum changes identities silently. Tests assert uniqueness of (tag, payload) across `MachineSemanticKind::ALL` (:1386-1395, :1764-1773), that `Crash` and `AtomicFence` tags alias nothing (:818, :2318), pin every alternative family tag (:1548-1700), trapping tags (:1728), atomic tags (:1777), and byte layouts for memory (except the two above), stack (3 of 4 variants; `TemporaryBytesV1` not in the case list :1972-1988), trap, control, applicability (6 of 7 variants; `OptimizationSelected` not in the list :2104-2145) and size.

**Fail-open.** None in the sense of accepting; identity collisions are the only risk (above).

**Same job elsewhere.** The `MachineSemanticKind` -> `SelectedInstructionKind` mapping in analyses/machine_effects.rs:473 and the key mapping in catalog.rs:163 and the tag mapping here are three parallel exhaustive tables over the same kinds.

# Greps run for the "unused / one-caller / never constructed" questions

All with the Grep tool across `omega-rust` in this checkout.
- `LiveRangeError::(VirtualRegisterMismatch|ArchitecturalUnitMismatch|InterferenceMismatch|TiedPairMismatch|EarlyClobberMismatch|NonCanonicalRows)`, `LivenessError::(RootMismatch|BlockMismatch|InstructionMismatch|SuccessorMismatch|NonCanonicalSet|TransferMismatch|FixedConstraintMismatch)`, `AllocationLegalityError::(FunctionMismatch|VirtualRegisterMismatch|NonCanonicalRows)`, `MachineEffectError::(NonCanonicalFunction|NonCanonicalBlock|InstructionMismatch|IdentityMismatch)`, `SpillChoiceError::(IntervalOverflow|ChoiceMismatch|UsageMismatch)`, `RecoveryClassificationError::(ClassificationMismatch|UsageMismatch)`, `FixedPrecoloredIntervalError::(UsageMismatch|NonCanonicalFunctions)`, `FixedPrecoloredSplitRequirementError::(UsageMismatch|NonCanonicalFunctions)`: 0 matches. These variants are declared and never constructed by qualified path (a `use Enum::*` construction was not searched).
- `AllocatorAvailabilityError::(NonCanonicalPlan|PlanMismatch)`, `FixedPrecoloredSegmentHomeError::(UsageMismatch|NonCanonicalFunctions)`: 0 matches. `FixedPrecoloredSegmentHomeError::SegmentMismatch`: 1 construction (fixed_precolored_segment_homes.rs:975).
- `ValidatedPreAllocationMachineEffects::new`: 1 call site (analyses/machine_effects.rs:881).
- `has_fixed_definition`: callers 05/runtime_spill_and_split.rs:4951 and 05/assignment.rs:1537 (plus 3 test uses in plan_identity.rs); `has_materialize_definition`: 1 caller, 05/runtime_spill_and_split.rs:115.
- `compute_function_units_filtered`: 1 caller, 05/runtime_spill_and_split.rs:3059. `block_unit_writer_indices`: 1 caller, 05/runtime_spill_and_split.rs:4466.
- `choose_spill_victims` and `classify_pressure_recovery`: call sites in 05/rewrites.rs:54,66 and 04/literal_fold_rewrites.rs:569,581. `analyze_fixed_precolored_intervals`, `analyze_fixed_precolored_split_requirements`, `assign_fixed_precolored_segment_homes`: one call site each in 05/rewrites.rs:1214, :1221, :1230 (I did not check whether that region is test-only).
- Combined `last_definition_before|isolated_declaration|effect_declaration|member_position|block_instruction_ids|edge_surface|charge_steps|integer_copy_row|carrier_pair|constant_fold_steps|shifted_boundary_settlements|register_occurs_once|copy_shaped|declares_pair|required_rewrite_budget|plan_function|cone_propagation|local_slot_mentions|successor_mentions|function_transports|rewrite_scan` followed by `(`: 125 matches in 9 files, 24 of them inside analyses/src/rewriting.rs and the rest in pipeline 04 (literal_fold, copy_and_extension, branch_and_compare, address_and_load, rewrites) and 05 (runtime_spill_and_split, rewrites, fixed_view_copy_recovery). From the name list for 13 of the helpers, every one has at least one pipeline caller; `edge_surface` is used only in 04/branch_and_compare_rewrites.rs; `shifted_boundary_settlements` in 04/copy_and_extension_rewrites.rs and branch_and_compare_rewrites.rs. Names `register_occurs_once`, `integer_copy_row`, `cone_propagation`, `local_slot_mentions`, `successor_mentions`, `effect_declaration`, `function_transports`, `rewrite_scan` were in the count pattern but not split out per name.
- `work_accounting::|WorkDefect`: 60 matches in 13 files; `Work` is used by 6 analysis files and pipeline 04/05 rewrites (counts above). `combine` and `admit` in work_accounting.rs not separately grepped.
- "Shared" claims: lib.rs header says the crate lets stages "share the exact dataflow and interference engines" (lib.rs:26-28); the outside users are pipeline 04 (`04_.../src/lib.rs`, `selected_optimization.rs`, `literal_fold_rewrites.rs`) and 05 (`assignment.rs`, `rewrites.rs`, `runtime_spill_and_split.rs`, `fixed_view_copy_recovery.rs`), which does confirm outside use. `allocation_legality::validate::admit` and `FunctionCounts` are `pub(crate)`/crate-internal but `UnsealedRecoveryFacts::seal` (lib.rs:1081) is its other caller.

# Coverage

Read in full (all lines, in order):
- analyses/src/allocation_legality.rs 1-1301 (1301)
- analyses/src/allocator_availability.rs 1-196 (196)
- analyses/src/fixed_precolored_intervals.rs 1-361 (361)
- analyses/src/fixed_precolored_segment_homes.rs 1-1277 (1277)
- analyses/src/fixed_precolored_split_requirements.rs 1-922 (922)
- analyses/src/legality.rs 1-296 (296)
- analyses/src/lib.rs 1-1209 (1209)
- analyses/src/live_ranges.rs 1-1849 (1849)
- analyses/src/liveness.rs 1-2312 (2312)
- analyses/src/machine_effects.rs 1-1035 (1035)
- analyses/src/recovery_classification.rs 1-558 (558)
- analyses/src/rewriting.rs 1-566 (566)
- analyses/src/spill_choice.rs 1-613 (613)
- analyses/src/work_accounting.rs 1-179 (179)
- src/atomic.rs 1-325 (325)
- src/blocks.rs 1-1018 (1018)
- src/calls.rs 1-1488 (1488)
- src/constraints.rs 1-50 (50)
- src/control_flow.rs 1-398 (398)
- src/effects.rs 1-26 (26)
- src/effects/catalog.rs 1-1685 (1685)
- src/effects/catalog/identity.rs 1-2335 (2335)
Total read in full: 19,999 lines. Skimmed: 0. Not read: 0.
Note on depth: the test modules in live_ranges.rs (about 330 lines of tests), liveness.rs `edge_values::tests` (about 220), calls.rs `tests` (about 120), lib.rs tests (120) and identity.rs tests (about 1550 lines) were read but are recorded in less detail than production code.

# Five things a maintainer most needs to know about this unit

1. Almost everything in `analyses/` is a "compute plan, then seal" pair; the `seal_*` / `validate::admit` steps only hash and count (`seal_plan!` lib.rs:234, `live_ranges::validate::admit` live_ranges.rs:1555, `admit_validated_liveness` liveness.rs:2087, `seal_pre_allocation_machine_effects` machine_effects.rs:877) and never re-check the rows. The "ValidatedX" types are therefore receipts of the producer's own work, not independent validation.
2. Incremental recovery is built on `Arc` row sharing and function-equality carry (`carried_digests` lib.rs:855, `compute_terminal_*_reusing`, `UnsealedRecoveryFacts` lib.rs:879). A function is "unchanged" iff its `SelectedFunction` shares storage or compares `==` with the previous one; digests and counts of carried rows are copied without recomputation.
3. Liveness has two forms: the full one and `compute_function_for_ranges` (liveness.rs:748) which leaves per-instruction virtual use/def/live lists empty; live_ranges.rs:752-813 has a matching branch that reads operands directly. `UnsealedRecoveryFacts::seal` (lib.rs:1007-1014) later upgrades to the full form and ignores failure of that upgrade, which cannot fail for any input the cheap form accepted (traced).
4. The early-clobber / tied-operand / UseDef rules are implemented in two places with different code (liveness.rs:839-1047 and live_ranges.rs:204-448), and `fixed_precolored_split_requirements.rs` and `spill_choice.rs` each reject tied registers again (:325, :109). The spill-choice and split-requirement analyses only support narrow shapes (single block, single fragment, one incoming edge per block, no tied pairs, no early clobbers at the pressure point).
5. Machine-effect identity is a hand-numbered byte layout (`effect_tag_arms!`, identity.rs:575-710 and the match arms at :111-181 and :277-558). One alias exists (tag 19 for both `ReadTableFieldV1` and `CatalogFenceV1`, identity.rs:282 and :440); tests pin many but not all variants. Catalogs are additionally cached in two process-wide mutable statics (catalog.rs:1595, machine_effects.rs:63).

# What I am unsure about

- Whether `CutIndex::finish` (fixed_precolored_split_requirements.rs:169) can ever fail: by reading, every `used` entry is derived from the same `transitions` map it is checked against, so it looks tautological; I did not run anything. And whether ignoring the return of `use_transition` at :455 can accept a mid-fragment cut with no declared transition (allocation_legality.rs:145-181 builds transitions for every Use fixed constraint when two or more views are pinned, which suggests it always has one, but I did not trace a counterexample with a single pinned view).
- Whether the terminator omissions in `BlockLookup` (blocks.rs:346-363 vs :314-316, `fixed_definition_bits`, `materialize_definition_bits`, `register_instruction_spans`) matter: depends on whether any terminator has a `Def` operand with a fixed view; callers are in pipeline 05, which I did not read.
- Whether the tag aliasing at identity.rs:282/:440 can produce an actual identity collision (needs following-byte alignment) and whether any ISA catalog uses both variants in the same position.
- Whether `fixed_precolored_*` call sites at 05/rewrites.rs:1214-1230 are production or test code.
- Whether the spill-choice simulation (spill_choice.rs) can report `choice: None` (no pressure) in a case where the real allocator would run out of views (fragments in different blocks are given one interval-wide candidate set, which is stricter, but interference is block-local overlap only).
- Tag uniqueness across all semantic/family tags for the dynamic families (float 230/231, atomics 194-229/240, trapping 122+, saturating) depends on carrier and form counts defined outside this unit; the in-file tests assert uniqueness of (tag, payload) for `MachineSemanticKind::ALL` but I did not run them.

