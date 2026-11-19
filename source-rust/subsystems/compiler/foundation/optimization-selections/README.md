# Optimization selections

Build and compiler stages share the exact source names, stable wire tags and
phase ownership in the [selection catalog](src/lib.rs). Build resolves an
authored selection once. Each phase receives its projection and the identity
of the complete selection. The portable Psi vocabulary lives with
[Psi](../../psi/optimization-selections/).

Native stages keep decision records and validate their manifests using
[optimization records](../../psi-to-native/representations/optimization-records/).
Each numbered stage implements its own rewrites. See the [exact-rule inventory](rules.md).

## Exact-rule promotion records

The [promotion contract](../../../../../wiki/spec/build/optimizations.md#release-rollback-and-promotion)
requires an explicit owner decision and evidence for one exact optimizer rule.
This directory holds completed, reviewed records, not per-rule progress logs.
No architecture gate checks these Markdown records yet.

Incomplete historical evidence pointers are in the
[promotion draft](../../../../../wiki/drafts/measurements/optimizer_promotion_evidence.md);
they grant no approval. The [inventory](rules.md) keeps those rules
experimental.

A completed record names the exact rule, approved status, owner decision,
semantic/corruption controls, differential execution, determinism and bounded
work, target coverage, measurements, and exact rollback evidence. Cite real
repository artifacts and observed commands/results, and name missing or
retired evidence. Promotion never implies an optimization-level bundle.
