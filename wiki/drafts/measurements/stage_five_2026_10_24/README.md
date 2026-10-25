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
