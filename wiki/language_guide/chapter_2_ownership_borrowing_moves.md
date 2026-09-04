# Chapter 2: Ownership, Borrowing, And Moves

Values have an owner. Ownership determines who is responsible for moving,
mutating, and cleaning up a value.

This chapter is the place for the rules other chapters rely on.

> **Core multiplicity.** Usage is an explicit type
> property with three cases: unrestricted, affine, and linear. Facts and
> permissions share control-flow/place infrastructure but not one algebra. See
> [core_multiplicity_and_linearity.md](../design_briefs/core_multiplicity_and_linearity.md).

## Usage Multiplicity

- **Unrestricted** values may be copied and discarded. `[copy]` establishes
  this property.
- **Affine** values may be moved at most once and may be discarded. This is
  the default for owned data.
- **Linear** values must be transferred or explicitly consumed exactly once.
  `[linear]` establishes this property.

Multiplicity is a type property, not a trait and not a qualifier repeated at
every binding. It composes structurally through records, sums, and generic
containers. `[copy]` and `[linear]` are mutually exclusive.

Establishing a new linear value creates exactly one obligation. Moves, calls,
returns, receives, and storage operations transfer that obligation; terminal
consumers discharge it. Implicit zero-filling does none of these. Linearity is
about use, not bit patterns: an explicitly constructed all-zero linear value
is owed if that bit pattern is valid for its type.

## Owned Values

An owned value may be moved into another location.

```omega
machine InventorySystem::repair(&mut self) {
    let replacement: Inventory;

    self.inventory = move replacement;
}
```

After a move, the old binding is no longer usable.

## Copy Values

Some values are copied instead of moved.

```omega
let depth: u32 = self.level_depth;
let next_depth: u32 = depth + 1;
```

Copy is a type property. Machine integers, booleans, and small proof values are
natural copy candidates. A copied value is unrestricted. Data with unique
cleanup responsibility is not.

## Linear Values

Linear values represent protocols that must reach an explicit conclusion: task
lifecycle claims, transactions, acknowledgements, DMA submissions, and similar
resources. A move transfers the live obligation. Ordinary scope exit with an
unconsumed linear value is a compile error; automatic drop cannot silently
discharge it.

Conditional ownership uses an ordinary sum such as `Idle | Running(Task<T>)`.
The obligation belongs only to the live payload. Zeroed storage is not a
universal consumed linear value.

This rule is unchanged for generic sums. The nominal container keeps its own
declared multiplicity while the active payload carries every affine or linear
obligation introduced by substitution. `Returned(LinearT)` and
`Rejected(LinearArguments)` therefore participate in the same conditional
permission accounting as `Idle | Running(Task<T>)`; a generic parameter does
not erase custody, and an inactive case does not acquire it.

Flow analysis therefore carries two different kinds of context. Propositions
can weaken or duplicate where logic permits. Permissions track establishment,
multiplicity, loan compatibility (`owned`, `shared`, `exclusive`), permitted
observation/mutation, and provenance with their own path-join rules. One CFG
walk may carry both, but a fact catalog must never silently forget a resource
obligation.

Carry policy is another independent consumer of canonical place liveness. It
does not change ownership: an exclusive move transfers ownership, while the
value's carry policy and selected runtime decide whether the destination
activation/CPU/thread/storage transition is legal. Shared references also need
an access contract that sanctions concurrent use. The compiler shares the CFG
traversal; ownership, carry, and proposition facts retain separate algebras.

## Shared Borrows

Shared borrows allow read-only access.

```omega
machine RoomFormatter::render(
    &self,
    room: &Room,
    out: &write [u8]
) {
}
```

Many shared borrows may coexist if no mutable borrow conflicts with them.

## Mutable Borrows

A mutable borrow is unique for the borrowed place.

```omega
machine Player::heal(
    &mut self,
    amount: i32
) {
    self.health += amount;
}
```

While `self.health` is mutably borrowed, code cannot also read or mutate the
same place through another active reference.

## Write-Only Borrows

A write-only borrow lends an existing valid value exclusively while denying
observation of its prior contents:

```omega
machine fill(destination: &write [u8]) {
    destination[0] = 42;
}

fill(&write buffer[..]);
```

