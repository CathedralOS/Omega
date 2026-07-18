# Design Brief: Core Multiplicity And Linearity

Settled 2026-07-18 at the semantic level (frozen decision 21).
This brief establishes the core usage discipline independently of dependent
types and the later general resource algebra. Terminal consumption is derived
from ordinary by-value ownership and result flow; it has no separate
annotation to settle.

## Multiplicity is a type property

Omega has three usage multiplicities:

- **Unrestricted** values may be copied and discarded. `[copy]` establishes
  this property.
- **Affine** values may be moved at most once and may be discarded. This is
  the default for owned data.
- **Linear** values must be transferred or consumed exactly once. The source
  spelling is `[linear]` on data/type declarations; it is a checked type
  property, not a trait and not a `lin` qualifier attached to every use.

Multiplicity is orthogonal to borrowing, provenance, representation, and
dependent facts. A value can be linear and fixed-size, affine and
value-indexed, or unrestricted and borrowed. `LinBuf<T, n>` combines two
features; it does not define either one.

`[linear]` and `[copy]` are mutually exclusive. Structural propagation is
mandatory: a record or live sum payload containing a linear value carries its
obligation; a generic container must declare and preserve the multiplicity of
its parameter rather than silently weakening it.

## Establishment and conservation

Establishing a new linear value creates exactly one obligation. Moves, calls,
returns, receives, and storage operations transfer that obligation; terminal
consumers discharge it. Implicit zero-filling does none of these.

Linearity constrains usage, not bit patterns. Keep three states distinct:

1. storage whose bits have been initialized;
2. a semantic value that has been established in that storage; and
3. a live linear obligation owned by a place.

Raw or implicitly zero-filled storage is not automatically an established
linear value. A particular linear type may gate establishment through its own
validity/default-domain invariant. Conversely, an explicitly constructed
all-zero linear value is legal and owed if all-zero is valid for that type.
There is no universal "zero means consumed" inhabitant and linearity does not
imply a zero-excluding predicate.

Conditional resource state is represented honestly as a sum:

```omega
data TaskState<T> {
    case Empty;
    case Live(task: Task<T>);
}
```

The obligation is path-sensitive and belongs to the `Live` payload. The zero
case is ordinary `Empty`; it is not a forged consumed `Task<T>`.

## Facts and permissions use different algebras

Flow analysis may share one CFG traversal and canonical place identity, but it
must not put propositions and resource obligations in one weakenable catalog.

- The proposition context (Gamma) contains facts. Facts can be duplicated,
  weakened, intersected, and forgotten where logic permits.
- The permission/resource context (Rho) tracks established values and loans.
  Entries carry at least multiplicity (`unrestricted`, `affine`, `linear`),
  access (`owned`, `shared`, `exclusive`), and lifetime/provenance.

The exact algebra depends on the entry. Shared loans may duplicate or
reborrow; exclusive loans must not overlap; affine ownership may die through
drop; linear ownership must reconcile exactly. At a control-flow join, facts
join by logical rules while resources join by ownership identity and
path-sensitive conservation. A branch may consume a linear value on both
arms, or transfer it on both arms, but may not consume it on one and silently
lose it on the other.

## Consumers and cleanup

A linear value must reach an explicit terminal consumer on every exit path.
`move self` is the ordinary transfer into a consuming machine; no terminal
annotation is added. Consumption is derived from ordinary ownership flow: a
returned outcome that contains the same obligation transfers it back, while an
outcome that does not contain it requires the callee to have consumed it.
Cancellation and failure paths obey the same conservation law. A `try_*`
operation that has not completed must therefore return the live linear value
in its pending/failure case.

Automatic cleanup is for affine ownership. It may execute infallible,
non-suspending relinquishment, but it cannot silently satisfy a linear
protocol. A `Task<T>` is therefore linear: `finish` terminally consumes its
lifecycle claim, while moving it into another owner transfers the obligation.
`request_cancel` retains the claim because a request does not prove that the
activation stopped. Scope exit with a live `Task<T>` is a compile error, not an
implicit blocking finish or detach. Strict result use does not prove this rule:
it catches a discarded return, not a bound handle reaching scope end.

Likewise, drop guarantees only that the program relinquishes ownership of an
affine handle. It does not promise that buffered bytes reached durable storage.
Fallible or suspending work is an explicit `flush`, `close`, `commit`,
`finish`, or cancellation/settlement machine with an ordinary result contract.

## Relationship to dependent types and resources

Core linearity requires no value-indexed types. V1 covers whole-value moves,
linear parameters and returns, structural propagation, path reconciliation,
and explicit consumers. Transactions, acknowledgements, DMA submissions, and
task lifecycle claims are immediate clients.

Dependent buffers are a later stress test. A borrowed split needs dependent
bounds and disjointness; a general *owned* split also needs the resource
algebra to prove conservation of the original ownership token across the two
results. Until that algebra lands, dependent-linear buffers may support whole
ownership and borrowed views without claiming general owned splitting.

## Representation law

Multiplicity and establishment are semantic information and must survive
through checked control flow. The IR needs:

- a first-class multiplicity enum, with affine as the default;
- established/unestablished place state distinct from initialized bits;
- a permission context carrying multiplicity, access, and provenance;
- path-sensitive resource state for sums; and
- explicit create, transfer, consume, and affine-drop events.

Implementation status (CML3, 2026-07-17): these events now survive the full
semantic pipeline with multiplicity, access, and transfer-stable provenance.
Existing shared/exclusive borrow loans enter the same permission context at
activation and leave it at weakening; their mature legality checks are not
reimplemented. The linear judgment reads this context exclusively. Legacy
move/drop summaries remain only as transitional producer input until ownership
discovery emits the semantic events directly.

Consuming calls are classified from result flow: if a by-value `self` call
returns a type carrying the obligation, it transfers rather than terminally
consumes. One unambiguous moved input preserves its origin into the result;
ambiguous multi-resource results remain conservative until the general
resource algebra can state their mapping.

A backend may erase multiplicity after it has received a checked ownership and
cleanup plan, but proof/debug artifacts must retain the conservation witness.
The current move/drop-only summaries are not sufficient; see
`architecture/semantic_taxonomy_representation.md`.

## Acceptance register

1. Establishing one linear value and moving it through several bindings still
   produces exactly one live obligation.
2. Implicit zero-fill creates no obligation; explicit construction of a valid
   all-zero linear value creates one.
3. A live linear value at ordinary scope exit is rejected.
4. Every branch either transfers or consumes the same linear obligation; mixed
   treatment is rejected.
5. `Empty | Live(Task<T>)` tracks the obligation only in the live case.
6. A record, sum payload, or generic container cannot erase a contained linear
   obligation.
7. `Task<T>` must be explicitly finished or transferred; requesting
   cancellation alone does not discharge it, and automatic cleanup never
   blocks.
8. Shared and exclusive loans keep their own permission rules rather than
   being forced into exact linear reconciliation.
9. Linearity works for a fixed-size acknowledgement token without any
   dependent-type feature enabled.

## Deferred design spaces

- The general resource algebra for owned splitting, merging, attenuation, and
  quantitative resources.
- Dependent-linear buffer ergonomics after the core checker exists.
- Interaction with suspension-safe loans, which depends on the effects and
  suspension settlement.

The first general resource-algebra customer is Region split/merge: splitting
consumes one parent authority and returns disjoint child authorities; merging
consumes the matching children and restores the parent. Region-backed task
pool leases reuse that algebra. General owned `LinBuf` splitting and
quantitative effect members come later.
