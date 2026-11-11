# Discharge on the lowered form

This crate decides the obligations that
[structure-only elaboration](../structure-elaboration/README.md) mints, and it
accepts one only when the proof kernel accepted a certificate for it. It is
the third piece of
[check on the lowered form](../../../../../wiki/drafts/designs/check_on_lowered_form.md):
every other outcome is a `Refusal` with reason `Unproved`. There is no
interval verdict, no admitted route and no assumption, so a discharged
obligation is one that [stage trust](../../../../../wiki/drafts/designs/stage_trust.md)
rule 2 lets remove a runtime check. It is not wired into the default compile;
[the route's verdict](#the-routes-verdict) is what a router will take from it.

Start at [lib.rs](src/lib.rs) (`discharge`). [minted.rs](src/minted.rs)
checks that the rows to decide are the ones the lowered form owes;
[graph.rs](src/graph.rs) answers dominance between program points;
[places.rs](src/places.rs) says what a place holds when a field read observes
it; [prove.rs](src/prove.rs) is the one prover.

## What it decides

| Kind | Goal | Hypotheses available at its point |
| --- | --- | --- |
| `OperationSafety` | Terminal semantics' canonical goal for the operation (`proof_bearing_scalar_leaf_semantics`): representability of an exact add, subtract or multiply, a defined division or remainder, a shift count in range | entry `requires`, result equations of the operations before it, callee `ensures` after earlier calls |
| `CallRequirement` | the callee's `requires` clause at that position, its scalar formals replaced by the call's scalar actuals and each field of a record formal by the one value that field of the record actual holds at the call | the same, before the call |
| `NormalGuarantee` | this machine's `ensures` clause at that position | the same, on the exit edge |
| `DataDomain` | one clause of a data type's `requires`, with `self` replaced by the values the place holds once the owing operation completes ([Data `requires`](#data-requires)); when a store or a zero establishment leaves it unproved, the same clause over what the place holds at each consumption point the [invariant window](#invariant-windows) reaches open | the same, before the operation or the consumption point |

Any other kind is refused as `Unsupported`; elaboration mints none yet.

Beside the obligations it makes two structural judgments, which share one
verdict per operation and terminator that touches storage
(`MachineDischarge::uses`), and it follows each invariant window to the
points that consume it (`MachineDischarge::windows`). Initialization: a use that some path reaches
with nothing established is refused. Payload access: a case payload named where
its case is not established on every path is refused, and so is a dispatch
with no edge for a declared case. All three are refused with reason
`Unestablished`, a compile error of the program like `Unproved`, not a
construct outside the subset; see [Initialization](#initialization) and
[Payload access](#payload-access).

Hypotheses come only from the lowered form and the tables. `discharge` fills
each machine's `knowledge` table with one row per entry `requires` clause
(`EntryRequirement`, at the entry block; a clause over a field of a record
formal is stated over that field's entry content value, and states nothing
when no operation names the field), per operation result equation
(`OperationSemantics`, on its normal successor), per field read whose place
holds one value on every path to it (`OperationSemantics` of the read; see
below), per `requires` clause of a data type that a read of established
storage can state on its own value (`OperationSemantics` of the read; see
[What a read learns from its type](#what-a-read-learns-from-its-type)), per
callee `ensures` clause (`CalleeGuarantee`, after the call),
per edge of a conditional (`BranchCondition`: the condition is `true` on the
true edge and `false` on the false edge) and per payload parameter a dispatch
edge binds (`DispatchBinding` of that edge: the parameter equals the one
value its payload leaf holds, when there is one). Each row's proposition is in `MachineTables::propositions`. A row is passed to the kernel for
an obligation only when its point dominates the obligation's point.
Requires and guarantees go in as assumptions, result equations as semantic
axioms, which is how the kernel's definition rules cite them.

## The rows are checked against the lowered form

A certificate for every row proves nothing about a row that is missing, that
names the wrong subject or that sits at the wrong point. So before anything
is decided, `follow_the_lowered_form` ([minted.rs](src/minted.rs)) recomputes
what each machine owes from the lowered form alone and refuses tables that
differ. Nothing is taken from elaboration's word, because the lowered form
already carries every identity:

| Row | What the lowered form says | Must be created at |
| --- | --- | --- |
| `OperationSafety` | each operation Terminal semantics gives a safety goal names its obligation | the point before the operation |
| `CallRequirement` | each call lists one obligation per `requires` clause of its callee, in clause order; the callee's contract gives the count | the point before the call |
| `PublishedGuarantee` | each `ensures` clause of the contract names its published identity | the entry block |
| `NormalGuarantee` | every return terminator owes one per `ensures` clause, naming its own edge and the clause's published identity | that return edge |
| `DataDomain` | counted by `Storage::owed_are_minted` from the writes the place walk finds | the point before the operation |

A row of the first four kinds that nothing owes is refused as well, and so is
a row owed twice. The point matters as much as the kind: it selects the
knowledge a proof may use, so a row moved past a guard, or past its own call
where the callee's `ensures` is already known, would be proved from facts
that do not hold where the obligation sits.

Two more tables feed the proofs and are checked the same way. An invocation
row must restate one call of the machine: its callee, its scalar actuals in
order and its record actuals as the roots the call passes. It is what a call
requirement and a callee guarantee are stated over, so a row for a call the
machine does not make would be a premise nothing established. An edge row
must have the source and destination of its terminator's edge, since
dominance is computed from the edge table.

A disagreement is a defect of the route, not of the program: the refusal has
reason `ForeignIdentity`, which `judge` returns as `Err`, so nothing is
accepted on it. The write log and the place table are not checked this way
yet (finding 17).

## What a place holds

A read, a whole copy, a dispatch's payload parameters and the two structural
judgments all ask what a place holds on every path to a point: which value a
scalar leaf holds, or which case a sum is established as.
[places.rs](src/places.rs) answers both with one walk (`Storage::reaching`)
over the tables and the lowered form. The two questions differ only in which
rows end a path and in what an edge can establish:

- **The place.** A place is its root and the segments from the root down to
  it: fields, and for a payload a `Case` segment followed by a payload
  field. A read names its fields by `StructuralFieldId` and its case by
  `StructuralCaseId`; a Terminal Psi storage path is resolved by the lowered
  form itself, one field against the prefix before it
  (`runtime_structural_field`). Every `Case` segment is checked against the
  declared cases of the exact sum it is applied to.
- **Overlap.** One function answers whether two places may share storage: a
  place overlaps itself, its ancestors and its descendants; two places that
  part by two different record fields are disjoint; any other pair of sibling
  segments overlaps, and places under different roots do not. Later lanes
  extend this function (indices, referents) and add no second relation.
- **Reaching.** Walking the graph back from the use, the last write to an
  overlapping place on each path is the one that reaches along it.
  - `Content::Value(v)` on exactly the leaf supplies `v`.
  - `Content::Overwritten` on the leaf or above it supplies contents with no
    value identity, unless the operation at that row is one that moves
    storage whole: a whole copy, a record operand of a literal, a record
    stored into a field. Then the leaf holds what the matching leaf of the
    operation's source held just before it, and the walk continues there.
    The write log cannot say this (its `Content` has no "copied from"
    variant), so the source is read from the operation.
  - A path that reaches the machine's entry with no write is supplied by the
    place's entry content when its root is a formal, and by nothing
    otherwise.

- **Leaves.** The leaves beneath a place are its scalars and its sums. A
  sum is a leaf itself, standing for its discriminator, beside the payload
  leaves of every one of its cases: it is established by a whole write,
  whatever case that write selected.

A store inside a block reaches only what that block leads to: a store in a
guarded arm that completes the machine reaches neither a read nor a copy in
the continuation, and a store before the branch reaches both sides.

### The read equation

A read produces a fresh value. It gets the row `read == v` when `v` is the
one value its place holds on every path to it
([fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md):
"A primitive read may introduce an equality between its copied SSA result
and an exact reaching stored SSA value. Every actual predecessor must supply
that same value"). Different values on different arrivals, or contents with
no value identity, give none: the read is still well formed and nothing is
known about it. A store never removes a row, so what was known about the
value a leaf held before the store stays known.

### Initialization

Every operation and terminator that reads storage is one use: a field read,
a read of a scalar home, a whole copy, a record operand, a record stored
into a field, a call's record arguments, a record return. Each scalar leaf
at or beneath the place it reads must be supplied with an established value
on every path: by a store into the leaf, by a whole write to a place above
it, by a formal's entry content, or by the zero its declaration established.
If some path supplies nothing, the use's verdict is a refusal with reason
`Unestablished`, concerning that operation or edge.

A local declared without an initializer is zero-constructed
(`local-without-initializer`), so in an elaborated program every path
supplies something: the zeros of the declaration, which are a value unless
the type is gated, described next. The
sentence "An unestablished local cannot be read"
([structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary))
governs declared shared view locals, which the references lane owns and this
route refuses; it does not govern a plain scalar or data local.

<a id="zero-establishment"></a>
A data local declared without an initializer is zero-established: its
declaration is a whole write that supplies every leaf, each scalar leaf of
the zero value with a zero constant the read equation can name, and a sum
with its first declared case. The declaration owes the `requires` clauses of
the zero value as `DataDomain` obligations. When one is not proved, the type
is gated
([dependent values](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization):
"If zero satisfies the default domain, the value is zero-constructible.
Otherwise the type is gated: construction or checked qualification must
establish the domain before observation."). The refusal then sits at the
observation, not at the declaration, because "Zeroing storage is permitted
independently of establishing a value.": the clause's verdict is
`Discharged::Window`, which rejects nothing by itself, and the declaration
opens an initial-establishment window for that clause on the place
([Invariant windows](#invariant-windows)). A store that proves the clause
closes it, so `let mut g: Both; g.a = 3; g.b = 2; take(g)` is accepted when
each store proves its field's clause. A use that observes a leaf the
declaration zeroed while some clause's window is still open is
`Unestablished`, with the declaration named beside it
(`UnestablishedUse::declared`): `let mut g: Both; g.a = 3; take(g)` is
refused at `take(g)`, because `g.b` is still the zero `1 <= b` fails on. A
read of a leaf a store wrote observes that store, not zeroed storage, and is
established as before.

A scalar local declared without an initializer is zero-constructed by the
same rule: elaboration logs the zero constant on its home at
`EstablishPrimitiveLocal`, so `let mut x: u32; f(x)` reads a value that is
provably zero, as the current checker accepts. Every scalar type this route
serves (`i8`..`u64`, `bool`) admits zero, and a qualified or ranged scalar
type is refused by the reader as `type constrained`, so a scalar home owes
no domain clause and no gated scalar occurs in this subset.

It is a structural judgment over the write log and the graph. It is not a
proposition, so it has no obligation row, no certificate and no part in the
prover.

### Data `requires`

<a id="data-requires"></a>
[Data and literals](../../../../../wiki/spec/language/data_and_literals.md):
"Construction and every restoring write prove the owning default-domain
clauses." Elaboration mints one `DataDomain` obligation per clause an
operation owes; `Storage::owed` recomputes which data-typed places those
are, in the same order, and `discharge` refuses a module in which an owed
clause has no row or two.

The goal is the clause under the one substitution (`substitute`), with the
names of the clause standing for what the place holds at the operation's
normal continuation:

- **A field** becomes the one value the write log says that leaf holds
  there, found by the reaching walk: the operand of a construction, the
  zero constant of a zero establishment, the stored value for the written
  field, and for every other field the value that reached the store. For a
  nested store the same walk serves each enclosing type. A record operand or
  a record stored into a field is followed to its source, as for a read.
- **A case test** `self in S::C` becomes `Truth` or `Falsehood`: whether the
  walk finds the place established as that case there, which a construction
  or a zero establishment decides by structure. The connective around it is
  simplified: a conjunction with a false conjunct is false, a disjunction
  with a true disjunct is true, and then their other parts are not looked
  at, which is what lets a clause project a payload under the test of its
  case.

So the goal names values and literals only, and it is proved from the
knowledge available before the operation
([structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#complete-record-construction):
"The obligation binds the initializer or replacement SSA value, never the
old field contents."). The stored value enters the goal from the write-log
row, not from a knowledge row, so the completed write contributes no fact to
its own proof. A field with no value identity at that point (what a call
returned, a join of two different stores) or a case no structure decides
leaves the clause unstated: the obligation is `Unproved`, with a detail that
says which, never dropped and never assumed.

A store whose written value is shown to keep its clauses discharges them
there. One that is not shown to keep a clause opens an invariant window,
described next; it is never refused at the store.

### Invariant windows

[Dependent values](../../../../../wiki/spec/language/dependent_values.md#invariant-windows):
"A write known to preserve the default domain keeps it established.
Otherwise the write opens a window on the place, whose actual new contents
remain available to ordinary flow reasoning. The domain must be proved again
at the next consumption point". So `let mut s = Span { low: 1, high: 9 };
s.low = 12; s.high = 20; width(s)` is valid: the first store breaks
`low <= high`, the second restores it, and only then is `s` consumed.

- **Opening.** A store's `DataDomain` clause that is not proved over the
  values the place holds after it, or a zero establishment's clause that
  zero does not satisfy, opens one window: on the exact place of the type
  that owns the clause (`s` for `Span`, `w.span` for a `Span` inside a
  `Wrap`, each enclosing type separately), for that one clause. The
  obligation is the window's identity; its verdict is `Discharged::Window`,
  which keeps why the clause was not proved there. A construction makes a
  value and opens nothing: its unproved clause is still `Unproved`.
- **Preserving.** A store that writes no storage a clause names (the
  places the one substitution meets in the clause, `Storage::untouched`)
  keeps that clause as it was: `Discharged::Preserved`, no window and no
  closing. A store that proves a clause over the place's values is a
  restoring write ("Construction and every restoring write prove the owning
  default-domain clauses"), and closes that clause's window on that place.
  Any other store beneath an open place changes its contents and nothing
  else.
- **Open or closed.** The one reaching walk answers it: going back from a
  point, the last row that is the opening write, a restoring store of the
  same place and clause, or a write that replaces the place whole (to it or
  above it) ends a path. The opening write supplies `Open`, the others
  `Closed`, and a path that reaches the entry `Closed`. A window open on one
  arrival is open at the join.
- **Consumption points.** The list in the specification, mapped to what
  this subset lowers:

  | Specification | Lowered construct |
  | --- | --- |
  | "A read relying on the domain or a dependent coupling." | a whole read of the place or of a place containing it: `StructuralLeafCopy`, a record operand of a literal, a record stored into a field; and a field read beneath the place that takes a clause of its storage's type as a premise (`read_premises` not empty). A field read with no premise, a case test and a dispatch rely on nothing and consume nothing. |
  | "Borrow creation or a call consuming the affected domain." | a call's record argument; this route passes records by value, copied first, so it is that copy, and the call itself (next row). Borrows are refused by elaboration. |
  | "Transfer, return, scope expiration or loan handback ..." | a record passed on a `Jump` edge; and every `Return`, `ReturnUnit`, `ReturnStructural` and `InvokeNoNormal`, on its edge, which ends the scope of every place of the machine. A `Crash` consumes nothing: "A crash may abandon an open window". Loans are refused by elaboration. |
  | "Boundary/capability-carrying calls that may observe or expose the affected storage" | every call operation, for every window open at it. |

  "A call need not close an unrelated domain window when its complete
  access, effect and outcome contracts prove that it cannot observe or
  expose the broken coupling ... Unknown access/effects are treated
  conservatively." The tables hold no callee summary to prove that from
  (`CheckTables::summaries` stays empty), so no call is treated as
  unrelated: `s.low = 12; same(7); s.high = 20; width(s)` is rejected at
  `same(7)`, as the current checker rejects it. Using the exception needs a
  summary term for a callee's access and effects.

  A window a zero establishment opened is consumed by every read under its
  root that still observes a leaf of the window's place the declaration
  zeroed, a field read included, and by no call or exit that does not read
  it (zeroed storage holds no value a callee could observe, and "Failed
  initial establishment instead creates no value"): "A zeroed storage representation can be
  accessed as an established value only after its default-domain
  obligations hold."
- **Proved again.** At each consumption point the window reaches open, the
  clause is stated by the same substitution over what the place holds there
  (`Storage::consumptions`) and decided by the same `decide`, from the
  knowledge dominating the point, under the window's obligation identity.
  One `WindowUse` per (window, consumption point) holds the kernel's
  evidence or the refusal. A consumption on an edge is proved at that edge;
  any other before its operation.
- **Rejected.** An unproved clause at a consumption point rejects the
  program with both sites
  ("An unclosed window rejects with both the opening write or storage cast
  and the failing consumption point."): the `UnprovedObligation` carries the
  opening write's site in `site` and the consumption point in `consumed`,
  and its diagnostic is placed at the consumption. For a window a zero
  establishment opened, the use is `Unestablished` instead and names the
  declaration. Each open window is reported once, at the first consumption
  point in program order that fails; the later ones keep their own
  verdicts.
- **No second window.** A store that does not prove a clause while that
  clause's window on the same place is already open opens no window of its
  own: the window stays open and names the store that opened it, as the
  current checker's "a prior write left ... FALSE" does. Into zeroed
  storage, a store that does not prove its clause keeps its own window,
  because the leaf it wrote no longer holds the zero.

A window lends nothing: no knowledge row is added for it, a read of a leaf
the machine stored gets no type premise (as before), and a read of an
arrived leaf that does take a premise is itself a consumption point, so it
is accepted only where the open clause is proved again. That premise is of
a clause naming only the one leaf it reads, which no store elsewhere can
break.

A window is a structural judgment plus a proof question, not a conserved
obligation row. `ObligationKind::DomainDebt` names its domain by a
`DomainSemanticId`, and a data `requires` clause has none: it lives in
`Elaboration::domains` by type and position. Minting `DomainDebt` would also
need a disposition on every edge, which elaboration does not lay yet (it
refuses any conserved obligation at a no-successor edge) and this crate
does not check. The `DataDomain` row of the opening write already names the
type, the place (through its operation) and the clause, so the debt reuses
its identity.

### What a read learns from its type

[Structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#complete-record-construction):
"A validated field read or selected-case payload copy instantiates that
invariant on its fresh scalar snapshot; other fields and unselected cases
provide no premise. The snapshot survives later writes".

A field read gets one row per `requires` clause of each data type its leaf
lies in, stated on the read's fresh value and justified by the read
(`OperationSemantics`), when two things hold.

- What the leaf holds arrived, on every path, inside a value another machine
  established: a formal's entry content, however it was copied since, or a
  leaf of the root a call returned that this machine never stored over. The
  clause must belong to a type inside that value, on the path from its root
  to the leaf; a type this machine built around it lends nothing
  ([dependent values](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization):
  "no consumption sees the value outside its required domain", and a call
  and a return each consume the value they pass). A leaf this machine wrote
  gets no such row: its `DataDomain` obligation proved the clause of the
  written value where it was written, and the read equation names that
  value. A store that broke the clause therefore lends it to no read.
- The clause names that one leaf and no other, and any case it tests is
  established at the read by structure. A clause that relates two fields is
  no premise on one read.

A clause that cannot be stated adds no row; nothing is assumed in its place.
A payload parameter of a dispatch edge gets no such row (finding 13).

### Payload access

A case's payload overlays its sum's storage, so naming one is sound only
where the sum holds that case.
[Structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary):
"Each `Case` step requires source admission to establish the selected case
for the same root and prefix at the read's observation epoch. A dominating
membership test or dispatch supplies that fact." and "A write to the sum or
any prefix retires these facts".

Every operation and terminator is judged for each `Case` step on the path of
each place it names: a payload read, a copy out of a payload, a store into
one, a case test or dispatch on a sum inside one. The walk asks which case
the sum before that step holds on every path to the use, and the use is
accepted only when the answer is that one case. Going back from the use:

- A write to the sum or to a place above it ends the path. A construction
  (`EstablishScalarCase`, `EstablishStructuralCase`) on exactly the sum
  supplies its `result_case`; a whole copy, a data operand or a data value
  stored into a field continues at the same sum inside its source, before
  the operation; anything else supplies no case. A store beneath the sum
  changes a payload, not the case, and does not end the path.
- An edge of a `StructuralCase` on exactly this sum supplies that edge's
  case. A block several of its edges enter, the wildcard arm, gets none.
- The true edge of a `Conditional` whose condition is the result of a
  `StructuralCaseMembership` on exactly this sum supplies the tested case,
  when the test stands in the branching block and no write to the sum or a
  place above it follows the test there.
- The machine's entry supplies none: which case a formal holds is not known.

So the three accepted shapes are a dominating dispatch edge, a dominating
true edge of a membership test, and a construction that is every reaching
write. The false edge of a test supplies nothing: "Otherwise the case holds
when every other declared case of that sum is refuted" needs proof about the
other cases and is refused here, not guessed. A refusal has reason
`Unestablished` and concerns the access.

A `StructuralCase` terminator is one more use, named by its block. Its
subject must be established, and its roster must list every declared case
of the sum it observes exactly once; a declared case with no edge is
`Unestablished` ([patterns](../../../../../wiki/spec/language/patterns.md):
"Every dispatch must cover the admitted subject. Missing runtime coverage is
a compile error, not an implicit trap or fallthrough."). Any other fault in
a roster (a case of another sum, a case twice, a payload field that is no
scalar field of its case) refuses the module.

### What a dispatch binds

A dispatch delivers the scalar payload fields of the selected case as the
parameters of its arm's block
([control flow](../../../../../wiki/spec/terminal-psi/control_flow.md):
"Selected payload fields become scalar parameters on that case's edge before
cleanup."). Each parameter gets the row a payload read would get: it equals
the one value its payload leaf holds on every path to the dispatch, when
there is one. The row is available on that edge and justified as its
`DispatchBinding`. A payload of a case other than the one the sum was
constructed as resolves to no value, because the last write to storage it
overlays was another case's.

The case itself is not a knowledge row. A dispatch has no Boolean value to
state it over, and knowledge names values only; it is the structural
judgment above and nothing else.

Knowledge names values only. No row states a proposition about a place and
no structural `Proposition` variant is used, so the prover gained no
field-aware path: it sees one more equation between two values.

A call's actuals are replaced into the callee's clauses by position in the
scalar lane: the callee's `k`-th scalar formal by the caller's `k`-th scalar
actual. A record actual is a place and no clause term can name it, because
elaboration refuses a clause that names or projects a record formal.

## The prover

One goal-directed prover serves all three kinds. It carries interval bounds as
proof: each integer value gets a best upper and lower literal bound, each with
a node the kernel checks, from the carrier bound of its type, a hypothesis
bounding it or relating it to another value, or, for an exact add or subtract
result, its operands' bounds through `IntegerExactAddDefinitionBound`. An exact
multiply result has no definition rule and needs none: the prover bounds the
mathematical product with `IntegerAffineBound` (the least or greatest of the
four corner products of the operands' bounds), carries that bound through the
result equation with `ValueEqualityTransport`, and restates it in the carrier
with `IntegerOrderSubstitution`. The result of a divide, a remainder, a shift, a
bitwise and or a bitwise or gets its bound from a one-step endpoint chain
([prove/ranges.rs](src/prove/ranges.rs)): the witness roots at the operand
that is a value, cites the result equation as its only definition and lands
the other operand on the earlier equation that fixes it to a literal, and the
kernel replays the step on the root's bound. This is the range propagation
that [required automatic discharge](../../../../../wiki/spec/proofs/contracts.md#required-automatic-discharge)
asks of every checker. An unsigned remainder is also below a divisor known
positive, whatever the divisor is. A
comparison closes by chaining the two sides' bounds through a closed literal
relation. An arithmetic safety goal closes through `IntegerAffineBound`, whose
image of the operands' bounds the kernel itself computes; an exact left
shift's goal uses the kernel's direct shift form, from the value's bound and
the count's literal or upper bound.

The same prover carries relations between values, with no literal endpoint.
A comparison between two values searches order steps between values:
hypotheses relating two values, and `a < a + d` or `a - d < a` from an exact
result when `0 < d` (`IntegerAddOrder`, `IntegerSubtractOrder`), chained by
order transitivity. An unsigned `a - b` is safe from `b <= a` through the
kernel's correlated unsigned subtract. A goal the direct search misses is
split over a disjunctive fact (a disequality guard transports to `v < k or
k < v`): each alternative proves the goal as a local assumption or
contradicts what is known (`DisjunctionElimination`, contradiction
elimination). Splits do not nest. A goal still unproved is tried last as a
goal on an unreachable path: when an order fact known at the point
contradicts the bounds of its own endpoints (`requires count >= 10` under the
guard `count < 5`), chaining the two gives a closed false relation between
literals, whose denotation is `Falsehood`, and the goal follows
([integer certificates](../../../../../wiki/spec/terminal-psi/integer_certificates.md#elementary-certificate-rules):
"closes an impossible arrival without pruning that arrival").

`verify_obligation` then decides the whole certificate. Its acceptance records
whether the mathematical core judged it as well. Most certificates are
`Judged`; a discreteness step from a transported strict guard (`a < 10` to
`a <= 9`, `0 < a` to `1 <= a`) is accepted by the rule checker and answered
`Refused` by the core. Both count as discharged, and the comparison harness
reports them apart.

### What the integer rules do not reach

Each of these is true in the programs that need it and has no derivation in
the rules of [integer certificates](../../../../../wiki/spec/terminal-psi/integer_certificates.md),
so the obligation is `Unproved`. None is worked around here.

- *A contradiction between two values with no literal.* `low <= high`
  under the guard `high < low` chains to `high < high`. A closed false
  relation has the denotation `Falsehood` only between two literals, so
  nothing follows. Required automatic discharge counts "contradiction of
  an unreachable path" among order atoms as floor
  (`pass/contracts/contradictory_relational_branch_exit`).
- *A quotient or remainder by a divisor that is not a literal.* The
  endpoint chain replays a divide or remainder only by a literal or
  landed divisor. `value / divisor` with `divisor >= 2`, and
  `value % divisor` bounded by the dividend's own bounds, get only what
  the relational unsigned remainder form gives (`result < divisor`)
  (`pass/arithmetic/runtime_full_width_unsigned_quotient_exit`,
  `runtime_bounded_dividend_remainder_exit`).
- *A quotient related to its dividend.* `value / 2 <= value` is a
  relation between two values; the chain maps literal endpoints only
  (`run/contracts/scalar_crash_route_exit`).
- *A bitwise operation of two known operands.* A mask bounds its result
  in `[0, mask]`, but `-9 & 15 == 7` needs the operation evaluated, and
  closed evaluation covers add, subtract, multiply and left shift only.
  Exclusive or has no rule at all
  (`pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit`).
- *A relation through a multiply.* `value * value >= value` for
  `value >= 1`; beyond the floor, which propagates ranges only
  (`pass/arithmetic/nonlinear_square_positive_operand_exit`).

## Why this route and not the arithmetic engine

The existing certificate builders (`proof/src/checker.rs`, module
`certificate`) take typed-tree expression handles and a `ProofPlan`, so they
cannot be driven from Terminal values; only their kernel interface
(`ProofNode`, `verify_obligation`) can. The arithmetic engine
(`validation/src/contract_entailment.rs`, `arithmetic_domains.rs`) decides by
interval and relational search and emits no derivation; making it emit one
the kernel re-decides would mean threading proof terms through it, and its
hypotheses are bound to source shapes. The kernel already has the rules this
subset needs, so the prover emits them directly.

## Findings

1. **Branch facts reach the prover through the kernel.** A `BranchCondition`
   row states `c == true` (or `false`); the prover transports it through the
   result equations of Boolean values and constants into the comparison it
   stands for (`ValueEqualityTransport`, whose conclusion the kernel
   computes with `check_value_equality_denotation`). The mathematical core
   does not cover every certificate this makes: a discreteness step from a
   transported `a < 10` to `a <= 9` is accepted by the rule checker and
   answered `Refused` by the core. `verify_obligation` accepts such a
   certificate; the comparison harness counts it apart from `Judged`.
2. **Knowledge propositions have a table.** `MachineTables::propositions`
   (machine-local, minted by `push`); `KnowledgeRow::proposition` indexes it.
3. **Summaries name no proposition.** `Summary` no longer holds
   `requires` and `result_relation`: the callee's clauses live once in its
   `MachineContract`, identified by machine and position, which is what
   `CallRequirement` and `CalleeGuarantee` use. `CheckTables::summaries`
   stays empty; the discharge reads callee contracts from the module.
4. **Shift-left representability uses the kernel's direct shift form.**
   The evidence is the value's bound and, for the count, the equation that
   fixes it to a literal or else its upper bound (with `0 <= count` when the
   count is signed); the kernel picks the count endpoint by the sign of the
   value's bound (`map_direct_shift_left_bound`).
5. **The read equation needs no kernel rule.** `read == v` is passed to the
   kernel as a semantic axiom, like every operation equation, and the
   existing equality steps carry it: a bound on `v` bounds the read, and two
   reads of one leaf are related through the value they share. An unsigned
   `p.x - p.x` over a formal discharges with a `Judged` certificate.
6. **Edge writes are refused, not placed.** The backward walk moves between
   blocks. A write logged on one edge reaches only that edge's destination,
   so a write at `ProgramPoint::Edge` that overlaps a use refuses the
   machine until the slice that logs one teaches the walk about edges.
7. **A copy's source is not in the write log.** `Content` cannot name a
   source place, so the walk reads it from the operation at the row
   (`StructuralLeafCopy`, a structural operand of `EstablishRecord`, the
   value of `StoreStructuralField`). An `Overwritten` row at any other
   operation keeps its own meaning: contents with no value identity.
8. **A scalar local with no initializer holds zero, here as in the current
   checker.** `let mut x: u32; f(x)` is accepted with `x == 0` known
   (`local-without-initializer`). Elaboration logs the zero at the
   establishing operation, as it does for a data local, and the walk needed
   no change: the row is one more write that supplies the home. A scalar
   type that zero does not satisfy would be gated like a data type, but
   none is served: the reader refuses a constrained scalar type.

9. **A case fact is not knowledge.** The lane plan asks for "the
   membership value on each `StructuralCase` edge, as the existing
   `BranchCondition` row". A `StructuralCase` terminator has no membership
   value, and the only proposition that could state the fact
   (`Proposition::StructuralCaseMembership`) names a place, which this
   route's knowledge never does. The case selected on an edge is therefore
   held by the payload judgment alone. What an edge binds is over values and
   is a row, justified as `Justification::DispatchBinding`.
10. **A payload judgment that source programs do not reach yet.** Stage 02
    leaves a directly authored payload member (`s.at`) with no field symbol,
    so elaboration refuses it as a spelling, and the payload members it does
    read are pattern bindings, which lie under their own dispatch edge.
    The refusing paths of the judgment (a read after a write to the sum, on
    a false edge, of a formal, of another case) are exercised by tests over
    closed bodies, not yet by a corpus program.

11. **A write that opens a window has a verdict that is neither a
    discharge nor a refusal.** `Discharged::Window` holds why the clause
    was not proved at the store or the zero establishment. The
    specification puts the refusal at the consumption, and an `Unproved`
    verdict at the write would reject `let mut g: Gated; g = Gated { .. };`
    and `s.low = 12; s.high = 20;`, which the current checker accepts.
12. **Invariant windows are judged, not handed back.** See
    [Invariant windows](#invariant-windows). The sentence "Terminal Psi
    opens no invariant window on a field" in structural_access.md is about
    predicate-bearing byte-view fields and does not govern data `requires`.
    Two consequences of the reaching walk: a window closed on one arrival
    and open on another is open at the join, and when the two arrivals hold
    different values the clause has no value to be stated over, so it is
    unproved there. Every arm of this subset completes the machine, so the
    only join is a short-circuit `&&`/`||`, where no store can be written;
    no expressible program is over-refused by it today.
13. **A payload parameter carries no clause of its type.** A dispatch edge
    binds a payload as a block parameter with no read operation, and the
    tables have no justification for "the type of the storage an edge bound
    this from". The equation with the value the leaf holds is filed under
    `BranchCondition` (finding 9); the clause is not filed under it too, so
    `match b { Byte { value } -> .. }` on a formal learns nothing of `value`
    from `ByteRead`'s `requires`.
14. **`Reached::Returned` tells a call's result from other unknown
    contents.** The walk could already say "contents with no value
    identity"; a read's type premise needs to know those contents arrived
    established. It is read from the operation at the row, like a copy's
    source, by matching the invocation table.
15. **A requirement over a field of a record actual needs that field to
    hold one named value at the call.** `peek(make())` with
    `requires packet.first == 7` is unproved: what `make` returned has no
    value identity in the caller, and no `ensures` over `result` can supply
    one yet. The current checker rejects the same call when `make` builds
    `first = 8`, by evaluating the callee.
16. **A clause over a field with no value identity is unproved even when it
    is true of every value.** After `let mut s = make(); s.low = 0;`, the
    clause `low <= high` is unproved: `high` has no value there. Stating a
    result leaf as a value needs the callee's `ensures` over `result`, which
    this slice does not serve.

17. **The write log is still taken on elaboration's word.** A read
    equation, a `DataDomain` goal and a requirement over a field of a record
    actual all name the value a `WriteRow` says a leaf holds. Rewriting one
    row's `Content::Value` to another value of the machine turns a rejected
    construction into an accepted one: in `make(v) -> Paired { neighbor: 17,
    value: v }` with `1 <= value` unproved, logging `17` for `value` proves
    the clause over `17`. The lowered operations state the same stores
    (an operand of a construction, the value of a field store), so the log
    can be derived here instead of read, which would also retire finding 7.
    Value rows are in the same position; the one case tried, a formal typed
    narrower than its declaration, was refused by Terminal semantics as
    `Unsupported`.

## Evidence seals

`Discharge::operation_seals` turns each discharged `OperationSafety`
obligation into a `terminal_codec::ObligationEvidenceSeal`: the obligation's
machine and identity, its goal, the hypotheses and the certificate, and a
SHA-256 fingerprint over their canonical encoding. Only that kind becomes an
accepted fact in Omega 00. The caller hands the seals to Omega 00 with
`lower_module_with_evidence`, the only way to attach them.

Omega 00 checks each seal before it fills the fact's
`proof_bundle_fingerprint`: the fingerprint matches the seal's parts, the
operation is in the module and owns the obligation, and the goal Terminal
semantics states for that operation equals the sealed goal. A seal that fails
a check, or names no accepted fact, refuses the unit. Omega 00 does not re-run
the kernel: that the kernel accepted the certificate under those hypotheses,
and that the hypotheses held where the obligation sits, it takes on faith.

Seals are in memory only. Terminal Psi's layout has no place for them yet, so
they are lost on a byte round trip and do not survive a stored artifact: a
module lowered from bytes, including one a cache rebuilt, arrives with
`ObligationEvidence::ReadFromBytes` and zero fingerprints.

## The route's verdict

An obligation the prover cannot discharge is a compile error for the program,
never a slower guarded form
([route one](../../../../../wiki/drafts/designs/check_on_lowered_form.md#route-one-before-anything-else-is-built-on-it)).
`judge` ([verdict.rs](src/verdict.rs)) is the one function a router takes the
route's decision from. It discharges an elaborated closure and returns:

- `Accepted`: every obligation of every machine was discharged by a
  certificate the kernel accepted and every use of storage is established,
  with the operation seals for Omega 00.
- `Rejected`, for a use of storage: one `UnestablishedUse` per operation or
  edge that some path reaches with nothing stored into what it reads, or
  that observes the zeroed storage of a gated type, with its source site
  and, for the latter, the declaration's. `reject_diagnostic()` gives `unestablished storage:
  <detail>`. One such use rejects the program even when every unproved
  obligation would only be `Incomplete`.
- `Rejected`: one `UnprovedObligation` per obligation that was not, each with
  its kind, its source site from elaboration's correspondence, and its cause
  (`UnprovedCause`). `reject_diagnostic()` turns it into the program error,
  `unproved obligation: <kind>: <cause> (<detail>)`, an ordinary `Reject` with
  no artifact. A clause not proved again where an open invariant window is
  consumed is one such entry with `consumed` set: `unproved obligation:
  <kind> inside an open invariant window: a prior write left the clause
  unproved and this use consumes the value before it is proved again:
  <cause> (<detail>)`, placed at the consumption, with
  `opening_diagnostic()` placed at the write. It says what was not proved and never that the obligation is
  false: "Failure to find a proof is not proof of falsehood"
  ([compiler_request.md](../../../../../wiki/spec/build/compiler_request.md#outcomes)),
  and "Unproved source contracts reject in Psi checking"
  ([review.md](../../../../../wiki/spec/packages/review.md)). A published
  clause that fails only because a return's guarantee failed is reported
  once, at that return.
- `Incomplete`: every unproved obligation's goal has a form the prover has no
  rule for (`NoRuleForGoalForm`: an order or equality over non-integers, a
  proposition form it does not reason about). The program
  may be valid and the route cannot judge it, so this is the outcome table's
  "unsupported valid case" (owner ruling on `unproved-goal-form-outcome`). No
  artifact and no fallback either. `Reject` wins when both kinds occur. The
  closed `Incomplete` resource table has no provision for a proof goal form
  yet, so this variant has no `Diagnostic` constructor: a plain error would
  publish it as `Reject`. The prover has no search bound, so no obligation is
  `Incomplete` for exhaustion.
- `FallBack`: elaboration or some obligation was refused as `Unsupported`, so
  Psi 05 and 06 decide the whole closure. This wins over `Unproved`.

There is no variant for a partly proved closure. Omega 00 accepts every
obligation of the module it is given and fills evidence only where a seal
arrives, so a module with an undischarged obligation must never reach it. Any
other refusal is a defect of the route and comes back as `Err`.

No router calls `judge` in the default compile yet: the comparison harness
below is its only caller outside tests, and prints each unproved
obligation's message with its outcome.

## Measuring against the current checker

`omega-rust/omega/compilation/compiler/examples/route_one_comparison.rs` runs
both checkers on corpus programs and prints one line per program, one per
obligation, one per unestablished use of storage and one per consumption
point of an invariant window; a program with such a use is
`route=rejected`. `discharge_with(_, Knowledge::WithoutBranchConditions)` exists only
so that comparison can count what branch facts buy; the checker uses
`discharge`.