`&write T` has the same alias-exclusion and lifetime rules as `&mut T`, but a
narrower operation set. An exclusive mutable loan may be explicitly attenuated
to `&write`; a write-only loan cannot become `&T` or `&mut T`. It may be
reborrowed only as `&write`. The physical ABI is the corresponding reference
ABI, while the write-only access set remains part of semantic signature and
artifact identity.

Explicit lifetimes use the same position as the other reference forms:
`&'buffer write T`. Receiver spelling follows the same rule, so `&write self`
is an exclusive non-observing receiver rather than a special capability.

The referent is a live `T` on entry and remains one when the loan ends.
`&write` never denotes `Vacant` storage and performs no construction or
definite-initialization transition. A future output/construction slot for
storage containing no live `T` is a separate feature.

> **Implementation checkpoint (August 2026):** the compiler recognizes and
> preserves the distinct `&write` source/type identity. Checked Omega bodies may
> replace unrestricted primitive scalars and fixed byte arrays, replace literal
> or proven-in-bounds dynamic byte elements, replace a supported fixed-array
> range with a
> same-width array literal when both bounds are integer literals or finite
> immutable local-copy chains, replace an unrestricted primitive leaf through a
> finite path of relevant unconstrained common fields in plain invariant-free
> records from either a fixed-integer or Boolean literal or one exact
> same-typed fixed-integer or Boolean parameter, read a literal fixed-array
> length as static metadata through the same eligible record paths, and forward
> the loan explicitly. One projected subloan form may pass
> `&write root.field...leaf` directly to a checked call when the complete field
> path and leaf meet that same non-observation referee. That direct-call form may
> finish with a finite nonempty suffix of ordered in-bounds literal indexes
> through recursively literal fixed arrays, either directly or after the
> eligible field prefix, when the ultimate leaf is an unrestricted non-Atomic
> primitive. The ordered fields and `FixedIndex` suffix cross checked and
> Terminal replay. Direct Unit calls also carry that exact path through native
> assignment, pointer adjustment, object construction, and installed replay on
> Linux x86-64 and AArch64; each stage reconstructs the offset from the retained
> structural declarations. Such a call may also pass checked scalar values;
> their ordinary ABI prefix does not grant or widen write authority.
> It cannot be retained in a local alias. Dynamic and range
> subloans remain gated, as do whole nested-array and aggregate elements,
> record-held slice descriptors, sum projection, and opaque providers.
> Structural parameters and calls preserve
> owned/shared/mutable/write-only access (first introduced in Terminal format
> 27); exact unrestricted record-leaf and literal-indexed field paths cross the
> codec and independent verifier. Terminal format 42/vocabulary 45 additionally carries
> one direct whole-root unrestricted primitive store from either a landed
> integer literal or a Boolean literal. Its ordinary SSA
> value producer precedes a Unit write-only event, the verifier reconstructs
> exact type/access/place custody without an old-value premise, and the
> reference interpreter mutates exact-typed stable target-neutral backing across an
> in-module call with fuel charged before the store. The Boolean form retains
> its exact one-byte referent and literal through target selection and physical
> assignment, and machine emission emits the exact one-byte store on both native
> architectures. Object and installation replay retain its exact definition
> ordinal, target bytes, and attribution. Opaque-provider execution remains
> gated. `&write` is never
> temporarily lowered as `&mut`.

Code may perform plain typed stores, content-independent field/index/range
projection, disjoint subdivision, and read view metadata such as a slice's
length. It may not load, compare, hash, pattern-match, take, swap, perform
read-modify-write, create a readable reborrow, or call a machine expecting one.
The governing rule is that no operation may obtain a premise or choose its
behavior by observing the referent. Static structure, values being written,
and proof facts explicitly supplied by the caller remain usable.

Projection is legal only when its location is known without reading content.
A record field or fixed-offset common field qualifies. A sum payload requiring
a tag read does not, unless an already-available refinement fixes the case.
Whole-value replacement writes the tag and payload together and needs no prior
observation.

Every store must also account for the displaced value. Plain replacement is
available only where the old content is freely discardable and requires no
content-dependent cleanup; write-only access cannot silently consume linear or
otherwise conserved custody. Whole-value replacement is validity-safe when the
incoming value is already a `T`, subject to that displacement rule.

