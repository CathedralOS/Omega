> Later production-route inspection corrected the phase attribution in this initial note: the measured family executes in Psi 06 before Terminal publication. See [result-review.md](result-review.md) and the final [result](README.md).

# Copy pressure: screen existing forwarding before changing edge capture

Read-only research, 2026-10-29, source inspected at `bdf900b6c810ed0a091290bd35a46668c33bf35a`; parent later reported timer-only `4ec622160d3`. Only this ignored note was written. No builds, program executions, compiler/config/Git edits or other-chat messages.

**Next screen: existing Stage 01 `CopyPropagation`, using the frozen optimized compiler and one private Build selecting that rule.** It already removes redundant block parameters before selected edge copies exist. The two-copy edge capture is a concrete subsequent candidate, but an empty emitted-work middle alone does not preserve allocatability on permutations/backedges. Do not implement that shortcut unconditionally.

## Existing forwarding and exact selection route

Paths below are under `omega-rust/omega/compilation/`, unless stated otherwise.

Stage 01 `pipeline/01_abstract-operations-to-abstract-operations/src/rules/copy_propagation/redundant_block_parameter.rs:35–157` gathers every incoming binding of a non-entry block parameter. It requires all incoming arguments to name the same replacement, refuses self-replacement and requires `replacement_dominates_parameter_uses`. It carries exact type, incoming-edge witness, affected blocks and provenance/fuel realization into `RedundantBlockParameterRewrite`. Its independent checker, `representations/abstract-operations/optimization-unit/semantics/src/candidates/copy_propagation/validation.rs`, reconstructs the incoming roster, type/position/substitution, removes the parameter, rewrites operations/bindings and reconstructs derived definitions/uses/ownership and closed-region observations. Different incoming values and non-dominating replacements remain distinct. This is an existing admitted forwarding operation, not an equivalence inferred from selected source IDs.

Stage 01 `rules/dead_scalar_elimination.rs`, its two classifier leaves and `rules/node_elision.rs:65–140` remove classified unused **operation nodes** under liveness/pure-effect checks, preserving realization/provenance/fuel. The inspected leaves classify literals and unconditionally total scalar operations; this is not a generic dead-parameter sweep. Do not combine it with the first screen.

The old spill attribution at `eb8e2e461a0` has 965 block-parameter and 1,161 `CopyI64` victim choices, 1,989 distinct function/source-value pairs and no repeated exact origin. These counts make forwarding worth screening, but do not establish eligibility or new timing.

Optimizations are empty-by-default and authored by the root Build (`wiki/spec/build/configuration.md`; `omega-rust/omega/foundation/optimization-core/src/optimization_core.rs:500`). The stack32 fact's `build.omg` contains target bindings and no optimizer selection. Existing `tests/omega/pass/optimizer/rollback_copy_propagation_selected/build.omg` supplies the exact syntax:

```omega
Optimizations::enable(&mut builder.optimizations, Optimization::CopyPropagation);
```

There is **no CLI enable flag**. `application/cli/src/cli/arguments/compile.rs:101–109`, `optimization_core/rollback/request.rs`, `rollback.rs:40–82` and `wiki/spec/build/optimizations.md:213` implement exact-name subtraction. Therefore an unchanged empty Build/config cannot enable the pass by flags alone. A private, independently admitted copy of the frozen witness with only that Build addition can use the **same baseline producer and same authored private Build** for both legs:

```powershell
& $frozenProducer --target windows_x86_64 --timings --build-dir $offDir --disable-optimization CopyPropagation $privateMain
& $frozenProducer --target windows_x86_64 --timings --build-dir $onDir $privateMain
```

Proposed commands only. Keep main/oracle unchanged, fresh distinct outputs and controlled private caches; record the source/Build/policy identities. Read back rollback requested/applied/effective identities: off must be effective `[]`, on `CopyPropagation`. The existing selected/control rollback fixture README has a `module:differs` vacuity guard and restoration to the never-selected semantic module. Equivalent counts/report evidence must distinguish actual parameter removal from an identity run.

**Stop** if no relevant parameters are removed or its admitted route fails quality/resource requirements. If it removes many, measure complete compile cost including optimizer analysis/revalidation before implementing anything. Retain native/Terminal/reject/lookup and resource controls; changing selected code may legitimately change image bytes. Do not add SCCP, dead-scalar or selected-copy passes to this first causal screen.

## Why simply enabling Stage 04 copy removal is a different experiment

`pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs:17–65` returns identity without an executed selection; otherwise it executes one selected-lowering or pre-allocation slice. `rewrites/pre_allocation/catalog.rs:54` already lists `SelectedSameBlockCopyI64RemovalV1`.

