# State-owner lookup research

2026-10-22. Question: Stage 05 repeatedly locates a stored state's owner;
macOS sampling names `TypedTrees::machine_holding_state` as the largest active
leaf (63 samples; wait frames excluded from this ranking). Can the existing
revision-owned sorted state-position index replace its fallback roster scan?

Exact search: `site.llvm.org Programmer's Manual avoid repeated linear searches compiler data structures`.
Read the current [LLVM Programmer's Manual, sorted vector maps](https://llvm.org/docs/ProgrammersManual.html#a-sorted-vector),
including its insert-then-query advice and sorted-map tradeoffs. It describes
binary search over contiguous key/value storage. This applies to Omega's
already-built, frozen declaration index; no new map, LLVM dependency or
incremental sorted insertion is needed. The repository's index retains stable
duplicate order and full generational symbols and refreshes after arena or
visible-roster changes. Those semantics, rather than the external source,
determine correctness here.

Rejected direction before implementation: adding a second hash table would
repeat ownership already represented by StatePositionIndex. Constructor
observation setup was an initial candidate from inclusive timing, but sampling
identified the state-owner scan as a smaller, direct experiment. It remains
unmodified.

Experiment: preserve the old parent-first lookup's exact selection and eligibility
rules while resolving only indexed matching occurrences. Compare the ordinary
CLI, phase attribution, focused source corpus, both-engine witnesses and an
independent old-scan oracle over mutation/duplicate cases. Reject for changed
answers or no measured benefit. Results are in the adjacent README and JSON.
