# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-18.

## 1. Runtime-provider behavior contract supply

Value-side carry spelling is settled: compiler-built-in `[carry(...)]` records
type-wide guarantees over four independent axes; transparent data derives;
opaque data is born strict; sealed `ensures` domains establish per-mint
relaxations. The normalized policy is compiler IR, not ordinary `omega::core`
data, a trait, or a machine-produced plan.

The remaining owner decision is only how a `TaskRuntime` provider authors or
supplies its behavior counterpart:

- Where do safe-point/asynchronous preemption, migration, affinity support,
  host-thread behavior, and continuation-storage stability enter the derived
  provider plan?
- Which claims are proved from a checked provider and which are accepted under
  admission receipt for an opaque/host runtime?
- How does a dynamically admitted runtime supply the same normalized contract
  without creating a second admission path?

Recommendation: add one normalized runtime-behavior contract to the existing
provider-plan/admission spine; derive checked claims, receipt-gate accepted
claims, and add no runtime type property or new declaration clause.

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
