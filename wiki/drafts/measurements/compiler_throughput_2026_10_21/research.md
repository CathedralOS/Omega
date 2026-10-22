# Field-inventory reuse research

Date: 2026-10-21 (America/Los_Angeles).

Question: can source-contract discovery avoid rebuilding a declaration's field
inventory for every application without caching scope-dependent judgments?

Existing evidence: `../stage_by_stage_optimization.md` documents earlier checking
work and a generic-synthesis snapshot experiment. This new experiment concerns
`SourceFields` inside `TypedTrees::applied_trait_roots`, not synthesis snapshots
or reuse of checked dependency bodies.

Query: `site.rustc-dev-guide.rust-lang.org queries memoization immutable query caching`.
Read the complete [query evaluation model](https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html),
especially Caching/Memoization and Input data. The useful requirement is that a
query's key, inputs and result remain immutable and its computation is pure.
This supports considering reuse, but provides no Omega performance evidence.

Local applicability: collecting field type handles reads one data declaration
and the borrowed `TypedTrees`; it does not read the lexical owner, application
frames, substitutions, visited occurrences or active recursion stack. Those
contextual inputs belong to the subsequent traversal and must still be applied
for every occurrence. An inventory owned by a single discovery call cannot
survive a mutation or leak into another compiler stage. Persistent caching and
new cross-stage invalidation machinery are unnecessary for this experiment.

Next comparison: measure collection calls, visited field nodes and elapsed
collection time; retain the same ordered field handles once per reached
declaration, then compare full sample-to-native time, outputs and affected
positive/rejection fixtures. Reject the candidate if it changes those outcomes
or introduces a reproducible end-to-end regression.
