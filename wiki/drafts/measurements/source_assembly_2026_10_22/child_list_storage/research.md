# Child-list construction research — 2026-10-22

Question: can nested parser lists share construction capacity without a counting pass, changing durable child order, or making syntax lifetime depend on a worker?

Consulted the existing [ingress ownership ledger](../../source_assembly_2026_10_21/README.md) and the [expression scratch experiment](../expression_scratch/README.md) before searching. Existing operator-stack bounds do not apply to arbitrary authored list lengths.

Query: `site.github.com/rust-analyzer/rowan GreenNodeBuilder children drain checkpoint`.

Read the actual [Rowan green builder implementation](https://raw.githubusercontent.com/rust-analyzer/rowan/master/src/green/builder.rs): the builder keeps one children vector, records its length at node entry, then finishes the node from its tail. Its checkpoint scope checks prevent crossing a closed node. This supports the nested-frontier hypothesis. Omega already has typed generational child arenas, so no Rowan green-node identity, hashing, interning or Arc ownership is adopted.

Also consulted the repository Rust systems skill's Squalr structure reference: mutable invocation/worker storage belongs at the owner performing the operation, rather than in a durable representation or a synchronized global cache.

Experiment: retain one temporary buffer each for expression handles, structural fields and match arms in explicit `ParseScratch`; nested closures publish only their tail and truncate on error. Reuse that capacity across a page's source files alongside the existing token scratch. Compare complete normalized syntax and errors, all-worker source assembly, allocator traffic, and separately uninstrumented timing. Reject if output differs or ownership survives a failed parse incorrectly. No extra token walk or authored size limit is permitted.

No additional search was needed once that falsifiable implementation existed. Results belong in [the experiment record](README.md).
