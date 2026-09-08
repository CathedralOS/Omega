# Device custody and ordering

[Extents](extents.md) supply authority; [placed access](placed_access.md) defines
primitive observation. Device protocols, DMA queues, and IPC ownership remain
consumer/provider code. They are not new clauses on every boundary or additions
to the service-reach vocabulary.

## External loans

DMA is an external borrow represented by a linear proxy for the device:

| Direction | CPU exclusion while lent |
| --- | --- |
| Device reads | Shared loan; CPU mutation excluded. |
| Device writes | Exclusive loan; CPU reads and writes excluded. |
| Bidirectional sharing | Explicit atomic/coherence protocol required. |

The grant binds borrower, direction, space, provenance, required rights, and
completion obligations, including target fence/cache facts. Each transfer also
requires an exact confinement receipt: the borrower contract or hardware
isolation confines that loan and direction to the actual range, address space,
mapping era, lineage, and attenuated rights. Numeric coincidence cannot grant
access to control storage. Missing, stale, or overbroad reach rejects before
transfer.

Completion binds the exact live loan, confinement receipt, borrower/device,
direction, range, mapping, rights, provenance, and lineage. It restores CPU
custody only after proving borrower release and every ordering/coherence
obligation. Device status is independent: failure may accompany proven release;
success without release does not restore Stable access. Failed start returns
the borrow; failed completion/acquisition returns the still-live pending token
and completion candidate for recovery. The token may cross suspension when its
carry contract permits it. Result-container multiplicity does not erase its
active payload's linear debt.

## Ordering roles

Publication, cache maintenance, notification, completion, and acquisition are
distinct protocol roles. A role discriminant participates in canonical identity;
equal payloads for different roles are not interchangeable. Each event binds
its exact ranges, mappings, stable device instance, and runtime queue/session
scope to admitted coverage. Roles may relate different data, descriptor,
doorbell, read-back, request, or completion coordinates; a uniform one-range
carrier is not a public ABI.

Build selection admits a provider and scope-capability schema, not runtime scope
occurrences. The installed provider issues opaque occurrences; source can carry
them but cannot construct, inspect, or compare their identity. Complete mapping
and schema/device evidence remains available; compact IDs cannot replace it.

Publication names the place and current write state. An intersecting write
invalidates it before notification may consume it. Erased evidence alone does
not constrain emitted instructions: publication adds a scoped Terminal Psi
ordering event preserved through target cache maintenance, barriers, OS
operations, or an instruction-free coherent realization.

Acquisition consumes completion tied to the same request, external loan, device,
and runtime scope. Only proven release restores a Stable CPU view. A resource
profile expresses backing potential; the active loan expresses its current
ownership phase. No mutable phase flag is needed in the profile.

One External write does not prove that a device observed a posted write.
Fences, read-back-to-flush, and device completion have separate checked
contracts; a CPU barrier cannot guarantee completion on every fabric/device.

A complete admitted DMA service boundary is an allowed provider implementation
route. It may keep register/cache/queue steps private under its declared contract;
admission is not proof of those internal steps. The checked-driver composition
surface still requires a concrete driver and admitted role-specific signatures.
Provisional structural row constructors establish no event, Stable view,
completion, or lowering authority. Missing/extra/duplicate/drifted provider
coverage rejects without consuming retry custody. [TASKS.md](../../../TASKS.md)
owns that implementation work.

## Shared-memory IPC

Shared-memory IPC and MMIO share external mutability, not identical observation
semantics. Proved or mutually trusted peers may use an atomic protocol returning
a linear lease with Stable payload access until explicit release. A hostile
writable peer's cooperation proves nothing: copy then validate, or require a
provider to revoke/remap its write permission and complete cross-core
invalidation before zero-copy validation. Translation and protocol policies
remain OS-owned.
