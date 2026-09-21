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

## Secondary-processor startup

Cathedral or another consumer package owns processor discovery and startup
sequencing, including requests, acknowledgements, retries, and cancellation.
Ordinary machines and explicitly selected boundary providers express that
protocol; no AP-specific language form or compiler-owned boot driver is needed.
The compiler compiles entry code and checks its calling, installation, stack,
state, and lifetime contracts. Hardware behavior that code cannot establish
remains an explicit admitted boundary assumption.

The selected provider's startup contract specifies its arrival regime, entry
representation, placement constraints, and completion guarantees. Bind that
contract to the exact installed entry, processor, resources, and invocation.
Caller-supplied profile numbers establish no hardware facts. A low-memory
trampoline and address-derived startup vector are requirements of particular
protocols, not a universal shape for firmware or other startup providers.

| Established outcome | Custody consequence |
| --- | --- |
| Definitely not dispatched | Return the account to pending custody so a fresh invocation can retry; no later arrival through the refused attempt may remain possible. |
| Possibly dispatched, arrival unconfirmed | The account stays invoked and held — neither withdrawable nor reissuable — and the outstanding carrier remains answerable by a later definitive receipt, which may still resolve the attempt to started. Timeout or lost acknowledgement releases nothing and erases no outstanding attempt. |
| Confirmed arrival at the agreed entry | The processor is marked started and its accounted stack and state stay held until retirement. |

Confirmation binds the exact invocation and installed entry. A checked
consumer-authored handshake or an explicitly admitted provider guarantee may
establish it; the compiler does not mandate a separate acknowledgement protocol.
Sending a request or constructing an ordinary success record is not confirmation.

Settlement of an unconfirmed attempt abandons it without a started record: a
settlement receipt must name the exact outstanding carrier and attest both that
the carrier can no longer dispatch — no later arrival through this attempt —
and that nothing executes on the account's dedicated stack or private state.
Only then does the account leave the ledger and return its complete custody. A
receipt missing either premise leaves the carrier outstanding, still answerable
by a later definitive startup receipt or a later settlement answer. It cannot
require a fabricated successful-start record to reach cancellation. Retirement
of a started processor needs a provider quiescence receipt naming the started
record exactly; an incomplete drain keeps the account held and hands the
started evidence back. An unresolved attempt cannot become a fresh independent
attempt by forgetting its identity: completion and settlement both bind the
record's outstanding invocation, so delayed or replayed receipts cannot resolve
another attempt or release its resources.

Each processor's startup resources are accounted separately — a dedicated
stack class and a private state extent — so no two admitted processors can
share one backing. An admitted account that has never been invoked may
withdraw with its resources returned intact; once an invocation is
outstanding the account can no longer withdraw, since the attempt may still
arrive, and a started account stays held until retirement. The startup
ledger borrows the installed entry for its whole lifetime: no completion,
settlement, or retirement edge releases that installation while an attempt
under it could still exist.

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

`StatePlan` is part of the public boundary contract. Stack and fuel realization
facts normally belong to the candidate and admission provision, not public
API identity. A replacement contract promising no reprovisioning must explicitly
publish the resource ceiling that promise requires. Equal public contracts do
not excuse replaying stale candidate-specific resource evidence.

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