The current bounded native compiler path carries one unrestricted whole-root
fixed-integer replacement from verified Terminal form through target-neutral
abstract operations, optimization validation, target selection, physical
assignment, and machine emission on x86-64 and AArch64. Object construction
independently replays its exact parameter identity, access, primitive type,
borrowed-reference placement and home, preceding typed scalar definition,
target store bytes, and semantic attribution. The Boolean sibling reaches the
same independently replayed physical assignment and exact machine-store
emission while retaining a distinct Boolean source and definition ordinal.
IEEE float literals retain their raw-bit source and definition ordinal through
the same pipeline without requiring floating-register custody. Object
construction and installation format 73 rejoin and transport all three
families. Opaque-provider non-observation guarantees remain fail closed.
The projected-record sibling carries one fixed-integer or Boolean literal, one
exact same-typed fixed-integer or Boolean parameter, or the exact fixed-integer
result of one immediately preceding ordinary scalar call or selected
boundary-operator realization into a relevant primitive field through a finite
field-only path. The parameter form keeps one
structural destination plus an ordered scalar roster and selects one exact
source; target assignment, machine emission, object construction, and installed
replay reconstruct its register, incoming-stack, or durable-result-home
location and exact field offset on both Linux targets.
Boolean sources retain their own one-byte ABI and definition custody without
an integer surrogate.
Bodyless boundary result homes, delayed uses, Boolean and IEEE results, and
arithmetic locals remain gated for projected stores.

A partial write must leave `T` valid at the ordinary invariant-window
consumption points. The checker may prove that from the written inputs, static
structure, and explicitly supplied facts, but never from a load through the
write-only loan. Cross-field invariants therefore often permit only a
whole-value replacement, while independently valid byte elements remain
straightforward.

An exact outcome contract distinguishes the modified range from the untouched
range. For a prefix-producing byte operation, `[0..count)` is changed as
specified and `[count..len)` is unchanged; the caller's facts about the suffix
survive. The count describes the effect and does not establish a previously
nonexistent value.

Checked Omega implementations enforce non-observation transitively through
every helper call. An opaque foreign implementation receives the corresponding
address and may be physically capable of reading it; its compliance is an
admitted provider claim unless target isolation enforces the restriction.

## Reborrow Authority and Restoration

A reborrow derives a child loan from one exact parent-loan occurrence. It does
not create authority, and borrowing the reference carrier itself is not a
reborrow of its referent. The child access must be an allowed attenuation of
the parent's retained access:

| parent access | child `Read` | child `Mutable` | child `WriteOnly` |
| --- | --- | --- | --- |
| `Read` | allowed | rejected | rejected |
| `Mutable` | allowed; shared freeze | allowed; exclusive suspension | allowed; exclusive suspension |
| `WriteOnly` | rejected | rejected | allowed; exclusive suspension |

`is_exclusive` is an interference classification, not this attenuation rule.
In particular, `Mutable` may attenuate to `WriteOnly`, while `WriteOnly` may
never acquire observation by becoming `Read` or `Mutable`.

The permitted cases have three different lifetime effects:

- `Read` to `Read` releases the child without suspending or restoring the
  parent. Shared descendants may coexist.
- `Mutable` to `Read` freezes the parent's mutation authority while a finite
  cohort of shared descendants exists. The parent remains readable, and its
  exact `Mutable` access returns once, only after the last descendant in that
  cohort ends.
- An exclusive child suspends its parent behind one descendant branch. The
  parent regains its exact original access only after that branch ends.
  The first release permits one active exclusive descendant branch; broader
  branching requires a separately specified resource algebra.

Restoration is therefore not the generic rule "child ends, parent becomes
available." Exclusive lineages close deepest-first. Shared descendants form a
dependency set: all members ending at one semantic boundary release before the
frozen parent is restored exactly once. A parent that retires while suspended
or frozen remains pending; closure follows the complete retained lineage to a
live parent or an exact direct-root occurrence. When that route reaches a root
at state exit, the borrow system returns custody to that root only. Transfer,
cleanup, and linear discharge remain ownership operations and are never
inferred by the borrow disposition.

