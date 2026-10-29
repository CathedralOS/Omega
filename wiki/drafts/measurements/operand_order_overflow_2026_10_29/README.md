# Operand-order checker stack overflow

## Publication scope

The compiler repair already reached main through
[PR #784](https://github.com/CathedralOS/Omega/pull/784), commit
`b189f54ce8ce0eeda807efecedb6855c74ffd79f`. Main enters the same active-query
guard before carrier selection as well as guarantee evaluation. Publication on
base `f1058703209f782fc08fc621471257095a2f7f47` preserves that implementation
unchanged and adds only the reduced regression pair and this evidence. The
archived `candidate.patch` below belongs to the historical fixed workbench; it
is not the publication diff. Historical timings and runtime records are not
claims about the current-main producer.

Fresh publication validation uses optimized dev producer
`5f0e2fc5dde252263b64b47a6dd593fb3a41c19585f78f45ccd15998b897501e`,
built in 4m28s from unchanged main compiler source. The added positive compiles,
executes natively with exit 0, and agrees with Terminal Psi; its reject twin
reports the expected unproved `abs_i32` requirement. Image hashes, exact inputs,
commands and logs are in [publication/results.json](publication/results.json).

The larger existing operand-order control does not have fresh main acceptance:
its first package update hit a 180-second bound; an extended replay was stopped
after 438.34 seconds following a noninvasive stack sample. The active stack was
in `infer_synchronous_invocations`, separate from contract-query recursion.
That replay is diagnostic, not a latency sample. The remaining larger controls
were not reached. This publication adds no compiler code and does not claim to
repair that newly exposed preparation problem. It is the next bounded experiment.

## Historical fixed-workbench experiment

Parent: ITERATION-PERFORMANCE. Fixed workbench
`643ffc583f8ef53be014b04b15b77f3ec8447431`; compiler source matches
`6c20da79bd9`. The preceding numeric-preparation candidate was removed.
Intended claim `aff0fa8de55c4ad0b8ec932333d90317` encountered coordination
timeouts; work continues in the isolated `pass-ablation` worktree.

Acceptance: the unchanged `runtime_numeric_operand_order_exit` and
`runtime_numeric_operand_single_evaluation_exit` fixtures must finish ordinary
package preparation, compile and execute with authored exit 0; check Terminal
agreement as well. Exact scalar snapshots, selected call requirements, partial
operation formation and mutable-source invalidation must remain enforced.
Do not raise the 256 MiB checker stack, remove library dependencies from the
acceptance programs, assume pending requirements, or recognize helper names.

## Baseline and reduction

Baseline SHA-256:
`10373901d2eee2c7d278da3e56e0b6dc17d8e2e5822a2ec85f69b84b74335b1d`.
The faithful unchanged package-update replay overflows `omega-check-19` with
exit 3221225725 after 177.56 seconds. Source and Build hashes match the preceding
iteration. Initial scratch copies were one directory too shallow for the
authored relative std dependency; their missing-path failures are harness
errors, not overflow reproductions. The corrected scratch layout preserves
both source files verbatim. All 106 std `.omg` inputs are hash-pinned.

A diagnostic program importing the same library with an empty `main` completes
package checking. The full original control under CDB raises first-chance stack
overflow and shows a repeating cycle through selected requirement judgment,
application resolution, integer comparison projection and prepared integer
premises. The debugger uses the previous rejected clone candidate only because
its matching PDB is available; its binary/PDB hashes are recorded. It is
diagnostic evidence, not the baseline or repaired producer. The original
baseline independently exhibits the same stack-overflow symptom.

Inlining the ordinary `abs_i32` helper into the smaller single-evaluation
control removes the external library and still overflows the exact baseline
in 86.78 seconds. The retained positive fixture adds a positive-valued call so
both helper branches are observable, while the counter verifies that the
nested mutating operand executes once. Its reject twin changes only the
helper's entry requirement to `value > 0`; the negative-valued call must reject.
The exact retained positive fixture also overflows the baseline in 88.89
seconds; its source digest is bound to the successful repaired observation.

## Hypothesis and proposed repair

The existing `ApplicationResolution` guard retains the complete application
payload and immutable premise/storage owners across cloned judges, but only
body unfolding enters it. The selected-contract equation route runs earlier
and asks for the application's entry requirements without entering the guard.
Integer premise projection can then ask for that same equation with a fresh
local depth, producing unbounded recursion.

Use the same active-query guard around selected-contract equations. A repeated
exact question yields no optional equation, never proof. Keep the guard's input
mode unchanged; the body-unfolding caller explicitly selects specification mode
before entering it, preserving its prior behavior. The guard consumes the
already-cloned judge so this refactoring does not add another body-query clone.
Scope: the guard, its existing unfolding caller and the selected-equation
caller. No cache key, proof rule, stack size, library body or source acceptance
oracle is weakened. The existing full operand-order fixture is the regression
pin that failed before implementation.

The decisive result is successful original-source execution and preserved
reject controls, not absence of the overflow alone. Compiler refusal of the
valid control is still a failed repair.

## Research

2026-10-29: consulted the preceding numeric-preparation research and local
`resolution_queries.rs` before searching the newly identified recursion path.
Query: `site.rustc-dev-guide.rust-lang.org queries cycle error in progress query`.
Read [The Rust Compiler Development Guide: query evaluation model](https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html),
especially immutable query input and Cycles. It describes detecting recursive
queries with the same inputs; it does not justify assuming a pending result.
Rustc's global query-error policy is not adopted here: Omega already has a
query-local conservative unknown result for optional application normalization.
Reuse that owner and its exact identity checks rather than add a global depth
limit or another memo. The source suggests the boundary to inspect; local
reproduction and quality checks determine whether the repair is valid.

## Result: retained correctness repair

The three-file repair builds in 3m58s with the optimized dev profile. Repaired
producer SHA-256:
`74a64fce6a09733bfd767d33da9d9811eba29be46427f7ebd87b053c6dc47ebf`.
The original operand-order package review now finishes its initial checking
step in 58.16 seconds, then resumes normally in 3.53 seconds. Its timed compile
finishes in 10.56 seconds, native execution exits 0, and `run --both` agrees.
The original single-evaluation control likewise finishes package review,
compiles in 8.71 seconds and passes native/Terminal agreement. Source and Build
files are unchanged. These are fixed-source crash-to-success observations,
not a percentage compile-speed comparison against a successful baseline.

All five positive fixtures compile and execute with their authored exit 0:
both originals, the smaller regression, a returned-reference scalar snapshot,
and a copied-call snapshot through a match join. All five also pass `run --both`
without `DECLINED`; their retained native image and Terminal artifact hashes
are recorded. Six negative controls reject with the authored diagnostics:
the new unmet requirement, another authored numeric requirement, signed-minimum
absolute value, a restricted nested call, a mutated reference bound, and a
callee-mutated source fact. Package-review `update --resume` returns the
documented no-pending-review refusal for dependency-free roots whose first
update already wrote the lock; this is setup, not a failed compiler gate.

The ordinary three-workload comparison preserves identical native images in
all 36 successful observations. Four unprofiled measurements per variant give:

| Workload | Baseline median ms (range) | Repaired median ms (range) | Median change |
| --- | ---: | ---: | ---: |
| stack32 | 2421.6 (2372.5-2503.8) | 2517.2 (2387.1-2672.1) | +3.95% |
| length | 2092.1 (1867.7-2511.8) | 2137.5 (1910.1-2267.6) | +2.17% |
| dispatch | 1879.2 (1739.3-3020.1) | 1790.5 (1752.0-2386.0) | -4.73% |

All ranges overlap and the directions disagree. Every valid slow observation
is retained. This small shared-host sample establishes no general latency
improvement or reliable regression. Retention is justified by restoring valid
program compilation and execution while preserving refusals, not by these
medians. There are 41 successful timed compile/native pairs (36 comparison,
five focused), plus five separate Terminal/native comparisons and six static
rejects. Review-internal compilation is excluded from these counts.

The source patch stayed identical from optimized build through validation.
Touched-file rustfmt, scoped whitespace checks, evidence-script parsing and
the identity/outcome assertions in [collect.py](collect.py) pass. No temporary
diagnostic logging or stack-size change remains in production. The existing
original controls remain unchanged; the smaller positive/reject pair is pinned
under `tests/omega/{pass,fail}/calls/selected_contract_equation_reentry*`.

Limits: Windows x86-64 execution only; other targets are bound in the new
fixture but were not run. No full corpus or other-host gate. Coordination
registration timed out twice and subsequent filtered status found no active
ticket; those failures did not block isolated engineering. No PR or push is
part of this local iteration.

## Next question

The overflow no longer blocks the numeric quality gate. The first review of
these std-dependent controls still costs 54-58 seconds, while their subsequent
timed compiles take 9-11 seconds. Attribute that complete package-preparation
path before another small numeric clone optimization: separate source graph,
checking and review artifact generation while preserving package custody and
ordinary review acceptance. Those wall times do not yet identify removable work.
