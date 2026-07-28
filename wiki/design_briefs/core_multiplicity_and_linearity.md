# Design Brief: Core Multiplicity And Linearity

Settled 2026-07-18 at the semantic level (frozen decision 21).
This brief establishes the core usage discipline independently of dependent
types and content-bearing resource decomposition. Terminal consumption is
derived from ordinary by-value ownership and result flow; it has no separate
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

Automatic cleanup is for affine ownership. It may execute terminating,
infallible, non-suspending, nonblocking relinquishment, but it cannot silently
satisfy a linear protocol. A `Task<T>` is therefore linear: `finish` terminally
consumes its lifecycle claim, while moving it into another owner transfers the
obligation. `request_cancel` retains the claim because a request does not prove
that the activation stopped. Scope exit with a live `Task<T>` is a compile
error, not an implicit blocking finish or detach. Strict result use does not
prove this rule: it catches a discarded return, not a bound handle reaching
scope end.

The receiver's type graph retains this distinction directly: `&self` and
`&mut self` are reference types, while bare `self` is owned. Permission-event
discovery therefore treats method-form and static-form consuming calls alike
without guessing ownership from the method name.

Likewise, drop guarantees only that the program relinquishes ownership of an
affine handle. It does not promise that buffered bytes reached durable storage.
Fallible or suspending work is an explicit `flush`, `close`, `commit`,
`finish`, or cancellation/settlement machine with an ordinary result contract.
If that consumer may suspend or block, its call uses the ordinary `suspend` or
`block` acknowledgement; acknowledgement does not itself discharge the
obligation.

## Relationship to dependent types and resources

Core linearity requires no value-indexed types. The core covers whole-value moves,
linear parameters and returns, structural propagation, path reconciliation,
and explicit consumers. Transactions, acknowledgements, DMA submissions, and
task lifecycle claims are immediate clients.

Divisibility is not implied by linearity. Most linear values are indivisible:
file handles, acknowledgement tokens, and DMA completions have no composition
operator. A content-bearing qualified claim may separately project into a
compiler-owned partial composition algebra. The initial closed vocabulary is
`Indivisible | Interval<Scalar>`, with indivisible as the default.

Owned decomposition proves one n-ary theorem: the separated composition of all
consumed content equals all produced content plus any remainder retired through
an authorized route. Per-output containment and scalar measures cannot prove
this because individually plausible children may overlap. Split and merge are
the same equation in opposite dataflow directions.

This refines rather than duplicates the edge-cleanup witness. Edge cleanup
accounts for every incoming whole claim exactly once; a content-bearing
transformation additionally accounts for every symbolic unit inside the mapped
claims exactly once. Proof/debug artifacts report both levels in one nested
conservation witness.

Borrowed division remains cheaper. Layout fields, subrange loans, placed views,
and borrow-backed Arenas retain one owned root and therefore do not split its
content. Runtime-indexed owned extraction remains rejected until the frontier
and prover can name the unique moved element.

## Representation law

Multiplicity and establishment are semantic information and must survive
through checked control flow. The IR needs:

- a first-class multiplicity enum, with affine as the default;
- established/unestablished place state distinct from initialized bits;
- a permission context carrying multiplicity, access, and provenance;
- path-sensitive resource state for sums; and
- explicit create, transfer, consume, and affine-drop events.

