# Stage 05 flow and checking reuse - 2026-10-24

Base: `main` at `9e49ea3065f`. Host: Windows x86_64, optimized `dev` profile
built with `tools/build_slot.py -- mbx build -p omega`. Workloads: the
`local` place of `samples/cli/facts/length`,
`samples/cli/facts/dyn_dispatch_selector_payload_bound` and
`samples/cli/facts/nested_nine_level_record_field_bound`, compiled for
`windows_x86_64` with `OMEGA_PHASE_PROFILE=1 --timings`. Each comparison runs
one warmup per producer, then alternates baseline and candidate processes in
ABBA order. The baseline producer is the unmodified `9e49ea3065f` binary. Every
published native image ran and exited 0. The SHA-256 of every image is
identical across producers.

Target set by the user: whole-program compilation under 1,000 ms
(`total elapsed`), with no stage over 1 s.

## Retained 1: cross-subphase checking memo retention

`CheckingContext::memos` is shared (`Arc<CheckingMemos>`). Rebuilding a context
with the same prepared effect plans or authored declaration selections keeps
it. Typed-tree memos keep `erased_initializers` and `call_frames`;
`with_prepared_mathematics` still resets `call_frames`. Write-frame demand
reuses the prebuilt `Arc<MachineSymbols>` from `program.call_frame_tables()`
and does not rebuild it per caller.

| phase (median, 3 rounds) | length base | length cand |
| --- | ---: | ---: |
| `05.d.3.c.5b.mutation_and_integer_writes` | 12.0 ms | 5.0 ms |
| `05.d.7.3b.check_flow_call_contracts` | 42.5 ms | 28.8 ms |
| `05.d.7.check_checked_facts` | 122.5 ms | 100.3 ms |

The other two workloads move in the same direction (see the table below).
`05.d.11_13` no longer spikes.

## Retained 2: one case-evidence batch per aggregate member walk

Per-group timing inside `05.d.3.b.flow_fixed_point.build_flow_facts` showed
that the critical path is not the largest machine group. It is the single
machine `std::targets::uefi_x86_64::UefiX86_64::plan`, whose states `build`
and `build_physical` each took about 105-120 ms of flow. Nested phase probes
narrowed this down: the final `BoundaryPlanResult::Accepted { plan: output }`
evaluates `constructed_values` for an aggregate child. For every captured
member fact, that function called `place_cases_are_selected`. Each call ran
`CaseEvidence::prepare` and rescanned every captured context. The state's
2,976 member facts drew from 30,645 captured context visits. That call
accounted for 165 of the 188 ms spent in `constructed_values`.

The evidence depends only on the captured contexts and the statement, and the
member walk reads it without changing it. `constructed_values` now prepares
`CaseEvidence` lazily, once per walk, on the first member place that crosses a
case. Every member still goes through `cases_are_selected`, so results do not
change. This is the batching that
`flow/place/case_access.rs` already documents.

Interleaved A/B, 5 rounds, medians (baseline = `9e49ea3065f`; candidate =
retained 1 + retained 2 + lazy mutation summaries):

| workload | wall base | wall cand | Stage 05 base | Stage 05 cand | `build_flow_facts` base | cand |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| length | 1146.3 | 1022.1 | 819.5 | 697.1 | 242.6 | 146.9 |
| dyn_dispatch | 1123.5 | 1070.0 | 808.6 | 732.0 | 244.1 | 145.8 |
| nested_record | 1149.8 | 1032.1 | 822.2 | 697.9 | 244.1 | 147.2 |

All values are in ms. Wall time is per process and includes about 80-120 ms of
Windows process start and teardown that `--timings` does not report. A single
profiled length run reports `total elapsed` falling from 1,070 to 984 ms.

## Retained 3: lazy state mutation summaries

