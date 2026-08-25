# Chapter 17: Drops And Cleanup

Owned values need a deterministic cleanup story. Omega represents that story
on graph edges, where ownership actually changes, rather than at lexical braces
or in backend-invented drop flags.

The core rules are:

- Plain values with no cleanup simply stop being live.
- Affine values may have automatic cleanup.
- Linear values require an explicit terminal consumer and reject if they would
  die on an edge.
- Moving a value transfers its obligation.
- Every ordinary outgoing edge carries a checked cleanup plan.
- Nuclear abort is not an ownership-graph edge and performs no cleanup.
- Cleanup remains visible in semantic, proof, debug, and resource artifacts.

## Cleanup Machines

Cleanup uses an ordinary reserved machine shape:

```omega
data MutexGuard<T> {
    mutex: &Mutex<T>;
}

machine MutexGuard::drop(&mut self)
    ensures self.mutex unlocked
{
    self.mutex.unlock_raw();
}
```

`drop` receives one whole valid value. It may call ordinary machines and carry
declared service reach, but automatic cleanup is always:

- terminating;
- infallible;
- non-suspending;
- nonblocking; and
- free of abort, trap, or another abnormal outcome.

The implemented terminal subset is deliberately narrower. A root-only Unit
return may invoke attached `drop` for a finite nonempty list of whole
claim-free, unqualified affine records that are empty or contain only relevant
Boolean or integer fields. Multiple cleanups run in reverse parameter
declaration order and may share one cleanup machine because the actions own
different places. Each body may
contain a finite source-ordered list of ordinary zero-argument calls to mutually
distinct exact-empty attached helpers; different bodies may share helpers. Psi
preserves each whole receiver and executes
the complete ordered list. Omega represents all return cleanup as one ordered
action stream, assigns nonempty receivers their ordinary ABI homes, and emits
only executable cleanup calls while retaining exact edge/action and helper
operation custody. Wider body shapes and nested or erased receivers remain pending engineering work under
the rules below.

A release that waits, suspends, may fail, or promises protocol completion is an
explicit consuming machine such as `close`, `flush`, `commit`, `finish`, or
`cancel`. A resource with no valid nonblocking terminal outcome must be linear.
An affine resource may instead have an authorized nonblocking fallback, such as
abandonment or transfer to a stable custodian.

Sound abandonment does not make silent abandonment appropriate. When forgetting
a claim permanently withholds external capacity, the claim remains linear and
the terminal choice is explicit: a potentially failing or blocking `release`,
or an infallible `abandon` that records the loss. Only resources whose contract
declares implicit disposal harmless may use affine scope exit for abandonment.
Deployment profiles may reject explicit or implicit abandonment independently
of memory safety.

Such an explicit consumer uses the ordinary call acknowledgement required by
its contract:

```omega
let closed: CloseResult = block file.close();
let outcome: TaskOutcome<T> = suspend task.finish();
```

The marker makes the waiting site visible; explicit consumption, not the
marker, discharges the linear obligation.

Enqueuing deferred reclamation transfers the obligation to the queue; it does
not discharge it. The queue must publish capacity, servicing, progress, and
resource bounds that cover eventual discharge. Without those contracts it is a
declared leak with extra data structure.

## Edge Cleanup

Cleanup is an edge property, not a node property. Every ordinary edge follows
one semantic sequence:

```text
select the edge
    ↓
evaluate and materialize successor arguments or the return value
    ↓
commit the ownership transfer map
    ↓
clean the remaining dying affine places
    ↓
verify the resulting frontier
    ↓
hand control to the successor or caller
```

This sequence applies to:

- explicit state transitions;
- terminal returns;
- ordinary success and failure edges;
- natural state completion; and
- compiler-synthesized call continuations.

Argument evaluation may itself contain ordinary graph edges. Each such edge
uses the frontier that exists after the evaluations preceding it. Nuclear abort
has no successor in the ownership graph and therefore has no cleanup plan.

For an edge `e`:

```text
Fₑ = permission frontier after outgoing values are materialized
Mₑ = source-place → target-place ownership mapping

transferred(e) = domain(Mₑ)
dying(e)       = owned(Fₑ) \ domain(Mₑ)

image(Mₑ) must fit EntryFrontier(target)
```

The target frontier describes its required shape, facts, multiplicity, access,
and operational metadata. Different predecessors may supply different
historical origins; they do not have to supply the same source-place identity.
A required target place with no mapped live source is an error. Extra affine
places are cleaned on their predecessor edges.

Example:

```text
        S1 {x, y, r}                S2 {x, r}
             │                          │
             │ e1: clean y              │ e2: no cleanup
             └──────────┬───────────────┘
                        ▼
                   S3 {x, r}
```

