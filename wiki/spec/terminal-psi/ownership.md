# Structural claims and cleanup

Structural ownership is an exact live set of roots, projected claims, and
remaining subtrees, not a Boolean property of an aggregate. Borrowed access is
governed separately by [loans](loans.md) and [structural access](structural_access.md).

## Claim paths

An entry claim names a whole structural parameter or a canonical typed path.
Record segments use the exact field identity: an authored numeric ID where
present, otherwise the unnumbered field's spelling. Fixed-array segments use
canonical zero-based literal indexes within a nonempty literal-length shape.
Resolve every segment against the type reached by the previous segment.

The admitted projected-claim vocabulary traverses relevant structural fields,
fixed indexes, and exact declared case payloads. A Case segment guards its
payload namespace; it adds neither physical storage nor an independent claim.
Typed catalogues may include mutually exclusive payloads, but live custody
contains only the paths selected by actual case membership. Missing membership
is an error, not evidence that a payload is inactive. Dynamic indexes,
scalar/erased leaves, unknown segments, duplicate paths, ancestor/descendant
overlap within the same active payload, and noncanonical order reject.
A projected claim is linear even in an affine aggregate. Several disjoint
linear sibling claims may coexist in one affine root.

Direct calls agree on the complete active claim-path set for each structural
argument, joined by exact typed relative path rather than catalogue position.
Content bindings name the same root/path. A structural result binds
its claims by path: each caller result binding continues the claim the callee's
returning edge holds at the same path beneath the returned value, so every
return is a bijection between the claims it hands back and the caller's result
bindings. Claim identities stay machine-local. Result custody is established
only on successful completion; it cannot also remain on the input owner.

An owned block parameter may declare a destination-local claim-path roster in
the machine's `block_entry_claims`. An edge captures all outgoing values and
claims, removes their source custody, performs its ordered residual cleanup,
then binds the complete active path set to the destination names. A binding
continues the obligation that arrived; it is not a claim establishment. Two
arms may supply different original identities, and simultaneous swaps or
backedges may reuse names removed by that edge. A destination name cannot
overwrite an obligation that survives the transfer and cleanup.

Case guards select binding rows from authenticated membership, never from a
missing incoming identity. Missing or extra active paths, duplicate bindings,
multiplicity drift and surviving-name collisions reject. After binding,
arrivals still agree on the exact live ownership frontier. A parameter with
no binding rows relocates the incoming identities unchanged; this is the
ordinary identity-preserving transfer used by expression continuations.

A claim declaration's path describes its establishment, not its permanent
location. Construction, replacement stores and state transfers can rebase the
same identity beneath a different owned home. Foundation validation authenticates
the declared identity and typed operands; live ownership checks authenticate
the current home and the exact path relative to the consuming argument. The
original declaration is not rewritten when custody moves.

Normal boundary completion carries the verifier-derived receipt set for every live
claim at each exact argument position, not one receipt per parameter. Missing,
duplicate, reordered, or path-mismatched rows reject. Commit consumption only
after validating successful completion and its result. Rejection leaves the
interpreter's ownership records unchanged; it cannot undo external effects a
host already performed. Suspension must not repeat a committed transfer.

## Partial ownership and residuals

Moving a projection makes its ancestor unavailable for whole-value calls,
returns, or disposal. Duplicate and prefix-overlapping moves reject regardless
of the root's multiplicity. Moving every member of a dense linear sibling claim
set settles that set; it does not license rebuilding an arbitrary value with a
hole.

For an admitted finite structural affine root, derive its residual
complement from the type and actual moves. Each row binds root identity, canonical
path, and exact subtree type. List maximal live subtrees in recursive reverse
declaration/index order. An untouched array is one residual subtree, not an
expanded list of leaves. A partially moved ancestor cannot be discarded whole.
An empty complement needs no residual row.

A `Case` path enters the payload namespace of the exact selected sum; it is
not another owned child. Its active payload fields and any mixed common fields
participate in the same complement. Inactive cases contribute no values or
cleanup. Each selected case must remain established at the transfer, and
conflicting case identities cannot be combined into one residual partition.
Plain scalar contents do not authorize copying an affine payload: an owned
projection transfers its original custody, while a leaf-copy observation still
requires unrestricted data or the separately admitted shared-descriptor loan.