Usable restoration requires checked evidence for the exact parent and child
resources, access pair, formation and weakening boundaries, projection path,
and suspension or freeze interval. The evidence must establish that forbidden
parent use did not occur and that the exclusive branch or complete shared
cohort ended. Lexical survival and a compiler-recorded disposition are not by
themselves authority. Terminal Psi independently reconstructs and replays this
evidence before publishing post-reborrow use or root custody.

The first checked post-restoration use row and its Terminal publication are
deliberately narrow. They accept one direct mutable parent and either an exact
mutable/write-only exclusive child or one exact shared child occurrence with no
sibling for that parent. Other non-overlapping sequential exclusive siblings
may occur. The exclusive form requires exact reactivation and
exclusive-suspension evidence; the shared form requires an exact sole-member
cohort restoration and shared-freeze evidence. The child ends by last use
before the exact next runtime-
receiver-free call
whose sole mutable parameter consumes the bare parent carrier and mutates the
whole restored referent. A nominally qualified static call is still runtime-
receiver-free. Checked replay independently rejoins the child and parent
resources, class-specific containment and disposition, weakening, call,
entry-loan, access, place, and target evidence. Multi-member or sequential
shared cohorts, multihop
children, concurrent siblings, state exit, projected arguments, receiver
calls, direct assignment, and partial mutation remain outside this row.
Terminal then independently matches the exact `CallUnit`, ordinal-zero call
coordinate, callee, restoration class, and encoded sole-member roster and
requires exactly one compatible whole-parent mutating `CallUnit` in a shared
caller. It publishes authority for that one call only. The source target identity is
committed custody rather than reconstructed from machine bytes. It does not publish cleanup,
transfer, or linear discharge.

Published root custody remains deliberately narrow but now covers a finite
linear exclusive lineage. One direct-root mutable loan may lend a mutable or
write-only child; a mutable child may continue with either access, while a
write-only child may continue only as write-only. The complete chain must close
at state exit through exact exclusive-suspension paths. Terminal retains that
handoff as semantic custody, not as executable cleanup or transfer. Shared
cohorts, branching, restoration before state exit, and restored-parent uses
beyond the one published whole-parent call still require further Terminal
publication support.

## Transitions And Ownership

A transition is a jump. Arguments passed to the target state must be valid on
the target edge.

```omega
machine InventorySystem::repair(&mut self) {
    transition {
        _ -> build_inventory()
    }

    state build_inventory(&mut self) {
        let replacement: Inventory;

        transition self.inventory_valid {
            true -> done()
            false -> copy_default_items(move replacement)
        }
    }

    state copy_default_items(replacement: Inventory) {
        self.inventory = move replacement;
    }

    state done(&mut self) {
    }
}
```

Working rules:

- Copy values may be copied into transition arguments.
- Owned values may be moved into transition arguments.
- References may cross a transition only when the referenced storage outlives
  the target path.
- Owned locals not moved into the target are cleaned up on the transition edge.

## Borrow Facts

Borrowing contributes facts to the proof system.

```omega
let a = &mut items[i];
let b = &mut items[j];
```

The checker must know that `i` and `j` refer to disjoint places. That fact may
come from arithmetic, from a domain, or from a helper machine that establishes
`i != j`.

Borrow checking coordinates two ledgers without collapsing them. The
Type/resource ledger owns the existence, provenance, polarity, lifetime, and
return of each loan. The proof ledger may establish relationships over
already-existing, versioned values, places, and authority occurrences. Because
`Prop` is erased and copyable and has no custody disposition, a proposition can
never create, amplify, transfer, extend, return, consume, or duplicate loan
authority.

This is a criterion rather than a closed obligation list. Spatial
disjointness, spatial containment, and non-interference are relational and may
be proved. Literal comparison, symbolic-bound normalization, arithmetic,
domains, and explicit theorem citation are different derivation methods for
those relationships, not separate borrow-obligation kinds. Loan descent from a
live owner, access attenuation, temporal containment within the parent loan,
and restoration remain resource judgments. A compound rule such as "no
conflicting writer" therefore splits: the write loan's existence is Type-side,
while whether its captured place interferes with another loan is relational.

