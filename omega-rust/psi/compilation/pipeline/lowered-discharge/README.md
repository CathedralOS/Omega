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
it, from the stores [places/log.rs](src/places/log.rs) derives from the
operations; [prove.rs](src/prove.rs) is the one prover.
[joins.rs](src/joins.rs) infers and checks join predicates;
[crashes.rs](src/crashes.rs) states crash-site coverage.

## What it decides

| Kind | Goal | Hypotheses available at its point |
| --- | --- | --- |
| `OperationSafety` | Terminal semantics' canonical goal for the operation (`proof_bearing_scalar_leaf_semantics`): representability of an exact add, subtract or multiply, a defined division or remainder, a shift count in range | entry `requires`, result equations of the operations before it, callee `ensures` after earlier calls |
| `CallRequirement` | the callee's `requires` clause at that position, its scalar formals replaced by the call's scalar actuals and each field of a record formal by the one value that field of the record actual holds at the call | the same, before the call |
| `NormalGuarantee` | this machine's `ensures` clause at that position, with `result` replaced by the value that exit returns | the same, on the exit edge |
| `DataDomain` | one clause of a data type's `requires`, with `self` replaced by the values the place holds once the owing operation completes ([Data `requires`](#data-requires)) | the same, before the operation |
| `PublishedGuarantee` | not proved: held when every return edge's `NormalGuarantee` for the clause is discharged | - |
| `ArrivalInvariant` | the destination join's inferred predicate, its parameters replaced by this edge's arguments (`Truth` when the join has none) | the same, on the arrival edge |
| `CrashCoverage` | the site guard's clauses (a `Crash` terminator's; a call site has none) and the disjunction of this machine's published alternatives for the cause (`Falsehood` when it publishes none) | the same, before the crashing terminator or call |

Any other kind is refused as `Unsupported`; elaboration mints none yet.

