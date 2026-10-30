# Compiler parallelism: primary-source research for Omega

Research date: 2026-10-28 (America/Los_Angeles). Local source snapshot to map:
`9a4ec4a40c352ea4eb51f49f2d23ee03e3e3a78e`.

The useful reference is a compiler's division of work and ownership, not its
advertised thread count. Start by measuring Omega's existing parallel pipeline,
then shorten its limiting dependency chain, improve task balance, and bound
aggregate CPU and memory consumption. The recommendations below are research
hypotheses; this note records no new Omega benchmark or implemented speedup.

This concerns the time Omega takes to produce a correct executable. Improving
the executable's runtime performance is a separate objective. Compiling Omega's
Rust implementation faster is also separate from making an invocation of Omega
faster. The projects below are selected for publicly inspectable engineering,
not ranked as the world's fastest compilers; their languages, optimization work,
hardware and benchmark boundaries differ too much for that ranking.

## Reference map

| Reference | Useful engineering question | Transfer worth investigating |
| --- | --- | --- |
| rustc | How do queries share immutable results and coordinate with an outer build? | Precisely keyed reuse and a process-wide concurrency budget. |
| Go compiler | How does a function queue avoid a long tail and unsafe metadata mutation? | Expensive tasks first, sealed shared inputs, reusable worker scratch. |
| Zig | How do independent files and bodies overlap without unbounded queued IR? | File-local representations, bounded producer/consumer queues. |
| LLVM ThinLTO | How can whole-program knowledge coexist with parallel body optimization? | Small global summaries followed by independent realization jobs. |
| MLIR | What ownership restrictions make parallel passes safe and cache-friendly? | Explicit isolated units, local pass pipelines, controlled result publication. |
| Cranelift/Wasmtime | Where does interprocedural optimization conflict with per-function concurrency? | Separate immutable callee bodies from per-function transformations. |
| mold, an adjacent linker | How do parallel resolution and final output remain reproducible? | Canonical results, disjoint output regions, measured thread saturation. |

## Findings from the owning sources

### rustc: pure query results and coordinated parallelism

Rust's query model requires immutable keys/results and providers that return the
same result for the same inputs. Results remain memoized in the query context;
this is the relevant model for avoiding repeated analysis without hiding
dependencies. Its cycle handling must not be confused with a dataflow solver's
intentional fixed point. [Query evaluation model](https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html).

