# Exact-rule promotion records

The [promotion contract](../../../../../wiki/spec/build/optimizations.md#release-rollback-and-promotion)
requires an explicit owner decision and evidence for one exact optimizer rule.
This directory is for completed reviewed records, not per-rule progress logs.
There is currently no architecture gate validating these Markdown records.

Incomplete historical evidence pointers live in the
[promotion draft](../../../../../wiki/drafts/measurements/optimizer_promotion_evidence.md);
they confer no approval. The [inventory](../rules.md) keeps those rules
experimental.

A completed record names the exact rule, approved status, owner decision,
semantic/corruption controls, differential execution, determinism and bounded
work, target coverage, measurements, and exact rollback evidence. Cite actual
repository artifacts and observed commands/results, distinguishing missing or
retired evidence. Promotion never implies an optimization-level bundle.