No runtime flag is needed. The two predecessor edges already identify the
different ownership situations.

## Joins And Runtime Discrimination

Audit-only origin may remain path-sensitive. Operational metadata may join only
when every alternative uses the same realization, or when the distinction is
already carried by ordinary runtime representation or control state.

Examples of valid existing discriminators include:

- a sum tag;
- an era-bearing handle;
- an admitted provider key; or
- an author-visible state distinction.

When cleanup or custody transfer differs and no discriminator exists, the
author must normalize to a common custodian or represent the alternative
explicitly. A sum contains the same runtime bit that a hidden drop flag would
have contained, but makes it part of the type, layout, and report.

The checker never duplicates a named semantic state to make an invalid join
typecheck. After checking, code generation may clone or share physical blocks
when every realization maps back to the same typed edge plan. Physical layout
cannot create a different frontier meaning.

## State Parameters Remain Explicit

Machine parameters are roots owned by the activation, but that ownership does
not make them ambient names in every internal state. A state receives the
values, borrows, and authority named by its parameters, and transitions spell
the corresponding mappings.

The storage planner may coalesce an unchanged source and target place, so an
explicit semantic transfer need not copy bytes or execute code. The permission
ledger still records the transfer. This preserves state signatures as
source-visible proof and authority frontiers without imposing a runtime tax.

## Cleanup Order

Cleanup order is deterministic:

1. Independent roots are cleaned in reverse declaration order.
2. Locals therefore precede by-value machine or state parameters declared
   before them.
3. A whole value's nominal `drop(&mut self)` runs before structural field
   cleanup.
4. Remaining fields are cleaned in reverse declaration order.
5. A sum cleans only its active payload.

The checker validates that cleanup-bearing borrow and ownership dependencies
agree with this order. A borrowed owner cannot die before a dependent cleanup
action.

Reverse declaration order is stable at joins. Dynamic acquisition history is
not. APIs needing a different release protocol express it through an explicit
owner or consuming machine rather than asking cleanup to reconstruct history.

## Partial Values

The frontier ranges over canonical places, not just bindings:

```text
resources before move:
  .file   → live
  .socket → live

move resources.file

resources after move:
  .file   → moved
  .socket → live
```

The first implemented frontier slice covers statically named fields of
transparent records. A record that is not itself declared `[linear]` derives
its contained linear field claims without adding an aggregate claim; local
construction, whole-record transfer, and field extraction retain those paths.
Moving one field therefore leaves its siblings live, and moving the same field
twice rejects. An explicit `[linear]` record remains one nominal
root. Literal-length fixed arrays likewise expose one canonical path per
contained element: literal-index extraction leaves sibling obligations live,
while runtime-indexed owned extraction remains conservative because it cannot
name one unique element. Active sums expose a case-plus-field path for every
contained claim. Constructing a case activates only its payload; moving one
payload field leaves same-case siblings live while impossible case alternatives
remain inactive.

An aggregate with structural field cleanup may be partially moved. Its cleanup
plan visits only the remaining live fields. The implemented terminal slice
accepts a finite nonempty set of pairwise prefix-disjoint, nonempty all-field
moves from one claim-free affine record, provided at least one residual subtree
remains. It cleans every maximal live residual subtree in recursive reverse
declaration order and never cleans a partially moved ancestor whole. Arrays and
cases, claims, content evidence, contracts, and nominal `drop` remain fenced
from that slice.

A type with a nominal whole-value `drop` body may not be partially moved:
the body is entitled to receive one whole valid value. Such a type exposes an
explicit consuming decomposition machine when field extraction is meaningful.
The cleanup body must return with the value valid; its resulting field frontier
then determines structural cleanup.

Dynamic-index owned extraction remains subject to the general requirement that
the checker can name one unique place. No cleanup-specific runtime bitmap is
introduced to compensate for an unnameable frontier.

## Contextual Droppability

Multiplicity is a type property; automatic droppability is checked at the
particular edge. Every `requires` clause on `drop` must be established there.
A value may therefore be automatically droppable in one state and require an
explicit consumer or fact-preserving transfer in another.

The implemented root-return subset currently proves a finite canonical set of
direct relevant Boolean receiver-field requirements in either polarity on a `drop` body that is
empty or contains only the bounded receiver-independent helper calls described
above, from matching caller facts. For example, `requires self.ready, self.armed` is
available for automatic cleanup at a Unit return whose caller establishes both
facts for each affected value; unrelated supported caller facts do not need to
appear in the `drop` contract. A finite list of roots is accepted and runs in
reverse parameter order. Roots sharing one cleanup target share its target-local
proof receiver, but every cleanup action receives distinct obligations for its
own caller root. Psi independently replays every exact receiver substitution in
the proof artifact; none is a native argument or Omega runtime contract. Other
predicates and bodies that can inspect or change premise-bearing receiver facts
remain outside this bounded subset.

