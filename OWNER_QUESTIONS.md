# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-18.

## 1. Carry and runtime contract spelling

The normalized model keeps suspension, CPU affinity, host-thread affinity, and
address stability independent. Runtime providers separately state preemption,
migration, affinity support, and continuation-storage behavior; admission joins
the two.

The remaining owner decision is declaration vocabulary and defaulting:

- Which restrictions use the existing declaration-property surface?
- Which are constructor/provider result contracts because they vary per mint?
- How are opaque resources born strict without making transparent ordinary data
  noisy?

Recommendation: structural permissive derivation for transparent data; opaque
resources born strict; declaration properties only for type-wide facts and
constructor contracts for mint-dependent facts. Do not add use-site keywords.

## 2. Executable publication evidence and lifecycle

The architecture distinguishes first publication of writable/unpublished bytes
from replacement of already-executable code. The first is a target provider
operation; the second belongs to component quiescence/versioning.

The remaining owner decision is the normalized evidence/API boundary:

- What value/domain proves bytes are finalized and eligible for first
  publication?
- What artifact records cache/coherence/W^X completion and target scope?
- How does the publication contract distinguish dormant future executors (for
  example an AP waiting for SIPI) from cores that may already be executing the
  range and therefore require quiescence/replacement authority?
- How do static images, runtime-generated code, and dynamically admitted code
  share footprint validation without pretending there is one universal final
  link step?

Recommendation: a linear publication-state transition producing an executable
artifact/capability; validate at every realization/admission boundary; keep
live replacement entirely separate.

Detailed settled context and engineering residue are in
[`wiki/design_briefs/os_memory_and_hardware_foundation.md`](wiki/design_briefs/os_memory_and_hardware_foundation.md).
