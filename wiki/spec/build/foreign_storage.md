# Foreign storage and lifetime

Safe parameter/result contracts own foreign access and custody.
[Calling plans](calling_plans.md) describe representation, not permission or
lifetime. [Content conservation](../resources/content_custody.md#call-conservation)
and [structural access](../terminal-psi/structural_access.md) apply across native
boundaries without exceptions. Native support is separate from these contracts;
unsupported backing and lifetime routes reject.

## Inert addresses and marshaling

`addr` is numerical address data. `Ptr<T>` is a sealed, inert ABI carrier whose
type parameter supplies representation/pointee shape. Neither grants authority;
ordinary code cannot dereference, index, or manufacture a reference from it.
A binding materializes a pointer only from established storage claims after
validating marshaling and calling policies. Inbound pointers become checked
views only through an authorized establishment route.

Policies explicitly associate native pointer/count pairs or descriptor graphs
with semantic extents. The compiler does not guess that association. A policy
defining a native slice ABI derives the ordinary reference case; several separate
native arguments are not equivalent to a record containing them. See
[boundary shapes](boundary_shapes.md). Raw data validation and encoding follow
[format codecs](../layouts/codecs.md), not stronger facts invented by a cast.

## Outbound custody

| Disposition | Required contract |
| --- | --- |
| Call-scoped | `&T`, `&[T]`, `&write T`, `&write [T]`, `&mut T`, or `&mut [T]` grants only its exact access until return. |
| Retained after return | A linear protocol result owns stable backing, an explicitly lifetime-parameterized result retains a checked loan, or a permitted semantic snapshot uses private stable backing. |
| Process-lifetime | Authority moves into an already-established static/process-lifetime root. There is no general permanent-custodian spelling. |

A retained claim owns keepalive and reclamation authority, not necessarily its
bytes inline. It may lend lexical views only over rights the foreign side does
not hold. Read-only foreign use may preserve semantic facts and permit Omega
read views. Foreign writes invalidate facts over exactly the writable extent;
terminal completion establishes the promised resulting facts. Partial release
splits an exact separated subextent from the remainder still in flight.

Published results and ordinary conservation determine whether use survives
return. Consuming `Buffer` into `PendingWrite` can transfer its content;
`submit(&buffer) -> PendingWrite` cannot create owned retention. An explicit
`PendingWrite<'a>` may instead retain a compatible checked loan for `'a`.
Infer unambiguous consumed-input-to-result mappings; an ambiguous mapping needs
an ordinary postcondition establishing the exact correspondence. Unsupported
mappings reject. Qualified content uses its owner-unique `Content<A>` projection,
not a foreign-only extent algebra or annotation.

Every retained native pointer slot needs exact stable-root, range, access,
lifetime, and revision/lease provenance. Unknown provenance rejects unless an
admitted provider route establishes it. Embedded layouts have finite structural
closure. Recursive or dynamically sized pointer graphs retain one arena/extent
root covering the graph; the compiler does not traverse runtime pointers to
discover custody.

Private snapshots require an explicit semantic contract permitting independent
copying, with neither identity preservation nor unchecked write-back. They do
not change the separately compiled public result type. Requirements publish
unavoidable caller lifetimes/custody; realizations validate their concrete
backing recipes. Concurrent native and Omega access is External placed backing.
Exclusive foreign mutation may instead move storage into the protocol and return
it with preserved, invalidated, or outcome-dependent qualifications.

Snapshot bytes count as persistent demand per live occurrence. Bounding aggregate
demand also requires finite live-occurrence capacity: success transfers the exact
capacity into the registration, rejection returns it unchanged, and successful
unregister returns the same occurrence. A consumable lifetime budget is distinct.
Static thunk storage is bounded by distinct admitted callback identities, not
live-registration count. [Callback registration](private_callbacks.md#registration-and-lifetime)
separately owns the code/component lease.

## Provider-owned views and completion

A view whose invalidators all require exclusive access to one receiver is an
ordinary borrow from that receiver. More precise protocols use a linear validity
claim that every invalidating operation consumes. Global, thread-local, or
asynchronously invalidated storage must be copied, mediated by such a claim, or
accepted under an admitted stability promise. A claim cannot prevent an opaque
provider from invalidating through an unmodeled route.

Completion correlates one event with one live claim through a unique/nonreused
identity, generation-checked identity, or exclusively ordered channel. Progress
releases nothing unless it returns an exact separated subclaim. Cancellation
releases nothing until terminal acknowledgement. Tokens need generations when
stale foreign copies may survive reuse.

[Boundary completion receipts](../terminal-psi/ownership.md#claim-paths) bind the
exact operation, boundary, argument position, and complete consumed claim set.
Interpretation and native realization join them to the admitted provider
execution. Rejected provider completion commits no receipt or custody transfer;
that does not undo effects already performed by native code.

A provider-created handle or pending operation pins the exact era whose state
gives it meaning. A rebindable service binding names a slot and does not itself
pin an era. Pins prevent reclamation, not teardown execution: the old era remains
callable while discharging its roots, then waits for application-held claims,
quiescence, and unload. Static custodians discharge outlives relationships at
build time without runtime ledger entries. See
[component publication](component_publication.md).

## Write-only foreign access

The ordinary [write-only rules](../terminal-psi/structural_access.md#write-only-authority)
govern `&write T`: an existing valid referent, no observation or access widening,
discardable displaced content, validity after replacement, and exact outcome
write frames. Checked providers enforce these through their call closure; opaque
compliance requires admitted evidence unless isolation enforces it.

A byte-producing outcome names its exact modified prefix or other footprint;
facts over the untouched complement survive. Its returned count cannot establish
a value absent at entry. Write-only access transfers no durable custody.
Identity-only retention is a stable keepalive claim lending no view. Typed
construction into storage with no live `T` is a separate feature, not another
interpretation of `&write`.

## Status

This records current implementation evidence against the sections above; it is
a status annotation, not a contract amendment. Surveyed at `500878c473`
(linux x86-64).

Enforced today:

- Outbound custody dispositions are explicit in
  `omega-rust/omega/backend/runtime/external-roots/src/program_local/
  program_local_extents/retained_foreign_arguments.rs`: call-scoped arguments
  stay borrows minted through `loan_under_activation`/
  `loan_mut_under_activation` under the establishing activation, and
  longer-lived dispositions are recorded as `LifetimeBorrowed`, `Moved`, or
  `Snapshot` custody entries — matching this chapter's three retained routes.
  Unknown ambient backing and substituted mapping revisions reject there, the
  provenance refusal this chapter requires.
- Foreign calls carry a normalized shape end to end: `normalized_foreign`/
  `normalized_foreign_call` surfaces run through
  `native-realization` (`providers/settlements`, `terminal_authority_policy` —
  `validate_boundary_entry_plan` validates the boundary entry plan against the
  call signature), `post-allocation-machine-to-selected-form-encoding` row
  encoding and validation, `target-operations-to-selected-instructions`
  legalization, both ISA encoders (`isa-x86_64`, `isa-aarch64`
  `selected_form_encoding`), and `image-emission` `object_artifact`
  `normalized_foreign_calls`.
- Provider-backed field classification and foreign-receipt rejection sit in the
  checked-content suite (`typed-trees-to-checked-trees` `checks::content`;
  recorded 14/14 green on the board at `a0591618c7ba`/later re-verifications).
- Authored rejections are pinned by corpus fixtures:
  `tests/omega/fail/entry/service_foreign_domain_rejected`,
  `tests/omega/fail/expressions/token_bound_machine_foreign_family_rejected`,
  `tests/omega/fail/core/content_projection_foreign_owner`, and the
  termination `*_foreign*` rejection rows.
- Write-only foreign access leans on the ordinary write-only enforcement
  surfaces (`checks/multiplicity/borrowed_windows.rs`,
  `checks/multiplicity/linear_validation/recorded_events.rs`,
  `checks/contracts/exits/scalars/write_transport.rs`,
  `checks/ranges/facts/call_writes.rs`).

Open frontier, unchanged:

- The provider-planning to native-settlement join for the installed occurrence
  is not landed; the surviving lane records live on the board under
  `PROVIDER-ATTACHMENT-MACHINE-PLAN`.
- The boundary-ABI lane for opaque-by-value carriers is open under
  `OPAQUE-BY-VALUE-BOUNDARY-ABI`.
- The `external-roots` subtrees continue under their own spec-alignment leg.