The bounded source forms are a bare field, a directly negated field, or direct
equality/inequality with a Boolean literal in either operand order. Psi
canonicalizes all of them to the exact expected Boolean value; nested negation
and broader logically equivalent predicates remain outside this slice.

Diagnostics name both the edge and the missing cleanup premise:

```text
cannot leave edge Transaction::active -> done

automatic Transaction::drop requires:
  custodied_by(transaction, stable_ledger)

available:
  custodied_by(transaction, PaymentProvider@v1)

transfer custody or call an explicit consuming operation
```

For a generic checked body, cleanup requirements are inferred from its edges
and enter the normalized generic contract. An instantiation diagnostic points
both to the failing caller and to the originating cleanup edge; the caller
should not have to discover a remote implicit drop from a bare unsatisfied
predicate.

## Reach, Resources, And Cycles

Cleanup contributes to the enclosing machine contract on the existing axes:

- reach, inferred mutation summaries, and capabilities combine by their ordinary
  union rules;
- structural work sums actions taken on one edge, then takes the maximum across
  alternative acyclic edges;
- stack uses peak composition within an edge, then takes the maximum across
  alternatives;
- every reachable cleanup action contributes its requirements and guarantees;
  and
- every cleanup action must satisfy termination and automatic-cleanup control
  restrictions.

For a measured cycle, cleanup on the backedge runs once per iteration. Bounded
work therefore composes with the same ranking that proves termination:

```text
total work
  = entry work
  + iteration bound × maximum cycle-edge work
  + exit work
```

Cleanup is included in the cycle-edge term. Max-over-edges alone is not a valid
bound for repeated backedges.

## Conservation Witness

The center of every cleanup artifact is one conservation theorem:

> Every incoming owned obligation is assigned exactly once to transfer,
> explicit terminal consumption, automatic affine cleanup, or validated
> no-code affine discard.

Nothing may appear in two categories or in none. The transfer map, ordered
cleanup actions, contracts, and backend realization are evidence for that
theorem.

When an obligation carries decomposable content, this whole-claim theorem also
contains the normalized content equation for its algebra: machine-entry content
plus sealed introductions equals returned content plus content that left checked
custody. `old(place)` names the callable-entry revision of a structural place,
the exact owner-unique `Content<A>::project` machine projects its content, and
`separate(...)` composes disjoint pieces. Here “left checked custody” does not
mean destroyed, reclaimed, or reusable; it records only the frontier transfer.

One useful report shape is:

```text
EdgeCleanupPlan {
    edge
    frontier_before
    established_during_materialization
    transfer_map
    explicit_consumptions
    ordered_affine_cleanup_actions
    trivial_affine_discards
    frontier_after
    effects_and_resource_composition
    conservation_witness
}
```

A linear place in the dying set is a compile error. A trivial affine discard is
an explicit checked no-code action, not evidence that nothing was live.

## Backend Realization

The semantic transfer map gives storage planning information directly. The
backend must not rediscover it from lexical scopes:

- loop-carried source and target places may coalesce into one storage slot;
- an address-stable borrow crossing an edge requires a storage realization
  that actually preserves that address;
- identical cleanup suffixes may share one physical block;
- checked code may be cloned for optimization while retaining its semantic
  state and edge identities; and
- every cleanup or no-code action maps back to the permission ledger.

Coalescing is a soundness requirement when a borrow contract promises stable
address, and a performance acceptance requirement for unchanged loop-carried
large values. It is never used to make an invalid semantic transfer legal.

## Acceptance Requirements

1. An affine local omitted from a successor is cleaned exactly once on that
   edge.
2. A moved result or transition argument is committed before remaining cleanup
   runs.
3. A live linear place in the dying set rejects.
4. Failure and success edges use the same cleanup rules.
5. Nuclear abort emits no cleanup or unwinding.
6. A partially moved structural aggregate cleans only its live fields.
7. Partial movement from a nominal-`drop` type rejects.
8. Different predecessor cleanup lists require no hidden runtime flag.
9. Operationally distinct alternatives require an existing represented
   discriminator or author-visible normalization.
10. A backedge cleans iteration-local values while preserving loop-carried
    places without copies.
11. Bounded cyclic work counts repeated cleanup through the termination
    measure.
12. Proof/debug artifacts retain the conservation witness and exact edge plan.
