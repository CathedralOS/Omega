# Runtime-sized activation storage

A runtime-sized activation allocation is a claimed extent of provisioned
activation-local storage: the program names a bound at authoring time and
commits an extent within it at runtime. The claim is the only route to a
runtime-sized region inside an activation — [dependent
values](../language/dependent_values.md#static-indices-and-runtime-witnesses)
denies the same request to a value binder, and no `let` local, extent index,
or runtime witness implies one.

## The bounded claim

An admitted claim surface requests the region by supplying a committed extent
and its bound, and yields a linear extent claim over the result. The committed
extent is chosen at runtime; the bound is a static byte extent, or a sub-extent
of provisioned activation storage the activation already holds under
[extent conservation](extents.md#conservation-and-loans). A claim whose bound
cannot compose into the activation's closed [stack
demand](storage.md#stack-demand-and-backing) rejects before any storage
exists — an activation without a closed demand cannot run.

The bound composes into provisioned demand the same way a selected local
does: simultaneously live claim bounds add with alignment, and claims
exclusive to mutually exclusive branches compose by maximum, matching the
storage contract's branch rule. Committed extents never widen demand — the
bound is the charge, whether it was a declared reservation or an attenuated
sub-extent of held provisioning.

At establishment the runtime extent must satisfy `committed <= bound`. The
claim surface checks it as an ordinary contract failure: rejection is the
checked admission outcome, never a trap and never an implicit clamp. The
extent's grant provenance names activation storage — it needs no provider
grant, receives no external provenance, and no provider seam accepts it back
as freed backing.

## Custody

The claim is linear and activation-scoped. Its backing is committed
activation storage, so the claim cannot outlive the activation: it cannot be
returned across the boundary, stored into survivor state, or transferred to
another activation, and every live claim releases before the frame unwinds.

Claims release in reverse establishment order; a claim cannot release while a
later-established sibling stays live. The ordering is the stack discipline
the realization replays against, not an allocator convention.

While live, the claimed region is nonmoving like every activation resident —
materialized addresses follow the frame's stable-address roster for the whole
claim life. A claim live at a suspension crossing joins the suspension plan's
live-claim roster with its exact storage role; parking retains the
activation's storage, so the region and its contents survive suspension
without duplication. A suspension-forbidden value held through the claim
rejects the crossing as usual.

Borrows over the claimed extent are ordinary loans. Splitting the claim obeys
extent conservation — children cover the parent exactly and merge requires
compatible lineage — but no child may outlive the parent's activation scope.

## Publication and replay

Each runtime-sized activation allocation is published per site with:

- its bound and its activation-storage provenance, never provider backing;
- the committed-extent computation the claim evaluates;
- the release ordering the plan requires; and
- the claim's live-claim rows at every suspension crossing where it is live.

The enclosing frame's committed extent, stack-probe plan, and unwind roster
cover every claim bound they carry, whichever realization the plan selects —
a bound committed with the frame at entry, or a lazily committed region whose
claim site records its own per-granule probe schedule. The published rows are
the complete record: replay recomputes each committed extent, requires it
within its bound and inside the activation's committed extent, and checks
site/plan bijection. A suppressed or invented extent replays false; a missing
or duplicate claim row rejects like a missing suspension row.