`build_state_mutation_summaries` no longer infers write frames for every state
of every machine. It asks the shared resolver for
`inferred_state_write_frame` only for borrow-fact states whose direct summary
is complete and whose body signature is concrete; those are the only states
that consult the frame. Summary lookup goes through a symbol index that keeps
the first summary for a symbol, matching the former linear search.

## Rejected: first-pass machine page reuse and callers-first sweep order

Hypothesis: the confirmation pass of a multi-machine flow group could adopt
first-pass pages for machines whose inputs did not change. Measured result:
only 9 to 17 of the 39 swept machines were reusable, and state visits fell from
470 to 452 to 461. Rows created for callees that were swept before their
callers made most machines differ from their first-pass snapshots. A
callers-first sweep order did not change this. `build_flow_facts` moved by less
than run-to-run spread (−3.8 to +19 ms). Per-group timing also showed that the
largest group (53 machines, about 36 ms) was never the critical path. The
source changes were removed.

## Remaining profile (length, candidate)

`total elapsed` ~ 984 ms. The largest remaining Stage 05 phases are
`05.d.3.b.flow_fixed_point` (~219 ms), `05.c.prepare_typed_trees` (~123 ms),
`05.d.2.prepare_typed_program` (~96 ms), `05.d.7.check_checked_facts`
(~95 ms) and `05.d.8_10.finalize_execution_identities` (~63 ms). Every phase
remains under 1 s.

## Rebased onto 261a48af29f

After rebasing onto `261a48af29f`, the same harness compared that base, rebuilt, against the rebased candidate (5 interleaved rounds, medians):

| workload | Stage 05 base | Stage 05 cand | `build_flow_facts` base | cand | `check_checked_facts` base | cand |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| length | 876.1 | 755.1 | 231.3 | 171.4 | 136.3 | 100.5 |
| dyn_dispatch | 826.5 | 736.9 | 232.1 | 168.0 | 120.4 | 99.3 |
| nested_record | 861.1 | 752.4 | 234.7 | 169.9 | 121.7 | 98.2 |

Native images are SHA-identical across producers and exit 0.

## Iteration 2: arithmetic domain memoization, difference-graph components, state-loan spans, and contract/exit reuse (base `448f6e11ca5`)

Base `448f6e11ca5` on Windows x86_64. Retained changes:
- `CheckingMemos` memoizes `mathematical_expression_nodes`, `expression_result_types`, `ordered_integer_meanings`, and `parameter_requires_intervals` (`requires_interval` and `declared_ensures_result_interval`), and stores subtree height `Option<u8>` in `bound_subtree_meanings` keyed by `(u32, u32, ExpressionHandle)` instead of `(u32, u32, ExpressionHandle, u8)` depth.
- `DifferenceGraph::new` computes undirected connected components (`component: Vec<u32>`) and endpoint direction flags (`has_outgoing`, `has_incoming`), skipping Bellman-Ford (`nonnegative_gap`) whenever `source` and `destination` lie in different components or lack incident edges. `refine_place_interval` applies exact `subject == candidate` bounds before consulting congruence and skips relations whose endpoint cannot tighten `interval`. `excludes_integer_value` checks whether the literal relation excludes `value` before calling `comparison.same_value`.
- `ArrivalWalk` and `incoming_environments` skip fixed-point iteration on states without in-machine state transitions/calls and skip condition narrowing on `Terminal` (`-> !`) transition arms; `RequirementSeeds` computes `self.required` lazily instead of reseeding after `incoming_environments`.
- `CurrentScalarValues::bind_parameters` batches argument bound queries via `occurrence_integer_expressions_bounds`; `check_call_requires` caches whole-clause `(contract_handle, bool)` verdicts per call site; `check_exit_ensures` reuses one `CurrentScalarValues` instance per exit and short-circuits proved entailment/evidence-assignment clauses before building `missing_origins`.
- `BorrowFacts::state_loans` slices each state's `HandleSpan<BorrowLoanFact>` instead of scanning `borrow.loans.iter()` across the program; empty-plan fast paths guard `conservation_plans`, `indexed_families`, `source_applications`, and `BorrowedStorageWindows`.

