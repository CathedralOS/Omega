# Reusing proof search

This is a performance hypothesis, not a selected design or implementation claim.
Reuse is worthwhile only when a measured workload repeats substantial search and
retaining the derivation costs less than finding it again.

[Compiler storage](../../spec/build/compiler_storage.md) permits in-memory reuse
within one build invocation, not a persistent compiler cache. Published artifacts
have a separate admission contract.

## Constraints on an experiment

Retain the complete proposition, subjects, selected laws, assumptions and
substitutions with a reusable derivation. A cached success flag or equal display
string is not evidence. A receiving local obligation needs evidence establishing
that exact obligation through ordinary proof admission.

Canonical identity cannot depend on search budget or which laws happen to be
found. Dependency changes invalidate affected reuse. A key for a proof term can
identify a stored derivation without answering how to find it from a new goal.

Measure repeated search, retained bytes, lookup and proof-admission cost before
choosing granularity or storage. Sharing a large proof may reduce allocation
while its validation remains expensive. No particular container, new artifact
format or source rewrite follows from this hypothesis.

[Published judgments](../../spec/terminal-psi/product.md#publication-and-separate-consumption)
are different: host authorization permits reuse of a trusted producer's exact
judgment without rediscovery or mandatory certificate replay. A local lookup
does not grant that authorization. Evaluation reuse separately preserves the
[logical accounting](../../spec/language/evaluation.md) of the computation.

No current instrumentation, cache implementation or hit rate is asserted here.
Superseded evaluator and measurement details belong in Git. Delete this note
when a measured repair establishes a concrete owner, or when the hypothesis is
no longer useful.
