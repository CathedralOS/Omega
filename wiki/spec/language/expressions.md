# Expressions and evaluation order

Expressions retain their selected operation identities and source evaluation
schedule through checking, semantic evaluation, and realization. This specifies
language behavior, not the current compiler's supported expression shapes.

## Evaluation schedule

| Form | Evaluation order |
| --- | --- |
| Named machine call | All authored arguments left to right. |
| Strict binary operator | Left operand, then right operand. |
| `left && right` | Left; right only when left is true. |
| `left || right` | Left; right only when left is false. |
| Index | Collection, then index. |
| Assignment `place = value` | Capture the destination place, then evaluate the value, then replace at the captured place. |
| Range | Present start, then present end. |
| Fixed-array literal | Increasing element index. |
| Record or case literal | Authored field-expression order. |
| Transition dispatch | Subject once before dispatch; only the selected arm runs. |
| Value-position `match` | Subject once; first matching arm only. |

Every evaluated child runs exactly once. Unary operations, borrows, value casts,
membership tests, and member access have one immediate runtime child. A
[machine-entry address query](counts_and_addresses.md#address-exposure)
instead selects a static declaration application; it has no runtime operand to
evaluate and does not invoke the machine. The short-circuit and dispatch rows
are the closed selective set for these forms;
a new selective form needs an explicit schedule, not an implicit lazy exception.

The right operand of `&&` may use the left operand's true-path facts; `||` may
use its false-path facts. These assumptions do not escape the expression.
Writes invalidate overlapping facts even within a selected path. An effectful
guard cannot be replayed as a fact about operands it may have changed. See
[live-fact identity](state_contracts.md#mutation-and-subject-identity).

Named fields are evaluated in authored order and installed at their declared
identities. Canonical aggregate identity uses declaration order, while physical
layout determines byte placement; neither changes evaluation order.
[Construction and disposal](ownership.md#construction-and-disposal-order)
distinguishes reverse-establishment cleanup of partial staging from
reverse-declaration cleanup of completed aggregates. Crash routes have no
cleanup successor. Reordering must preserve results, effects, failures, moves,
and cleanup under the permitted observation contract.

## Calls, places, and temporaries

Named decision: `boundary-requirement-static-call-form`, settled 2026-10-14.

A call invokes a machine; an internal state jump transfers within the current
machine. Transition dispatch may also select a final call whose result completes
the enclosing machine. A named machine call selects a static path and supplies
every ordinary parameter explicitly. A qualified declaration such as
`Buffer::clear(buffer: &mut Buffer)` is called as `Buffer::clear(&mut buffer)`,
not `buffer.clear()`. The qualification establishes a namespace and semantic
home, not an implicit receiver. Dot access remains data-field projection; it
does not select or invoke a machine.

A boundary requirement uses the same qualified static form:
`R::operation(arguments)`. Every declared parameter is explicit and appears in
source order; there is no receiver or extra binding argument. The exact closed
requirement application is a static
[provider slot](../build/component_publication.md#provider-slots-and-era-entry).
Checking retains that slot demand, and realization rejects unless provider
planning selects one admitted backing. No ambient source search, hidden value,
or implicit argument insertion repairs missing or ambiguous supply. Calls obey
the exact [suspension/blocking acknowledgement and position
rules](effects.md#call-site-acknowledgements).

Machine invocations require their full ordinary argument list. Application of
a [mathematical function term](../proofs/mathematical_bindings.md#named-mathematical-definitions)
instead consumes a prefix of its nested dependent-function parameters; the
result may be another function term. This grants no implicit machine-to-function
conversion, runtime closure, pending execution or obligation to finish applying
the term. Expected-type and executable-demand checks still apply.

`address_of<Selected<Arguments>>()` exposes a selected executable entry;
`address_of(reference)` exposes the referenced storage location. These are
compiler-known core queries, not casts or equal-size bit reinterpretation.
A qualified machine name does not capture a data instance. See
[address exposure](counts_and_addresses.md#address-exposure). `size_of<T>()`
and `align_of<T>()` query a data layout; `code_layout<Selected<Arguments>>()`
queries late native code regions. Static arguments select types or exact machine
applications without introducing runtime type objects or first-class machines.

Assignment requires writable storage, compatible value type, and the applicable
[invariant-window obligations](dependent_values.md#invariant-windows). It first
evaluates the destination's collections and selectors in their ordinary order,
once, capturing the exact storage location. It then evaluates the right-hand
side and replaces the value at that captured location. Changing a selector or
reference binding during the right-hand side does not redirect the pending
store; no destination expression is reevaluated for cleanup or installation.
Place validity and bounds are established when the destination is captured.
Ordinary loan and ownership rules must keep that backing valid through the
store; capturing a place does not permit the right-hand side to invalidate it.
Capturing identifies storage, not a read or move of its old value.

Right-hand-side effects still invalidate the facts they overlap as they occur.
After evaluating the value, the store invalidates and replaces facts about the
captured destination, as specified by
[mutation and subject identity](state_contracts.md#mutation-and-subject-identity).
A block introduces local scope and cleanup edges, not a new machine or
state. A terminal value completes the invocation. A call-shaped arm resolving
to a local state is an internal transfer; one resolving to a machine is a
[terminal call](state_contracts.md#terminal-calls-in-transition-arms), with
ordinary call, suspension and completion obligations.

Temporary storage must outlive every derived borrow and receive cleanup on
every cleanup-bearing exit. Shared lending may be implicit at a shared-reference
parameter under ordinary argument-admission rules. For named machine calls,
mutable and write-only lending require explicit `&mut` and `&write` expressions.
The token-specific collection adaptation for [indexing](#indexing-and-ranges)
does not change named-call argument admission.
Checked identity and published consumers retain the access kind and
rejoin the explicit argument with its declared parameter type; source spelling
cannot substitute for that identity.

## Operator declarations

Numeric helper names such as `min`, `max`, `abs`, `clamp` and `sqrt` select
[ordinary standard-library machines](numeric_values.md#library-numeric-helpers),
not compiler-known call forms. Their call syntax follows ordinary resolution
and argument evaluation; none is implicitly available by spelling alone.

Equality between type expressions in generic constraints and static branches
uses [static type equality](generics.md#static-type-equality), not runtime value
comparison or user operator discovery. Type/value role resolution happens before
choosing that operation. It produces no type object or runtime metadata lookup;
ordinary `==` on values keeps its selected value-comparison semantics.

Builtin comparisons between fixed integers of different widths or signedness
follow [exact integer equality and order](numeric_values.md#integer-equality-and-order),
not a common-carrier promotion. This does not change operator selection or add
conversions to ordinary machine arguments.

An operator binding is a property of an ordinary named machine declaration,
package-private unless declared `pub machine`. A qualified path does not inherit
its namespace type's visibility. An optional fixed token immediately after
`machine` binds operator syntax to that declaration:

```omega
pub machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 {
    add_vectors(left, right)
}
```

The named helper above is ordinary checked code; delegation adds no binding
mechanism. `machine +`, `machine ==`, `machine []`, and `machine [..]` use the same
declaration grammar with their existing fixed-token arity rules. Without a token,
the declaration is an ordinary named machine. There is no separate `operator`
declaration introducer or second executable identity for a token-bearing machine.

The token vocabulary is closed and compiler-owned: lexical spelling, precedence,
associativity, fixity, allowed arities, and source-position mapping are fixed.
Tokens are not strings or user-defined punctuation. Declaration path plus
normalized signature/overload is canonical operation identity. Checked and
Terminal calls retain that identity; adding, removing, or changing the public
token binding is a breaking source revision. There is no alternate `spelling`
clause.

| Arithmetic tier, tightest first | Forms | Binary associativity |
| --- | --- | --- |
| Grouping and prefix | `(expression)` and prefix unary operators | Not applicable. |
| Multiplicative | `*`, `/`, `%` | Left. |
| Additive | `+`, `-` | Left. |

Grouping is separate from the evaluation schedule. One declaration binds at
most one fixed token. Distinct normalized operand/domain shapes may share a
token; duplicate participating token/shape candidates reject. A second spelling
needs a separate declaration, which may forward to the same implementation.
A machine with no fixed token remains callable by name, including compiler-owned
semantic operations without punctuation bindings.

Overload signatures are structural. Renaming or reordering generic binders
without changing their occurrence relationships creates no new candidate:
`index<T>(&[T], u64) -> T` and `index<U>(&[U], u64) -> U` are duplicates, as
are `combine<T,U>(T,U)` and `combine<A,B>(B,A)`. Return carrier or predicate
refinement alone cannot distinguish overloads. Fixed operator syntax is
operand-directed; explicit named calls may additionally use
[result-domain selection](domains.md#result-domain-overloads).

Ordinary parameters occupy normalized operand positions in declaration order,
starting at zero, with their exact ownership/access modes. There is no hidden
receiver operand. Selection uses the complete operand telescope. Being an
operator grants no mutation: mutation requires an admitted mutable operand shape. Checked consumers
retain the authored selection occurrence, not a guess from the leaf name.

Merely occupying operand position zero does not grant receiver status or an
implicit loan. Explicit operands retain ordinary argument-admission
rules and the specified collection-view adaptations; operator syntax adds no
general search for borrowed variants of operands. An owned operand remains
owned. If permitted adaptations leave competing candidates, ordinary ambiguity
rejection applies, not an implicit preference for an owned or borrowed overload.

Named decision: `boundary-trait-token-binder`, settled 2026-10-13.

Trait requirements may bind tokens. Their conformances supply implementations,
never new token bindings. A trait-backed token use requires one exact conformance
already selected by an explicit proof-static binder in the surrounding source
telescope. At generic specialization, that binder substitutes its exact
requirement row before lowering; lowering performs no implementation search.
No binder rejects even when one conformance or Build provider is visible, and
several applicable binders are ambiguous. `Build::select_provider` realizes an
already-selected boundary requirement; it never supplies proof-static evidence,
activates a trait's token, overrides a primitive/direct family, or changes source
resolution. Changing provider configuration therefore cannot change what an
operator token means.

An explicit named requirement call selects other meanings. A concrete type may
publish one canonical direct wrapper for a token/operand shape; a second wrapper
with that shape rejects rather than becoming a selectable alternative. Direct
operators need no conformance selection.

When operator syntax selects a machine, that machine's `requires` is a call
obligation in executable and proof-term formation. Resolution does not discharge
it, and a proof-only caller cannot bypass it. Contextual `==` over one
non-primitive data type in a fact is instead
[proof-relevant identity](../proofs/contracts.md#data-identity-in-facts) and
selects no machine. Ordinary `Nat - Nat` and named `Nat::subtract` still require
the same operand-order evidence; explicit saturating subtraction has its own
total contract. See [call preconditions](machines.md#call-preconditions).

### Executable supply

Operand-directed resolution selects a declaration. Its context fixes the supply
mechanism; any required conformance/provider selection is explicit, never an
implicit search among visible satisfying machines.

| Declaration context | Executable supply |
| --- | --- |
| Ordinary direct `machine + Name(...) { ... }` | Its own checked machine body. |
| Bodyless token-bearing requirement inside an ordinary trait | The explicitly selected implementation, under existing requirement rules. |
| Bodyless machine requirement inside a boundary trait | The existing boundary realization/provider selection mechanism. |
| Exact compiler-owned bodyless machine, with or without a token | Its authorized closed-catalog realization. |

A nonboundary direct machine outside a required-body context or the compiler
catalog must have a checked body. Missing supply rejects at the declaration,
not only when execution is demanded. A separate `satisfies` machine cannot supply
that missing body; satisfaction establishes compatibility with a requirement,
not permission to select an implementation. Do not create a direct-operator
registry, package-level canonical-body search, or build-provider override for
ordinary checked machine bodies.

For a token-bearing requirement in a boundary trait, the token binding identifies
the required operator slot; a realizing boundary machine uses the ordinary exact
`satisfies` relationship without redeclaring the token. A named bodyless boundary
operation is likewise a requirement in a boundary trait, using a one-member trait
when it needs an independently selected provider.
Removing operator punctuation must not silently turn required supply into an
admitted theorem declaration or vice versa.

The body accepts ordinary machine contracts, states, and transitions and obeys
ordinary ownership, effects, termination, evaluation, and cleanup rules. A public
machine may delegate to private helpers without exposing those helpers for
consumer selection. Token calls and named calls retain the same declaration/body
association and exact static arguments. Operand order, once-only evaluation,
conditional-arm invocation, result qualifications, and contract obligations
survive ordinary lowering and independent Terminal replay. No separate operator
interpreter or producer-asserted body association is permitted.

Compiler ownership is exact declaration/signature/semantic-catalog identity, not
a recognized spelling or a package opting in. One declaration has one authorized
supply; a catalog realization and a separate source body cannot compete. A
compiler primitive such as the canonical `Float::meaning32` is automatic and
requires no build selection. It keeps its proof/runtime eligibility restrictions.
Canonical `f32 + f32`, in contrast, is a boundary-trait requirement: target defaults
choose its implementation, with an explicit admitted override available under
the existing provider rules. Build selection cannot redefine its arithmetic
contract. Merely naming a declaration `Float::meaning32` grants no primitive
implementation or proof authority.

### Operator families

A qualified declaration such as `Domain::operation` establishes its semantic
home. An unqualified name may establish the same home through one unique
declared-domain constraint across the operand tuple. Resolution considers the
complete operand tuple's binding qualifications, not incidental flow facts.
Association with a domain grants no authority to establish its membership;
`established by` remains the separate establishment route. Inactive same-carrier
declarations may coexist; competing participating meanings at a use reject.
[Domain semantic roles](domains.md#semantic-roles-and-operators) separately
determine whether their theories compose into one operation meaning.

Operator families are closed by default. An open family designates a dispatch-
owner operand position, with one owner per implementation key. A third party
owning neither operand introduces an explicit adapter domain rather than
injecting an implementation into unrelated carriers. Candidate discovery uses
participating domains, their owning packages, and the declared family, never
an arbitrary scan of imports. Open-family declaration syntax and dispatch-owner
selection syntax remain unsettled; these rules do not approve a spelling.

For a closed direct family, only its semantic-home owner may publish its direct
token bindings. Validate ownership and duplicate participating token/normalized
operand shapes against that owner's complete declaration set, independently of
downstream imports. A declaration in an unauthorized package rejects there;
it does not enter another package's candidate set and wait to collide at a use.
Owning an operand is not a blanket grant to extend somebody else's closed family.
Existing overload ambiguity checking still applies to legitimately participating
families; declaration-owned supply removes body-choice ambiguity, not overloads.

Primitive identities are compiler-owned. The toolchain designates exact core
declaration/package owners for their canonical families, not any package named
`core` or any authored primitive-qualified path. Declaration authority and
compiler-supplied execution are separate: authorized core code may own an
ordinary checked body, whereas bodyless primitive supply requires its catalog
entry. A package owning `Degrees` may define the domain's operations over a
foreign primitive carrier; that does not extend the carrier's unqualified family.

Adding an unrelated dependency cannot change existing resolution, invalidate
its typechecking, or introduce a collision. Resolution is a compile-time choice
retained in the checked artifact; runtime dispatch does not repeat it. At a
replacement boundary, a body changes only under the declaration's contract and
laws. A declaration-surface change requires dependent recompilation, not a
silent runtime rebind. A commutative operand flip may be an explicit delegating
operator; commutativity does not grant automatic candidate reversal.

## Indexing and ranges

Indexing and range slicing select ordinary core operators with visible bounds
contracts; neither is raw pointer syntax. Fixed arrays, vectors, and borrowed
slices may share the syntax without sharing their bounds evidence. Mutable
forms additionally require the appropriate loans and write authority. Descriptor
construction implements the selected contract but cannot establish its proofs.

For a static index machine such as
`machine [] Buffer::at(buffer: &Buffer, index: u64)`, `buffer[index]`
supplies the collection at operand zero and forms the shared loan required by
that parameter. An admitted index machine whose collection parameter is
`&mut Buffer` similarly requires and forms an exclusive collection loan.
This is the existing token-specific collection adaptation for `[]` and `[..]`,
independent of receiver identity; it is not method-call sugar. Authors do not
need `(&buffer)[index]` or a distinct `&[]` operator. Named calls remain ordinary
static calls: `Buffer::at(buffer, index)` follows shared argument admission,
and a mutable collection parameter requires an explicit `&mut buffer` argument.
This rule introduces neither write-only collection adaptation nor automatic
borrowing for other operator operands.

Collection borrowing does not copy or move the collection, weaken bounds or loan
checks, or permit mutable access through shared authority. Collection and index
expressions are each evaluated once, collection first, and the resulting loan
retains its exact captured place and lifetime. The index argument keeps its own
declared access and multiplicity; borrowing the collection does not borrow an owned
index argument. Explicit conformance selection and provider requirements are
unchanged.

Index results follow ordinary machine result rules: they may be computed values
or borrowed views. There is no universal reference-return requirement or
implicit dereference of every user-defined index result. Writing through an index
requires the selected operation to supply writable storage under its declared
contract; returning a value does not make it an assignable place. This adds no
result-type-directed operator selection. The same collection-parameter rule
applies to a static range-slicing machine; existing core collection adaptations
remain valid.

### Borrowed range access

For core collections, a bare range `items[start..end]` forms a shared view
when received as a reference. Creating a mutable subrange requires explicit
`&mut items[start..end]`. The source must already permit exclusive access;
the spelling cannot grant mutation through a shared ancestor. This rule,
`bare-range-reference-access`, applies uniformly to local initializers, call
arguments, state transfers, returned values and constructed fields. An expected
`&mut [T]` type never inserts the mutable borrow, including on a local declaration.

```omega
let shared: &[u8] = view[2..4];
let writable: &mut [u8] = &mut view[2..4];
// Rejected: let writable: &mut [u8] = view[2..4];
```

The examples are alternative borrows; simultaneous overlapping loans still
obey ordinary compatibility rules. The result retains its exact backing,
extent, access and lifetime. This adds no result-type-directed operator
selection and does not change the declared result of an ordinary selected
machine. Collection-parameter adaptation above is distinct from creating a
mutable result view.

An existing mutable reference may still be forwarded under ordinary argument,
move and reborrow rules: `process(writable)` need not spell `&mut` again merely
because the parameter expects `&mut [u8]`. The explicit-new-loan rule does not
copy an exclusive reference or extend its lifetime. Bounds and loan obligations
are unchanged, including for empty ranges.

### Range endpoints

`a..b` is exclusive; `a..=b` is inclusive. Open forms are `a..`, `..b`, and `..`.
Omitted endpoints use zero and collection length. Inclusive ranges normalize
to `a..(b+1)`: exclusive end validity is `b <= len`, inclusive end validity is
`b < len`, exactly validity of index `b`. An inclusive nonempty range also
establishes nonemptiness. An invalid or overflowing endpoint calculation is a
proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an
inclusive maximum must not silently wrap its exclusive successor.

Range formation also proves start/end ordering and the independent lower bounds
in [collection bounds](numeric_values.md#collection-bounds). Borrow conflicts
use normalized extents: a live `items[1..]` need not conflict with `items[0]`,
but conflicts with overlapping access to `items[1]`. Dynamic extents remain
conservative without disjointness evidence. Byte indexing of UTF-8 is not
character or grapheme indexing; those require separately named operations.