A loan captures its exact place occurrence when it is formed.

The current checked representation retains the first automatic certificate
for this split without promoting proof facts into authority. Each admitted
loan/loan non-interference judgment records a separate zero-premise
`Structural` row naming the formation's machine, state, and statement, both
exact loan occurrences, their frozen places, and the normalized relational
conclusion. For every dynamic selector position consulted by the judgment, the
row also freezes its forming/active side, place-path position, selector
coordinate, and exact normalized integer or immutable-symbol value; a
conservatively unknown coordinate is retained explicitly and grants no positive
evidence. Rerunning checked-fact validation independently normalizes the exact
typed formation expression, requires equality with the frozen rows, and then
replays the relationship. Runtime changes cannot retarget the immutable-symbol
occurrence captured by an existing loan; typed formation drift rejects.
Proposition premises and Terminal verification remain a later rung; this
checked record does not change ordinary borrow admission.

In:

```omega
let view = &mut buffer[table[index]];
```

later mutation of `table[index]` does not retarget `view`. Any proposition used
to license compatibility must dominate formation and be valid for the exact
value/place versions captured at that event. The resulting compatibility fact
is about those frozen loan occurrences; its premises may later expire without
moving or merging the captured places. A supposedly retargetable place instead
violates the Type-side pinning/provenance rule.

The proof context participates from the beginning; it is not a fallback after
a separate borrow checker rejects. The ordinary checker is the default tactic
that constructs the same compatibility certificate automatically. A failed
automatic derivation remains an ordinary borrow diagnostic unless source has
explicitly engaged with proof vocabulary.

Shared symbolic boundaries are an ordinary automatic case:

```omega
let left = &mut items[start..mid];
let right = &mut items[mid..end];
```

After the usual range-validity obligations, the identical half-open boundary
`mid` proves adjacency without requiring literal endpoints or a separately
authored disjointness theorem.

No public `footprint(...)` contract surface follows from this rule. Most
source contracts state ordinary value relationships such as `mid <= items.len`,
from which the checker derives projected-place relationships. Public abstract
footprints for opaque modular APIs remain a separate future feature. Semantic
`Content<A>` projections, logical place footprints, and physical effect
footprints are distinct; a checked carrier-specific bridge may relate them,
as an `Extent` can relate its address-interval content to a place range.

> **Implementation direction (August 2026):** checked trees already retain
> first-class loans and use borrow accesses to invalidate proof facts, but the
> convergence is incomplete. Symbolic range comparison is still largely
> literal-only, arbitrary valid proof facts do not yet discharge one canonical
> place-compatibility obligation, and ordinary loan compatibility is not yet
> retained as an independently replayable Terminal certificate.

## Owners And Borrowed Views

A borrowed view (a slice over an array, a text view over a bounded byte carrier,
or a slice over a `Vec`) keeps the owner pinned for the view's lifetime. While such a view is
active the checker rejects any write to the owner that overlaps the borrowed
window:

```omega
let view: &[Entry] = self.entries.as_slice();
self.entries[0].value = 7; // rejected: view is still active
let first: Entry = view[0];
```

Disjoint windows are allowed when disjointness is provable from compile-time
bounds. A subslice `view[1..]` does not conflict with a write to
`self.entries[0]`, because index `0` is provably outside the `1..` window.

The same rule applies to `Vec`: a `Vec` mutation or reallocation
(`push`, `pop`, or anything that may move the backing storage) must reject while
a slice view derived from that `Vec` is still active, because the view may be
invalidated by the reallocation. This is the borrow-conflict rule for `Vec`; its
canary is parked under `tests/omega/pending/borrow/vec_view_invalidated_by_push`
until the `Vec` runtime/lowering is ready to exercise it end to end.

## Lifetime Parameters

Omega uses Rust-style lifetime parameters: a call's
output may borrow an input, and LIFETIME PARAMETERS — declared in the same
`<>` list as type and `const` parameters, tick spelling — say which:

```omega
machine header<'buf>(buffer: &'buf [u8], scratch: &mut [u8]) -> &'buf [u8] {
    // the returned view aliases `buffer`; the checker extends buffer's loan
    // for as long as the result lives. `scratch` is unentangled.
}
```