Implementation status (CML4 migration, through 2026-07-28): these events now
survive the full semantic pipeline with multiplicity, access, and
transfer-stable provenance.
Existing shared/exclusive borrow loans enter the same permission context at
activation and leave it at weakening; their mature legality checks are not
reimplemented. The linear judgment reads this context exclusively. Affine
cleanup is discovered directly from typed state ownership; the legacy drop
summary is no longer producer input. Semantic transfers and consumes run the
same canonical typed move-discovery traversal through an independent event
sink, so the legacy move summary is likewise compatibility output only.
Both compatibility arenas now terminate at control flow. Abstract operations
and every later backend plan carry only the canonical permission ledger.
The backend ledger now also records one fail-closed realization per canonical
event: exact selected instruction indices, or a narrow checked no-code reason
for an explicit zero-code terminal consume, no-live-debt event, or trivial
affine discard. An empty selection site alone cannot prove a live establishment
or transfer. Provenance-preserving folds deliberately let one materialization realize
several transfers of the same obligation. Missing/foreign candidates or an
invalid no-code proof publish no partial ledger and surface as `UNLINKED` in the
backend report. Dispatch-edge and state-call argument materialization are joined
to target-state entry establishment, exact ordinals survive runtime/direct state
calls and statement-position host calls, and every current ownership pass canary
has a complete ledger. Named transition targets reserve their canonical ordinal
before nested argument calls, and target-symbol filtering separates their
permission events. A live linear obligation also remains intact across a
dispatched call's synthesized continuation and is consumed afterward; this is a
same-place/provenance carry, so the continuation itself does not add a semantic
permission event. Repeated same-symbol nested transition calls retain distinct
ordinals and join both materializations to their shared target-state event.
Normalized platform-entry parameter writes now realize program StateEntry
events directly; missing inbound code fails closed, and later consumes cannot
launder zero storage into establishment. Nontrivial state-exit cleanup lowers
through a checked per-edge plan: outgoing values materialize first, their
ownership mapping commits, and the remaining affine places clean in reverse
declaration order. The plan retains the exact conservation witness that every
incoming obligation transfers, is explicitly consumed, is automatically
cleaned, or receives a validated no-code affine discard exactly once. Nominal
whole-value cleanup forbids partial extraction; purely structural aggregates
clean only their remaining live field places. Composite field extraction uses
one path-indexed frontier: an explicit `[linear]` declaration contributes one
nominal root, while a transparent aggregate derives contained child claims
without adding another root. The first live slice covers statically named
transparent-record fields through local construction, whole-record transfer,
and extraction: moving one field leaves its sibling obligations live, and
duplicate moves reject. The permission ledger retains each field path and its
independently propagated source provenance through backend realization. Fresh
claim identity is distinct from provenance/root lineage and survives those
local transformations. Checked state results conserve multiple claims when
direct paths or record-constructor fields provide a unique structural output
map; checked bodies now publish that complete map for opaque n-ary callers to
consume. Input-relative entries bind through actual arguments, established
entries retain their exact identity and provenance, and fixed-point composition
covers expression calls, qualified tail transitions, and multi-hop wrappers.
Ambiguous or bodyless targets remain fail-closed. The structured maps are
retained in the checked `05_claim_outcomes.json` proof/debug artifact.
Carry policy is retained independently by transfer-stable claim identity:
every qualification-evidence origin starts strict, exact positive permissions
relax only that origin, and both multiple origins and multiple claims
intersect. The identity-preserving n-ary outcome maps therefore preserve each
child's policy without copying domain membership, and checked carry artifacts
publish the effective policy per claim. Literal-length fixed arrays now
enumerate canonical fixed-index paths through construction, literal-index
extraction, partial moves, and n-ary output maps; runtime-indexed owned
extraction remains fail-closed. Active sums likewise enumerate canonical
case-plus-field paths: known construction activates only the selected case,
same-case siblings remain independent, impossible alternatives stay inactive,
and checked output maps propagate live case paths through opaque calls.
Symbol-keyed substitutions already retain contained claims through nested
generic transparent records.

Consuming calls are classified from result flow: if a by-value `self` call
returns a type carrying the obligation, it transfers rather than terminally
consumes. One unambiguous moved input preserves its origin into the result;
claims transferred through a target state's direct path or record-constructor
result preserve their callee identities and origins when the caller binds the
matching paths. Opaque multi-resource calls without a normalized output map
reject. Content-bearing n-to-m transformations additionally discharge the
selected compiler-owned algebra's conservation theorem.

Generic conditional sums resolve payload multiplicity through symbol-keyed
type-argument substitution. Consequently `TaskOutcome<LinearT>::Returned` and
`StartOutcome<T, LinearArguments>::Rejected` carry live debt, while their
payload-free or affine cases do not; generic template parameters cannot launder
a concrete linear substitution.

A backend may erase multiplicity after it has received a checked ownership and
cleanup plan, but proof/debug artifacts must retain the conservation witness.
Compatibility move/drop summaries alone are not sufficient; see
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
10. A linear claim is indivisible unless its qualified claim kind selects a
    decomposable compiler-owned content algebra.
11. A split whose children overlap or duplicate content rejects even when each
    child is individually contained and their scalar measures add up.
12. Permission attenuation cannot be undone by merge; authority that must
    return is represented by a claim or loan.

## Deferred design spaces

- Additional content algebras beyond `Indivisible | Interval<Scalar>`.
  Correspondence-bearing virtual/physical decomposition requires a compact
  canonical symbolic mapping algebra with decidable containment, restriction,
  equality, and separated composition; independently conserving the two
  projections is unsound because it permits their association to be swapped.
- Quantitative operational resource accounting remains a separate algebra from
  content-bearing claim conservation.
- Dependent-linear buffer ergonomics after the core checker exists.
- The first conservative cross-suspension loan subset. Four-axis carry policy
  is settled independently; this item is the remaining borrow-rule and
  implementation work for values live at suspension points.

The first content-algebra customer is Extent split/merge: splitting
consumes one parent range authority and returns disjoint child authorities;
compatible-common-lineage merging consumes exactly composing children and
restores their parent content.
Arena-backed task-pool leases reuse the conservation discipline without
conflating allocation permission with range authority. General owned `LinBuf` splitting and
quantitative effect members come later.

The compiler foundation now has an executable `omega-extents` conservation
model: roots come only from one-shot admitted grants; split is exact; merge is
restricted to compatible children of the same split; attenuation only removes
normalized rights; and consuming failures return their authority inputs. This
does not replace the Omega `[linear]` checker work. Source integration replaces
the temporary sibling-only restriction with compatible common root lineage,
algebra-denominated receipts, one normalized claim-content projection, and the
generic n-ary conservation witness. Permission attenuation stays orthogonal:
merge never restores a discarded permission, and authority that must return is
a separate claim or loan.