`rewrites/copy_removal/admission.rs:117–291` requires the target's plain [Use, Def] copy; identical scalar type/class; unique copy-owned destination origin; no entry pin, second definition or intervening source redefinition; and only flexible later same-block operand uses. It refuses edge transport, fixed/tied/early-clobber uses, call/storage/memory references and dependent origins. Removal redensifies references, slots/contracts/successors and shifts settlements. Its seal binds whole-plan source/result hashes; independent replay is a separate boundary operation.

The pre-allocation driver `execution.rs:973–1013,1150–1309` restages liveness/ranges/legality after **each** committed transformation and meters a decreasing measure/budget. Moreover its Stage 04 entry uses `stage_optimized_allocation_legality_for_frameless_leaf`; `analyses/legality/policies.rs:28–68` restricts unconstrained views to caller-saved units. Enabling that pass changes availability policy as well as copies, so it is not an isolated copy A/B against ordinary all-environment allocation.

`rewrites/copied_call_operand.rs:25–75` already rebinds admitted call operands while retaining the producer and ABI contract; `load_forwarding.rs:1–70` preserves load result identity through a checked store/load relation. Neither removes arbitrary phi carriers. Copy affinities are preferences (`analyses/live_ranges/compute/copy_affinities.rs`); Stage 05 `assignment/same_home.rs` requires ties and live edge argument/parameter pairs to share a home. Hard-unioning all copy affinities would weaken the existing interference/constraint checks.

## Concrete Stage 03 double capture and its physical hazard

`pipeline/03_target-operations-to-selected-instructions/src/selection/edge_transfers/construction.rs:73–83,166–293` collects active scalar bindings and unconditionally emits:

```text
all A_i -> S_i snapshots
descriptor/reference snapshots; cleanup
all S_i -> T_i transfers
structural stores; continuation binds T_i -> parameter_i
```

Both copy outputs have fresh IDs, the argument's type/class and definition site, and exact argument provenance. The bridge and predecessor retain semantic edge/fuel roles; continuation fuel is cleared. `edge_transfers.rs:12–22` requires preparation before snapshots and cleanup after snapshots, off the predecessor so untaken edges do no speculative work and preserve branch flags.

The parent's proposed **emitted-work criterion** is more coherent than a pure-scalar feature list: emit phase 0, record the instruction frontier, run descriptor/reference snapshots and append cleanup unchanged, and reuse phase-0 outputs as transfers only if the frontier did not advance. Preparation/case snapshots remain before phase 0; all final stores and helper-produced metadata/slots remain. This proves absence of intervening emitted runtime instructions. It is not permission to skip actual work.

**It does not prove physical feasibility.** Reusing `S_i` as the continuation argument gives it the destination parameter's mandatory home **during the remaining captures**. On a backedge `x,y := y,x`, the shortened stream is `S_x = old_y; S_y = old_x`, with homes `S_x ≡ x` and `S_y ≡ y`. The first definition overlaps still-unread `old_x`, although that old parameter and `S_x` must share a home. The original snapshot phase reads both old values into unconstrained intermediates before the destination-associated transfers.

Stage 04 liveness records every operand/access (`analyses/liveness/compute.rs:274–310`), and `analyses/live_ranges/compute/fragments.rs` reconstructs use/def intervals and interference. This detects clobber conflicts; it does **not** guarantee a feasible assignment. Stage 05 `home_assignment/compute/domain.rs:245–290` rejects interference within a required home component, potentially as `UnsupportedEdgeTransfer`. `home_assignment/mod.rs:161–168` does not admit that error to runtime recovery. Thus this can reject previously compilable programs, not merely increase recoverable pressure. This is a source-grounded falsifier, not an executed failure result.

No inspected validator structurally mandates two copies: Stage 03 `selection/mod.rs:53–65`, `construction/mod.rs` and `identity.rs` construct/hash the actual checked plan; they do not replay a two-copy template. Later `analyses/liveness/edge_values.rs:321–394,411–522` checks bridge ownership, roles, empty continuation fuel, destination position and source value/type/class. `backend/machine-emission/src/fragment_emission/projection/validation/control.rs:190–207` preserves exact successor provenance. No chain-length predicate was found there. Those lineage checks are necessary, not a proof of arbitrary alternative copy schedules.

