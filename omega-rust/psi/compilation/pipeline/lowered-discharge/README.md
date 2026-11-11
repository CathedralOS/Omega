# Discharge on the lowered form

This crate decides the obligations that
[structure-only elaboration](../structure-elaboration/README.md) mints, and it
accepts one only when the proof kernel accepted a certificate for it. It is
the third piece of
[check on the lowered form](../../../../../wiki/drafts/designs/check_on_lowered_form.md):
every other outcome is a `Refusal` with reason `Unproved`. There is no
interval verdict, no admitted route and no assumption, so a discharged
obligation is one that [stage trust](../../../../../wiki/drafts/designs/stage_trust.md)
rule 2 lets remove a runtime check. It is not wired into the default compile.

Start at [lib.rs](src/lib.rs) (`discharge`). [graph.rs](src/graph.rs) answers
dominance between program points; [places.rs](src/places.rs) says what a
place holds when a field read observes it; [prove.rs](src/prove.rs) is the
one prover.

## What it decides

| Kind | Goal | Hypotheses available at its point |
| --- | --- | --- |
| `OperationSafety` | Terminal semantics' canonical goal for the operation (`proof_bearing_scalar_leaf_semantics`): representability of an exact add, subtract or multiply, a defined division or remainder, a shift count in range | entry `requires`, result equations of the operations before it, callee `ensures` after earlier calls |
| `CallRequirement` | the callee's `requires` clause at that position, its scalar formals replaced by the call's scalar actuals and each field of a record formal by the one value that field of the record actual holds at the call | the same, before the call |
| `NormalGuarantee` | this machine's `ensures` clause at that position | the same, on the exit edge |
| `DataDomain` | one clause of a data type's `requires`, with `self` replaced by the values the place holds once the owing operation completes ([Data `requires`](#data-requires)) | the same, before the operation |

Any other kind is refused as `Unsupported`; elaboration mints none yet.

Beside the obligations it makes two structural judgments, which share one
verdict per operation and terminator that touches storage
(`MachineDischarge::uses`). Initialization: a use that some path reaches
with nothing stored is refused. Payload access: a case payload named where
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
edge binds (`BranchCondition` of that edge: the parameter equals the one
value its payload leaf holds, when there is one). Each row's proposition is in `MachineTables::propositions`. A row is passed to the kernel for
an obligation only when its point dominates the obligation's point.
Requires and guarantees go in as assumptions, result equations as semantic
axioms, which is how the kernel's definition rules cite them.

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

[Structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary):
"An unestablished local cannot be read: a whole read of the home, a loan of
it to an ordinary or boundary call, and its forwarding on a state edge each
require a whole store into it on every path reaching that use."

Every operation and terminator that reads storage is one use: a field read,
a read of a scalar home, a whole copy, a record operand, a record stored
into a field, a call's record arguments, a record return. Each scalar leaf
at or beneath the place it reads must be supplied on every path: by a store
into the leaf, by a whole write to a place above it, or by a formal's entry
content. If some path supplies nothing, the use's verdict is a refusal with
reason `Unestablished`, concerning that operation or edge. A record is
therefore established by a whole write or by a store into every leaf, and
not by fewer.

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
supplies that leaf again. A scalar local is unchanged: it holds nothing
until a store.

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

Every store owes its clauses; there is no window. A store that breaks a
clause is unproved even when a later store would restore it (finding 12).

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
`BranchCondition`. A payload of a case other than the one the sum was
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
result, its operands' bounds through `IntegerExactAddDefinitionBound`. A
comparison closes by chaining the two sides' bounds through a closed literal
relation. An arithmetic safety goal closes through `IntegerAffineBound`, whose
image of the operands' bounds the kernel itself computes.

The same prover carries relations between values, with no literal endpoint.
A comparison between two values searches order steps between values:
hypotheses relating two values, and `a < a + d` or `a - d < a` from an exact
result when `0 < d` (`IntegerAddOrder`, `IntegerSubtractOrder`), chained by
order transitivity. An unsigned `a - b` is safe from `b <= a` through the
kernel's correlated unsigned subtract. A goal the direct search misses is
split over a disjunctive fact (a disequality guard transports to `v < k or
k < v`): each alternative proves the goal as a local assumption or
contradicts what is known (`DisjunctionElimination`, contradiction
elimination). Splits do not nest.

`verify_obligation` then decides the whole certificate. Its acceptance records
whether the mathematical core judged it as well. Most certificates are
`Judged`; a discreteness step from a transported strict guard (`a < 10` to
`a <= 9`, `0 < a` to `1 <= a`) is accepted by the rule checker and answered
`Refused` by the core. Both count as discharged, and the comparison harness
reports them apart.

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
4. **Shift-left representability is not built.** The kernel has a direct
   shift route (`map_direct_shift_left_bound`); this prover does not build
   its evidence, so an exact `<<` whose count is not a literal zero is
   `Unproved`. No multiply result bound is built either: the kernel has no
   multiply definition rule.
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
8. **A scalar local with no initializer is unestablished here and zero in
   the current checker.** `let mut x: u32; f(x)` is accepted today with
   `x == 0` known; this route rejects the read. The question is in
   `OWNER_QUESTIONS.md` (`local-without-initializer`). If the answer is
   zero, elaboration logs the zero at the reserving operation, as it does
   for a data local, and nothing here changes.

9. **A case fact is not knowledge.** The lane plan asks for "the
   membership value on each `StructuralCase` edge, as the existing
   `BranchCondition` row". A `StructuralCase` terminator has no membership
   value, and the only proposition that could state the fact
   (`Proposition::StructuralCaseMembership`) names a place, which this
   route's knowledge never does. The case selected on an edge is therefore
   held by the payload judgment alone. What an edge binds is over values and
   is a row; it reuses `Justification::BranchCondition`, because the tables
   have no justification for an edge binding and the lane adds no variant.
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
12. **No invariant window.** The current checker lets a store break a
    clause and a later store restore it before the value is consumed
    (dependent_values.md, "Otherwise the write opens a window on the
    place"). This route owes the clauses at every store, as Terminal Psi
    states them ("Terminal Psi opens no invariant window on a field"), so
    such a program is `Unproved` at the first store. The conserved
    `DomainDebt` obligation is where a window would live.
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

## Measuring against the current checker

`omega-rust/omega/compilation/compiler/examples/route_one_comparison.rs` runs
both checkers on corpus programs and prints one line per program, one per
obligation and one per unestablished use of storage; a program with such a
use is `route=rejected`. `discharge_with(_, Knowledge::WithoutBranchConditions)` exists only
so that comparison can count what branch facts buy; the checker uses
`discharge`.
