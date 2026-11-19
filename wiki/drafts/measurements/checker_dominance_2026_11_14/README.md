# Checker dominance measurement

The checker prepared one Boolean dominator row per block before answering point-availability questions. A large machine therefore retained quadratic metadata. The repair shares the existing immediate-dominator algorithm, keeps traversal intervals for queries, and prepares the consumer traversal once. The retained dominance representation is linear in blocks and edges; construction remains iterative, with no worst-case linear-time claim.

The algorithm reference is Cooper, Harvey and Kennedy's [A Simple, Fast Dominance Algorithm](https://www.cs.tufts.edu/~nr/cs257/archive/keith-cooper/dom06.pdf). The additional virtual entries and checked-table topology are the checker's own conservative adapter, covered by the independent graph oracle below.

The source baseline is `9b4898865ecd5789bea4def6c2da078192a4ab82`. Measurements use release-built producers, the same graph shapes and deterministic 100,000-query batches, one warmup pair and eight measured AB/BA pairs per case. The Windows host leaves six logical CPUs outside the agent affinity mask and runs the children at Idle priority. `mbx reserve --cpus 18` excludes cooperating mbx builds during measurement; other desktop activity remains a source of noise. The intervals are descriptive paired percentile bootstrap intervals, not a guarantee about other hosts or applications.

The final graph run, observations and validated summary are retained locally in `build/performance/dominance/graph-paired-v2/`. Each row below shows medians from eight pairs; these are graph construction times, not whole-compiler timings.

| Shape | Blocks | Construction before / after | Retained graph bytes before / after |
| --- | ---: | ---: | ---: |
| Chain | 4 | 8.60 / 13.25 microseconds | 488 / 528 |
| Chain | 512 | 0.233 / 0.121 ms | 315,448 / 57,424 |
| Chain | 4,096 | 7.783 / 0.873 ms | 17,203,256 / 458,832 |
| Diamonds | 4,096 | 7.597 / 0.756 ms | 17,236,016 / 491,592 |
| Loops | 4,096 | 7.562 / 0.804 ms | 17,215,576 / 471,152 |
| Disconnected chains | 4,096 | 7.598 / 0.784 ms | 17,174,640 / 430,216 |
| Reversed identity order | 512 | 13.159 / 0.119 ms | 315,448 / 57,424 |

For the 4,096-block chain, retained graph storage falls 97.3%; the process peak working-set median falls from 25,477,120 to 8,642,560 bytes. The paired construction change is -88.5%, with a descriptive 95% interval of [-89.5%, -88.0%]. The reversed-order 512-block construction change is -99.1%, with interval [-99.13%, -99.08%]. These gains remove quadratic matrix work; they do not establish linear worst-case running time.

Small graphs pay extra preparation: the four-block chain retains 40 more bytes and its construction median rises by 4.65 microseconds. That case's paired construction interval crosses zero. Queries are a fixed batch of block-entry point comparisons: the large-chain query change is -14.9%, interval [-33.5%, -10.5%]; several small and medium cases have inconclusive intervals, and not every query point estimate improves. Actual operation/edge point semantics are covered by correctness tests rather than this query timing mix.

`retained_bytes` sums the graph struct and the actual capacities of its owned vectors, including vector headers. It excludes allocator bookkeeping, temporary preparation storage and source/fixture storage. `peak_working_set_bytes` is the Windows primary process peak resident working set, queried through a native handle retained before exit; it includes fixture generation, the test harness, graph preparation and query execution. Neither metric is allocator-requested bytes or committed memory. Operation-ID-indexed storage is unchanged, so the linear-size claim applies to dominance/topology metadata rather than arbitrary operation-ID extents.

The focused graph driver is a Rust libtest executable using its default allocator; the production omega CLI uses mimalloc. The two graph roles use the same allocator, but these constructor times and process peaks are mechanism measurements rather than absolute production CLI costs. The source-to-checker control also compares matching diagnostic-example executables.

The query checksum counts true answers and checks consistency across the compared producers. It does not prove each answer. The separate correctness tests compare every ordered block pair against an independent path-exclusion oracle for all 65,536 four-block directed graphs and larger random graphs, and cover operation order, duplicate incoming edges, surplus table edges, disconnected SCCs, malformed points and large nonrecursive traversals.

