# Value-dependent facts and views

Range predicates, contracts, and layout/view conditions may name in-scope program
values. These are flow-sensitive facts about exact values and places, not
arbitrary runtime computation of nominal types. This systems contract does not
restrict the general mathematical quantification of the
[selected foundation](../proofs/foundation.md). Its
[mathematical source binders](../proofs/mathematical_bindings.md) and the
[W-based profile](../proofs/inductive_profile.md) are selected.

## Static indices and runtime witnesses

| Dependency | Representation and checking |
| --- | --- |
| Const index | Canonical static value or licensed symbolic expression participating in static application identity. |
| Runtime-capable generic index | Exact value subject bound by a non-const value binder; its runtime content is not a static specialization key. |
| Stored witness | Ordinary field/parameter whose value constrains another field or region. |
| View witness | Ordinary value such as count or stride fixed when a borrowed view is established. |
| Flow fact | Erased proposition valid for its exact subjects and current program point. |

Runtime dependencies do not become const-generic arguments. They may supply
runtime-capable value binders under [generic staging](generics.md#value-binders-and-const-requirements).
Static eligibility and canonical encoding follow
[semantic evaluation](evaluation.md#canonical-static-identities); runtime-index
compatibility instead retains exact subjects and checked relationships.
Type relevance and erased witness identity/multiplicity follow
[proof erasure](../proofs/contracts.md#identity-availability-and-erasure), not a
blanket prohibition on proof-only values.

A slice's stored length and a case arm's selected payload facts are ordinary
examples of dependence. A runtime descriptor view may use `count` and `stride`;
access must prove the actual range lies inside its backing extent, not substitute
the compiled record size for the foreign stride. Row-major access similarly
needs the relation between dimensions, coordinates, and total extent. Boundary
output parameters can serve as stored witnesses through exact postconditions.

Dynamic-sized regions live behind checked views or provisioned buffers/storage;
runtime witnesses do not imply variable-sized stack locals, implicit boxing,
or a hidden global layout descriptor. The sanctioned route to a runtime-sized
region inside an activation is an explicit bounded extent claim under
[runtime-sized activation storage](../resources/activation_storage.md).
[Layout plans](../layouts/plans.md) and
[recasts](../layouts/recasts.md) determine representation and view legality.
Runtime values have no implicit proof tuple or hidden witness allocation merely
because a fact names them.

`Buffer<count>` can retain a runtime extent when Buffer declares a runtime-capable
value binder and has a valid storage realization. It does not by itself allocate
count elements, instantiate count-specific code, or authorize runtime-sized inline
stack storage — that stays a bounded claim under
[runtime-sized activation storage](../resources/activation_storage.md), including
when the buffer's realization is activation-local. A `const` binder still requires
static knowledge. Existing
const-indexed domain families retain their declared binding time; runtime-capable
indices do not reinterpret them or turn arbitrary runtime data into nominal types.

## Establishment and witness loans

Prove a dependency statically, establish it through a visible guard or contracted
decode/validator, or reject the use. `as` follows
[domain casts](domains.md#domain-casts). The compiler does
not insert residual checks to make an unproved refinement true. An explicit
runtime check owns its ordinary false outcome; a decoder establishes only the
predicates it checks, not firmware truth or routed authority.

A live borrow of a dependent place also read-loans the witnesses on which its
validity relies. Those witnesses cannot change while the dependent borrow is
live. Facts and loans are checked together before erasure; forgetting a layout
parameter or a displayed qualification cannot remove this obligation.

Across calls, transport facts only through their exact parameter/result
substitution and checked contract. Machine-arrival requirements are proved on
every incoming edge and assumed only at arrival. Mutation invalidates stale
entry facts; a backedge cannot prove itself from a premise invalidated by its
own writes. See [invocation contracts](state_contracts.md).

## Default domains and zero initialization

A data declaration's field qualifications and `requires` clauses define its default
domain; omission adds no predicates beyond its ordinary field/type obligations.
An optional data-declaration `dropped by` clause contributes the default domain's
[automatic cleanup selection](domains.md#domain-selected-automatic-cleanup),
independently of those validity predicates.
This does not mean an uninhabited domain. Zero-initializability is a storage
representation guarantee, not universal semantic membership or ambient write
authority. A zeroed storage representation can be accessed as an established
value only after its default-domain obligations hold.

Field qualifications may also carry
[domain-owned read/write restrictions](domains.md#domain-owned-read-and-write-authority).
Establishing validity does not grant access forbidden by that policy. It remains
attached to the governed field through parent borrows, projections, whole-place
replacement, and domain removal. Invariant windows permit temporary
invalidity only under otherwise-authorized access; they do not waive a reader or
writer restriction. Fresh construction and policy installation follow the domain
establishment rules, not an implicit file-local or constructor privilege.

Constraints on fields and active-case payloads are written in data
`requires`. Its `self` denotes the complete value, as in named-domain predicates.
Payload projections require a prior case guard; case declarations do not accept
`requires`. See [case-dependent default domains](data_and_literals.md#case-constraints).
Bracketed scalar range suffixes are not a second default-domain syntax.

Boundary provider slots are not data fields and add no default-domain obligation.
Provider planning must satisfy every demanded exact requirement application
before realization; that independent check neither changes the containing data's
validity nor manufactures authority or storage.

[Case-dependent default predicates](data_and_literals.md#case-constraints) can
relate the active case to type parameters and payload values. Data-level
obligations remain conjunctive with that case's payload validity. The zero
representation keeps the first declared case, even when its constraints fail
for a particular specialization; there is no selection of a first possible case.

If zero satisfies the default domain, the value is zero-constructible. Otherwise
the type is gated: construction or checked qualification must establish the
domain before observation. A literal without `..` supplies every runtime field.
With [explicit omission](data_and_literals.md#explicit-field-omission), it
supplies every field whose zero value would violate its obligations and proves
the complete coupling. Zero-initializable storage does not implicitly authorize
source-literal omission. Establishment is monotone as observed: later mutation may temporarily open an invariant window, but no
consumption sees the value outside its required domain.

Generic bodies prove establishment from their declared contracts and available
flow facts. They may use proved zero-validity, but cannot rely on a later
specialization to justify an otherwise unchecked observation. Zeroing storage is
permitted independently of establishing a value. For `Value<T>` in the case-constraint
example, `T == i32` suffices to establish the all-zero Integer value, whereas an
unconstrained `T` does not and `Value<bool>` remains gated until valid construction.

Permitting [uninhabited domain declarations](domains.md#uninhabited-domains)
does not relax this establishment gate. An impossible qualification cannot be
established merely because storage was allocated or zeroed.

Gating propagates through containment; a zero-valid first sum case can represent
emptiness without constructing a gated payload. Machine-owned storage may begin
zeroed while gated fields remain inaccessible until established. Storage
validity is not a claim that partially established runtime-indexed arrays are
automatically proved; such uses need exact element/extent establishment evidence.
Proof propositions and proof-only values with no runtime representation acquire
neither ZII nor layout obligations.

<a id="mutable-receivers-and-declared-field-domains"></a>
### Mutable arguments and declared field domains

An ordinary `value: &mut T` argument carries the declared field/default-domain
obligations of `T` at call, entry, and return. The
caller establishes them, the callee assumes them, and returning execution
restores them under the ordinary invariant-window rules. Whether zero satisfies
those domains is irrelevant to an already established incoming value.

Contracts may state `requires value in Domain` and corresponding
`ensures`. An additional input domain
need not survive a call unless the ordinary contract guarantees it. Zeroed or
partially established storage still needs establishment before a readable borrow;
a qualified machine name grants no construction or validity exception.

That predicate-fact rule does not authorize dropping a governed subject's access
policy. Removing the displayed domain or invalidating a flow predicate cannot
create a reader/writer grant; retained restrictions follow their governed lifetime.

#### Lent storage retains its declared domains

Named clarification: `lender-domain-restoration`, ratified 2026-11-03.

A local declared `value: T in D` has the same restoration obligation for `D`
as a stored field declared with that qualification. Lending it does not turn
that obligation into an expiring input fact. A mutable loan, including a view
whose displayed type omits `D` after a written
[cast on the reference](domains.md#mutable-views), retains the original
storage's obligations.
Writes may open the ordinary invariant window. The owner's required predicates
must hold before its qualified storage is consumed as valid or handed back to
it. A borrower's internal calls and terminal transfers check the receiving
contracts and retain the owner's restoration obligation as described below;
they do not each constitute handback to the owner. A call cannot silently change
the local's declared qualification or make invalid storage usable under its old type.

An additional input qualification instead describes a fact supplied for a
particular call. If the original storage does not itself owe that qualification,
the input fact may expire unless the contract promises it afterward. Thus an
entry-qualified helper may be valid for an unqualified lender but inadmissible
for a lender whose declared domain its post-state fails to restore. Checking the
call and loan handback must retain that distinction; the formal's spelling alone
cannot decide which obligations the owner owes.

For example, a library-defined `Ascii` domain over a byte array with a predicate
excluding 255 must be restored when a declared `in Ascii` local is lent and
returned. No encoding name is compiler-known. A predicate-free domain has no
predicate debt, but its establishment provenance, access policy, custody and
carry restrictions still apply. An unrestricted empty tag promises no hidden
invariant; protocols relying on restricted witnesses or mutations must declare
the corresponding routes and access policy. A cast cannot bypass them.

### Contract-based repair

Ratified 2026-11-05: existing `requires` and `ensures` compose repairs of a
lender's named qualification. No new repair syntax is introduced.
The receiving machine must establish its own parameter/default domains and
entry requirements from the current contents. It may receive a weaker view
while the original owner's stronger qualification remains owed, provided
exclusive custody and the exact restoration obligation remain attached to
that storage until handback. The caller writes the weaker view as a
[cast on the mutable reference](domains.md#mutable-views), which can remove
any domain the lender could add again itself when the loan ends. That cast
never cancels the obligation: each removed domain is re-established by its own
`+D` rules for the current contents.

A helper's checked normal-completion guarantees may discharge the enclosing
machine's restoration promise. At a terminal call, prove the successor's
requirements at entry and compose its guarantees into the enclosing completion
contract. A future guarantee is not a fact at the transfer's entry. Missing
restoration evidence rejects the enclosing completion or handback; an extra
helper layer or a recursive cycle cannot turn an unproved promise into evidence.
Recoverable outcomes must satisfy the obligations of their actual handback;
cleanup, suspension and crash retain their existing contracts.

This distinction does not weaken a data type's default domain. A successor
accepting `&mut Tree` still requires `Tree`'s default domain on arrival, even if
it promises that domain on completion. An unestablished whole, absent field or
invalid reference cannot be passed as a valid value. For multi-step repair,
the intermediate view must itself be valid under its weaker declared type.

After an overlapping write invalidates predicate-only membership, fresh proof
of the domain's defining predicates for the current subject may re-establish
that membership. Ordinary checked guarantees and theorem evidence are usable
for this proof. The named predicate and its defining conditions must not take
different admission paths merely because mutation occurred. Establishment
routes, provenance, access and cleanup policies remain separate obligations;
unfolding a predicate cannot manufacture them. Unaffected facts remain available
under the exact mutation frame; invalidated facts require new evidence.

### Collection invariants

For `per-element-array-bounds`, use an ordinary pure, total, terminating
machine over the collection as a predicate in `requires`. Its body
walks the elements with ordinary recursive machines; collection invariants introduce no
`forall`, `foreach`, or bracketed scalar-range type suffix.

For example, a field `cells: [i32; 16]` can be constrained by
`requires cells_valid(&cells) == true;`. The ordinary machine
`cells_valid(cells: &[i32; 16]) -> bool` recognizes precisely arrays whose
elements are each zero or one. Its use in a proposition must meet the ordinary
[semantic evaluation](evaluation.md) admission rules. This asserts its
denotation, not an implicit runtime traversal on every write.

Construction proves the collection predicate; mutation preserves or restores
it under the ordinary invariant-window rules. A checked helper theorem can
establish that a selected element is zero or one from that predicate and a
valid index. Merely naming an arbitrary predicate does not grant the checker
its elementwise consequences: those follow from checked reasoning about the
machine, not a new quantifier or an unchecked decomposition rule.

## Invariant windows

An invariant window is compiler-derived proof debt, not a source `relax` mode,
runtime poison flag, or mutable truth bit.
[Exclusive storage recasts](../layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts)
may open initial target-establishment debt before a valid target value exists.
Geometry, authority and custody are immediate requirements; target-shaped storage
is not a valid value merely because a cast names its type. Field initialization
and checked evidence must establish the whole before ordinary typed consumption.
The cast's pending-view formation is allowed, not a general exception for later
borrow creation, calls or publication. Source handback and partially initialized
resource cleanup remain separate obligations.
[Borrowed-storage ownership windows](ownership.md#borrowed-storage-invariant-windows)
apply the same discipline to temporarily absent content: a move records exact
restoration debt, and an establishing store closes it. A hole has no current value
to reason about, unlike present contents whose domain is temporarily unproved.
Ownership restoration and domain restoration are separate obligations; satisfying
one cannot discharge the other. The ownership rules govern calls on detached
values and disjoint siblings without weakening the domain fences below.

A write known to preserve the default domain keeps it established. Otherwise
the write opens a window on the place,
whose actual new contents remain available to ordinary flow reasoning.
The domain must be proved again at the next consumption point:

- A read relying on the domain or a dependent coupling.
- Borrow creation or a call consuming the affected domain.
- Transfer, return, scope expiration or loan handback releasing the relevant
  custody or supplying a value under that domain. Contract-based repair of a
  lender's named qualification follows the delegation rule above.
- Boundary/capability-carrying calls that may observe or expose the affected
  storage, including through authority not named as an argument.

A call need not close an unrelated domain window when its complete access,
effect and outcome contracts prove that it cannot observe or expose the broken
coupling, interfere with its witnesses or restoration, or release the custody
excluding observers. Include argument evaluation, callbacks and authorized
capability state in that determination. An empty mutation frame alone proves
neither absence of reads nor non-exposure. Unknown access/effects are treated
conservatively. This exception grants no new borrow authority and does not make
an unfinished value valid; initial-establishment and ownership-hole restrictions
remain in force.

An unclosed window rejects with both the opening write or storage cast and the
failing consumption point. Borrow exclusivity prevents another observer seeing it; a dependent loan
pins its witnesses, so a conflicting write cannot open a window in the first
place. Write-only access must restore validity from written values, structure,
and supplied facts, never by reading its referent.

Recoverable failure does not cancel a window. Once established, a value cannot
be discarded while its cleanup or obligations rely on a broken invariant.
Failed initial establishment instead creates no value, leaving raw storage under
its existing storage claim. A crash may abandon an open window but supplies no
survivor-safety theorem; [crash semantics](../terminal-psi/calls_and_outcomes.md#crash)
governs that distinct outcome.

## Mutation frames

Named decision: `published-write-frames`, settled 2026-11-10;
[the design draft](../../drafts/designs/published_write_frames.md) records the
change. A call's frame is the existing storage it may write. A caller's fact
about current storage survives the call when its storage dependencies are
provably disjoint from the frame, aliases included. A checked body's frame is
inferred, and it is part of the machine's published contract:

```omega
public machine set_time(entry: &mut Entry, time: u64) {
    entry.at = time;
}
// published frame: entry.at

set_time(&mut e, 5);
use(e.owner);        // facts about e.owner survive
```

A frame has no authored form. A wide frame invalidates only facts about
current storage it may write: snapshots taken before the call stay valid, and
the callee's guarantees establish facts afterward as usual. A frame
establishes no contents and grants no authority.

**Where a frame comes from.**

| Callee | Frame |
| --- | --- |
| A checked body, public or private | Inferred from the body and published. |
| A static requirement or callable parameter | The selected machine's published frame, by substitution. |
| A runtime callable value, a boundary or foreign operation | The ceiling: all existing storage the callee is admitted to write. |

A body cannot write beyond what its signature hands it and the authority it
holds, so an inferred frame never exceeds the ceiling. That is the reason
[reach](effects.md#static-binder-contract-dependencies) infers on a body, and
frames follow the same rule. A caller in the same fingerprinted verification
unit may use the body's exact summary; every other caller uses the published
frame.

**Identity.** The inferred frame is part of the published contract and of
interface identity, as an inferred reach row is. A body edit that widens it is
an interface change even when the authored header is unchanged, and dependents
revalidate. Narrowing it is compatible.

**What a frame lists.** A published frame is a set of static paths, each a
parameter followed by zero or more field selections. An indexed write lists
the indexed place whole, and a sum is written whole. A path covers the place
it names and the backing that place owns through custody evidence, and it
passes through references as the parameter itself does. That footprint is
identified at entry and followed through moves. Moving pre-existing storage
into a place does not bring that storage under the place's path.

**Every access path counts.** The frame covers writes to existing storage
through all admitted access: mutable and write-only borrows, owned backing,
reference fields, atomic or synchronized access through a shared reference,
and capability-held authority. It includes argument and index evaluation
writes, delegated calls and callback invocations. A write that restores the
earlier value is still a write. A callee whose storage effects are unknown
contributes its ceiling, never an empty set, and a complete empty frame
differs from an unknown one. [Service reach](effects.md) is a separate axis
and neither bounds nor exempts a storage write.

**Fresh storage.** Writes to fresh private storage are outside every frame:
locals, scratch, and fresh backing that is initialized and then installed in
a place or returned. Exposure follows ordinary aliasing and access rules, and
allocator state keeps its own accounting.

**Wrappers.** The frame of a generic machine is a term over its static
callbacks: its fixed paths together with each callback's frame applied to the
argument places it is called with. The term is published as a row of
[static binder contract dependencies](effects.md#static-binder-contract-dependencies),
and each application substitutes the selected callback's published frame:

```omega
public machine apply<
    const F: machine(entry: &mut Entry, time: u64) crashes none
>(entry: &mut Entry, time: u64) {
    F(entry, time);
}

apply<set_time>(&mut e, 5);   // this application's frame is e.at
```

**Opaque calls.** A runtime callable value and a boundary operation have no
body to infer from and no static selection to defer to, so their frame is the
ceiling. Two existing tools narrow what a caller loses. Borrowing the
subplace, `&mut entry.at`, narrows the ceiling itself and is the first
choice. A requirement can also state the end state,
`ensures entry.owner == old(entry.owner)`, which returns value facts to the
caller. That postcondition permits a write between entry and return and
re-establishes value facts only, under
[restored values](state_contracts.md#restoring-a-saved-scalar-value).
Here the observation is through a reference: bare field reads describe final
contents, while `old(...)` describes entry contents. This does not make a
by-value parameter's contract name denote its final local working value; see
[contract inputs](state_contracts.md#contract-inputs-and-working-values).

Invalidate flow refinements atom by atom on overlapping written places; exact
outcome guarantees may restore them or preserve untouched paths. Default-domain
obligations and explicit domain qualifications must be re-established before
consumption, not assumed to be unchanged values.

Frames retain exact subject substitution across aliases, calls, and terminal
transitions. Where only collection-level origin is known, later field
projections cannot recover narrower precision without new evidence. An
unrepresentable origin or incomplete recursive summary uses the conservative
ownership ceiling, never an empty write set. Frame inference grants no borrow
authority and cannot preserve facts through unmodeled external writes.
Current analysis bounds belong beside the lowered-form checker,
not in the language's dependent-value limits.
