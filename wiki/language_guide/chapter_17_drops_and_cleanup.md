# Chapter 17: Drops And Cleanup

Owned values need a deterministic cleanup story. Omega represents that story
on graph edges, where ownership actually changes, rather than at lexical braces
or in backend-invented drop flags.

The core rules are:

- Plain values with no cleanup simply stop being live.
- Affine values may have automatic cleanup.
- Linear values require exactly one authorized terminal disposition. A
  type-owned automatic disposition may serve as that consumer; otherwise the
  value must reach an explicit consuming machine and rejects if it would die on
  an edge.
- Moving a value transfers its obligation.
- Every ordinary outgoing edge carries a checked cleanup plan.
- Nuclear abort is not an ownership-graph edge and performs no cleanup.
- Cleanup remains visible in semantic, proof, debug, and resource artifacts.

## Cleanup Machines

Cleanup uses one reserved, owner-attached machine shape:

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

Only the data declaration's owning package may declare the exact attached
`T::drop`. At most one exists. It is a compiler-selected hook, not a trait
requirement or an authored callable: source selection of that exact declaration
as a method, qualified call, static-machine argument, or forwarded reference
rejects. An unrelated ordinary machine named `drop` has no reserved meaning.

At a cleanup edge the compiler has already begun consuming the owning
occurrence and temporarily lends the whole valid value as `&mut self` to the
hook. The signature therefore remains honest: the hook borrows during an
enclosing consumption; it does not turn an ordinary mutable call into a hidden
move. When the hook returns, structural field cleanup completes the one
consumption.

The hook may call ordinary machines and carry declared service reach, but
automatic cleanup is always:

- terminating;
- infallible;
- non-suspending;
- nonblocking; and
- free of abort, trap, or another abnormal outcome.

These restrictions, not multiplicity alone, determine whether a linear type
can authorize automatic cleanup. The disposition must be expressible using the
receiver's owned authority and facts established at the edge, with no extra
runtime argument, returned custody, failure, blocking, or suspension. Many
linear protocols therefore remain explicit: unregistering may require
quiescence evidence, returning storage may need allocator authority, and
committing may fail. A valid owner-attached hook is nevertheless a legitimate
exact-once terminal disposition for a linear type.

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

## Explicit Early Disposal

Authored code ends a value's lifetime early through the ordinary consuming core
machine, conceptually:

```omega
pub machine drop<T>(value: T) {
    // `value` reaches its checked cleanup edge here.
}
```

`omega::core::drop(value)` moves the whole value into that machine. The caller's
occurrence is dead immediately, and the callee's return edge invokes the same
cleanup plan that scope exit would use. This is not a direct call to
`T::drop(&mut self)` and does not require a `Drop` or `Disposable` trait.

The core machine is checked once against a symbolic contextual-cleanup row. At
each application the concrete type supplies its exact structural plan,
prerequisites, effects, reach, work, and guarantees. A concrete nongeneric
machine that lets an owned parameter die produces the same row; the mechanism
belongs to death edges, not to the core helper that first exposes it.

Cleanup is deliberately not a core trait. Ordinary Omega traits permit several
separately named conformances for the same type and do not authorize dedicated
syntax to discover one ambiently; automatic cleanup requires one structurally
unique owner attachment. Static generic code consumes symbolic cleanup rows,
and dynamic code consumes the type descriptor described below, so neither needs
a nominal `T: Drop` bound. A future general facility for owner-unique,
compiler-selected protocols could subsume this hook, but a superficial trait
would otherwise add nomination and conformance machinery without changing the
cleanup semantics.

Named consuming machines remain the source surface for protocols with results
or stronger behavior. `close(self)`, `finish(self)`, `commit(self)`,
`unregister(self, ...)`, and `abandon(self)` may fail, block, return authority,
or promise more than the automatic fallback. They are distinct operations, not
alternative spellings of the hook.

A release that waits, suspends, may fail, or promises protocol completion is an
explicit consuming machine such as `close`, `flush`, `commit`, `finish`, or
`cancel`. A resource with no valid nonblocking terminal outcome must be linear.
An affine resource may instead have an authorized nonblocking fallback, such as
abandonment or transfer to a stable custodian.

Sound abandonment does not make silent abandonment appropriate. When forgetting
a claim permanently withholds external capacity, the claim remains linear and
the terminal choice is explicit: a potentially failing or blocking `release`,
or an infallible `abandon` that records the loss. Only resources whose contract
declares implicit disposal harmless may use automatic scope-exit abandonment.
Affine ownership permits that disposition by default; linear ownership
additionally requires the type owner's exact authorization.
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
5. Fixed-array elements are established from lowest to highest index and the
   remaining live elements are cleaned from highest to lowest index.
