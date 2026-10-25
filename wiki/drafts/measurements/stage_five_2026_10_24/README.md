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