On the 3 standard fact workloads (`length`, `dyn_dispatch`, `nested_record`, 5 interleaved rounds under concurrent host load), `05.c.prepare_typed_trees` improves by 25.9–97.0 ms (`263.6 -> 166.7 ms` on `length`), `05.d.2.prepare_typed_program` improves by 21.4–87.3 ms (`220.9 -> 133.6 ms` on `dyn_dispatch`), and all emitted native images are SHA-256 identical and exit 0.

On multi-argument calling-plan witnesses (`--check --timings` with `OMEGA_PHASE_PROFILE=1`):

| witness | Stage 05 base (`448f6e11ca5`) | Stage 05 cand | `prepare_typed_program` base | cand | `check_flow_call_contracts` base | cand | `ordered_relations.rows` base | cand |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `sixteen_arg_stack_calling_plan/other_machine` | 6,164.6 ms | 1,444.6 ms (4.27x) | 642.1 ms | 146.9 ms | 4,028.1 ms | 41.2 ms (97.7x) | 26,956 | 456 |
| `thirtytwo_arg_stack_calling_plan/other_machine` | > 95,000 ms | 1,900.0 ms (> 50x) | — | 559.1 ms | — | 190.6 ms | — | 616 |

## Iteration 3: backend register allocation (`06.1.d.05_stage_register_allocation`) dense O(1) indexing, memoized block liveness/slot summaries, and incremental single-function spill recovery (base `8023f4f24da7`)

Base `8023f4f24da7` on Windows x86_64. Retained changes across `selected-instructions`, `register-homes`, `04_selected-instructions-to-selected-instructions`, and `05_selected-instructions-to-register-homes`:
- **Incremental single-function spill recovery (`runtime_spill/recovery.rs`, `runtime_spill.rs`, `selected_instructions.rs`):** `recover_selected_instructions` previously ran the full whole-module `analyze_selected_instructions` pipeline on every spill iteration across all `~3,300` spilled registers on `thirtytwo_arg_stack_calling_plan/other_machine` (re-running `runtime_rematerialization`, `runtime_split`, and multi-function analyses when only one function was spilling). Added `SelectedInstructions::replace_function` and `reanalyze_spilled_function` to re-analyze only the spilling function (`liveness`, `live_ranges`, `allocation_legality`) in place during the spill recovery loop.
- **Dense `O(1)` virtual-register and instruction-point indexing (`live_ranges`, `liveness`, `allocation_legality`, `home_assignment`):** Replaced linear `.iter().find(...)` and `BTreeMap`/`BTreeSet` lookups over virtual registers, instruction points, block headers, and physical units with dense `Vec`/`u64`-bitset tables in `FunctionRanges` (`range_index_by_register`), `FunctionLiveness` (`point_index`, `block_index`, `register_point_indices`), `FunctionAllocationLegality` (`view_index`, `early_clobber_index`), `FunctionDomains` (`domain_index`), `PreparedConflicts` (`conflict_index` + dense `ConflictSet` bitset), `PreparedAffinities` (`by_register`, `move_related`), `same_home`, and `physical_requirement`.
- **Sorted interval pruning & single-pass block/point collection (`live_ranges/compute/fragments.rs`, `constraints.rs`, `architectural_units.rs`, `allocation_legality/compute/*`):** Sorted fragments and unit intervals by `start` and broke early when `interval.start > fragment.end`; replaced per-register full-function scans in `collect_register_fragments` with a single pass over `function_liveness.points` bucketed by `VirtualRegisterId`.
- **Single-pass `BlockLookup` memoization of unit liveness transfers and spill-slot summaries (`selected_instructions/blocks.rs`, `liveness/compute/flow.rs`, `liveness/compute.rs`, `runtime_spill/unit_effects.rs`, `runtime_spill/slot.rs`):** Extended `BlockLookup` (built once per block modification in `SelectedBlocks`) with `upward_unit_uses`, `unit_kills`, and `BlockSlotSummary` (`events`, `loads`, `non_load_registers`). `FunctionFlow::compute_domains` reads precomputed block unit transfers instead of walking all instructions twice; `compute_function_units_filtered` and `preserve_live_units` skip dead unit-liveness construction in blocks with empty `unit_exit`; `slot::classify_all` scans memoized per-block slot events and sequentially indexes `address_owner[reg]` (`O(1)`) over `summary.non_load_registers` instead of scanning all instructions and operands on every spill.

