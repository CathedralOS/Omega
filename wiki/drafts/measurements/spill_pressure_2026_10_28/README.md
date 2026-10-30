# Spill-pressure batching experiment

Fixed compiler-source baseline: `bf39a3c901bc8ea3ee60b8b57fdc408a591dbacb`.
Continuation workbench: `8717155c5578dbf5de35f08dc91102dc08c55451` (same compiler
sources). Original release producer and measurement instruments are retained by
[the worker sweep](../compiler_parallelism_2026_10_28/README.md).

## Hypothesis and rejection rule

On the 32-argument `other_machine` fact, a fresh instrumented baseline takes
4,138.5 ms cold and 1,829.5 ms warm. Warm register allocation takes 1,099.9 ms,
including 3,300 bounded spills, 103 reanalyses (278.2 ms) and 103 assignment
probes (204.0 ms). These spans identify work; they are not uninstrumented
optimization results or a guaranteed speedup.

`compute_function_packed` records a failed component without assigning it a
home, then continues placement. Recovery nevertheless drops neighbouring failed
components from the pending batch after rewriting this unplaced component.
Test retaining those neighbours: an unplaced component releases no proposed
home. Keep the existing assigned-victim policy, rewrite admission, fresh
reanalysis, final home validation and manifest construction. Reject if the real
workload does not activate the rule or has no repeatable end-to-end benefit.
Native bytes are compared, and native/Terminal agreement and fact-sensitive
rejects remain separate quality gates. No remote source publication is part of
this experiment.

## Primary-source search, 2026-10-28

Question: can allocation distinguish unassigned spill candidates from physical
interferences rather than restart analysis for both?

Queries: `LLVM regalloc greedy selectOrSplit unassigned spill interference live
intervals source`; `regalloc2 spill all unallocated bundles allocation loop
source`.

Read LLVM's current [RegAllocGreedy.cpp](https://llvm.org/doxygen/RegAllocGreedy_8cpp_source.html),
specifically `LRE_CanEraseVirtReg`, `LRE_WillShrinkVirtReg`, `tryEvict` and
recolouring. It unassigns physical occupancy only when `hasPhys` is true;
recolouring interferences are explicitly allocated variables. This supports
checking Omega's proposal-state distinction, not copying LLVM's algorithm or
claiming the two allocators are equivalent. regalloc2's search result was not
used: GitHub fetch timed out and the attempted raw main-branch path returned
404. Existing broader parallelism findings remain in the
[reference aggregation](../../reference/compiler_parallelism.md).

## Result: rejected

Candidate `83a674d4382` was freshly built with checksum freshness and a verified
release profile. The rule activated only eight times; bounded spills changed
3,300 to 3,297, and all 103 reanalyses remained. Four interleaved warm pairs had
medians 1,815.5 ms baseline and 1,771.8 ms candidate (2.4% lower); individual
paired reductions ranged from -0.05% to 11.1%. This does not establish the
hypothesized removal of repeated analysis or a substantial stable benefit.

All 12 screen-produced native images exited 0. Baseline and candidate image
sizes were equal at 114,176 bytes, but their bytes differed. No semantic
equivalence or Terminal agreement is claimed for this rejected candidate.
Eight focused pressure tests passed (nine unrelated package tests excluded),
including indexed/unindexed tied components, order and other-function exclusion.
Touched-file formatting passed. Package-wide formatting reported inherited
differences and was not repaired. Clippy and expanded runtime qualification were
skipped after the mechanism failed screening.

The source edits were reverted. [rejected.patch](rejected.patch) preserves the
exact experiment; [evidence.json.gz](evidence.json.gz) retains observations and
producer receipts. Raw logs remain in ignored `build/spill-pressure-experiment`.
The next question is whether choosing unplaced components as victims can form
a useful batch, rather than usually removing an assigned blocker.
