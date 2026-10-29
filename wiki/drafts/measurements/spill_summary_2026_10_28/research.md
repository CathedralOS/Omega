# Contiguous spill-candidate summaries

2026-10-28. Continue the [slot-reuse investigation](../spill_loop_2026_10_28/README.md)
from fixed documentation head `4253e7254728b9189485337aa55db0d901be3d8d`, whose
compiler, bundled sources, lockfile and toolchain are unchanged from published
`2d87f18a195ec8523aeb4cdccc4ca0f793ba0e1b`. The retained optimized baseline
digest `75dcfeb5061db9f92709ad3d9a6f8246c7fb789a2222b910d662bc3f91f74b1d`
was checked before reuse. Do not rebuild or rebase merely because main moved.

Question: can candidate-summary construction avoid distributing the existing
event buffer into per-block vectors, while retaining exact slot-sharing results?
The previous diagnostic charged 95.797 ms to 58,212 summary queries, so the
possible whole-compile benefit is modest; that total is not all avoidable work.

Candidate: retain the raw event allocation, stably sort by block and instruction,
deduplicate with the existing last-event rule, then index contiguous block ranges.
Remove the unused candidate `has_loads` vector while retaining the consumed
load-block list. This is temporary derivation storage, not a durable IR change or
a cross-rewrite cache. No new invalidation protocol is needed. The extra global
sort and wider retained tuples may offset saved allocations: test, do not assume.

Scope: `RawCandidateSlots::get`, `CandidateAccesses`, and the two block-stream
consumers in `shareable`; caller recovery, slot ordering, budgets, validators,
source fixtures and public interfaces remain the same. Tests compare summaries
against a map that keeps the last event at each position, cover empty blocks,
terminal loads and invalidated summaries, and distinguish disjoint from
destructive storage windows. The real-path comparison uses the existing
isolated-cache A/B harness on stack32, length and dynamic dispatch; every native
image must remain byte-identical. Reject or park if no relevant benefit survives
measurement, or any correctness/resource gate changes.

Reused the previous [register-allocation research](../spill_loop_2026_10_28/research.md).
New query, prompted by the exact duplicate-order contract:

- `site.doc.rust-lang.org std slice sort_by_key stable dedup_by consecutive elements opposite order`

Read Rust's official [stable sort documentation](https://doc.rust-lang.org/std/primitive.slice.html#method.sort_by_key)
and [Vec deduplication contract](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.dedup_by).
Stable sorting preserves equal-key order; deduplication gives the later element
before the earlier one, allowing the retained event to be updated. Stable sorting
can allocate auxiliary memory, so this is not an allocation-free proposal.
These API contracts motivate the equivalence check, not a performance claim.
Other search hits were not used. No repeated general allocator search was needed.

Result: [inconclusive and parked](README.md). Two Rust tests and all 42
compile/native observations passed with identical images, but the unprofiled
stack32 median was effectively unchanged. Repeated diagnostic profiles favored
B while several untouched phases also improved. That attribution uncertainty
and the small whole-compile opportunity do not justify retention. The source
was restored and the exact patch retained for replay.