Interleaved A/B across the 3 standard fact workloads (5 rounds) and multi-argument calling-plan witnesses (`16_arg` 5 rounds, `32_arg` 2 rounds), medians:

| workload | wall base (`ms`) | wall cand (`ms`) | `total elapsed` base (`ms`) | `total elapsed` cand (`ms`) | `compile` stage base (`ms`) | `compile` stage cand (`ms`) | backend (`compile - Stage 05`) base (`ms`) | backend cand (`ms`) | native SHA-256 identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| `length/local` | 1,520.3 | 1,191.4 (1.28x) | 1,399.2 | 1,078.6 (1.30x) | 1,354.2 | 1,037.1 | 293.3 | 232.9 (1.26x) | `03bbe8fbb5c920af` (exit 0) |
| `dyn_dispatch/local` | 1,729.7 | 1,262.1 (1.37x) | 1,499.6 | 1,109.0 (1.35x) | 1,430.2 | 1,039.1 | 320.6 | 239.5 (1.34x) | `db9e97949698bb03` (exit 0) |
| `nested_record/local` | 1,675.4 | 1,405.9 (1.19x) | 1,467.3 | 1,254.2 (1.17x) | 1,425.4 | 1,169.3 | 295.9 | 266.7 (1.11x) | `fd152a003c29a915` (exit 0) |
| `sixteen_arg_stack_calling_plan/other_machine` | 2,819.5 | 1,597.5 (1.77x) | 2,666.9 | 1,482.6 (1.80x) | 2,601.2 | 1,421.8 | 1,560.7 | 592.6 (2.63x) | `aa7e492b4e75d192` (exit 0) |
| `thirtytwo_arg_stack_calling_plan/other_machine` | 32,755.7 | 6,156.0 (5.32x) | 32,589.1 | 5,979.7 (5.45x) | 32,509.9 | 5,876.8 | 30,906.9 | 4,669.0 (6.62x) | `de2397b694f047f9` (exit 0) |

Under `OMEGA_PHASE_PROFILE=1` on `thirtytwo_arg_stack_calling_plan/other_machine`, `06.1.d.05_stage_register_allocation` dropped from `41,625.4 ms` to `4,603.0 ms` (`9.04x` speedup; `07.a.1_preserve_live_units` `169.4 -> 46.9 ms`, `07.a.2_slot_assign` `532.6 -> 352.6 ms`).

## Iteration 4: paged register-prefix digest caching, unsealed intermediate spill recovery, crossing-spill deduplication, bitset `BlockLookup`, and CSR conflict/affinity/batch tables (base `b9643fb8b52e`)

