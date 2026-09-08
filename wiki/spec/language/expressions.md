# Expressions and evaluation order

Expressions retain their selected operation identities and source evaluation
schedule through checking, semantic evaluation, and realization. This specifies
language behavior, not the current compiler's supported expression shapes.

## Evaluation schedule

| Form | Evaluation order |
| --- | --- |
| Attached call | Receiver, then authored arguments left to right. |
| Free or path-qualified call | Authored arguments left to right. |
| Strict binary operator | Left operand, then right operand. |
| `left && right` | Left; right only when left is true. |
| `left || right` | Left; right only when left is false. |
| Index | Collection, then index. |
| Range | Present start, then present end. |
| Fixed-array literal | Increasing element index. |
| Record or case literal | Authored field-expression order. |
| Transition dispatch | Subject once before dispatch; only the selected arm runs. |

Every evaluated child runs exactly once. Unary operations, borrows, casts,
membership tests, and member access have one immediate runtime child. The
short-circuit and transition rows are the closed selective set for these forms;
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

A call enters another machine; a transition transfers within the current
machine. A path-qualified call uses a static namespace, not an implicit value
receiver. Calls obey the exact [suspension/blocking acknowledgement and position
rules](effects.md#call-site-acknowledgements).

Assignment requires writable storage, compatible value type, and the applicable
[invariant-window obligations](dependent_values.md#invariant-windows). It
evaluates the right-hand side before invalidating and replacing destination
facts. A block introduces local scope and cleanup edges, not a new machine or
state. A terminal value completes the invocation; a call-shaped transition
target is instead an internal transfer.

Temporary storage must outlive every derived borrow and receive cleanup on
every cleanup-bearing exit. Shared lending may be implicit at a shared-reference
parameter. Mutable and write-only lending require explicit `&mut` and `&write`
expressions. Checked identity and published consumers retain the access kind and
rejoin the explicit argument with its declared parameter type; source spelling
cannot substitute for that identity.

## Operator declarations

An operator is an independently named declaration, package-private unless
declared `pub operator`. A qualified path does not inherit its namespace type's
visibility. A fixed-token binding writes the token after `operator`:

```omega
operator + i32::add(left: i32, right: i32) -> i32;
```

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
An operator with no fixed token remains callable by name.

Overload signatures are structural. Renaming or reordering generic binders
without changing their occurrence relationships creates no new candidate:
`index<T>(&[T], u64) -> T` and `index<U>(&[U], u64) -> U` are duplicates, as
are `combine<T,U>(T,U)` and `combine<A,B>(B,A)`. Return carrier or predicate
refinement alone cannot distinguish overloads. Fixed operator syntax is
operand-directed; explicit named calls may additionally use
[result-domain selection](domains.md#result-domain-overloads).

An attached receiver occupies normalized operand position zero with its exact
ownership/access mode; otherwise the first ordinary parameter occupies zero.
Both forms use the complete operand telescope. Being an operator grants no
mutation: mutation requires an admitted mutable operand shape. Checked consumers
retain the authored selection occurrence, not a guess from the leaf name.

Trait requirements may bind tokens. Their conformances supply implementations,
never new token bindings. A trait-backed token use requires one exact conformance
already selected by an explicit proof-static binder: no binder rejects even
when only one conformance is visible, and several applicable binders are
ambiguous. An explicit named requirement call selects other meanings. A concrete
type may publish one canonical direct wrapper for a token/operand shape; a
second wrapper with that shape rejects. Direct operators need no conformance
selection.

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
