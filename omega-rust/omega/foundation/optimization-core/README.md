# Optimization core

See [`rules.md`](rules.md) for the exact-rule inventory, rollback procedure, and promotion rules.

# Exact-rule promotion records

The [promotion contract](../../../../wiki/spec/build/optimizations.md#release-rollback-and-promotion)
requires an explicit owner decision and evidence for one exact optimizer rule.
This directory holds completed, reviewed records, not per-rule progress logs.
No architecture gate checks these Markdown records yet.

Incomplete historical evidence pointers are in the
[promotion draft](../../../../wiki/drafts/measurements/optimizer_promotion_evidence.md);
they grant no approval. The [inventory](rules.md) keeps those rules
experimental.

A completed record names the exact rule, approved status, owner decision,
semantic/corruption controls, differential execution, determinism and bounded
work, target coverage, measurements, and exact rollback evidence. Cite real
repository artifacts and observed commands/results, and name missing or
retired evidence. Promotion never implies an optimization-level bundle.