Base `b9643fb8b52e` on Windows x86_64. Retained changes across `optimization-core`, `selected-instructions`, `04_selected-instructions-to-selected-instructions`, and `05_selected-instructions-to-register-homes`:
- **Chunked `DigestEncoder` and 64-register paged prefix caching (`optimization-core/src/digest_encoder.rs`, `selected_instructions/{functions.rs,plan_identity.rs,plan_identity/contracts.rs}`):** Buffered `DigestEncoder` field updates into 64-byte SHA-256 block writes and added `CachedFunctionPrefix` on `FunctionEntry` so appending spill virtual registers during the `3,330` spill rewrites hashes only the `< 64` trailing registers and modified block digests instead of re-hashing all `25,000` virtual register entries on every step (`seal_digest` on `thirtytwo_arg_stack_calling_plan/other_machine` dropped from `932.5 ms` to `160.3 ms`, `5.8x` speedup).
- **Unsealed intermediate recovery reanalysis & crossing-spill deduplication (`04_selected-instructions-to-selected-instructions/src/analyses/mod.rs`, `04_selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/rewrite.rs`, `05_selected-instructions-to-register-homes/src/assignment/{home_assignment/compute/mod.rs,runtime_spill/recovery.rs}`):** Added `UnsealedRecoveryFacts` and `FunctionHomeReuse::probe_all` so the `103` intermediate spill-batch probes skip whole-plan receipt hashing until placement succeeds (`reanalyze` dropped from `1,508 ms` to `642.9 ms`). Extended `spill_selected_runtime_value_with_crossing_status` to report `crossed_unit_write: bool`; when `UnitWriteCrossing` crosses no unit-writing instruction, `recovery` reuses the already-computed rewrite without running a redundant reanalysis and placement probe (`crossing_probes` dropped from `103` to `0`, saving `~650 ms`).
- **Bitset `BlockLookup` register summaries, lazy `RawCandidateSlots`, and fast-forwarding slot sharing (`selected_instructions/blocks.rs`, `runtime_spill/slot.rs`, `runtime_spill/rewrite.rs`):** Replaced per-block `referenced_registers: Vec<VirtualRegisterId>` and `non_load_registers: Vec<VirtualRegisterId>` (which pushed and sorted `25,000` registers on every block replacement) with dense `u64` bitsets (`referenced_register_bits`, `non_load_register_bits`), cached `operand_count` and `unit_writer_indices`, and sorted `instructions_sorted` lexicographically by `(id, index)` for `O(log N)` earliest-duplicate lookup. Replaced per-block `owner_registers` iteration in `slot::classify_all` with word-level bitset intersection (`non_load_word & owner_word`), deferred `CandidateSlot` sorting to lazy `RawCandidateSlots::get`, and added `partition_point` fast-forwarding in `shareable()` (`slot::assign` dropped to `258.4 ms`, `rewrite` dropped to `230.8 ms`).
- **Incremental `materialize_block` liveness and flat CSR range/conflict/affinity/batch tables (`liveness/{compute.rs,edge_values.rs}`, `live_ranges/compute/{function.rs,fragments.rs,constraints.rs,edge_transfers.rs}`, `home_assignment/compute/{prepared_conflicts.rs,prepared_affinities.rs}`, `runtime_spill/recovery.rs`):** Maintained `current_virtual_live` and `current_unit_live` incrementally across instructions in `materialize_block` (`liveness` dropped to `183.7 ms`), replaced per-register `Vec` allocations with flat CSR tables in `operand_rows`, `PreparedConflicts`, `PreparedAffinities`, and `BatchFunctionIndex`, and precomputed dense `physical_matrix: Vec<u8>` and `direct_pressure` in `FailedDomain` (`batch_idx` dropped from `180.5 ms` to `29.2 ms`, `place` dropped from `902.5 ms` to `666.9 ms`).
- **Rejected sub-experiment:** Iterating over function-wide `owner_registers` (`~1,700` registers) inside `for block_index in 0..block_count` (`~100` blocks) in `slot::classify_all` performed `170,000` checks per spill step (`560M` iterations across `3,330` steps = `+196 ms`); replaced with word-level `u64` bitset intersection (`< 0.2 ms`).

Interleaved A/B across the 3 standard fact workloads (5 rounds) and multi-argument calling-plan witnesses (`16_arg` and `32_arg`, 3 rounds), medians:

| workload | wall base (`ms`) | wall cand (`ms`) | `total elapsed` base (`ms`) | `total elapsed` cand (`ms`) | `compile` stage base (`ms`) | `compile` stage cand (`ms`) | backend (`compile - Stage 05`) base (`ms`) | backend cand (`ms`) | native SHA-256 identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| `length/local` | 1,232.3 | 1,207.9 | 1,081.3 | 1,042.3 | 1,013.3 | 1,000.5 | 241.3 | 228.9 (1.05x) | `03bbe8fbb5c920af` (exit 0) |
| `dyn_dispatch/local` | 1,397.2 | 1,688.9 | 1,127.5 | 1,203.2 | 1,084.4 | 1,155.1 | 247.6 | 268.1 | `db9e97949698bb03` (exit 0) |
| `nested_record/local` | 1,298.7 | 1,307.3 | 1,106.9 | 1,161.2 | 1,063.2 | 1,114.5 | 241.6 | 255.2 | `fd152a003c29a915` (exit 0) |
| `sixteen_arg_stack_calling_plan/other_machine` | 1,597.1 | 1,585.3 | 1,376.7 | 1,383.2 | 1,315.8 | 1,324.2 | 537.0 | 471.2 (1.14x) | `aa7e492b4e75d192` (exit 0) |
| `thirtytwo_arg_stack_calling_plan/other_machine` | 5,757.4 | 4,606.9 (1.25x) | 5,620.0 | 4,450.4 (1.26x) | 5,547.9 | 4,366.7 (1.27x) | 4,277.8 | 3,071.8 (1.39x) | `de2397b694f047f9` (exit 0) |

## Iteration 5: Stage 05 non-bitwise unsigned constant pruning & backend `O(instructions + operands)` live-range fragments, unmaterialized intermediate liveness sets, block instruction spans, and paged block digests (base `2c9365be9724`)

Base `2c9365be9724` on Windows x86_64. Retained changes across `validation`, `selected-instructions`, `04_selected-instructions-to-selected-instructions`, and `05_selected-instructions-to-register-homes`:
- **Stage 05 non-bitwise `bitwise_known_unsigned` pruning & `retain_completed_observations` fast paths (`validation/src/proof_contracts/arithmetic_domains/{expression_analysis/unsigned_constants.rs,expression_analysis/binary.rs,copied_values.rs}`):** Guarded `ExpressionNode::Binary` recursion in `bitwise_known_unsigned` to `BitwiseAnd | BitwiseOr | BitwiseXor` so non-bitwise `u64` arithmetic chains (`a1 + a2 + ... + a31`) return `None` in $O(1)$ instead of walking subtrees quadratically; checked interval-only `unsigned_representability::binary_fits` in `Operation::result_fits` before computing `known_unsigned`; and skipped `CallFrameResolver::new` in `retain_completed_observations` when `ordered_relations` is empty or the expression contains no calls.
- **`O(instructions + operands)` live-range fragment derivation & unmaterialized intermediate `InstructionLiveness` sets (`live_ranges/compute/{fragments.rs,architectural_units.rs,constraints.rs}`, `liveness/compute.rs`, `analyses/mod.rs`):** Replaced the `O(instructions * |live|)` per-instruction live-set scans in `register_fragments` (`1,080,000` `append_point` calls per batch on `pick32`) and `architectural_units` with `O(instructions + operands)` reverse interval-boundary tracking (`open_end` / `open_unit_end`) that visits only block boundaries and explicit register/unit operands. Added `compute_function_for_ranges` and `liveness_materialized` in `UnsealedRecoveryFacts` so the 27 intermediate spill-recovery batches do not clone or binary-search per-instruction `virtual_live_in`/`virtual_live_out` vectors (`121,500` `Vec` allocations saved per intermediate batch), materializing full `FunctionLiveness` only once inside `seal()`. Added `any(|op| op.early_clobber)` / `any(|op| op.tied_to.is_some())` guards in `derive_early_clobbers` and `derive_tied_pairs`.
- **Memoized `BlockLookup` instruction spans, 64-instruction paged `CachedBlockEncoding`, and slice-copied rewrites (`selected_instructions/{blocks.rs,functions.rs,plan_identity.rs}`, `rewrites/{mod.rs,runtime_spill.rs,runtime_spill/rewrite.rs,runtime_spill/slot.rs,runtime_rematerialization/admission.rs,runtime_rematerialization/rewrite.rs,runtime_split/admission.rs,runtime_split/rewrite.rs}`, `assignment/{same_home.rs,runtime_spill/recovery.rs,home_assignment/compute/mod.rs}`):** Added `transport_count` and `register_instruction_spans` (for blocks with `>= 64` instructions) to `BlockLookup`, `CachedBlockEncoding` (64-instruction byte pages) + `replace_block_contents` + `mutate_preserving_register_prefix`, `FrameAddress` pre-check in `runtime_spill.rs`, `O(1)` `instruction_index_in_block` lookup in `runtime_rematerialization/admission.rs`, `FunctionRoster` + `FailedDomain::visit_overlapping_registers` in `recovery.rs`, and CSR `SameHomeComponents` in `same_home.rs`.

