# Proof preparation on ordinary contract workloads

The checker copied derivations for every available fact before knowing which
evidence a search would request. Keeping compact origins beside the facts cuts
requested allocation bytes by 69–81% during preparation of three real source
programs' successful-obligation contexts. Isolated preparation is 28% faster
for the tiny ownership workload and 44% faster for square-sum in paired runs;
the larger scalar-contract workload's timing is inconclusive. The returned proofs and complete
kernel acceptance results retain their baseline fingerprints. This measures
proof preparation; it does not establish a whole-compiler latency improvement.

Each fact now owns its proposition and an assumption, axiom, conjunct,
integer-equality or transport origin. Equations reference those same facts.
Proof search expands an origin into the existing owned `ProofNode` tree when it
requests evidence. Input description reads propositions and expands no proof.
Search can still construct exploratory candidate proofs; this is not a new
search algorithm or a cache. Ordinary proposition cloning and denotation work
remain.

Conjunct facts retain their original children-before-parent order and exact
positional eliminations, including repeated propositions. Only the whole
original semantic axiom retains its axiom index. Normalized consequences
capture the ordered equations available before the consequence was added.
The kernel and portable proof format are unchanged: expanded logical node
occurrences, premise scopes and depth budgets are checked as before.

The fixed production base is `5b3cc09bbdbd1227e38193380d73c7b73462baf2`.
Baseline producers were built and retained before the representation changed.
Baseline-only preparation added private Debug derivations and explicit module
paths for the diagnostic example; the temporary full-route profiling hook was
removed before allocation and latency comparisons. The candidate additionally
includes the already-landed tests-only prerequisite from #3241, which repairs
two stale `psi_work` crate references. It changes no production behavior.

Measurements ran on the Windows workstation with Idle priority, affinity
`0xFFFFC0`, `MBX_SCHEDULER_CPUS=18`, `CARGO_BUILD_JOBS=18`, pinned mbx 1.29.2,
and release producers. One heavy chain ran at a time in this session; unrelated
host activity remained possible. Every compiler process received EOF on stdin.
The library tree is `122e2e3abddb3af9b1f130111cd45300b6dff607`.

The primary inputs are existing source programs, not generated conjunction
stress. `scalar` is
`tests/fixtures/run/calls/scalar_contract_route_exit/main.omg`; `borrowed` is
`tests/fixtures/run/ownership/borrowed_root_replacement/main.omg`. `square`
uses the retained canonical-literal square-sum source reconstructed by
`tools/benchmark/dominance/prepare_control.ps1`, preserving its algorithm,
constants and checksum. Its source archive remains under the raw-data root.

Full source-route profiling counted every `Known` invocation, including
private descriptions and nested search. The geometry below counts only the
conjunction expansion performed by preparation, not full proposition or term
AST depth.

| Source | Calls | 0 facts | 1–8 | 9–32 | 33+ | Max facts | Max conjunction width/depth | Description calls |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| scalar | 305 | 0 | 24 | 95 | 186 | 56 | 8 / 1 | 7 |
| borrowed | 12 | 1 | 7 | 4 | 0 | 9 | none | 3 |
| square | 38 | 3 | 16 | 19 | 0 | 22 | 5 / 1 | 5 |

The distributions differ substantially by source. Ordinary scalar summaries
produce shallow 7–8-wide conjunctions, while the ownership workload stays
flat and small. Normalization also retains repeated propositions; their order
and evidence identities are preserved rather than deduplicated.

The private example replays each successful obligation's full ordered
assumption and axiom rosters, including unused facts, with its real value
table and goal. One pass contains all contexts at their original frequency;
100 passes amplify the measurement. It excludes description, result-summary
queries, failed searches and separately prepared generic forms. Source
preparation and independent kernel checks occur outside these timers.

Allocation calls include reallocations. Requested bytes and retained payload
exclude allocator metadata. The preparation table is per source pass; the
retained column is the largest individual context's live `Known` payload,
not the sum of simultaneously retained contexts or OS memory.

| Source | Contexts/pass | Allocation calls, before → after | Requested bytes/pass, before → after | Largest retained context, bytes |
|---|---:|---:|---:|---:|
| scalar | 16 | 8,117 → 5,865 | 1,560,048 → 476,432 | 144,400 → 14,384 |
| borrowed | 2 | 140 → 119 | 35,373 → 8,325 | 22,256 → 2,680 |
| square | 12 | 2,436 → 1,746 | 858,502 → 160,822 | 97,344 → 9,288 |

Preparation's maximum live requested payload above the fixed replay inputs
falls from 145,048 to 18,699 bytes for scalar, 24,069 to 4,493 for borrowed,
and 97,897 to 12,273 for square. This counter excludes allocator overhead and
transient storage internal to `realloc`; the observer records process working
set separately. The two largest scalar contexts account for 32% of the summed
individual baseline footprints; the two largest square contexts account for
42%. Those sums are not a stage peak.

Complete proof search also requests fewer bytes per pass: scalar
126,230,230 → 73,089,246; borrowed 46,579 → 19,531; square
1,134,210 → 436,530. These include unchanged proof-search and denotation work.
The counting allocator performs atomics on allocations, so its times are
diagnostic only and are not used as uninstrumented speedup claims.

A second build of the same replay compiles allocation counters out through a
constant generic parameter and forwards directly to System. It reports only
the preparation timer through the existing `omega-bench` reported-work mode.
Eight alternating AB/BA pairs follow one warmup pair. The scalar, borrowed and
square runs repeat the original context mix 5,000, 200,000 and 10,000 times per
observation, respectively. Work counts and prepared-fact checksums are fixed
before the comparison.