Before a source experiment, count exact emitted-empty middles, paired snapshot/transfer coordinates and victims by machine/edge/binding ordinal; shared `ValueId` alone is insufficient. If numerous, the missing design predicate is non-destructive scheduling under the complete mandatory-home relation, with cycle snapshots or an admitted fallback preserving the original plan. The empty middle criterion alone is insufficient; do not suppress interference, repair receipts manually or add feature combinations to hide the missing relation.

Nearest controls read (none run):
- `tests/omega/pass/optimizer/rollback_copy_propagation_{selected,control}`: selection/subtraction and semantic difference/restoration. Main has identical branch outcomes, so it is not a sufficient runtime substitution falsifier alone.
- `samples/cli/facts/thirtytwo_arg_stack_calling_plan`: distinct ordered arguments and result/headroom checks across places.
- `tests/omega/pass/calls/runtime_state_reference_permutation_exit`: observable three-way reference rotation; `control_flow/dispatch_loop_nonzero_unsigned_exit`: changing scalar backedge.
- `control_flow/case_arm_scalar_payload_arguments_exit`: duplicate payload argument to multiple parameters, a structural-case fallback control.
- `structural/primitive_array_snapshot_swap_exit`: observable [3,7] → [7,3] snapshot ordering.
- `drops/runtime_three_field_projected_transition_cleanup_exit`: outgoing children survive sibling disposal; `drops/runtime_two_nominal_parameter_loans_cleanup_order_exit`: observable OCED cleanup order.
- Stage 04 `analyses/liveness/edge_values/tests.rs`: operation trap/loop and forged lineage; fragment projection's `continuation_retains_exact_successor_role_and_destination`: rejects altered role/destination.

No direct scalar-capture permutation unit was located in the inspected Stage 03 edge module. A source prototype needs that direct-owner test first: distinct scalar swap/backedge with odd/even observable outcomes, longer permutation, duplicate source, source read after an earlier constrained capture, and nonempty cleanup/structural middle preserving the full schedule. Descriptor rotation is surrounding coverage, not a substitute.

## Block lookup rebuild: plausible repeated scan, no timing verdict

`representations/selected-instructions/src/selected_instructions/blocks.rs:171–473` lazily constructs a broad `BlockLookup` by scanning body/terminator/edges: instruction IDs, counts, implicit unit/fixed-view summaries, register bitsets/dense spans and copy affinities. Arbitrary mutable escape invalidates it (`:709–717`). Spill's `replace_block_contents` (`:836–866`) creates a new entry with an empty lookup while optionally retaining digest encoding chunks. Unchanged blocks keep their memos.

The next split/rematerialization negative filter, spill register-reference query, total-operand budget query, next-instruction-ID query or exclusion-mask query can force that whole rebuild. Split-negative results are already cached by the function roster; all subsequent queries on one unchanged entry share the rebuilt lookup. Do not assume one rebuild per queried victim.

A diagnostic can time **only the initializer**, count misses and scanned instructions/operands/dense-ID extent, and separate lookup from slot-summary/digest work. Compare inclusive time against derive/split/rematerialization without double-counting. This is repeated O(changed body + operands + transports + summary extent) work; dominance is unknown. Stop if small. If large, exact owner-local summary separation/update is a separate experiment, retaining invalidation on arbitrary mutation. It should not be combined with forwarding.

The prior busy-host `bb3f3c4` trace has 448 ms allocation / 1,762 ms process wall. That is scope headroom, not a forecast; approaching 20% overall would require eliminating much of allocation. Fewer copies or rebuilds alone establish no win. Do not revive cap32/private-slot/vector/container/PGO/LTO experiments unchanged.

## Primary-source research provenance

Read `wiki/drafts/reference/compiler_parallelism.md` and the earlier `spill_attribution_2026_10_29/research.md` copy-web section first. The latter already inspected regalloc2 `4a233442f781dd7b84d5ea40504d3be07263b3e3`; its exact non-overlap/constraint predicates do not authorize equivalence from a source ID.

One new exact query: `site:github.com/llvm/llvm-project PHIElimination.cpp IncomingReg PHIDefReg copy insertion`. Then read **LLVM llvmorg-20.1.4** [PHIElimination.cpp](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.4/llvm/lib/CodeGen/PHIElimination.cpp), [raw implementation](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-20.1.4/llvm/lib/CodeGen/PHIElimination.cpp). `LowerPHINode` at 330–370 creates/reuses an incoming join register and emits a separate destination copy after PHIs; 498–559 inserts source copies at safe predecessor locations, then updates liveness/kill positions. This supports separating captures from destination assignment. LLVM's coalescing/allocation differs from Omega's mandatory edge homes; it proves neither the proposed omission nor a speedup.
