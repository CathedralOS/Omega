# Repeated trait-root discovery

Date: 2026-10-22 UTC. Baseline: local commit 1c2376d0575, continuing the
[previous experiment](../compiler_throughput_2026_10_21/README.md).

Question: which of the 21 applied-trait root queries can be removed or shared
without reusing stale source scopes after mutation?

Reviewed the previous immutable-query research before searching.
Query: `site.llvm.org/docs NewPassManager cached analyses invalidation getResult`.
Read [Using the New Pass Manager](https://llvm.org/docs/NewPassManager.html),
analysis-manager reuse and invalidation sections. LLVM requires a preserved or
updated analysis when transformations change its inputs. This supports limiting
reuse to a single immutable borrow here; it supplies no Omega performance claim.
A persistent analysis cache would require broader invalidation ownership.

Local inspection rules out sharing roots across logical signature preparation:
`visit_declarations` visits applied trait signatures, and preparation appends
those signatures. Matching root counts are not proof of identical input.
Specialization, projection and seeded continuations also alter source roots.

The baseline length trace records 21 calls: four projection calls and one
retained-argument validation call have zero retained implementation applications.
Their root inventory has no consumer. Every remaining query separately runs
`TypeOccurrences` three times to collect conformance occurrences, type
occurrences, and qualified requirement calls from the same immutable program.

Hypothesis: remove discovery when the retained-application consumer is empty;
within each remaining query, prepare those three occurrence lists in one walk.
Preserve occurrence order, lexical ownership, generational handles, all root
categories, signature-derived occurrences, and all nonempty validation.
Expected work: 21 root queries / 63 occurrence walks becomes 16 / 16 on length.
No cross-mutation result survives. Compare full native compilation on the same
three fact workloads, byte-identical images, source diagnostics, and native vs
Terminal behavior including calling-policy cases with retained applications.
Reject if outcomes change or end-to-end work/time regresses reproducibly.
