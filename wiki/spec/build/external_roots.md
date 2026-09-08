# Installed external roots

An external root admits entry without an Omega caller. Program entry,
interrupts, and foreign callbacks use [target slots](entry_roots.md) and the
same distinction between selected plans, installed authority, and runtime
liveness. OS table schemas, publication policy, and device protocols belong to
consumer packages, not compiler-owned lifecycle types.

## Admission and removal

A reclaimable inbound stub needs a registration owning the foreign-held edge
and any required code/component lease. The installed root ledger retains:

- exact entry, boundary plan, selected provider/effect/receipt, and artifact;
- normalized service reach and provider exit assurance;
- stack domains, nesting/preemption and acknowledgement relationships;
- independent resource ceilings, realized facts, and validation evidence; and
- liveness, installed-code occurrence, and component-version pins.

Installation consumes independently supplied publication authority. Its linear
root handle borrows installed-code custody. Removal returns authority only
after exact unreachability and required quiescence. A process-lifetime static
callback needs the same build-time plan and report, but no live replacement
ledger when neither its code nor state can be reclaimed.

The validated selected provider plan survives in the root's normalized identity
and provider execution. Execution cannot accept a second independently supplied
plan. Drift in entry, reach, provider, resource realization, installed occurrence,
or boundary commitment rejects. Compact fingerprints are report coordinates;
the exact facts and strong identities remain available for replay.

## Resource columns

| Resource | Ceiling | Realization evidence |
| --- | --- | --- |
| Stack | Admitted domain provision | Composed WCSU and entry-context derivation. |
| Logical work | Logical-fuel provision | Maximum logical work and IR proof/admitted-provider evidence. |
| Machine state | `StatePlan` | Actual footprint and final-code evidence. |

These columns are independent; a calling-plan digest substitutes for none of
them. Root admission composes the artifact-wide stack/nesting relation, not only
each body's demand. See [entry-stack composition](../resources/entry_stacks.md).
Logical-work evidence distinguishes reconstructed Terminal entry/segment facts
from admitted opaque-provider claims, and joins each to its actual installed
occurrence. A segment claim is not a whole-entry certificate.

Reports retain ceilings, realized facts, and validation receipts without exposing
private proof data or numeric entry addresses. Maximum logical work is not WCET;
it may support analysis but reserves no register, changes no ABI, and authorizes
no sponsor, transfer, or resume operation. See [logical work](../resources/logical_work.md)
and [machine-state evidence](machine_state_evidence.md).

## Installation-bound reach

An installation-bound requirement may declare one bounded abstract row
`reaches <= Bound`. The root manifest retains its exact requirement-path
identity, normalized bound, and all internal dependencies before selection.
Ordinary callable package/component contracts cannot carry the unresolved row.

Selection supplies the exact operation row. Installation proves it is within
the bound, substitutes it through the complete root closure, and rejects any
remaining unresolved row. Equal service sets do not establish entry/completion
or multi-operation protocol coherence; that requires exact provider execution
and lineage.

## Interruption ordering

Installation is the source of evidence for `Atomic::interruption_fence`.
The root route identifies handler, asynchronous source, execution context, and
interrupted-code relationship sufficiently to derive same-context entry.
The operation cannot assert that relationship itself. Missing installed-root
or provider evidence rejects. The relation orders compiler-visible coherent
memory; device and cross-core ordering need their own contracts.

Current entry-origin and provider support is documented beside
[external-root implementation](../../../omega-rust/omega/backend/runtime/external-roots/README.md).
The [component-publication contract](component_publication.md) owns live eras
and publication transactions; emitting a stub alone grants no installed root.