The current online CLI reference documents a common `--jobs` limit and specific
frontend/backend/linker limits, plus jobserver constraints. Its `sync` setting
keeps synchronization overhead with one worker, a useful experimental control.
These are online-documentation observations, not a claim that Omega's pinned
Rust toolchain accepts these options. [rustc CLI reference](https://doc.rust-lang.org/rustc/command-line-arguments.html#-j--jobs---jobs-frontend---jobs-backend---jobs-linker-limit-parallelism).

rustc can coordinate internal work with an invoking build system through a GNU
Make jobserver, including `MAKEFLAGS` and `CARGO_MAKEFLAGS` integration. The
transfer is coordination across levels, rather than one full-size pool per
compiler subprocess. [Jobserver integration](https://doc.rust-lang.org/rustc/jobserver.html).

Version caution: the development guide's parallel-compilation chapter expressly
warns that much of it is outdated as of November 2024. Its historical
`-Z threads` guidance and phase-status statements do not establish current
defaults or an optimum worker count. [Parallel-compilation caveat](https://rustc-dev-guide.rust-lang.org/parallel-rustc.html).

### Go: prepare shared metadata, schedule large functions early

At inspected revision `430d19f07a57ee5ed4e3b1a04b0ab54dd5a24487`, `prepareFunc`
initializes symbols and parameter sizes before concurrent compilation.
`compileFunctions` sorts by pre-walk node count and removes from the largest end
to reduce stragglers; race builds randomize order instead. Workers receive a
worker ID, compile functions, and release no-longer-needed IR. Size computation
is disabled during the parallel section because it is unsafe there.
[Go function preparation and queue](https://github.com/golang/go/blob/430d19f07a57ee5ed4e3b1a04b0ab54dd5a24487/src/cmd/compile/internal/gc/compile.go).

The same revision's build driver allocates backend concurrency using a token
pool. Its comments explicitly describe the risk of `GOMAXPROCS` processes each
requesting `GOMAXPROCS` backend workers; allocation retains a minimum while
sharing extra capacity. This is a practical example, not proof its exact token
formula suits Omega. [Go build/compiler concurrency allocation](https://github.com/golang/go/blob/430d19f07a57ee5ed4e3b1a04b0ab54dd5a24487/src/cmd/go/internal/work/gc.go#L220-L292).

**Omega hypothesis:** if traces show a long final machine/group, prioritize large
ready units while publishing results in their original roster order. Measure
whether the cost proxy predicts actual work. Scheduling order must never select
the winning diagnostic, symbol or proof result.

### Zig: independent representations and bounded in-flight work

Core developer Matthew Lugg's July 2026 account describes parallel per-file
parse/AstGen work, explicit semantic dependencies, and independent function
codegen. It specifically warns that queued AIR can grow rapidly when codegen
lags. Its linker remains serialized in that account, and the demonstrated
incremental workflow has target/version qualifications. These are architectural
observations; the post's demonstration timings are not Omega projections.
[Inside Zig's incremental compilation](https://mlugg.co.uk/posts/incremental-compilation-internals/).

The current source was checked at Codeberg revision
`fe6ee5d4b32d70e5ea14e74a66213910f8f8aea9`, not the abandoned GitHub default branch.
`Zcu.CodegenTaskPool` bounds both function count and AIR bytes in flight; its
comments budget AIR using observed AIR-to-MIR expansion, so a count-only limit
would miss an important memory cost. Its progress-state comments still describe
future semantic-analysis threading. Presence of parallel codegen does not imply
all semantic analysis is concurrent.
[Zcu source at the inspected revision](https://codeberg.org/ziglang/zig/src/commit/fe6ee5d4b32d70e5ea14e74a66213910f8f8aea9/src/Zcu.zig).

**Omega hypothesis:** if a future body pipeline overlaps checking/lowering/native
work, bound the bytes retained between stages and preserve cancellation and
failure propagation. An unbounded queue can trade a latency improvement for
paging and worse aggregate throughput.

### LLVM ThinLTO: global summaries, parallel transformation

ThinLTO writes compact module summaries, merges them into a combined index for
whole-program analysis, then performs importing and transformation in parallel
backends. This separates global decisions from heavyweight mutable bodies.
Its documentation also exposes backend job limits and explicitly notes that
default concurrency may be too aggressive on some machines. The page contains
old release-history text and internally differing descriptions of default core
selection; use the actual installed linker's help/source before selecting flags.
[ThinLTO design and concurrency controls](https://clang.llvm.org/docs/ThinLTO.html).

**Omega hypothesis:** preserve a small, settled catalog for selected bodies,
layout, ABI and effect obligations, then parallelize realizations only where
every input is explicit. A summary cannot omit requirements merely to make work
look independent. This is a pattern to assess, not a proposal to adopt LLVM or
add another IR.

### MLIR: ownership creates the parallelization boundary

MLIR's pass rules forbid inspecting sibling operations, mutating unrelated
operations, and relying on execution order or global mutable pass state.
Pass-manager anchors must be `IsolatedFromAbove`. Consecutive passes can execute
as a local pipeline on each function, improving locality and reducing scheduling
work compared with repeatedly sweeping all functions.
[MLIR pass restrictions and scheduling](https://mlir.llvm.org/docs/PassManagement/#operation-pass).

**Omega hypothesis:** evaluate an existing pass by listing its immutable shared
reads, owned writes, and publication step. Combine adjacent body-local passes
only when they share the same dependency boundary; do not reorder global
admission, proof, custody or realization decisions for locality.

### Cranelift/Wasmtime: per-function compilation has interprocedural costs

Cranelift explicitly targets compiler speed while accepting less aggressive
optimization than LLVM/GCC. That makes it a useful design reference without
making its runtime-code tradeoffs automatically acceptable for Omega.
[Cranelift project goals](https://cranelift.dev/).

Wasmtime exploits Cranelift's per-function boundary to compile functions in
parallel. Its first-party inliner article explains the conflict: inlining needs
callee bodies and therefore synchronization or additional read-only copies.
The transformation lives in Cranelift while an embedder callback supplies the
decision and body. The article's default-status statement is dated November
2025; this research does not establish the current inlining default.
[Wasmtime/Cranelift inliner design](https://bytecodealliance.org/articles/inliner).

**Omega hypothesis:** keep immutable selected-body information available before
workers start. Parallel body jobs must not rediscover or mutate one another's
selection, layout, transitive effects or calling-plan inputs.

### mold: output determinism and the cost of excessive threads

mold is a Unix/Linux linker, not a source compiler. Its author's August 2026
paper separates input parsing from resolution, applies parallel loops across
the pipeline, and writes disjoint output regions. It preserves reproducibility
through order-independent operations or canonical sorting after parallel work.
Its Firefox experiment reports unchanged wall time from 32 to 64 threads while
CPU time rises from 21.1 to 38.2 seconds, attributed to cache/DRAM effects.
Those numbers belong to that workload and hardware; they are evidence that
thread saturation should be measured, not a recommended Omega thread cap.
[mold paper, sections 3–4 and 7.3](https://arxiv.org/html/2608.23228v1).

**Omega hypothesis:** assign deterministic identities and final order separately
from worker completion. For native output, workers may fill disjoint regions
only after layout and relocation obligations permit that partition. Globally
dependent branch relaxation is not made independent by changing the loop API.

## What “maximum parallelism” should mean here

Choose the worker count that minimizes successful end-to-end compile latency
under the intended RAM/CPU budget. For a build queue, separately optimize
completed programs per unit time and tail latency. High utilization alone is
not the objective.

An analytical model, not a measurement: with total useful work `W`, dependency
span `D`, and `P` effective workers, elapsed time cannot be below
`max(W/P, D)`. A serial fraction `s` imposes the idealized Amdahl ceiling
`speedup(P) <= 1 / (s + (1-s)/P)`. Required memory traffic adds another resource
constraint: moving `B` unavoidable bytes through a bottleneck with sustained
bandwidth `b` takes at least `B/b`. Real execution also pays scheduling,
synchronization, allocation, cache misses and I/O. These quantities can change
with the parallel design, so the formulas are diagnostic models, not promised
speedups or a substitute for profiling.

Inspect four distinct limits:

1. **Dependency span:** a single expensive group, sequential source discovery,
   global merge, or final layout can leave workers idle. More workers do not
   shorten that dependency without a valid new decomposition.
2. **Task granularity:** tiny jobs can spend more time scheduling and allocating
   than computing. Overlarge jobs create a tail. File count, byte count and
   machine count are competing proxies, not interchangeable cost measures.
3. **Shared ownership:** concurrent workers contending for lazy indexes,
   allocators or arenas can increase total CPU work. Prefer preparing immutable
   tables once and giving each worker reusable scratch where valid.
4. **Resource multiplication:** several compiler processes, each with multiple
   pools and large stacks, can exceed useful CPU concurrency or memory budgets.
   Reserved stack address space, committed memory and resident memory must be
   recorded separately; a configured stack size is not an observed RSS cost.

## Priorities suggested by the references

First measure the existing scaling curve, then choose one causal experiment:

| Measured limit | Candidate | Evidence needed to retain it |
| --- | --- | --- |
| One long final task | Go-style expensive-ready-task-first scheduling. | Lower end-to-end latency with original roster-indexed publication and identical outcomes. |
| Repeated construction or lock contention | Prepare one specific immutable table; reuse worker-local scratch. | Complete dependency keys, less measured work/contention, bounded memory. |
| Serial body-local backend work | ThinLTO/MLIR-style frozen shared inputs and independently owned outputs. | Explicit layout/effect/callback dependencies, deterministic merge, engine agreement. |
| Repeated-edit cost | Precisely keyed reuse inspired by rustc/Zig. | No-op, body-edit and dependency-edit results agree with fresh compilation. |

Freeze revision, producer digest/profile, source, target, flags, cache state and
oracle. Interleave matched A/B runs and retain slow valid runs. Require a
successful nonempty exit status and expected artifact before accepting a timing;
then validate runtime outcomes, fact-sensitive rejects and canonical evidence.
Compare bytes where the output contract is deterministic. A hash mismatch needs
an explicit explanation. A source analogy alone does not satisfy these gates.
The local appendix specifies the first bounded comparison; stop after resolving
that comparison rather than implementing this entire table as a campaign.

## Omega mapping checked in this session

Source inspection at `9a4ec4a40c352ea4eb51f49f2d23ee03e3e3a78e`; no new compiler
timings or runtime observations. These are implementation facts, not measured
bottlenecks. Relative source links refer to this repository.

| Current owner | Observed implementation | Question worth measuring |
|---|---|---|
| [Driver source batches](../../../omega-rust/psi/compilation/driver/src/source_parsing/batches.rs), `parse` | Fused lexing/parsing; contiguous file-count chunks; worker-owned syntax trees and reusable token scratch; source order fixes page identity and error priority. The ordinary path caps desired pages at four times the ambient Rayon worker count. | Do a few large files dominate a chunk? Does page-count selection outside the dedicated pool reflect that pool's actual size? Byte-weighted contiguous chunks are only a candidate if this appears in a profile. |
| [Parser pool](../../../omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs), `with_parse_workers` | A process-local `OnceLock<ThreadPool>`; explicit 256 MiB worker stack. Separate [lexing API](../../../omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/lex_sources.rs) also has a parallel path. | Which entry points actually initialize each pool, and what is the first-use cost? Do not double-count fused driver work as two standalone stages. |
| [Machine validation](../../../omega-rust/psi/compilation/checking/src/program_validation.rs), `checking_thread_pool` and machine loop | A second process-local pool with 256 MiB worker stacks; indexed parallel collection with local results, followed by roster-order publication. Neither parser nor checking builder sets an explicit thread count. | Active versus idle threads; largest machine versus stage wall time; shared lookup contention and merge cost. Separate pools alone do not prove CPU oversubscription. |
| [Flow groups](../../../omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/flow/builder.rs), `machine_groups`, `build_groups`, `converge` | Union-find dependency groups, worker-local fixed points/fallbacks, shared immutable baseline, paged output adoption. Actual grouping treats ordinary machine entries under declared contracts separately from non-entry/extracted-region deliveries and target families. | Largest dependency-group duration, dirty revisits, fallback counts, output-adoption time. Splitting a real dependency group needs a sound dependency algorithm. |
| [Call contracts](../../../omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts.rs) and [indexed accesses](../../../omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/ranges.rs) | Already fan out across states or machines on the checking pool. | Worker imbalance, repeated preparation, and lock/allocator costs before adding more tasks. |
| [Native sequence](../../../omega-rust/omega/compilation/native-production/src/pipeline.rs) | Runs dependent selection, optimization, allocation, frame, encoding, layout and object steps in order. | Determine which work is genuinely body-local after provider/layout/callback inputs are settled. The sequence alone does not establish that whole-body fan-out is safe. |
| [Register-home computation](../../../omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/assignment/home_assignment/compute/mod.rs), `compute_terminal_register_homes`, `FunctionHomeReuse::probe_all` | Sequential function iteration, with mutable function reuse and pressure state. | Can expensive body-local work own its scratch while preserving reuse, ordered failures, whole-plan identities and publication? A mechanical `par_iter` change does not answer this. |
| [Compile worker](../../../omega-rust/omega/compilation/driver/src/checked/checking/compile_thread.rs) and [corpus policy](../../../tools/testing.md#corpus-outcome-gate) | Compiler operations already on their marked compile worker run inline; separate invocations can still coexist. Corpus worker count is independent of Rayon pools. | Compare one compiler's latency separately from a fixed batch's throughput and total thread/memory demand. |

[Cargo.lock](../../../Cargo.lock) pins Rayon 1.12.0 and rayon-core 1.13.0.
[Rayon 1.12.0 builder documentation](https://docs.rs/rayon/1.12.0/rayon/struct.ThreadPoolBuilder.html#method.num_threads)
documents automatic worker selection from `RAYON_NUM_THREADS`, otherwise logical
CPU count. Because Omega's static pools initialize once, vary that environment
in **fresh processes**, before either pool initializes. This provides an initial
sweep without adding a compiler option. It is a per-pool setting, not a shared
budget across processes.

256 MiB is the **configured worker stack size**; it is not a measurement of
resident memory. Record virtual reservation, committed memory and RSS separately
where the host supports them. Preserve the documented deep-input stack contract
while investigating memory; lowering the constant is not a justified experiment
in task scheduling.

### Existing evidence to reuse with its original identities

The [stage ledger](../measurements/stage_by_stage_optimization.md) already
records sealed indexes removing validation contention (Iteration 14), then a
first-pass parallel flow implementation (Iteration 15), then its replacement by
group-local fixed points (later “Iteration 16: Flow Fixed Point per Machine
Group”). Current source matches the latter architecture. The earlier 15-fixture
divergence documents why scheduling changes need semantic checks; it does not
establish a current regression.

The [2026-10-24 measurement record](../measurements/stage_five_2026_10_24/README.md)
contains earlier checking and register-allocation work. Its timings remain
bound to its revisions and workload identities. They do not measure this
checkout or forecast a gain from additional workers.

### First bounded experiment

Freeze one optimized exact-source producer and a small existing fact workload
set: an ordinary small compile, a many-machine check, and a native compile
with allocation pressure. Select existing witnesses; add no new fact rows merely
to benchmark counts. First establish exit-0 compilation and the authored
`omega run --both` oracle for each selected positive, plus applicable
fact-sensitive rejects. Keep the compiler binary, source/library inputs,
target, optimization selection, cache regime and consumed artifact fixed.

1. Sweep fresh processes at 1, 2, 4, 8 workers, physical-core count and logical-CPU
   count, removing duplicates and omitting configurations the memory budget cannot
   support. Determine topology on the actual host rather than assuming the
   historical 7950X is still the measurement host.
2. Measure process wall time, user/kernel CPU time, peak RSS/commit and thread
   count. Keep cold/first-use and warm-cache regimes separate. Capture detailed
   per-task spans, largest group/body, merge time and lock/allocator samples in
   separate profiling runs; summed concurrent phase time is not elapsed time.
3. Run a small fixed batch of independent compiles separately, keeping the same
   total workload while trading outer jobs against inner workers. Report batch
   makespan and per-compile latency. An approximate `outer jobs × inner workers`
   is only a planning heuristic when stages and multiple pools overlap.
4. Alternate configurations and retain every valid observation and its spread;
   reject failed compiles and missing/stale artifacts explicitly. Compare emitted
   bytes where deterministic output is expected, actual Terminal/native outcomes,
   ordered diagnostics and the relevant existing rejection controls.
5. Change **one measured limiter**: total runnable budget if contention dominates;
   weighted scheduling if a tail dominates; worker-local state if locks dominate;
   body fan-out if a significant body-local native share is demonstrably serial.
   Reject the change if end-to-end time, resource limits or semantic behavior
   regress. No full-corpus or full-workspace sweep is part of this research.

Stop this experiment once it identifies the best observed worker configuration
for these workloads and one evidenced next limiter. No absolute optimum or
implementation speedup is established by this research note.

### Supplemental research log from the local mapping

On 2026-10-28, opened (no search query) and read Rayon's builder documentation
(`num_threads`, `stack_size`), Go's `gc/compile.go` preparation and worker loop,
rustc's parallel-compilation guide, and MLIR's operation-pass restrictions.
The rustc guide explicitly warns that much of its parallel status discussion is
outdated; its historical four-thread contention observation is not a present
limit. Direct opening of `https://llvm.org/docs/ThinLTO.html` returned an internal
error in the parent's web tool; the external aggregation records the accessible
first-party alternative. The queries and other sources are logged below.

## Search and reading log

Date for all batches: 2026-10-28 (America/Los_Angeles). Earlier local research was
checked first: [field-inventory query memoization](../measurements/compiler_throughput_2026_10_21/research.md)
and [stage optimization ledger](../measurements/stage_by_stage_optimization.md).
The question here extends that work to cross-compiler parallel architecture.

| Exact search queries or direct inspection | Read beyond snippets | Outcome |
| --- | --- | --- |
| `site.rustc-dev-guide.rust-lang.org parallel compiler threads jobserver` | Query-model page; parallel-rustc warning; current rustc CLI and jobserver pages. | Use query purity and resource coordination; qualify historical status. |
| `site.go.dev compile concurrent backend gc compileFunctions` | Pinned `gc/compile.go` and `go/internal/work/gc.go` through GitHub connector. | Task balance, preparation boundary and token allocation. |
| `site.ziglang.org multi threaded compiler incremental compilation`; `site.ziglang.org "multi-threaded" compiler`; `site.ziglang.org "parallel" "semantic analysis"`; `site.mlugg.co.uk zig incremental compilation multithread`; `site.codeberg.org/ziglang/zig "Compilation.zig" "Thread"` | Lugg's complete article; relevant official 2026 devlog text; current Codeberg `Compilation.zig`/`Zcu.zig` scheduling and task-pool excerpts. | Bounded in-flight IR and incremental dependencies. GitHub master ended at the migration commit, so it was excluded as current evidence. |
| `site.llvm.org docs ThinLTO parallel backend threads` | ThinLTO introduction, concurrency controls and incremental section. | Compact summaries then independent transformation; version-specific flags require verification. |
| `site.mlir.llvm.org PassManagement multithreading isolated thread` | Operation-pass restrictions, isolation and local-pipeline scheduling. | Ownership and locality patterns. |
| `site.bytecodealliance.org cranelift compilation parallel functions` | Cranelift project goals; Wasmtime/Cranelift article introduction and implementation. | Per-function boundary, inlining tradeoff. |
| `site.github.com/rui314/mold parallel linking thread` | Author's arXiv v1 paper: architectural design, deterministic output, evaluation/scalability and limitations. | Adjacent emission patterns and a concrete overscaling counterexample. No cross-project benchmark ranking. |

Access notes: Codeberg's browser page could not be fetched by the web tool, and
the GitHub connector does not support Codeberg. The public Codeberg API and raw
source at the pinned commit were read with PowerShell HTTP requests. Search
results from Reddit, third-party wikis, mirrored repositories and uncited
benchmark claims were excluded. No Jai/Odin conclusion is asserted: this bounded
pass did not obtain sufficient relevant firsthand evidence to add either as an
architectural recommendation. No compiler was installed or benchmarked.

## First experiment

The [worker-count measurement](../measurements/compiler_parallelism_2026_10_28/README.md)
separates retained-graph and empty-private-cache compiles. It verifies actual
thread creation before interpreting scaling, retains source/producer identities
and all observations, and records why no default change is established.

The [parser-pool experiment](../measurements/parser_pool_2026_10_28/README.md)
then avoided the extra global pool. It reduced observed threads without a
reliable time or memory benefit, so the candidate was rejected and reverted.

The [spill-pressure screen](../measurements/spill_pressure_2026_10_28/README.md)
then found 103 repeated register-allocation analysis rounds. Retaining pressure
only when an unplaced component was rewritten barely activated and preserved
all 103 rounds; that candidate was also rejected and reverted.

The subsequent [unplaced-component batching experiment](../measurements/unplaced_batch_2026_10_28/README.md)
changed victim selection and reduced reanalysis from 103 rounds to 11. Six
pairs per cache regime measured 31.8% lower warm and 21.1% lower cold wall
time on the 32-argument workload, with fewer spills and a smaller executable.
Focused native/Terminal/reject checks passed; tiny workloads showed no
consistent effect in follow-up blocks. The change is retained locally, without
remote publication. This meets the bounded search's substantial-win target by
shortening repeated serial work rather than increasing worker count.

That allocator change landed in [PR #755](https://github.com/CathedralOS/Omega/pull/755).
The next [retained-graph experiment](../measurements/retained_graph_2026_10_29/README.md)
attributed the large warm restore and tested bounded buffer preallocation.
It preserves native bytes, invalid twins, malformed-record refusal and fresh
admission while lowering warm peak private commit by about 97–119 MiB
(15–19%). Timing differences did not establish another latency improvement.
That memory change landed through [PR #757](https://github.com/CathedralOS/Omega/pull/757).
The landing receipt and descendant-main smoke evidence for PR #755 are recorded
with the experiment.

The [root-scoped whole-graph experiment](../measurements/root_graph_2026_10_29/README.md)
then avoided loading unrelated projects while retaining the complete coupled
graph and fresh admission. The real candidate reduced three-root warm median
latency by 19–30% and six-root warm median latency by 34–56%. All native images
matched the baseline bytes; focused native/Terminal, reject, corruption, source
change, foreign-root and pruning controls passed. This next substantial latency
win is committed locally, with raw evidence and limitations in its record.

The [bounded native queue integration](../measurements/target_native_queue_landing_2026_10_29/README.md)
uses application Build's newer source-free product boundary. A fresh fixed-base
comparison measured the heavy four-target compile at 2682.9→1749.2 ms (34.8%
lower), with all four balanced blocks improving. Exact target images, actual
native/Terminal agreement, rejects, cache custody and ordered failure controls
passed. Peak private commit increased 107 MiB and root CPU rose 5%; small and
single-target programs showed no general speedup. The initial baseline-native
launch timeout and incomplete cohort remain separate from the complete replay;
its cause is unresolved after 32 successful bounded replays. Source-bound pinned
qualification passed. Publication is tracked with the experiment rather than
inferred from the local commit.

The [post-queue Windows ancestry screen](../measurements/windows_ancestry_postqueue_2026_10_29/README.md)
missed its declared two-subject gate: length improved 27.5%, dispatch 12.0%,
stack32 13.6%, including one losing dispatch block. All 60 image/native checks
passed; full conditional qualification and confirmation did not run. Its
unpublished production patch was parked. The next
[invocation setup attribution](../measurements/invocation_setup_2026_10_29/README.md)
measures startup and remaining serial setup before choosing another candidate.