| Source | Median microseconds/context, before → after | Paired median change | 95% paired bootstrap interval | Result |
|---|---:|---:|---:|---|
| scalar | 32.976 → 20.187 | −30.88% | −45.04% to +14.86% | inconclusive |
| borrowed | 4.146 → 2.932 | −28.22% | −33.91% to −25.75% | lower in this run |
| square | 10.821 → 6.075 | −43.84% | −45.83% to −40.42% | lower in this run |

The baseline eagerly retains 654 ordinary proof-node occurrences across the
16 scalar contexts, 20 across the two borrowed contexts, and 314 across the
12 square contexts. The selected certificates contain 203, 2 and 43 nodes
respectively. Both borrowed certificates are Truth and cite no premise.
The candidate retains zero ordinary proof nodes during preparation. All 30
real-source context shapes, fact/equation counts, selected proof-node counts,
proof checksums and complete kernel acceptance checksums match the frozen
baseline. Fingerprints use deterministic FNV64 over Debug representations;
binary, source and observation custody uses SHA-256.

Two bounded secondary controls select one leaf, with independently checked
certificates and unchanged baseline fingerprints:

| Control | Allocations/pass | Requested bytes/pass | Retained bytes | Selected proof nodes |
|---|---:|---:|---:|---:|
| 64-wide conjunction | 599 → 20 | 2,544,592 → 91,600 | 915,472 → 39,952 | 2 → 2 |
| Eight nested conjunctions | 1,021 → 82 | 518,800 → 43,760 | 254,992 → 19,856 | 9 → 9 |

Latency comparisons use separate uninstrumented `route_probe` producers and
the existing `omega-bench` paired statistics. One warmup pair precedes eight
alternating AB/BA measured pairs per source. Every observation must return
the exact accepted verdict. This process includes source preparation and
Product checking, bypasses Build and native publication, and is not a full
compiler measurement. Host wall-time variability is substantial.

| Source-route run | Median seconds, before → after | Paired median change | 95% paired bootstrap interval | Result |
|---|---:|---:|---:|---|
| scalar | 10.230 → 10.198 | +12.11% | −23.73% to +40.29% | inconclusive |
| borrowed | 7.759 → 7.338 | −5.31% | −5.78% to +0.47% | inconclusive |
| square, first | 3.711 → 5.150 | +13.95% | +4.61% to +57.76% | slower in this run |
| square, focused repeat | 5.991 → 5.604 | +7.48% | −26.00% to +19.28% | inconclusive |

The first square slowdown triggered the isolated timing investigation above
and a focused repeat of the same source process comparison. The repeat did
not reproduce a statistically detected slowdown. In the first square run,
baseline observations alone ranged from 3.59 to 10.13 seconds and approximately
12.3 to 20.3 billion process CPU cycles. Other host Rust builds and Defender
were present. These observations do not establish the cause of the first
slowdown or a stable whole-route effect. Both runs are retained; the result is
a smaller, faster measured preparation operation, not a whole-compiler claim.

The baseline and candidate also execute the same byte-identical isolated
scalar-contract project with `omega run --both`: native/Psi agreement, exit 0.
These single native runs are semantic controls, not a timing comparison. An
earlier invocation's unsupported `run --build-dir` option was refused with
exit 64 before compilation; the corrected retained runs omit that option.

The reproduction driver is
`omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/examples/proof_preparation.rs`.
`tools/benchmark/proof-preparation/README.md` explains its measurement boundary.
Build with `mbx build --release -p terminal-psi-to-terminal-psi --example
proof_preparation`, then run the retained executable on each source with
`100`, or on `wide64` and `nested8`. The observer script closes stdin and saves
streams and resource counters. `plan.ps1` prepares the uninstrumented source
comparison, run with `target/release/omega-bench compare <plan> <new-directory>`.
The adjacent `proof_preparation_timing` example shares the driver in
`tools/benchmark/proof-preparation/harness.rs`; `timing_plan.ps1` binds its
`prepare` mode to the repeated context work count and checksum.

Validation passed on the changed owner and its immediate consumers:

```text
mbx test --release -p terminal-psi-to-terminal-psi --lib -- known::tests tests::mathematical_clauses tests::entry_inputs tests::width_parameters tests::inferred_bounds tests::guard_contradiction tests::ranges a_branch_fact_on_a_join_parameter_reaches_its_use a_branch_fact_on_a_conjunction_join_bounds_its_arm --test-threads=1
mbx clippy -p terminal-psi-to-terminal-psi --all-targets -- -D warnings
mbx xtask fmt --check
mbx xtask check-boundary
```

The 58 tests include both-orientation integer lifting, positional conjunction
rejection, invalid empty conjunction rejection, duplicate definition selection,
normalized consequence snapshots and false-branch rejection, private input
summaries, arithmetic ranges and generic widths. After the benchmark driver
was shared with the uninstrumented variant, a final build replayed all 32
source/control contexts, matched the frozen proof/acceptance checksums and
matched the earlier per-pass allocation counts exactly. The initial owning
crate all-target check failed at the fixed base's two stale `psi_work` test
references; the #3241 prerequisite repaired them without changing expectations.

Raw data, baseline/candidate producers, source and harness snapshots, all
individual streams, allocation summaries and paired journals remain outside
Git under `build/performance/proof-preparation/` in worker-28. Existing
`build/performance/dominance/` evidence is preserved. SHA-256 manifests bind
the retained artifacts and measurements to their inputs.
