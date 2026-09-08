# Placed access and resident custody

[Content custody](content_custody.md) defines claim-local roles.
[Boundary realization](../terminal-psi/boundary_calls.md) defines demanded
applications and selected provider evidence.

Package-aware `Placed<P, S>` discovery resolves exact policy/schema identities
and checks both declarations' visibility and direct-dependency authority before
synthesis. Because the compiler shell is erased before ordinary type-selection
capture, both inputs must be public even for local use. The inert opaque field
carrier follows shell visibility; operation visibility follows exact
`AccessExposure`, and binding-private access stays inside the policy package.
Statement operations retain their exact generated target. Equal spellings cannot
launder private declarations through a source-free shell.

## Occurrences and views

Terminal Psi does not serialize a concrete address as authority or re-resolve
source fields, placement `P`, or resident type `T`. A `PlacedOccurrenceId`
binds the qualified root occurrence, normalized placement, provider/profile
receipt, mapping/revision, range, lifetime, and boundary reach without an address.

A separate `ResidentClaimId` names owned dormant content.

| View | Establishment | Resident-preserving retirement |
| --- | --- | --- |
| Owned | Transfer the dormant claim into one temporary placed occurrence. | Return the same dormant claim. |
| Borrowed | Establish a loan naming exact parent claim, range, polarity, and lifetime. | Release the loan; do not remint custody. |

Installation metadata describes the demanded binding; the installer must consume
the corresponding provider receipt and custody. Metadata alone is not authority.

Provider-backed Stable and Atomic resident lifecycles replay exact placement,
profile/resource, observation, origin, lineage, geometry, provenance, era, claim,
and receipt relationships. Atomic borrowed views retain the lender's authority;
exclusive borrowing adds no Atomic permission. Rejection returns the complete
inputs or active carrier unchanged.

## Projected events

Each event retains its placed occurrence, normalized `P`/`T`, canonical field
key, normalized displacement and width, logical extent, physical effect footprint,
operation family, plan receipt, and applicable lifetime, revision, reach, and
custody identities.

The backend receives the bound base and retained displacement. It must not
reevaluate layout or resolve names.

Closed access families are Stable read/take/write/swap, External read/take/write,
atomic load/store/swap, observing/non-observing decisive/single-attempt
compare-exchange, and the individual fetch families.

| Event | Effect |
| --- | --- |
| Pre-event specialization/installation rejection | Return all inputs unchanged; emit no access event. |
| Admitted write | Commit the write. A physical fault is a no-successor crash, not a program-visible write rejection. |
| Stable take | Transfer the exact resident field, leaving a structurally partial occurrence. |
| Stable swap | Return displaced custody. |
| External take | Advance the external content version and return a provider-provenanced whole snapshot. Introduce a content root only when the snapshot contract is content-bearing. |

There is no generic External swap. An authored provider exchange is a separate
operation.

## Atomic compare-exchange outcomes

Observation and attempt policy are separate axes; `Once` means single-attempt,
not non-observing.

| Operation | Result type and canonical cases | Resident requirement |
| --- | --- | --- |
| `AtomicCompareExchange<T>` | `AtomicCompareExchangeOutcome<T>`: `Mismatched(observed: T)`, `Exchanged`. | Copyable `T`. |
| `AtomicCompareExchangeOnce<T>` | `AtomicCompareExchangeOnceOutcome<T>`: `Mismatched(observed: T)`, `Exchanged`, `Uncommitted(observed: T)`. | Copyable `T`. |
| `AtomicTryExchange<T, Key>` | `AtomicTryExchangeOutcome<T>`: every case owns one `T`, proposed on failure and displaced on success. | Affine or linear `T` is allowed. |
| `AtomicTryExchangeOnce<T, Key>` | `AtomicTryExchangeOnceOutcome<T>`: same custody rule, including uncommitted attempts. | Affine or linear `T` is allowed. |

Observing failure exposes a copy of the resident; affine/linear residents reject.
The checked observing contract binds exact field and resident type, unrestricted
multiplicity, transfer width, permission identity, and the selected closed result
shape. A try-exchange-only field gains no observing permission.

For non-observing operations, the copyable key and checked raw-transition law
determine comparison without constructing another owned `T`. The law cannot
erase displaced custody: success returns it, and ordinary multiplicity determines
whether it may be discarded. Key and encoding law remain call evidence; they are
not runtime outcome parameters.

An independently checked join between a provider-backed request and a resident
contract compares the complete placement structure, field key/width, claim,
occurrence, operation axes, and result shape. It returns the unchanged non-clonable
request on rejection. This join is not an attempt, result, source call, provider
selection/installation, or Terminal/native lowering authority.

## Generic provider demand

`ResidentContentTransfer<P, T>` is one requirement schema, not an ambient provider
slot for every monomorph. Artifacts retain concrete applications and export
symbolic applications. Final composition substitutes reachable arguments, derives
closed demand, and verifies the selected realization for every application.
Installation binds the resulting exact occurrences.

Distinct provider slots are justified only when applications require independent
selection. Provider-asserted generic/family coverage and indexed arity/string
schemas do not establish a realization. Missing checked application coverage
rejects; it grants no admission, resident custody, transfer, or installation authority.