Interleaved A/B across the 3 standard fact workloads and multi-argument calling-plan witnesses (3 interleaved rounds each), medians:

| workload | wall base (`ms`) | wall cand (`ms`) | `total elapsed` base (`ms`) | `total elapsed` cand (`ms`) | `Stage 05` base (`ms`) | `Stage 05` cand (`ms`) | backend (`compile - Stage 05`) base (`ms`) | backend cand (`ms`) | native SHA-256 identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| `length/local` | 1,240.1 | 1,221.9 (-1.5%) | 1,108.1 | 1,071.4 | 785.0 | 791.5 | 258.5 | 237.6 (-8.1%) | `03bbe8fbb5c920af` (exit 0) |
| `dyn_dispatch/local` | 1,362.7 | 1,286.1 (-5.6%) | 1,148.5 | 1,132.4 | 832.7 | 844.6 | 254.0 | 246.3 (-3.0%) | `db9e97949698bb03` (exit 0) |
| `nested_record/local` | 1,525.6 | 1,282.1 (-16.0%) | 1,182.9 | 1,038.3 | 890.5 | 753.0 (-15.4%) | 245.5 | 241.0 (-1.8%) | `fd152a003c29a915` (exit 0) |
| `sixteen_arg_stack_calling_plan/other_machine` | 1,480.4 | 1,418.8 (-4.2%) | 1,320.7 | 1,276.2 (-3.4%) | 784.2 | 775.3 (-1.1%) | 470.6 | 416.8 (-11.4%) | `aa7e492b4e75d192` (exit 0) |
| `thirtytwo_arg_stack_calling_plan/other_machine` | 4,754.5 | 3,818.6 (-19.7%) | 4,340.0 | 3,507.9 (-19.2%) | 1,236.9 | 1,153.0 (-6.8%) | 3,020.7 | 2,340.0 (-22.5%) | `de2397b694f047f9` (exit 0) |

## Iteration 6: Stage 05 non-relational difference-graph pruning & backend `u64` candidate bitmask placement, lazy `BlockSlotSummary`, `O(1)` dense register IDs, and zero-sort singleton `FailedDomain` (base `ceea19358b30`)