The old matrix admits every block as a dominator of a rootless disconnected cycle. The repair instead supplies conservative virtual entries to every member of each disconnected source SCC and preserves valid downstream dominance. It also refuses malformed equal points and prevents an entry backedge from establishing a first-entry fact. These are explicit Graph-level hardening changes; no whole-program miscompilation is claimed. The retained logs demonstrate the old Graph-level counterexamples and the candidate tests pin their safe answers.

We tried the actual dungeon crawler before timing it. Its ordinary compilation stops in std's Build declaration; its whole-subject Product probe stops on the sample's retired file-based imports. A scratch import migration then stops on unavailable membership carrier selection. The byte-pattern and masked-byte scan programs stop on mutable sequence support; record-range search stops on a missing extent descriptor. None of these refusal timings is a compiler speedup measurement. The owner explicitly allowed generated graph workloads while applications remain unavailable.

For a small source-to-checker control, [prepare_control.ps1](../../../../tools/benchmark/dominance/prepare_control.ps1) makes a canonical copy of the existing square-sum workload from the pinned baseline. It preserves the algorithm, two-million iteration count and exact final accumulator check, adding explicit carrier/domain qualifications to its literal operands. [control_plan.ps1](../../../../tools/benchmark/dominance/control_plan.ps1) creates an existing `omega-bench` process plan for the whole-subject `route_probe` command. That boundary includes source preparation and Product checking, and excludes Build execution, provider settlement and native publication. A separate baseline native compilation, with all six authored optional Psi rules explicitly disabled, executed successfully with exit 0. This is a small computational control, not evidence that a large application now compiles or that scan throughput improved.

The retained `build/performance/dominance/product-control-paired/report.json` is **inconclusive**: elapsed medians are 3,526.244 ms before and 3,582.299 ms after; the paired change is +3.18%, with interval [-5.20%, +9.52%]. Peak resident medians are 709,365,760 and 709,832,704 bytes, also inconclusive. All warmup and measured executions produced the exact accepted verdict and exit 0. The complete local journal and raw streams pass `omega-bench validate`. This control does not establish a whole-pipeline speedup or regression.

The baseline graph executable SHA-256 is `325065741F9097D49F553E67E719DFCFD2E32E4DFC37FBC5A3B3014FC8FB2C25`; the final graph executable is `0895C748C087D6345775B4F0EFE187CE5B341212EB6654E199DB53DBF0599E4E`. The source-to-checker executables are `31C6752AAD2D43B28E9DD1281F038A45C42B32CA6A00A11392D2719BC4698691` and `3E288AABCCB2AA6681E4CB2F9892F5C87EE8B03B4A765E532974AFFD2AB260ED`. The local `build/performance/dominance/report-archive/source-identities.json` pin the final implementation and test driver; the two measurement journals separately pin their inputs and binaries.

The graph runner is [measure.ps1](../../../../tools/benchmark/dominance/measure.ps1); `omega-bench dominance-summary` validates the observations and summarizes them with the existing Rust statistics engine. [native_peak.ps1](../../../../tools/benchmark/dominance/native_peak.ps1) supplies the Windows resource query. Missing or nonfinite observations fail summary validation; an absent CPU interval caused by OS accounting resolution remains explicitly unavailable. Raw per-child logs and retained executables stay under the worktree's ignored `build/performance/dominance/`; the ignored journals retain the measured samples, executable identities and completion digest.

To reproduce on Windows, use isolated pool worktrees and the documented host build budget. Build and retain each producer before switching source. The baseline graph needs a test-only attachment: create `src/graph/tests.rs` from [baseline_harness.rs](../../../../tools/benchmark/dominance/baseline_harness.rs), append `#[cfg(test)] #[path = "graph/tests.rs"] mod tests;` to the original `src/graph.rs`, and copy the submitted `tests/dominance.rs` integration wrapper into its owning crate. These changes do not alter production graph behavior. Only the ignored scaling observation is invoked for baseline timing; the hardening regression tests intentionally fail on the old implementation.

