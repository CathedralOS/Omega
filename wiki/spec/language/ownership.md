# Ownership and multiplicity

Multiplicity controls use of established values, independently of representation,
borrowing, qualification, dependent facts, and carry. It is a type property,
not a trait or a qualifier repeated on each use.

| Multiplicity | Source property | Permitted use |
| --- | --- | --- |
| Unrestricted | `[copy]` | Copy and discard. |
| Affine | Default owned data | Move at most once; discard through eligible cleanup. |
| Linear | `[linear]` | Transfer or consume exactly once. |

`[copy]` and `[linear]` are mutually exclusive. Structural records, active sum
payloads, and concrete generic substitutions retain every contained obligation.
A nominal container's declared multiplicity cannot erase linear debt in an active
payload. An inactive case creates no obligation for its absent payload.
Copyability requires compatible owned contents and cannot duplicate unique
cleanup responsibility.

## Property declarations

Lowercase bracket properties attach to the data declaration or type parameter,
as in `data Box<T [copy]> [copy]`. They are distinct from value-range constraints
and generate no callable behavior. `sized` and structural carry are derived
judgments, not authored requests. Explicit `copy` and `linear` requests are
checked at the declaration; failure rejects rather than weakening the request.

Opaque boundary properties require accepted provider evidence. Ordinary packages
cannot append structural properties to foreign types or grant themselves opaque
claims. Apart from specified derived judgments, omitted properties are not
inferred, and there is no negative property syntax. Colon bounds such as
`T: copy` and detached attribute-prefix forms do not declare these properties.
The core property set beyond the specified multiplicity and
[carry](../resources/carry.md) rules remains open, not an extensible attribute
namespace. Binding-local `[erased]` has its separate
[relevance contract](../proofs/contracts.md#explicit-erased-bindings).

## Establishment and erasure

Distinguish initialized storage bits, an established semantic value, and a live
ownership obligation. Implicit zero-filling establishes no linear obligation.
Explicit construction of an all-zero linear value is legal and owed when that
value meets its type's validity conditions. There is no universal zero-valued
consumed inhabitant and no requirement that linear values exclude zero.
[Default-domain gating](dependent_values.md#default-domains-and-zero-initialization)
separately controls when storage may be observed as a value.

Conditional ownership uses an ordinary sum such as `Empty | Live(Task<T>)`.
The live case owns the task obligation; empty storage is not a fabricated
consumed task. Establishing one value creates one obligation. Moves, calls,
returns, receives, and storage operations transfer it rather than create copies.

Logical facts may be reused and weakened where logic permits. Consumable
permission is an affine or linear Type carrier, even if it has no runtime bytes.
Zero size does not require an `[erased]` marker and does not remove ownership.
Explicitly erased Type witnesses likewise retain identity, multiplicity,
validity, and conservation. Resource-sensitive mathematics may describe a linear
object logic without making the language's logical facts consumable.
See [proof erasure](../proofs/contracts.md#identity-availability-and-erasure).

## Paths and control flow

The permission context tracks exact established places, multiplicity, access,
loans, lifetime, and provenance. It is not a weakenable proposition catalog,
even when both analyses share one control-flow/place traversal.

Every normal path accounts for each live linear obligation by transfer or an
authorized terminal disposition. At an actual reconvergence, predecessor paths
must agree on the live ownership frontier. Independently terminating outcome
cases need not choose the same disposition: success may consume an input while
failure returns it, provided each case satisfies its exact published contract.
A path cannot consume an obligation and also return it or silently omit it.

[Process exit](process_exit.md#abandonment-and-survivors) explicitly ends its
bound domain with abandonment, including outstanding linear obligations. It is
not an ordinary consumer or proof of release. No post-exit frontier merges into
a returning branch, and no successful disposition receipt is fabricated.
Cross-domain survivor guarantees still require their specified evidence;
domain-ending authority cannot waive them.

Partial moves retain unselected siblings and make ancestors unavailable for
whole-value use or disposal. Transparent child paths do not duplicate a nominal
root obligation. They cannot evade a nominal cleanup hook's whole-value
entitlement. Duplicate and overlapping moves reject. An owned extraction
must identify the unique moved element and retain its residual frontier;
unsupported runtime-indexed extraction cannot fall back to forgetting the root.

Source and result claims keep their provenance through calls, state transitions,
wrappers, and generic substitutions. A moved input returned in an outcome
remains that same obligation. Ambiguous opaque multi-resource output maps reject;
equal bytes or matching result types are not transfer evidence. Terminal
[claim paths and residuals](../terminal-psi/ownership.md) specify independent
reconstruction and publication.

## Borrows and aliases

| Source access | Authority |
| --- | --- |
| `&T` | Shared observation under its loan contract. |
| `&mut T` | Exclusive observation and mutation. |
| `&write T` | Exclusive mutation without observation of existing content. |

Loans retain their own compatibility rules; they are not all exact-once linear
values. Shared loans may duplicate or reborrow, while exclusive loans must not
overlap incompatibly. A by-value reference transfers or copies its loan carrier,
not ownership of its referent. Borrowed arguments continue to denote the original
storage, not a caller-side snapshot chosen because the referent is small.

`&write T` requires an existing valid `T`, not vacant construction storage.
Explicit attenuation from mutable access cannot be reversed into readable access.
Exclusivity prevents reading old contents through a second alias. Metadata,
written inputs, and supplied proof facts may support a write; loading the referent
may not. Displaced custody and invariant-window obligations still apply.
[Structural access](../terminal-psi/structural_access.md) owns the full operation
and opaque-provider non-observation contract.

Reborrowing retains exact immediate parent lineage. Shared children freeze the
parent's mutation; exclusive children suspend the corresponding parent branch.
Restoration needs the complete child/cohort closure, not merely expiration of
a convenient alias name. Access attenuation does not change the parent's original
authority. [Loan compatibility](../terminal-psi/loans.md) owns captured places,
formation proofs, lifetimes, closure order, and restored-use evidence.

[Source lifetimes](lifetimes.md) specify explicit binders, result elision, and
structural carried-loan transport through values and state transitions.

## Consumers and cleanup

Ownership is determined by the receiver type: bare `self` is owned; `&self` and
`&mut self` are references. Method and static-call forms obey the same rule.
`move self` transfers into a consuming machine; there is no terminal-consumer
annotation and no inference from the method's name.

A call returning an outcome containing the obligation transfers it back. A
normal return without it requires its authorized disposition within the
callee. Pending or failure outcomes of incomplete consuming operations retain
the live input rather than discard it. A borrowed-receiver call cannot silently
become an owned consumer.

Affine ownership permits eligible automatic cleanup. Linear ownership permits
it only when the type owner declares that exact plan as a valid terminal
disposition. The hook must return normally and be infallible, nonblocking,
non-suspending, free of abnormal outcomes, and require no runtime authority
beyond its receiver. Termination alone is insufficient: the transitive hook
contract must exclude process exit as well as crashes. The compiler begins
consumption before temporarily lending
`&mut self` to the hook; this is not consumption by an ordinary borrowed call.
[Nominal cleanup](../terminal-psi/ownership.md#nominal-cleanup) owns the reserved
`T::drop` edge and ordinary consuming early-disposal call.

Only the data declaration's owning package may declare its exact attached
`T::drop`, and at most one exists. The hook receives and must return a whole
valid value before structural field cleanup. A nominal-drop type cannot be
partially moved; meaningful decomposition requires an explicit consuming
machine. An unrelated ordinary machine named `drop` has no reserved meaning.

Every owned death edge derives an internal contextual cleanup row: Type-side
disposition eligibility, proposition prerequisites, operational effects/reach/
work, and derived guarantees remain separate. Prerequisites must be proved
locally or already authored in `requires`; neither public nor private body
analysis may invent a new caller demand. Generic rows remain symbolic until
substitution supplies the exact structural plans and contracts. Containers need
no nominal `Disposable` bound, and an instantiation lacking legal element
disposition rejects. Diagnostics identify the authored hook clause and edge,
not an internal synthesized predicate.

Cleanup promises only its owner's disposition, not durable output or protocol
completion. Fallible/coordinating work uses explicit `flush`, `close`, `commit`,
`finish`, or cancellation/settlement operations and their ordinary result
contracts. Source `suspend`/`block` acknowledgements apply when needed but do not
discharge custody. A task without automatic disposition must be finished or
transferred; `request_cancel` retains its claim. Scope exit cannot implicitly
wait, detach, or lose a bound task. Strict use of returned results alone does
not enforce that lifecycle.

Automatic abandonment requires a contract declaring implicit disposal harmless.
A claim whose loss permanently withholds external capacity remains linear with
an explicit terminal choice: release, or an authorized abandonment recording
the loss. Deployment policy may forbid abandonment independently of memory
safety. Enqueuing deferred reclamation transfers the obligation rather than
discharging it; the queue needs capacity, servicing, progress and resource
contracts covering eventual discharge.

## Construction and disposal order

Independent roots clean up in reverse declaration order, so later locals precede
earlier by-value parameters. The nominal whole-value hook precedes structural
field cleanup; only the active sum payload is present. Borrow and ownership
dependencies must fit this order: an owner cannot die before cleanup depending
on its borrow. A different release protocol requires an explicit owner or
consumer, not reconstruction of dynamic acquisition history.

Named record/case literals establish fields once in authored expression order.
Abandoning partial construction disposes the established prefix in reverse
establishment order; partial call-argument staging follows the same rule.
After successful construction, aggregate cleanup instead follows recursive
reverse declaration order. Canonical field identity and physical layout do not
reorder evaluation or partial-construction cleanup.

Literal fixed arrays establish elements in increasing index order and clean
the statically known live residual set in decreasing order, recursively and
excluding moved elements. Authored moves retain authored order. No runtime
liveness bitmap or data-dependent cleanup loop replaces this static schedule.
Each normal edge retains its own exact transfers and cleanup; a crash has no
cleanup successor. [Calls and outcomes](../terminal-psi/calls_and_outcomes.md)
separately governs successful completion and budget suspension without replaying
an already committed transfer.

## Content and carry

Whole-claim conservation does not require dependent types or imply divisibility.
Only an exact qualification's owner-unique `Content<A>` projection adds
[content conservation](../resources/content_custody.md). Its n-ary theorem is
additional to accounting for each whole claim; per-child containment or equal
scalar totals cannot rule out overlapping children. Retired content means
leaving checked custody, not necessarily destruction or reclamation.

Borrowed subranges and placed views may retain one owned root without splitting
its content. Independently owned children require conservation. Merging cannot
restore discarded permissions; authority that must return is a separate loan
or claim. A counted pool of `n` units is not a protocol requiring delivery to
`n` distinct destinations.

[Carry](../resources/carry.md) independently checks suspension, CPU, thread, and
address transitions. Copyability is not concurrent shareability. Erasure or
qualification forgetting cannot drop carry obligations or grant migration.
Multiplicity may leave executable representation only after complete ownership
and cleanup checking; artifacts retain the required conservation evidence.
Every published permission event needs exact realization or a checked no-code
reason. Missing or invalid realization cannot publish a partial permission ledger.