Base `ceea19358b30` on Windows x86_64. Retained changes across `checking`, `selected-instructions`, `04_selected-instructions-to-selected-instructions`, and `05_selected-instructions-to-register-homes`:
- **Stage 05 non-relational `DifferenceGraph` pruning (`checking/src/proof_contracts/arithmetic_domains/{ordered_values.rs,ordered_values/congruence.rs,ordered_values/congruence/difference_graph.rs,ordered_values/transport.rs}`):** Tracked `has_relational_edges` on `DifferenceGraph` so `derive_ordered_relations` returns an empty vector immediately when no relational difference edge between two variables exists (`ordered_relations.rows` on `thirtytwo_arg_stack_calling_plan/other_machine` dropped from `3,286` to `76`).
- **`u64` candidate bitmask placement & bitset `referenced_in_relations` (`05_selected-instructions-to-register-homes/src/assignment/home_assignment/{compute/placement.rs,compute/domain.rs,physical_requirement.rs}`, `04_selected-instructions-to-selected-instructions/src/analyses/allocation_legality/compute/{view_candidates.rs,live_points.rs}`):** Added `compute_function_bitmask` for targets with `<= 64` physical units (`x86_64` and `arm64`), replacing `vec![false; physical_unit_count]` scans per candidate domain with a single `u64` bitwise mask (`domain_mask & !occupied_mask`), and replaced `BTreeSet<VirtualRegisterId>` in `referenced_in_relations` with a dense `u64` bitset (`06.1.d.05.j_probe_all` dropped by `47.2%` on `32_arg`).
- **Lazy `BlockSlotSummary`, `materialize_definition_bits`, `O(1)` dense `next_register_id`, and zero-sort singleton `FailedDomain` (`selected_instructions/{blocks.rs,plan_identity.rs}`, `rewrites/{runtime_spill.rs,runtime_spill/slot.rs,runtime_rematerialization/admission.rs}`, `assignment/{same_home.rs,runtime_spill/recovery.rs,home_assignment/compute/prepared_conflicts.rs,home_assignment/compute/prepared_affinities.rs}`, `live_ranges/compute/fragments.rs`):** Split `BlockSlotSummary` into its own lazy `OnceLock` on `BlockEntry` (skipping slot-event scanning on large functions that exceed the slot-reuse budget), added `materialize_definition_bits` to `BlockLookup`, added an `O(1)` dense-ID fast path to `next_register_id` and `use_blocks`-only `span_closes` iteration in `runtime_spill.rs`, streamed `CachedBlockEncoding` byte pages directly into `DomainDigestEncoder`, replaced `SameHomeComponents` binary search with `O(1)` `root_by_id` indexing, and eliminated `covered`/`direct_covered`/`relieved` sorting and `direct_pressure` binary searches for singleton components in `FailedDomain`.

Interleaved A/B across the 3 standard fact workloads and multi-argument calling-plan witnesses:

| workload | wall base (`ms`) | wall cand (`ms`) | `total elapsed` base (`ms`) | `total elapsed` cand (`ms`) | `Stage 05` base (`ms`) | `Stage 05` cand (`ms`) | backend (`compile - Stage 05`) base (`ms`) | backend cand (`ms`) | native SHA-256 identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| `length/local` | 935.6 | 992.4 | — | — | 548.2 | 562.1 | — | — | `03bbe8fbb5c920af` (exit 0) |
| `dyn_dispatch/local` | 927.2 | 991.1 | — | — | 558.0 | 543.8 | — | — | `db9e97949698bb03` (exit 0) |
| `nested_record/local` | 988.7 | 908.4 (-8.1%) | — | — | 581.4 | 579.6 | — | — | `fd152a003c29a915` (exit 0) |
| `sixteen_arg_stack_calling_plan/other_machine` | 1,504.9 | 1,461.8 (-2.9%) | 1,353.6 | 1,140.9 (-15.7%) | 680.1 | 638.6 (-6.1%) | 603.3 | 443.0 (-26.6%) | `aa7e492b4e75d192` (exit 0) |
| `thirtytwo_arg_stack_calling_plan/other_machine` | 3,804.7 | 3,135.1 (-17.6%) | 3,619.3 | 2,906.5 (-19.7%) | 878.0 | 1,010.7 | 2,661.7 | 1,830.3 (-31.2%) | `de2397b694f047f9` (exit 0) |