6. A sum cleans only its active payload.

The checker validates that cleanup-bearing borrow and ownership dependencies
agree with this order. A borrowed owner cannot die before a dependent cleanup
action.

Reverse declaration order is stable at joins. Dynamic acquisition history is
not. APIs needing a different release protocol express it through an explicit
owner or consuming machine rather than asking cleanup to reconstruct history.
Fixed-array cleanup is the same reverse-establishment rule with indices as its
structural positions; authored element moves still occur in authored order.

Complete-value cleanup and abandoned construction intentionally use different
orders. A complete record or case owns its fields structurally and cleans them
in recursive reverse declaration order. A record or case literal evaluates
named fields in authored order; if an ordinary cleanup-bearing edge abandons
that partial construction, only its established prefix exists and cleans in
reverse establishment order. Partial call-argument staging follows the same
rule. Physical layout and completed-value canonicalization determine neither
schedule. A trap or nuclear abort remains a no-successor edge and cleans
nothing.

If fixed-array construction leaves through an ordinary cleanup-bearing edge,
only the successfully established prefix exists and it is cleaned from its
highest established index to its lowest. This is ordinary edge cleanup, not
exception unwinding. A trap or nuclear abort is a no-successor edge and cleans
nothing.

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

Checked transition planning has one deliberately smaller path-sensitive rung.
An attached two-state Unit machine may make one unconditional ordinary jump
whose sole non-self affine parameter is a claim-free, unqualified record of
exactly two structural fields. Moving either whole direct field to the sole
exact-same-type successor parameter retains the other field as one maximal
no-code residual. The checked row is separate from the whole-root edge rows,
so existing Terminal consumers fail closed instead of silently dropping the
path. Executable Terminal control, codec/runtime/native replay, nested or wider
records, extra roots, and arbitrary control flow remain unimplemented for this
transition form.

An aggregate with structural field cleanup may be partially moved. Its cleanup
plan visits only the remaining live fields. The implemented terminal slice
accepts a finite nonempty set of pairwise prefix-disjoint, nonempty all-field
moves from one claim-free affine record, provided at least one residual subtree
remains. It cleans every maximal live residual subtree in recursive reverse
declaration order and never cleans a partially moved ancestor whole. Arrays and
cases, claims, content evidence, contracts, and nominal `drop` remain fenced
from that slice.

For a partially moved fixed array, the compiler constructs one static cleanup
sequence from the exact live index set: decreasing indices with every moved or
otherwise discharged element absent. It does not emit a traversal with runtime
liveness flags. Cleanup recurses structurally, so `[Record; 3]` cleans the live
fields of element 2 before element 1 and element 0, while `[[T; 2]; 3]` applies
decreasing-index order at both levels.

The implemented nested multiple-residual slice accepts `[[T; N]; 2]` for
`N = 3`, `N = 4`, `N = 5`, `N = 6`, `N = 7`, or `N = 8` when `T` is the exact claim-free,
unqualified affine record leaf without nominal cleanup. It permits exactly one
literal leaf
move from each outer element. The length-eight form cleans the fourteen-leaf
complement in decreasing outer-then-inner order. All six forms retain authored
call order and charge five closure fuel units; the extra residuals are static
no-code cleanup metadata. Inner length nine, another outer length, dynamic or
deeper paths, and the existing type and ownership fences remain unsupported.

A type with a nominal whole-value `drop` body may not be partially moved:
the body is entitled to receive one whole valid value. Such a type exposes an
explicit consuming decomposition machine when field extraction is meaningful.
The cleanup body must return with the value valid; its resulting field frontier
then determines structural cleanup.

Dynamic-index owned extraction remains subject to the general requirement that
the checker can name one unique place. No cleanup-specific runtime bitmap is
introduced to compensate for an unnameable frontier.

The second bounded construction-prefix slice admits an uninitialized mutable
`[T; 4]` only when `T` is the same empty, unqualified, claim-free affine record
with no nominal cleanup accepted by the first slice. Establishments must be the
literal prefix `[0, 1, 2]`; an ordinary Unit return records three distinct
zero-ABI element occurrences and cleans them in reverse order `[2, 1, 0]`.
Missing, reordered, duplicate, dynamic, wrong-root, or wider construction
shapes at that slice remain unsupported, and trap or nuclear-abort edges still
clean nothing.