ELISION keeps the common cases annotation-free, exactly as in Rust: a single
ref input means the output borrows it, and a `&self` method's output borrows
self. Most signatures therefore never write a tick:

```omega
machine decode_body(buffer: &[u8]) -> &[u8] { ... }         // borrows buffer
machine Level::find_room(&self, id: CellId) -> &Room { ... } // borrows self
```

Borrow-carrying data is in-model from day one — a type holding views is
generic over the lifetime of what it views, which is what makes zero-copy
decoding spellable:

```omega
data ChatMessage<'buf> {
    sender_id: i64;
    body: &'buf [u8];       // view into the receive buffer; zero bytes copied
}
```

House style: descriptive lifetime names (`'buf`, `'arena`, `'msg`), never
`'a`. The tick was kept deliberately after surveying the alternatives
(argument-naming clauses, keyword region/origin parameters, Mojo-style
bracket origins): it is lexically self-identifying at use sites, collides
with none of Omega's bracket meanings (slices, properties, invariant
parameters), and elision makes it rare.

Implementation is staged. The frontend preserves explicit lifetime tags and
their declaration binders through every semantic tree phase. Binders are
erased regions stored separately from type/const/machine parameters, so they
do not change runtime generic arity or monomorphization; duplicate declarations
and undeclared tags reject. The checker applies the single-reference and
`self` elision rules, rejects ambiguous multi-reference results, and links a
returned view to the one input it names.
Borrow carrying is structural: nested records, active sum payloads, fixed
arrays, constraints, and concrete generic arguments cannot hide an inner
loan, and recursive data is walked cycle-safely. Literal construction records
every carried source; a returned aggregate is valid only when all of those
sources outlive the call, and projecting a named field retains only that
field's loans. Fixed-array literal positions retain exact ordinals too:
projecting a constant index keeps only that element's loans, while a dynamic
index conservatively keeps every candidate element's loans.

Named borrow-carrying data accepts explicit erased lifetime applications:

```omega
machine select<'left, 'right>(
    first: &'left [u8],
    second: &'right [u8]
) -> ChatMessage<'left> {
    let selected: ChatMessage<'left> =
        ChatMessage { sender_id: 0; body: first };
    transition {
        _ -> selected
    }
}
```

Lifetime arguments precede runtime type/const/machine arguments, validate
against the data declaration's lifetime arity and the lexical owner's declared
binders, and remain separate from runtime generic arity, layout, and
monomorphization identity. A call-produced aggregate governed by one explicit
result lifetime keeps the corresponding input loan active while unrelated
inputs such as `second` remain independently usable. Moving a borrow-carrying
local—or projecting and moving one of its nested fields—transfers the contained
loan paths and their read/mutable polarity to the destination local; ordinary
data assignment cannot erase a borrow. Reassigning an existing local or one of
its aggregate fields follows the same rule: the right-hand side is evaluated
while the old loans remain active, the overwritten field's carried loans end,
and the replacement value's exact field/index loans become active. Replacing
one field neither retains its old source nor releases loans carried by an
unrelated sibling. Dynamic indexes remain conservative.

For an explicitly multi-lifetime result, the checker derives the result
contract structurally from the data declaration:

```omega
data Pair<'left, 'right> {
    left: &'left mut i32;
    right: &'right mut i32;
}

machine pair<'left, 'right>(
    left: &'left mut i32,
    right: &'right mut i32
) -> Pair<'left, 'right> {
    let result: Pair<'left, 'right> = Pair {
        left: left,
        right: right,
    };
    transition {
        _ -> result
    }
}
```