Run the same separate commands for each source version so Cargo feature unification stays matched:

```powershell
mbx build --release -p omega -p omega-bench
mbx test --release -p terminal-psi-to-terminal-psi --test dominance --no-run
mbx build --release -p compiler --example route_probe
```

Retain the printed integration-test executable as `graph.exe` and the example as `route_probe.exe`. The source baseline is the pinned commit above with the test-only attachment; the candidate is this PR's submitted source. Do not put builds inside the timed experiment. From `tools/benchmark/dominance/`, with paths replaced by the retained producers:

```powershell
mbx reserve --cpus 18 -- pwsh -NoProfile -File ./measure.ps1 -Baseline <before-graph.exe> -Candidate <after-graph.exe> -Output <new-graph-run>
<retained-omega-bench.exe> dominance-summary <new-graph-run>
./prepare_control.ps1 -Output <new-control-project>
./control_plan.ps1 -Baseline <before-route_probe.exe> -Candidate <after-route_probe.exe> -Project <new-control-project> -LibraryRoot <pinned-source/library> -Output <plan.json>
mbx reserve --cpus 18 -- <retained-omega-bench.exe> compare <plan.json> <new-control-run>
<retained-omega-bench.exe> validate <new-control-run>
```

The affinity and scheduler settings belong on the launching shell too, as required by the host build budget. Native/Psi validation uses a separate generated control made with `prepare_control.ps1 -IdentityBuild`: it preserves the same executable body and explicitly selects no optional optimization rules. That keeps the gate on the ordinary identity phase while the selected Psi transformations lack a second lowered-form check. Run `omega run --both --psi-work-budget 100000000 --keep <control/main.omg>` with stdin closed. The work budget covers the unchanged two-million-iteration checksum; it does not alter the workload or the acceptance oracle.

Both the baseline (`build/performance/dominance/agreement-baseline-observation/observation.json`) and candidate (`build/performance/dominance/agreement-candidate-observation/observation.json`) completed that native/Psi gate with exit 0 in both engines. Their source hashes match exactly: executable body `F069E526D1DA8440718195F266437C92345F5D9ACE8B8461DAD1BBAD2968253D`, identity Build `EABA73728026898495728DEDFF5C7005F89949CA70CEE61A3F48D1CCC9354D32`. The CLI binaries are `F443E3B12BEFAC80E241AF392DE9813B00BB86CB74C4CF3A20DE3C67342195FE` and `26231773506D7EBC4501E53DA97F5BB7A257547090FB262FC767457DCC677A01`. These are correctness runs, not paired timing observations; their elapsed times are not a speedup claim. The local `build/performance/dominance/dungeon-baseline-probe/stderr.txt` retain the larger-project limitations described above.


Raw data is intentionally not committed. Its custody was first recorded in a claim note (ticket `18e80798827daf8c0000000018e934e0`), which claim pruning later dropped; registry commit `c5e5deebd226ac4eb3b35065eed81607c06d97f1` on `refs/coordination/omega-claims/main` still holds it, and its content is recorded here. The root also retains the `baseline/` and `candidate-v2/` producers, the full streams and the native/Psi agreement observations. The final source identities (`report-archive/source-identities.json`) have SHA-256 `88F245A68A371D069ACD800088B25FD64F959056BA1DCFD63871615F2A079E3A`. On this host the retained run root is `C:/Projects/Omega/.agents/worktree/worker-28/build/performance/dominance/`; releasing the worktree preserves this ignored build directory. The graph observation journal SHA-256 is `83B05261F48859CB2B32C91462E43A8F8C3BBBBCC9CC8BFC28DC301F523900C7`. The Product journal SHA-256 is `E72B7AC08D253EFCDDD73A7DA75C372BB35A1F1982B55BB0C9A8A44DB8004028`; its report SHA-256 is `7DC23E51A4981D90A24259D8F3DE4C118796490AE320DB42D94470719DC5ECF0`. The Rust graph summary SHA-256 is `4636989AFB72EC9E2CC56C5D564EB8B5B872C7DDF0C56C4865378CC81F61F9F0`. The retained streams and source/binary identities permit local verification; these hashes are provenance records, not signatures.