The third bounded slice admits `[T; 5]` under those same restrictions.
Establishments must be the literal prefix `[0, 1, 2, 3]`; an ordinary Unit
return records four distinct zero-ABI element occurrences and cleans them in
reverse order `[3, 2, 1, 0]`. Missing, reordered, duplicate, dynamic,
wrong-root, wrong-length, or wider construction shapes remain unsupported, and
trap or nuclear-abort edges still clean nothing.

The fourth bounded slice admits `[T; 6]` under those same restrictions.
Establishments must be the literal prefix `[0, 1, 2, 3, 4]`; an ordinary Unit
return records five distinct zero-ABI element occurrences and cleans them in
reverse order `[4, 3, 2, 1, 0]`. Missing, reordered, duplicate, dynamic,
wrong-root, wrong-length, or wider construction shapes at that slice remain
unsupported, and trap or nuclear-abort edges still clean nothing.

The fifth bounded slice admits `[T; 7]` under those same restrictions.
Establishments must be the literal prefix `[0, 1, 2, 3, 4, 5]`; an ordinary
Unit return records six distinct zero-ABI element occurrences and cleans them
in reverse order `[5, 4, 3, 2, 1, 0]`. Missing, reordered, duplicate, dynamic,
wrong-root, wrong-length, or other prefix drift remains unsupported, and trap
or nuclear-abort edges still clean nothing.

The sixth bounded slice admits `[T; 8]` under those same restrictions.
Establishments must be the literal prefix `[0, 1, 2, 3, 4, 5, 6]`; an ordinary
Unit return records seven distinct zero-ABI element occurrences and cleans them
in reverse order `[6, 5, 4, 3, 2, 1, 0]`. Missing, reordered, duplicate,
dynamic, wrong-root, wrong-length, or other prefix drift remains unsupported,
and trap or nuclear-abort edges still clean nothing.

The seventh bounded slice admits `[T; 9]` under those same restrictions.
Establishments must be the literal prefix `[0, 1, 2, 3, 4, 5, 6, 7]`; an
ordinary Unit return records eight distinct zero-ABI element occurrences and
cleans them in reverse order `[7, 6, 5, 4, 3, 2, 1, 0]`. Missing, reordered,
duplicate, dynamic, wrong-root, wrong-length, or other prefix drift remains
unsupported, and trap or nuclear-abort edges still clean nothing.

The eighth bounded slice admits `[T; 10]` under those same restrictions.
Establishments must be the literal prefix `[0, 1, 2, 3, 4, 5, 6, 7, 8]`; an
ordinary Unit return records nine distinct zero-ABI element occurrences and
cleans them in reverse order `[8, 7, 6, 5, 4, 3, 2, 1, 0]`. Missing,
reordered, duplicate, dynamic, wrong-root, wrong-length, or other prefix drift
remains unsupported, and trap or nuclear-abort edges still clean nothing.

The ninth bounded slice admits `[T; 11]` under those same restrictions.
Establishments must be the literal prefix `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]`; an
ordinary Unit return records ten distinct zero-ABI element occurrences and
cleans them in reverse order `[9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`. Missing,
reordered, duplicate, dynamic, wrong-root, wrong-length, or other prefix drift
remains unsupported, and trap or nuclear-abort edges still clean nothing.

