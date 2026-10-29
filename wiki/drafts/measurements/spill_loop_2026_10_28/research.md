# Next allocation experiment

2026-10-28. PR740 passed both source and merge-group compiler checks and merged
as `2d87f18a195ec8523aeb4cdccc4ca0f793ba0e1b`; fetched-main ancestry verified.
The next fixed base is that published commit, in the existing isolated detached
`pass-ablation` worktree. The intervening main changes touch the allocator, so
the previous producer cannot establish the next experiment's baseline.

Reused [equality-loop profiles](../checking_simplification_2026_10_28/README.md)
and the [prior allocator iterations](../stage_five_2026_10_24/README.md).
The former records 1504 ms allocation, 627 ms bounded spilling, 384 ms batched
reanalysis and 260 ms home probing. Within spills, derivation is 335 ms,
mutation 191 ms and sealing 96 ms. Nested scopes overlap.

Prior work already reuses unchanged functions, block summaries, exact register
spans and prefix digests. Do not reintroduce broad recomputation or present an
existing optimization as new. Next step: fresh isolated-cache profile, then
split spill derivation into subject/use/target/home/span/slot costs before
choosing one change. Preserve current source-bound admission, finite work
budgets, target reservations, value identity, and final home validation.

Search question: how do register allocators reduce repeated spill work without
weakening operand constraints or approximating live ranges?

Exact queries:

- `site.cfallin.org regalloc2 performance spilling data structures allocation`
- `site.llvm.org register allocation compile time spilling analysis invalidation`

Read the author's [regalloc2 account](https://www.cfallin.org/blog/2022/06/09/cranelift-regalloc2/),
especially the performance and correctness sections. Its relevant lessons are
to measure scans and representation costs, avoid producing unnecessary work,
and preserve precise liveness when approximation would cause more splitting.
This supports profiling Omega's current derivation and existing block summaries;
it does not establish that any proposed cache or heuristic is safe or faster.
Other search hits were screened only; no claims are based on them.
Result: [slot-reuse ablation](README.md). Candidate-list collection was only
2.380 ms; access scanning and candidate summary preparation dominated the slot
cost. Removing reuse reduced derivation work but increased the larger function's
explicit stack reservation from 648 to 19,944 bytes. The ablation was rejected
and reverted. Overall timing remains confounded by shared-host variation.
