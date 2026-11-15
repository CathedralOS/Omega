# Proof preparation on ordinary contract workloads

The checker copied derivations for every available fact before knowing which
evidence a search would request. Keeping compact origins beside the facts cuts
requested allocation bytes by 62–81% in fresh current-main context replays.
The original fixed-base experiment also measures complete preparation plus
search 23–41% faster across the three source workloads. Exact proofs and kernel
results match their respective baseline producers. These are local proving
operations; whole-compiler latency is not established.

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

The initial experiment's fixed production base is
`5b3cc09bbdbd1227e38193380d73c7b73462baf2`. Tables before the integration
paragraph below belong to those producers and inputs.
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
the portable `omega-bench proof-square` command for reproduction, preserving its algorithm,
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

To check whether deferred materialization merely shifts work into selection,
the same counter-free driver also times complete `Prover::new` plus `prove`.
Independent kernel verification and exact baseline fingerprint checks stay
outside the timer. The original producer pair and context mix are unchanged;
eight AB/BA pairs use 100 scalar, 200,000 borrowed and 10,000 square source
passes per observation. All complete-search intervals are below zero:

| Source | Median microseconds/context, before → after | Paired median change | 95% paired bootstrap interval |
|---|---:|---:|---:|
| scalar | 2,384.597 → 1,824.471 | −23.49% | −27.37% to −22.14% |
| borrowed | 3.685 → 2.596 | −26.84% | −31.89% to −25.53% |
| square | 13.037 → 7.578 | −41.13% | −45.02% to −39.74% |

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
a smaller, faster measured proving operation, not a whole-compiler claim.

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
`100`, or on `wide64` and `nested8`. It never reads stdin. Portable
`omega-bench proof-plan source` creates a source-route plan with an explicit
hosted `--target`; `prepare` and `prove` create target-neutral timed plans with
explicit repeat counts, work units and checksums. Run them with
`omega-bench compare <plan> <new-directory>`, which closes stdin and owns
resource/oracle capture. No PowerShell is required for reproduction.
The adjacent `proof_preparation_timing` example shares the driver in
`tools/benchmark/proof-preparation/harness.rs`. The square generator verifies
main SHA-256 `f069e526d1da8440718195f266437c92345f5d9ace8b8461dad1bbad2968253d`
and default Build `9813449ec4cd31761ff0dfe63aa76b1a050d9985e279f56a80414a02795faa87`;
the default retains six optimization selections. `--identity-build` also has
its own frozen hash. Fetch the pinned Git object if a shallow clone lacks it.

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

Actual PR conflicts required integration onto
`a0802ba24dbe2ba7e6252f1924201fe2093f3410`. The queue conflict was resolved
without losing a newer, unrelated concern. Main's broader integer-equality
transport and inhabitation preparation were retained. The embedding control
uses expressions on both sides because direct value aliases now legitimately
serve as definitions. Fresh 58-test, Clippy, formatting and boundary checks pass.

New baseline/candidate drivers on this integration base independently match all
40 proof/complete-kernel-result comparisons: 17 scalar, two borrowed, 19 square
and two secondary controls. These are fresh results, separate from the initial
30-source-context timing experiment. Preparation per source pass is:

| Current-base source | Allocation calls, before → after | Requested bytes | Largest retained context, bytes |
|---|---:|---:|---:|
| scalar | 16,131 → 11,730 | 2,354,519 → 887,423 | 185,824 → 25,240 |
| borrowed | 140 → 119 | 35,373 → 8,325 | 22,256 → 2,680 |
| square | 4,040 → 3,018 | 1,370,321 → 254,873 | 100,448 → 9,336 |

The current-base candidate again completes scalar native/Psi agreement, exit
0. Its native source is byte-identical to the current kept fixture, SHA-256
`abe2e510430a8bf7caf7abc2641bd18e4f6b40b09a82b69c554158e675a66280`.
This also verifies the old queue report about `settle(product)` is already
resolved; its stale bullet is removed without claiming a new semantic repair.

Portable plan/input preparation is carried from helper commit
`d619a243381bafdb0981edad8a02890cb08d067e` (exact source from the tested helper slice). Its five focused planning tests,
release all-target Clippy and formatting pass. Default and identity square
CLI preparation reproduce all three pinned hashes; source/prove A/A plans
complete the existing runner's oracles in two-pair smoke checks, not timing
claims. Those exact tool checks are reused: benchmark and bounded-process
sources/dependencies are unchanged between the helper's `4d7ea467f89e7` base
and the `a0802ba24dbe2` integration base. Root helper records remain under
worker-31's ignored `build/performance/portable-*` directories.

Raw data, baseline/candidate producers, source and harness snapshots, all
individual streams, allocation summaries and paired journals remain outside
Git under `build/performance/proof-preparation/` in worker-28. Existing
`build/performance/dominance/` evidence is preserved. SHA-256 manifests bind
the retained artifacts and measurements to their inputs.