Beside the obligations it makes two structural judgments, which share one
verdict per operation and terminator that touches storage
(`MachineDischarge::uses`). Initialization: a use that some path reaches
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
callee `ensures` clause (`CalleeGuarantee`, after the call, with the
callee's `result` replaced by the value the call defines;
[calls and outcomes](../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#call-contracts):
"Normal guarantees refer to the actual result and appropriate parameter
versions."),
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
| `CrashCoverage` | a `Crash` terminator owes one for its cause; an `InvokeNoNormal` terminator, or a `Call` or `CallUnit` with crash continuations, one per cause among them | the point before the terminator or the call |
| `ArrivalInvariant` | a `Jump`, or a `Conditional` successor, that passes scalar arguments owes one for its edge | that edge |

A row of the first four kinds, or of the last two, that nothing owes is refused as well, and so is
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
accepted on it.

### The tables that say what storage holds

A goal over a field is stated over the value its leaf holds, so a table that
names another value, another place or another kind of value changes what is
proved without touching an obligation row. Three tables say what storage
holds, and none is taken on elaboration's word.

**The write log is derived, then compared.** The walk below does not read
`MachineTables::writes`. [places/log.rs](src/places/log.rs) derives every
store from the operation that makes it (`Storage::stores`):

| Operation | What it stores |
| --- | --- |
| `EstablishRecord` | each operand into its field of the fresh root: a scalar operand as that value, a record operand as what its source held |
| `EstablishScalarCase`, `EstablishStructuralCase` | the fresh root established as `result_case`, then each payload operand as above |
| `EstablishZeroedStructural` | the fresh root zeroed, then a zero constant into each scalar leaf of the zero value that has one |
| `StructuralLeafCopy` | what its source held, into the fresh root |
| `StructuralScalarFieldStore`, `WriteOnlyPrimitiveStore` | its value into exactly the leaf it names |
| `EstablishPrimitiveLocal` | its operand, the zero a scalar local is constructed with, into the fresh home |
| `StoreStructuralField` | what its source held, into the field it names |
| a call with a record result | what the callee returned, into the fresh root |
| a read, a case test, an operation over scalar values alone | nothing |

Any other operation is refused as `Unsupported`: it may store somewhere, and
an operation left out of the derivation would be a store nothing reaches. The
log elaboration wrote must then list exactly these stores, per operation and
in order (`Storage::log_is_stated`). A row with another value or place, a
missing row, a row at another point and a row no operation makes are each
refused.

A zero establishment has no operand, so the zero of a leaf has a value
identity only through a constant. Elaboration emits one zero constant per
scalar leaf directly before the establishment, in leaf order, and that slot
is where the derivation looks. A leaf whose slot holds anything else is zero
with no value identity.

**The place table must be one place per path** (`places_are_interned`). A
table names a place by its identity: an invocation's record actual, the place
of an entry content value, a write row. The walk reads that identity as a
path, so each root's row must be its own, each projection must be listed by
its parent under its own segment, a parent must come before its projections,
and a parent's projections must be sorted with one per segment. Two paths
that share a row would let one field's contents answer for another's.

**The value table must be the lowered form's declarations**
(`values_are_declared` in [minted.rs](src/minted.rs)). Each formal, the scalar
result, each block parameter and each scalar operation result must have its
declared type and the origin of what declares it. The type is what Terminal
semantics states a goal over and what the kernel bounds a value by; the
origin is what the payload judgment reads to learn which test a condition is
the result of. The one row the lowered form does not declare is the content a
formal's leaf held at entry. Such a value must be of a scalar leaf of a
formal, with that leaf's declared type, and a place has at most one
(`Storage::entry_contents_are_of_formals`). Exchanging two entry content
values of one type renames them and proves nothing, so it is not refused.

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
- **Reaching.** Walking the graph back from the use, the last store into an
  overlapping place on each path is the one that reaches along it.
  - A store of the value `v` into exactly the leaf supplies `v`.
  - A store into the leaf or above it that moves storage whole (a whole
    copy, a record operand of a literal, a record stored into a field)
    supplies what the matching leaf of its source held just before it, and
    the walk continues there.
  - A call's record result supplies what the callee returned, which has no
    value identity here; any other overlapping store supplies contents with
    no value identity.
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
(`local-without-initializer`), so in an elaborated program the path that
supplies nothing is the zeroed storage of a gated type, described next. The
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
`Discharged::Gated`, which rejects nothing by itself, and the walk answers
`Unestablished` for whatever that declaration's rows supply, so every read,
copy, argument and return they reach is refused. A store into a leaf
supplies that leaf again.

A scalar local declared without an initializer is zero-constructed by the
same rule: elaboration logs the zero constant on its home at
`EstablishPrimitiveLocal`, so `let mut x: u32; f(x)` reads a value that is
provably zero, as the current checker accepts. Every scalar type this route
serves (`i8`..`u64`, `bool`) admits zero, and a qualified or ranged scalar
type is refused by the reader as `type constrained`, so a scalar home owes
no domain clause and no gated scalar occurs in this subset.

It is a structural judgment over the stores and the graph. It is not a
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

- **A field** becomes the one value the stores say that leaf holds
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
old field contents."). The stored value enters the goal from the store the
operation makes, not from a knowledge row, so the completed write contributes no fact to
its own proof. A field with no value identity at that point (what a call
returned, a join of two different stores) or a case no structure decides
leaves the clause unstated: the obligation is `Unproved`, with a detail that
says which, never dropped and never assumed.

A store whose written value is shown to keep its clauses discharges them
there. One that is not shown to keep a clause would open an invariant window,
which this route does not have yet: its verdict is `Unsupported`, so the
whole closure goes back to the current checker and is never rejected for it
(finding 12).

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
6. **No store is on an edge.** Every store is derived from an operation and
   takes effect at its normal continuation, and the backward walk moves
   between blocks. A write row at `ProgramPoint::Edge` or at a block entry
   is one no operation makes and is refused. The slice that stores on an
   edge (a return into the result place, a transfer) has to give the walk
   edges first.
7. **The write log cannot say what the walk needs.** `Content` has no form
   for a source place, for the case a construction established or for a
   call's result, so those stores are all `Overwritten` in the log. The walk
   reads the derived store (`Stored`), which says each of them, and the log
   is only compared with it.
8. **A scalar local with no initializer holds zero, here as in the current
   checker.** `let mut x: u32; f(x)` is accepted with `x == 0` known
   (`local-without-initializer`). The establishing operation names the
   zero as its operand, so the home's store is derived from it like any
   other, and elaboration logs the same row. A scalar
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

11. **A gated declaration has a verdict that is neither a discharge nor a
    refusal.** `Discharged::Gated` holds why the zero value's clause was not
    proved. It exists because the specification puts the refusal at the
    observation, and an `Unproved` verdict at the declaration would reject
    `let mut g: Gated; g = Gated { .. };`, which the current checker
    accepts. One over-refusal remains: after `g.a = 3;` on a gated two-field
    type, the leaf no store reached is still supplied by the gated
    declaration, so a use of `g` is `Unestablished` even when the clauses
    hold of `(3, 0)`.
12. **Invariant windows are not built yet.** A store may break a clause
    and a later store restore it before the value is consumed
    (dependent_values.md, "A write known to preserve the default domain
    keeps it established. Otherwise the write opens a window on the place
    ... The domain must be proved again at the next consumption point").
    This route proves a store's clauses where it can and refuses the
    closure as `Unsupported` where it cannot, so such a program is judged
    by the current checker. The sentence "Terminal Psi opens no invariant
    window on a field" in structural_access.md is about predicate-bearing
    byte-view fields and does not govern data `requires`. The conserved
    `DomainDebt` obligation is where a window will live.
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
    established. It is the derived store of an operation that is a call of
    the module with a record result.
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

17. **A data type's `requires` clauses are still taken on elaboration's
    word.** Terminal Psi's type declaration has no slot for them, so they
    ride beside the module in `Elaboration::domains` and nothing in the
    lowered form restates them. A clause weakened or dropped there weakens
    or drops the `DataDomain` goal of every construction and store of that
    type, and `owed_are_minted` counts against the same list. A machine's
    `requires` and `ensures` are not in this position: they are in the
    module's `MachineContract`. Closing it needs the clauses in the module,
    or the typed trees as a second source at this check.

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
  edge that some path reaches with nothing stored into what it reads, with
  its source site. `reject_diagnostic()` gives `unestablished storage:
  <detail>`. One such use rejects the program even when every unproved
  obligation would only be `Incomplete`.
- `Rejected`: one `UnprovedObligation` per obligation that was not, each with
  its kind, its source site from elaboration's correspondence, and its cause
  (`UnprovedCause`). `reject_diagnostic()` turns it into the program error,
  `unproved obligation: <kind>: <cause> (<detail>)`, an ordinary `Reject` with
  no artifact. It says what was not proved and never that the obligation is
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
obligation and one per unestablished use of storage; a program with such a
use is `route=rejected`. `discharge_with(_, Knowledge::WithoutBranchConditions)` exists only
so that comparison can count what branch facts buy; the checker uses
`discharge`.

## Reusing a saved proof (experiment)

[reuse.rs](src/reuse.rs) is an experiment the owner asked to see before
anything is built on it; no production path calls it. `reverify` takes a saved
certificate for one obligation and has the kernel decide it for the goal and
hypotheses the consumer computes from its own lowered program, through the same
`verify` as `discharge`. It runs no search and cannot reach the prover. A stale
proof, a missing assumption, or a proof attached to another obligation or
machine fails because the kernel checks the certificate's conclusion against
the consumer's goal and each citation against the consumer's own hypotheses.
`reverify_at_call` moves a guarantee proved over a callee's formals to one
call by substituting the actuals into the guarantee, the `requires` and the
certificate; the call's own requirements must already be discharged there.
That is substitution of actuals for formals, not type-parameter
instantiation. Certificates have no byte form yet, so a saved proof lives in
memory.

## Generic bodies (experiment)

A generic machine is proved once, against the contracts its parameters
require, and each specialization of it re-checks those proofs instead of
searching again. This section covers one kind of parameter, an
implementation parameter such as `Selected satisfies Step`, whose body calls
`Selected::apply(..)`. No production path calls any of it. Type and value
parameters are not served; [what is missing](#what-the-lowered-form-does-not-carry)
says why.

The example the tests and the corpus fixture use:

```omega
trait Step {
    machine apply(value: u32) -> u32 requires value <= 10 ensures result <= 20;
}
machine advance<Selected satisfies Step>(start: u32) -> u32
requires start <= 10
ensures result <= 21
{ Selected::apply(start) + 1 }
```

In the tests `Double::apply` publishes the requirement's own contract and
returns `value + value`, and `Shift::apply` asks less and promises more
(`requires value <= 12 ensures result <= 17`) and returns `value + 5`. The
corpus fixture selects `Shift` and `Hold`, which returns `value` and promises
`result <= 10`, so that both of its uses satisfy a bound the requirement
does not give.

**The generic check form.** Elaboration lowers `advance` once with
`Selected` rigid ([structure-elaboration](../structure-elaboration/README.md#generic-machines)):
the requirement `Step::apply` is a machine of the module that has its
formals, result and contract and no block, and the call through the
parameter is an ordinary direct call of it. Nothing else is new. The call's
`CallRequirement` obligations are the requirement's `requires`, and the
knowledge after the call is the requirement's `ensures`, by the same rows as
for any callee. `discharge` skips the requirement itself: its guarantees are
each implementation's to prove, and their verdict is `Unsupported`, so `judge`
never accepts a generic form as a program.

**Refinement.** That an implementation may stand for the requirement is a
proof too. Its statement is a conformance witness: a machine with the
requirement's contract whose body is one call of the implementation. Its call
requirements say the requirement's `requires` imply the implementation's; its
guarantees say the implementation's `ensures` imply the requirement's.
`discharge` proves it like any machine, once per implementation.

**Carrying.** `reverify_specialization` takes the generic body's verdicts,
one witness per selected implementation, and the concrete module in which the
specialization calls the implementation directly. For each obligation of the
specialization it computes the goal and the hypotheses from that module, as
`reverify` does, and offers the kernel a proof assembled from the saved ones:

- a value keeps its identity, because selecting an implementation changes
  only which machine a call names (the function checks that the two
  machines have the same values and calls, and refuses otherwise);
- a hypothesis the generic proof cites becomes the specialization's row with
  the same justification;
- where the generic proof cites the requirement's guarantee after a call, the
  specialization does not assume that: it assumes the implementation's
  published guarantee. The citation is replaced by the witness's proof of the
  requirement's clause, with the witness's formals renamed to the call's
  actuals and its result to the call's result;
- where the witness's proof cites the requirement's `requires`, the generic
  body proved that at the call, and that proof is carried in its place;
- the specialization's own call requirement is the implementation's
  `requires`, and its proof is the witness's, moved the same way.

The kernel decides the assembled proof through `verify`, the one acceptance
path. Nothing about it is trusted, and nothing searches: `reuse.rs` cannot
reach the prover, and the tests count prover starts.

**What the tests show** (`tests.rs`, `generic_bodies`):

| Test | Shows |
| --- | --- |
| `the_generic_body_is_checked_against_the_requirement_alone` | three certificates for `advance` from `Step::apply`'s contract; the requirement has no block; `judge` falls back on a generic form |
| `its_certificates_reverify_in_two_different_implementations` | the same three proofs accepted in `advance<Double>` and `advance<Shift>`, no prover started, although the two specializations assume different things after the call |
| `a_body_that_only_holds_for_the_implementation_in_use_is_rejected` | `advance` promising `result <= 18` is proved for `advance<Shift>` alone and refused as a generic body; the specialization then has nothing to carry and is refused without a search |
| `an_implementation_that_does_not_refine_the_requirement_is_refused` | `Wide` (`ensures result <= 25`) has no refinement proof, so the proofs that rest on the guarantee are refused |
| `a_refinement_proof_for_another_implementation_is_refused_by_the_kernel` | `Shift`'s witness offered for a specialization that calls `Wide` is refused by the kernel, three times |

`compiler`'s example `generic_certificate_carry` runs the same three steps
from source, on the typed trees of
`tests/omega/run/generics/implementation_parameter_contract` and its fail
twin. The twin differs only in the bound `advance` promises, `result <= 18`:
a search over each of its two specializations alone proves that bound, and
the generic check refuses it.

### What the lowered form does not carry

Read from the types, not built:

- **A value of a rigid type.** A value row has a `ScalarType`, which is
  `Boolean`, `Integer(sign, width)` or `IeeeFloat(format)`, and a place has a
  declared `StructuralTypeId`. There is no row for a value whose type is a
  parameter, so a body over `T` cannot be lowered once.
- **An operation on it.** Terminal operations are selected per type
  (`ExactIntegerAdd` holds its `IntegerType`), and the safety goal of each
  comes from that type's range. One generic body has no single operation.
- **A certificate that holds for every type.** `PropositionContext` takes a
  concrete type per value, terms and rules name `ScalarType`, and the
  integer rules depend on the width. Moving a certificate to another type
  would have to rebuild it, and the correspondence of values and calls this
  experiment relies on would no longer hold.
- **A rigid value parameter** (`const N: u64`) has the same shape as a
  formal with no argument; it was not tried.

Limits of what was built:

- Refinement is proved for `requires` and `ensures` only. A requirement that
  leaves `crashes` to its member publishes no route in the generic form, so
  the generic check decides nothing about crashes through that call; each
  specialization's concrete module still reads the member's own routes.
- The witness must be one call that passes its formals on, and its proofs may
  cite only the two contracts.
- Carrying is per machine. The route cannot yet judge the whole closure of
  the fixture (its `main` ends in terminal calls), and `judge` does not use
  carried proofs.
- No timing was taken.

## Facts across a join

A branch fact holds on its edge only, and "A fact from one arm does not
survive the join" (`fact_validity.md`). So after `transition total > 9000 ||
total < 3`, the false edge says only that the `||` join's Boolean parameter
`p` is `false`, and a use of `total <= 9000` there learned nothing. The
specification's mechanism is a block predicate checked on every arrival
before it justifies anything downstream (`fact_validity.md`, "Premise
availability"; `control_flow.md`, "Equality relations"). [joins.rs](src/joins.rs)
builds one per join, so no shape of join is special:

- Lowering mints an `ArrivalInvariant` obligation per edge into a block with
  parameters.
- The predicate is inferred as the disjunction over arrivals of what each
  arrival knows and the join does not, in the predicate's scope (formals and
  the join's parameters): `p == a` with the argument `a` unfolded through the
  Boolean and literal definitions on that edge, and each path fact
  transported into its denotation. For the example: `(p == (9000 < total)
  and 9000 < total) or (p == (total < 3) and total <= 9000)`.
- Each arrival obligation is that predicate with `p` replaced by the edge's
  argument, proved by the same prover and decided by the kernel. Only then is
  the predicate a `BlockInvariant` knowledge row at the join's entry. If any
  arrival is unproved, the predicate becomes `Truth` and the join is listed in
  `MachineDischarge::weakened_joins`; a weaker predicate is still a valid one,
  so this never rejects a program, but nothing downstream relies on the join.
- At the use, the prover splits over the predicate's disjunction. In each
  alternative `p == (...)` is a definition of `p`, so the branch fact `p ==
  false` transports through it (`ValueEqualityTransport`, the existing rule)
  into `total <= 9000`.

The prover's transport now takes any proved definition of a Boolean value,
not only an operation's result equation, and a goal whose transported
denotation is `Truth` or a known fact closes by the same rule. A join on a
cycle gets no predicate: cyclic cuts need induction, which this route does not
do.