The mapping follows nested records, sum payloads, fixed arrays, and concrete
generic arguments, preserving each carried field's projection and polarity.
Here `result.left` retains only `left`, while `result.right` retains only
`right`. The same mapping survives when that helper result, or a moved
borrow-carrying local, initializes a field or fixed-array element of another
aggregate: the checker prefixes the inner loan path with the enclosing
field/index path without merging sibling sources or weakening shared versus
mutable polarity. A same-carrier value cast preserves the same carried loans:
explicitly erasing a non-owning qualification cannot erase ownership, its
source place, or its shared/mutable polarity. Borrow representation recasts
remain subject to their separate footprint and overlap judgment. A validated
recast of a whole named value or member retains an ordinary shared or mutable
loan on that exact source place. A recast at an exact literal index into a fixed
byte array may now retain one complete half-open fixed-range loan when its
target is a fact-free primitive, one nonzero closed acyclic tree of nongeneric,
quotient-free, all-relevant fact-free records, or one recursively nonzero
literal fixed array ending in either exact shape, provided the ordinary recast
judgment has proved the whole footprint in bounds. The
primitive-array extent comes from its normalized exactly tiled representation;
record arrays repeat the complete normalized padded record extent under exact
symbol identity. Eligible records may themselves contain recursively literal
array fields ending in the same exact primitive or record shapes. A zero-length
field participates only when its terminal independently qualifies and the
whole record remains nonzero; its element alignment can still induce protected
padding. Fully specialized type plus scalar-integer `const` or exact-replayed
acyclic structured-data `const` instances participate under their exact
synthesized symbol, validated carrier/value origin, and substituted-field
eligibility. Structured atoms are completely decoded under fixed resource
bounds and replayed in declaration order against the exact resolved record or
pure-sum carrier, including the selected case and ordered payload; layout
remains a property only of the substituted instance fields. Runtime or merely
bounded offsets, slices, total zero-size targets, open/unresolved,
mixed/recursive/custom-canonical structured-const, lifetime/machine/proposition
generic instances, and invariant-bearing/erased/cased records remain
conservative. Last-use accounting compares the canonical
field/index path, so a
later use of `result.right` does not artificially keep `result.left`'s loan
active.
Program-static views stored in persistent aggregate fields carry their stable
field, case, and fixed-index identity across named graph states. A runtime index
also crosses when it is an immutable state parameter or immutable local
forwarded unchanged, or through direct immutable local copies, into an
immutable target-state parameter; the edge rebases that shared identity to the
target symbol. A mutable or computed alias, rewriting or omitting the argument,
an inconsistent predecessor, a possibly overlapping mutation, or an opaque
call discards the provenance rather than guessing that two runtime indexes
agree.
General outlives constraints, persistent-storage assignment across state
transitions, and the remaining aggregate expression forms remain
implementation work; they are not new language-design questions.

## Storage Carried By Placed Views

The borrowed form of `Placed<P, T>` carries the exact source borrow from which
placement was admitted. The owned form instead carries a split `Extent` and
must eventually return or release that conserved claim through an authorized
terminal route. Neither form turns special backing into an ordinary `&mut T`.
Normal references remain unchanged:

```omega
machine inspect(uart: &Placed<UartMmio, UartRegisters>);
machine configure(uart: &mut Placed<UartMmio, UartRegisters>);
```

The current borrow of the view and the retained source borrow answer different
questions. `&mut` proves exclusive use of the view value; it does not upgrade a
view created from a shared source borrow. Stable ordinary mutation is legal
only when its `AccessPlan` permits the operation, the current view borrow is
exclusive, and the retained source borrow is exclusive. External and atomic
operations instead follow their exact admitted operation contracts; an Omega
`&mut` borrow cannot exclude a device.

Field projection is pure and preserves the narrowed borrow path. The resulting
accessor cannot outlive its view or name bytes outside its planned field.
Disjoint subrange views may coexist when a validated layout certificate or a
checked interval proof establishes place non-overlap and their physical effect
footprints do not conflict. Logical bitfields sharing one transfer word are not
independently exclusive for destructive reads or read-modify-write. Each child
receives only the parent resource profile restricted to its interval and
attenuated rights.

See [Chapter 20](chapter_20_memory_layout_abi.md#placed-and-externally-mutable-memory)
for placement and access semantics.

## Relationship To Drops

Ownership decides who must clean up a value. The cleanup machinery itself is
covered later in [Drops And Cleanup](chapter_17_drops_and_cleanup.md).

The compiler records first-class `Establish`, `Transfer`, `Consume`, and
`AffineDrop` permission events. The older parallel move/drop summaries have
been deleted; cleanup-plan completion is tracked in
[semantic_taxonomy_representation.md](../architecture/semantic_taxonomy_representation.md).