Jump, conditional, and case successors carry this same residual schedule within
their ordered cleanup actions. Only the selected edge disposes its listed
subtrees, after capturing outgoing arguments and before establishing destination
bindings. Whole-root, residual and nominal actions follow the same reverse
establishment order; consumers must not regroup them by action kind. A transferred
subtree retains its descriptors even when a later edge disposes its former
owner's complement.

Structural fields carry their required cleanup; primitive and bounded-owned-byte
fields keep their no-cleanup classification. Numeric range or arithmetic policy
does not turn a primitive into an owned structural carrier, and a reference to
that primitive does not inherit owned disposal.

Source establishes fixed-array elements in increasing index order. Ordinary
disposal names the remaining established elements in decreasing order, excluding
moved elements and applying the rule recursively. Partial construction uses
only its established prefix. Producer-supplied order, runtime liveness flags,
or data-dependent cleanup loops cannot replace this static schedule.

Verification reconstructs the complement, rejecting missing/extra rows and
path/type/order drift. Check the required output size before enumerating array
children: a forged huge dimension with a short residual list must not force an
unbounded scan. No-code disposal emits no target instruction or runtime bitmap.

## Borrowed-storage restoration

The [source ownership window](../language/ownership.md#borrowed-storage-invariant-windows)
requires an exact relationship between an exclusive loan, its captured place,
the projected move, and the later establishing store. Checking
reconstructs that relationship from operations, loan authority, and control flow.
A producer assertion that the owner is restored is not evidence.

The move transfers custody once and leaves restoration debt at the captured
place. A checked store of a valid replacement clears that debt; matching a field
name, carrier type, or bytes at another place is insufficient. Contained loans
travel with their values, and filling vacant storage must not dispose the moved
value again. Residual siblings and the removed value retain their own obligations.

Opening a window first rejects overlap with any live reference's carrier slot
or possible referent origin, including references stored in other owned carriers.
Only the exact exclusive parent permission authenticated for this move is exempt;
its descendants must already have ended. Every candidate origin pins its region,
and an unresolved projection conservatively pins its whole spelled root. Opaque
ingress identities bound interference without identifying a runtime pointer or
granting permission to repair another carrier's debt.

Whole-value replacement of intact freely discardable contents is distinct from
restoring a vacancy. Its operation retains that disposition and the exact
incoming value's unrestricted custody, with no contained claims or loan
resources to discard. It requires write authority, exact receiving validity,
and no overlapping open window or live descendant loan. It does not extract
or observe the old contents. A missing extraction alone proves neither vacancy
nor permission to discard. The whole replacement installs tag and payload
together and preserves neighboring storage.

Whole-value assignment displacing an obligation through an exclusive loan (`referent = value` where
`referent: &mut T`) is the same window rooted at the loan itself: the move names
no field and an empty path, vacating the whole referent, and the store reseats
it. Only a mutable-borrowed machine or block parameter opens a root window; an
owned home's whole value moves by ordinary transfer, and shared or write-only
loans open no window.
Retained exclusive descendants may spell the access through their live
carriers while restoration debt stays on the captured parameter storage.
Owning a carrier alone supplies no whole-root authority over unrelated storage.
The displaced value carries the referent's exact type and
its own declared multiplicity, exactly as a field window's: displaced affine
contents owe ordinary disposal on a following edge, while plain contents need
none. A borrowed parameter's multiplicity describes its loan carrier, not the
referent, so it is no evidence for either value. Every observation of the root
overlaps the vacated referent until the repair, and the repair invalidates every
fact observed beneath the root. A plain sum loan, like a record loan, may join a
block parameter as its referent's address, so a later state can replace it.

An element of an exclusive element view (`&mut [T]`, a machine or block
parameter) is a place reached through that view's loan. The view owns none of
its elements and has no static extent, so the window keeps the view as its root
and selects the element with a `RuntimeIndex`, a literal selector included,
owed against the view's dominating `ElementViewLength`. The hole lies in the
viewed collection; the displaced value carries the element type's own
multiplicity, so an affine element is disposed before an occupied store and a
moved-out element is repaired without disposal. A whole-collection view of
visible owned or borrowed storage instead names that storage's own path.
`omega run --both tests/omega/pass/ownership/runtime_view_parameter_element_window_exit/main.omg`
replaces copy and affine elements through view parameters, a forwarded state
parameter and a subslice, and exits 0 in both engines on macOS ARM64.

Replay checks reads, calls, joins, loan restoration, and exits against the exact
frontier. Reject missing repair on a returning path, duplicate extraction,
overlapping observation, wrong-place repair, whole-owner cleanup while incomplete,
and loss of restoration debt at a join. Suspension retains the window and its
exclusive custody under the carry contract; it cannot publish a complete owner
or repeat the move on resume. Crash and process-exit abandonment retain their
separate outcome rules. Lowering preserves these relationships without replacing
the caller's storage by a staged copy.

Normal exclusive-call completion returns custody of the checked caller region,
not the identity of its displaced value. The completion names the exact call,
owned home and root-relative region, together with the possible caller-local
payload claim paths. Stored case guards select the live subset after the callee
has restored the referent. Callee-local extraction claims do not become caller
claims by matching bytes or renaming an old case's payload. Refresh only this
region's claim roster and affine complement; a disjoint moved sibling remains
moved. A crash, suspended call or abandoning boundary publishes no normal-return
completion. Host boundaries join only after their authorized writeback.

## Roots, continuations, and order

Parameter, named-local, and call-result roots retain distinct establishment
identities. A result ordinal is not a parameter index. An anonymous projected
argument retains its real producer expression rather than a fabricated local;
the producer completes before the consumer even if captured call ordinals are
preorder.

A dying temporary's normal continuation owns its remaining cleanup. A final
consumer may use the return edge; a continuing body uses an explicit continuation
edge. Even an empty complement retains source continuation correspondence.
Partial moves do not retire the whole root early. Other live roots remain live.

Region disposal and home retirement are distinct. A replacement disposes the old
field value but retains the containing home for the establishing store. The
checked dying-owner partition publishes `RetireHome` after its complete ordered
region actions, including an empty complement; a last projected action alone
does not identify a dying home. Whole-root discard and a single whole nominal
leaf action consume their home intrinsically and need no second retirement.
An affine region action may select the whole value with an empty path while
retaining its binding for explicit retirement. This is distinct from
`DiscardRoot`, which disposes and retires together; case selection does not
change either action's claim or loan obligations.
Retirement grants no claim discharge or loan restoration: linear claims must
already be settled, and captured outgoing backing survives the frame binding.
The reservation may include fields that never became values; only the checked
partition identifies their absent obligations. A later store rejoins its exact
region to an owned home's frontier, not to a borrowed referent's callee frame.

Each jump or selected conditional successor has its own exact cleanup/transfer
partition. The transaction validates the current frontier, charges the edge,
materializes scalar successor arguments, disposes the named remainder, then
binds successor parameters. Fuel exhaustion leaves disposal uncommitted.
Scalar arity/types/definitions and ownership are independently checked.
Reconverging paths must establish the same ordered structural frontier; their
scalar values need not be identical.

This is an ownership-obligation frontier, not a catalog of every evaluated value.
Plain unrestricted primitive-array, scalar-case, and plain record payloads have no disposal
obligation. As with primitive local storage, their uses require an independently
established producer that dominates the use, including operation order within a
block. A branch-local payload cannot be used after a join reached without its
construction, but an unused payload does not prevent that join. This does not
erase claims, loans, partial moves, or affine/linear obligations.

Audit origin may remain path-sensitive. Operational metadata joins only when
all alternatives use the same realization or an existing runtime discriminator
preserves the distinction, such as a sum tag, provider key, or explicit state.
Otherwise the author must represent the alternative or normalize custody.
The checker cannot duplicate a semantic state to repair an invalid join.
A call result whose claim lineage depends on the callee's exit is such a join:
each alternative holds a claim of the same realization at every result path,
so Terminal binds the result by path and the checked join receipts keep the
per-exit origin.

On an ordinary continuing or returning edge, every incoming owned obligation
occurs exactly once in the edge's transfer map,
explicit terminal consumption, eligible automatic cleanup, or validated no-code
affine discard. A dying linear obligation requires an owner-authorized terminal
disposition. The target frontier checks shape, facts, multiplicity, access, and
operational metadata; predecessor origins need not be identical. The complete
partition and conservation evidence remain in proof and diagnostic artifacts.

Every explicit parameter participates in this accounting. A machine's qualified
name cannot remove a place or its claims from the frontier. An owned parameter
requires evidenced disposition; a return alone supplies none.
Affine cleanup/discard retains its ordinary charge and
ordering, and linear cleanup still requires the owner's authorized plan.

Calls retain authored order, independently of reverse cleanup order. Whole and
residual cleanup must share a valid establishment-order schedule before a
consumer may support their combination. Transformations unable to preserve an
edge's residuals reject rather than silently dropping them. A crash retains its
abandoned frontier and has no cleanup successor; see [calls and outcomes](calls_and_outcomes.md).

Canonical [process exit](../language/process_exit.md#abandonment-and-survivors)
is a distinct non-crashing terminal abandonment. It creates no return value,
consumption receipt, cleanup successor, or post-exit frontier. A reconstructed
local frontier is only a lower bound, not a complete domain-wide resource census
or evidence of safe survivors. Conditional exit does not alter the partition
required on the returning branch.

## Nominal cleanup

The static `T::drop(value: &mut T)` target is a compiler-owned edge dependency,
not a source-selectable call or static-machine value. Authored early disposal
uses the ordinary consuming `omega::core::drop(value)` call. The cleanup plan
keeps Type-side discharge eligibility, logical prerequisites, operational
reach/effects/work, and derived guarantees separate.

The target must return normally. Termination and exclusion of crashes alone
are insufficient if its transitive contract permits external process exit.

Each action names an exact place, type, and target. Reusing a target for different
places remains multiple actions and multiple work charges. Empty cleanup emits
no call; executable cleanup belongs to its edge/action ordinal and runs before
teardown. The [normal cleanup contract](calls_and_outcomes.md#normal-cleanup)
owns sequential execution and suspension.

A parent invocation retains its intact subject while its checked descendant
disposal partition remains pending. That retention is explicit action data,
not inferred from the next action on the same edge: selected child cleanup may
cross ordinary control blocks. A leaf invocation consumes its exact region.
Neither retention nor case selection settles child claims or retires a home;
the complete disposal partition must finish before explicit home retirement.

Cleanup prerequisites must already be proved locally or authored in `requires`;
analysis cannot add a new caller demand. When the hook's value parameter is
proof-only after checked erasure, the plan still substitutes the exact owned
place for that parameter in each target clause and binds each action's
proof independently, not by assuming identical positions in two fact lists.
Only that erased form has a zero-argument operational target; erasure does not
remove its source parameter, subject substitution or disposition obligations.
Proof-site identities erase at
the verified Psi-to-Omega boundary and cannot reappear downstream.

An owning erased descriptor must retain hidden payload/storage custody, size,
alignment, movement, and the concrete type's eligible cleanup plan. Reconstruct
that relationship under retained facts. Borrowed erased views have no
referent-cleanup disposition. Descriptor lifecycle metadata is not a user-defined
Drop conformance.

Erasure into an automatically cleaned owner requires eligibility under that
owner's retained invariant, or retained stable facts and authority establishing
the concrete plan's prerequisites. Otherwise a linear payload needs an explicit
consuming owner. Erasure neither supplies missing authority nor changes the
disposition. A collection's structural plan invokes each owned erased element's
descriptor plan; borrowed views do not dispose their referents.

## Physical realization

Storage planning consumes the checked transfer map rather than reconstructing
ownership from lexical scopes. Source and target storage may coalesce, but a
borrow promising stable address requires a realization preserving that address.
Cleanup suffix sharing and physical block cloning must retain every action's
exact semantic edge and order. No-code actions still map to the permission ledger.
Coalescing unchanged loop-carried large values is a performance acceptance
requirement, never permission to accept an invalid transfer. An address-stability promise is instead
a soundness obligation, whether realized by coalescing or another valid plan.

Current producer and ABI limits are documented beside
[Terminal production](../../../omega-rust/omega/compilation/driver/terminal-production/README.md#partial-ownership-and-cleanup)
and [native lowering](../../../omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md#structural-results-and-residual-cleanup).
Admitting a Terminal claim or result does not establish native storage or cleanup
support.