The tenth bounded slice admits `[T; 12]` under those same restrictions.
Establishments must be the literal prefix
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]`; an ordinary Unit return records eleven
distinct zero-ABI element occurrences and cleans them in reverse order
`[10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`. Missing, reordered, duplicate, dynamic,
wrong-root, wrong-length, or other prefix drift remains unsupported, and trap
or nuclear-abort edges still clean nothing.

The eleventh bounded slice admits `[T; 13]` under those same restrictions.
Establishments must be the literal prefix
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]`; an ordinary Unit return records twelve
distinct zero-ABI element occurrences and cleans them in reverse order
`[11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`. Missing, reordered, duplicate,
dynamic, wrong-root, wrong-length, or other construction-prefix drift remains
unsupported, and trap or nuclear-abort edges still clean nothing.

The twelfth bounded slice admits `[T; 14]` under those same restrictions.
Establishments must be the literal prefix
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]`; an ordinary Unit return records
thirteen distinct zero-ABI element occurrences and cleans them in reverse order
`[12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`. Missing, reordered, duplicate,
dynamic, wrong-root, wrong-length, or other construction-prefix drift remains
unsupported, and trap or nuclear-abort edges still clean nothing.

The thirteenth bounded slice admits `[T; 15]` under those same restrictions.
Establishments must be the literal prefix
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]`; an ordinary Unit return
records fourteen distinct zero-ABI element occurrences and cleans them in
reverse order `[13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`. Missing,
reordered, duplicate, dynamic, wrong-root, wrong-length, or other
construction-prefix drift remains unsupported, and trap or nuclear-abort edges
still clean nothing.

The fourteenth bounded slice admits `[T; 16]` under those same restrictions.
Establishments must be the literal prefix
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]`; an ordinary Unit return
records fifteen distinct zero-ABI element occurrences and cleans them in
reverse order `[14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`. Missing,
reordered, duplicate, dynamic, wrong-root, wrong-length, or other
construction-prefix drift remains unsupported, and trap or nuclear-abort edges
still clean nothing.

The fifteenth bounded slice admits `[T; 17]` under those same restrictions.
Establishments must be the literal prefix
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]`; an ordinary Unit
return records sixteen distinct zero-ABI element occurrences and cleans them
in reverse order `[15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`.
Missing, reordered, duplicate, dynamic, wrong-root, wrong-length, or
length-eighteen and wider construction shapes remain unsupported, and trap or
nuclear-abort edges still clean nothing.

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

Every owned occurrence reaching a death edge derives one internal contextual
cleanup row. Its parts retain their distinct meanings:

- Type-side eligibility checks whether the occurrence and its custody may be
  discharged by that plan;
- proposition prerequisites must be proved from local facts or already appear
  in the machine's authored `requires` contract;
- effects, reach, and work enter the checked operational summary; and
- cleanup postconditions may contribute derived guarantees.

Body analysis may derive guarantees and operational dependencies, but it never
invents a caller-facing demand. This is why an acyclic body may derive a
termination guarantee while a dying parameter cannot silently add a cleanup
precondition: the former gives callers a fact, while the latter would make them
owe an undeclared fact. The rule applies equally to public and private
machines.

For a generic checked body the row remains symbolic in its type parameters and
is discharged after substitution; the body is not rechecked as an unrelated
template. Containers require no nominal `Disposable` bound. Their structural
plans compose from the plans of their live elements, and an instantiation whose
elements lack a legal disposition rejects. Diagnostics expand synthesized rows
back to their authored origin, naming the exact hook clause and cleanup edge
rather than exposing an internal predicate such as `CleanupEligible<T>`.

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
> explicit terminal consumption, automatic cleanup (affine or an
> owner-authorized linear terminal disposition), or validated no-code affine
> discard.

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
    ordered_automatic_cleanup_actions
    trivial_affine_discards
    frontier_after
    effects_and_resource_composition
    conservation_witness
}
```

A linear place in the dying set is a compile error unless its exact type-owned
cleanup plan is an authorized terminal disposition and all contextual
requirements hold. A trivial affine discard is an explicit checked no-code
action, not evidence that nothing was live.

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

An owned erased or dynamic value carries the same obligation after its concrete
type is hidden. Its compiler-built descriptor therefore retains size,
alignment, movement, and the exact structural cleanup plan. Moving the erased
owner transfers payload custody and this disposition together; final
consumption invokes the descriptor plan exactly once. A borrowed dynamic view
carries dispatch metadata but never cleanup ownership for its referent.
Manually destroying a heterogeneous collection remains ordinary:
`omega::core::drop(collection)` consumes the collection, whose structural plan
then invokes each owned element's descriptor cleanup.

Erasure into an automatically cleaned owner is legal only when the concrete
plan is eligible under the erased package's retained invariant, or when the
package also carries the stable facts and authority needed by that plan. A
linear value with no such automatic disposition cannot be placed in an
auto-cleaned heterogeneous container; it needs an explicit consuming owner.
This descriptor entry is lifecycle metadata, not trait evidence, and third
parties cannot attach cleanup to a foreign type. They wrap it in a type they
own when they need a different disposition.

Coalescing is a soundness requirement when a borrow contract promises stable
address, and a performance acceptance requirement for unchanged loop-carried
large values. It is never used to make an invalid semantic transfer legal.

## Acceptance Requirements

1. An affine local omitted from a successor is cleaned exactly once on that
   edge.
2. A moved result or transition argument is committed before remaining cleanup
   runs.
3. A live linear place in the dying set rejects unless its owner-authorized
   automatic disposition is valid at that edge.
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
13. Authored selection of the reserved attached hook rejects; explicit early
    disposal consumes through `omega::core::drop`.
14. Static generic cleanup rows and dynamic descriptors preserve the same exact
    concrete disposition without trait or conformance lookup.
