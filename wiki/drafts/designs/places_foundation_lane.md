# Places foundation lane

Staging plan, 2026-11-10, for the first lane after route one of
[check on the lowered form](check_on_lowered_form.md).
[What landed](check_on_lowered_form.md#started-before-the-gate-paused-and-resumed)
is recorded there.

The new checker can only check programs whose values are plain integers and
Booleans. Any program that builds a record, reads a field, matches a case or
names a constant is refused. That covers almost every real program. This lane
lets the checker hold storage: each exact place a program touches
(`packet.count`, the payload of `Outcome::Ok`), the root that place belongs to,
and a log of every write to it in program order. Each lane after this one
needs those three tables, so this lane builds them once and the later lanes
reuse them instead of each building their own.

The lane does not add references, loans, windows or ownership transfer. Those
belong to the first parallel group. The lane also proves nothing by a new
method. Field bounds, data `requires` and payload access become obligations
or structural checks over the tables, and the one route-one prover discharges
them through `proof/admission`.

## Scope

### In

Each row names the source construct, the Terminal Psi form elaboration
emits, and the sentence it implements. Quotations are verbatim.

| Source construct | Terminal Psi form | Specification |
| --- | --- | --- |
| A `data` type with fields, as a formal, local or result | Structural root (`StructuralPlaceDeclaration`), `ScalarType` leaves under `Segment::Field` | [ownership](../../spec/terminal-psi/ownership.md#roots-continuations-and-order): "Parameter, named-local, and call-result roots retain distinct establishment identities. A result ordinal is not a parameter index." |
| Record literal `R { a: x, b: y }` | Operands in authored order, then `EstablishRecord { fields }` with `RecordFieldValue::Scalar { range_obligation }` | [structural access](../../spec/terminal-psi/structural_access.md#complete-record-construction): "`EstablishRecord` produces one exact `OperationResult` record from a complete roster of declaration-identified field operands." and "Each operand is evaluated in authored order before this operation. Establishment is atomic after all operands are available". [Expressions](../../spec/language/expressions.md#evaluation-schedule): "Record or case literal \| Authored field-expression order." |
| Case literal `S::C { p: x }`, payload-less `S::C` | `EstablishScalarCase` / `EstablishStructuralCase { result_case, fields }` | [data and literals](../../spec/language/data_and_literals.md): "Case values always use their carrier-qualified member path." and "No raw integer silently establishes a sum value." |
| Field read `r.a`, nested `r.a.b` | `PrimitiveScalarRead { source, path }`; one `ExactPlaceId` per projection | [expressions](../../spec/language/expressions.md#calls-places-and-temporaries): "Dot access remains data-field projection; it does not select or invoke a machine." [Ownership](../../spec/terminal-psi/ownership.md#claim-paths): "Record segments use the exact field identity". |
| Field write `r.a = x` on an owned local | `StructuralScalarFieldStore { destination, path, field, value, range_obligation }` | [structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary): "A bounded integer field store additionally retains its own `range_obligation`; unbounded fields must not carry one. ... using only pre-write facts. The completed write's new field equality cannot prove its own validity." |
| Whole assignment `r = R { ... }`, `s = S::C { ... }` | Construction, then `StoreStructuralField` with no path (whole replacement) | [structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary): "A whole record assignment constructs its operands before replacing the destination through the ordinary structural replacement operation." and "Root, field, and indexed destinations do not change construction semantics or decompose the assignment into member stores." |
| Case test `s in S::C` | `StructuralCaseMembership { source, path, case }`, a Boolean value | [structural access](../../spec/terminal-psi/structural_access.md#case-observation): "Its case identity must belong to the exact selected sum type; matching case spellings in another type supply no authority. The source must be readable and established on every incoming path." |
| `match` and `transition` over a sum, with payload bindings and `_` | `StructuralCase { source, path, cases }` terminator; payload reads on the selected edge | [patterns](../../spec/language/patterns.md): "A dispatch evaluates its subject once, tests alternatives in authored order, and executes only the selected arm." and "Every dispatch must cover the admitted subject. Missing runtime coverage is a compile error, not an implicit trap or fallthrough." |
| Value-position `match` | The same terminator; arms join with the result as a block parameter | [expressions](../../spec/language/expressions.md#evaluation-schedule): "Value-position `match` \| Subject once; first matching arm only." |
| Payload field read or write `s.p` under a case fact | `PrimitiveScalarRead` / `StructuralScalarFieldStore` with a final `Segment::Case` step | [structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary): "Each `Case` step requires source admission to establish the selected case for the same root and prefix at the read's observation epoch. A dominating membership test or dispatch supplies that fact. Otherwise the case holds when every other declared case of that sum is refuted." and "A write to the sum or any prefix retires these facts". |
| Record pattern `let R { a, b } = r;`, arm payload bindings | Field reads of the subject; each binding is the read's value | [patterns](../../spec/language/patterns.md#irrefutable-data-bindings): "A pattern that can fail rejects; a binding introduces no hidden branch, trap, fallback or runtime assertion." |
| Data `requires` (default domain) on construction and on writes | Obligation `DataDomain` (below) at `EstablishRecord`, `Establish*Case`, field store and whole store | [data and literals](../../spec/language/data_and_literals.md): "Construction and every restoring write prove the owning default-domain clauses." [Structural access](../../spec/terminal-psi/structural_access.md#complete-record-construction): "Record and case construction and scalar-field replacement use the same declaration-owned requirement. The obligation binds the initializer or replacement SSA value, never the old field contents." |
| Field-domain premises at a read | Knowledge row on the read's value | [structural access](../../spec/terminal-psi/structural_access.md#complete-record-construction): "A validated field read or selected-case payload copy instantiates that invariant on its fresh scalar snapshot; other fields and unselected cases provide no premise. The snapshot survives later writes". |
| A primitive local declared without an initializer, assigned later | A root zero-constructed at `EstablishPrimitiveLocal`: its home holds the zero constant, an established value because every builtin scalar admits zero (`local-without-initializer`). A scalar type that zero does not satisfy would be gated, but a constrained scalar type is outside this subset | [dependent values](../../spec/language/dependent_values.md#default-domains-and-zero-initialization): "If zero satisfies the default domain, the value is zero-constructible. Otherwise the type is gated: construction or checked qualification must establish the domain before observation." The sentence "An unestablished local cannot be read" in [structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary) governs declared shared view locals, which stay refused here (references lane), not a plain scalar. |
| A `data` local declared without an initializer (`let v: Stored;`) | `EstablishZeroedStructural`, then a `DataDomain` obligation on the zero value before first use | [dependent values](../../spec/language/dependent_values.md#default-domains-and-zero-initialization): "A zeroed storage representation can be accessed as an established value only after its default-domain obligations hold." [Data and literals](../../spec/language/data_and_literals.md): "The ordinary case-bearing zero representation selects the first declared case and recursively zeroed common fields/payload." `pass/data/prefix_case_pure_sum_retirement` reads such a local and expects its zero. The lane confirms against `state_contracts.md` that the two rows do not conflict, and asks in `OWNER_QUESTIONS.md` if they do. |
| Named `const` of scalar, record or copy-eligible case type | The constant's `materialized_initializer` elaborated in place, at each use, as literal operands and construction | [constants](../../spec/language/constants.md): "`const` names an immutable compile-time value, not runtime storage." and "A constant has no stable address and grants no authority." |
| `requires` / `ensures` clauses that project a formal or `result` (`p.first == 7`, `result in S::C`) | Clause terms over the entry content of the formal's place, or over the result root | [fact validity](../../spec/terminal-psi/fact_validity.md#restored-scalar-values): "a valid initialized ordinary storage place at entry may supply its exact reaching value version; it need not have a store inside the current body." |
| Copying a record or case value (`let b = a;`, passing it by value) | `StructuralLeafCopy` / `StructuralCaseLeafCopy` into a fresh root | [ownership](../../spec/terminal-psi/ownership.md#roots-continuations-and-order): "Plain unrestricted primitive-array, scalar-case, and plain record payloads have no disposal obligation." |

Only `[copy]`-eligible plain data is in: data with no disposal obligation, no
linear or affine field, no reference field and no attached `Drop`. A data type
outside that set is refused as `Unsupported` with detail `type data with
custody`, and the ownership lane serves it.

### Out

| Construct | Why | Owner |
| --- | --- | --- |
| References to a record or a field, `&mut r.a`, `&write`, borrowed payloads | Needs a loan. [Structural access](../../spec/terminal-psi/structural_access.md#write-only-authority): "`&write T` borrows an existing valid `T`; it is not uninitialized output storage." | references and loans |
| A machine whose formal is `&mut R` (including `machine R::m(r: &mut R)`) | The write goes through a loan, and a domain repair needs an invariant window: [dependent values](../../spec/language/dependent_values.md#invariant-windows) "Otherwise the write opens a window on the place ... The domain must be proved again at the next consumption point" | references and loans |
| Moving a non-copy field out, partial moves, residual roots | [Ownership](../../spec/terminal-psi/ownership.md#partial-ownership-and-residuals): "Moving a projection makes its ancestor unavailable for whole-value calls, returns, or disposal." | ownership and cleanup |
| Fixed arrays, `RuntimeIndex`, `AllElements`, slices and text | Runtime elements need a bound per write and a may-overlap answer over selector values. [Structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary): "A field or primitive store whose path crosses a runtime element names no canonical leaf". | slices, collections, indices and text |
| Named domains on fields, casts, `qualified` fields | Domain lane: "Attached domains are part of the declared type and change only at a named cast" ([check on the lowered form](check_on_lowered_form.md#the-model-the-checker-holds)) | integers, floats, casts and domains |
| Generic data, `Indexed<const V: Bounded>`, conformances | | generics and traits |
| Loops and recursion, so block parameters on back-edges | | loops, recursion and termination |
| Byte views, representation views, wire schemas, layouts | | second group (`layouts`) |
| Structural `==` between records or payload-bearing cases | The equality expansion needs synthesized structural comparison evidence ([structural predicates](../../spec/terminal-psi/structural_predicates.md)). Payload-less case `==` is in: it is a membership test. | proofs, evidence and quotients |

Everything out is refused with `RefusalReason::Unsupported` and a detail
naming the construct. Nothing is skipped.

## Table design

The three tables already exist in `terminal-check-tables`
(`bindings.rs`: `PlaceRow`, `PlaceOrigin`, `Segment`, `WriteRow`, `Content`)
and are empty in the scalar spine. The lane fills them. It adds no table and
no map: the lookups it needs are the derived columns the tables already
provide (`roots`, `PlaceRow::children`, `PlaceRow::writes`).

### Places and roots

- **Root.** Elaboration mints one Terminal `PlaceId` per structural formal,
  local, call result and constant use, pushes a `PlaceRow` with
  `PlaceOrigin::Root`, and records it in `MachineTables::roots`. A scalar
  local stays an SSA value, as in route one. It becomes a root only when it
  is declared without an initializer (below) or when a later lane lends it.
- **Projection.** A place is interned when an operation first names it:
  binary search of the parent's `children` by `Segment`, push if absent. So
  `r.a` names one `ExactPlaceId` however often it is written. Fields resolve
  once to `StructuralFieldId` against the type reached by the previous
  segment ([ownership](../../spec/terminal-psi/ownership.md#claim-paths):
  "Resolve every segment against the type reached by the previous segment").
  The Terminal path's `StructuralPathSegment::Field(TerminalIdentityIndex)`
  is filled from the same resolution, so the two forms cannot disagree.
- **Payload namespace.** A payload field is `Segment::Case(c)` followed by
  `Segment::Field(f)`. The case segment "adds neither physical storage nor an
  independent claim" ([ownership](../../spec/terminal-psi/ownership.md#claim-paths)).
  Two different case identities under one parent overlap: "Variant payloads
  overlay, so different case identities do not prove disjoint storage"
  ([structural access](../../spec/terminal-psi/structural_access.md)).

The overlap relation is one function, written once in this lane, over parent
links: two places overlap when one is an ancestor of the other, or when they
share a parent and their segments are both `Case`, or are equal. Unequal
`Field` siblings are disjoint. Later lanes extend that function (runtime
indices, referents). They do not add a second relation. This is the place
relation `validation/src/places.rs` meant to be and nothing calls.

### The forward write log

Every store, construction into a root, whole replacement, and entry state
pushes a `WriteRow { place, at, content }`. `at` is
`ProgramPoint::AfterOperation`. Entry content of a formal's place is not a
write. It is the value with `ValueOrigin::EntryContent { place }`, which the
row type already has.

A read resolves its value as follows. Take the reaching writes to its place
or to any overlapping place, on every path from entry. The result is one of
three:

- **One `Content::Value(v)` reaches, or the entry content does.** The read
  gets a knowledge row, `read == v`, justified as
  `OperationSemantics(read)`. [Fact validity](../../spec/terminal-psi/fact_validity.md):
  "A primitive read may introduce an equality between its copied SSA result
  and an exact reaching stored SSA value. Every actual predecessor must
  supply that same value".
- **Different values reach on different arrivals.** No equality is
  introduced, and the read is still well formed. Joins in this lane come from
  short-circuits and value `match`. Elaboration does not make the place's
  content a block parameter. "A fact from one arm does not survive the join."
- **No write and no entry content reaches on some path.** This is a program
  error, not an unsupported construct. See [initialization](#initialization).

A write never edits an earlier row. Knowledge about the old value stays true;
the write changes only which value the place holds from `at` on. That is the
correction to fact transport in [check on the lowered form](check_on_lowered_form.md#corrections-to-fact-transport).

Before a write, a live equality between a field and an immutable value may
be captured. [Structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary):
"Before a write or mutating/consuming call invalidates current field facts, a
live exact equality between a scalar field and an immutable SSA value may
capture the affected scalar occurrences in those facts." In this model that
needs no step: facts are already over values, never over the place.

### Initialization

A root declared without an initializer is zero-constructed
(`local-without-initializer`): it holds an established value when zero
satisfies its type's default domain, which every builtin scalar's does, and
is gated when it does not. The unestablished root is therefore one of gated
type, or a view local, which this lane does not serve, and not a plain
scalar. Each read, whole copy, call argument or return of such a root must be
dominated, on every path, by a whole write to the place, or to every leaf
beneath it. This is a
structural judgment over the write log and the dominance graph
(`06_terminal-psi-to-terminal-psi/src/graph.rs`). It is not a proposition, so it makes no
certificate. A failure is a refusal with a new closed reason,
`RefusalReason::Unestablished`, reported as a compile error. The reason is
new because this is neither unsupported nor unproved. The lane reports this
change to `terminal-check-tables` as a refusal reason, not a table.

### Obligations

The lane adds one obligation kind. It reuses one more.

| Kind | Family | Where | Goal | Discharged by |
| --- | --- | --- | --- | --- |
| `OperationSafety` (existing) | question | each `range_obligation` on `EstablishRecord`, `Establish*Case`, `StructuralScalarFieldStore`, `EstablishPrimitiveLocal`, `WriteOnlyPrimitiveStore` | the field's or local's declared integer range, on the operand value. Terminal semantics states the goal for the operation, as it does for exact arithmetic. | the route-one prover, one certificate, `verify_obligation` |
| `DataDomain { operation, clause }` (new) | question | each clause of the owning data type's `requires`, at each construction and each field or whole store into a place of that type, and at each enclosing type on the path for a nested store | the clause with `self` replaced by the constructed or post-write field values. For a field store, the values of the other fields come from the write log at the store point. The goal never names the old value of the written field. | the same prover and certificate route |

`DataDomain`'s doc comment quotes [data and literals](../../spec/language/data_and_literals.md):
"Construction and every restoring write prove the owning default-domain
clauses", and [structural access](../../spec/terminal-psi/structural_access.md#complete-record-construction):
"The obligation binds the initializer or replacement SSA value, never the old
field contents." A clause that tests a case (`self in S::C && self.p <= 9`)
is stated over the `StructuralCaseMembership` value and the payload values,
so it stays a proposition over values.

**Payload access is not an obligation.** It is the structural judgment that
[structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary)
describes: "A dominating membership test or dispatch supplies that fact."
Elaboration emits a payload read or write only where one of three things
holds:

- a `StructuralCase` edge for that case dominates the access with no
  overlapping write between them;
- the true edge of a `Conditional` on a `StructuralCaseMembership` value for
  that place does;
- the place was established as that case by a construction, and every
  reaching write is that construction.

Any other access is refused as `RefusalReason::Unestablished` with the
access as its construct. "Otherwise the case holds when every other declared
case of that sum is refuted" needs proof about the other cases, so it stays
out of this lane and is refused, not guessed. The dead-arm discharge in
[patterns](../../spec/language/patterns.md) ("an explicit proof discharging
the dead arm") is the same, and belongs to the proofs lane.

**Exhaustiveness is structural.** A `StructuralCase` terminator lists every
declared case, or a default edge. Elaboration builds it from the declaration's
case table, so a missing case is visible at elaboration. A missing case is a
refusal, `Unestablished`, at the terminator.

**Knowledge the lane adds** (as rows in the existing `knowledge` table, with
the existing justifications):

- the read equation above, `OperationSemantics(read)`;
- a data type's `requires` instantiated on a read's fresh value,
  `OperationSemantics(read)`, when the read is of a place whose root holds an
  established value of that type ("A validated field read ... instantiates
  that invariant on its fresh scalar snapshot");
- the membership value on each `StructuralCase` edge, as the existing
  `BranchCondition` row of `#2305`, so case facts and branch facts use one
  route.

### One route, no second carrier

AGENTS.md stop rule 6 applies to every row above. In particular:

- Field bounds reuse `OperationSafety`. Terminal Psi already owns the
  `range_obligation` slot, so a second "field range" kind would be a second
  route for the same question.
- `DataDomain` is one kind for record construction, case construction, field
  store and whole store. It is not one kind per site.
- Everything is decided by `prove.rs`, through `verify_obligation`. The
  prover gains no field-aware path. It sees values and propositions. The
  write log turns places into values before the prover starts.
- The proposition form. `semantic_vocabulary::Proposition` has structural
  variants that name storage by `PlaceId` plus a canonical path, beside this
  crate's `ExactPlaceId` (`terminal-check-tables` README, "Open"). This lane
  chooses **values only**. Knowledge never names a place. A storage
  observation enters as the value its read produced. The lane uses no
  structural `Proposition` variant, and it refuses a contract clause that
  could only be stated as one. That keeps one form, and it closes the open
  question for the later lanes.

## Measures and acceptance

The lane is done when each of these holds.

1. **Kernel-checked evidence.** Every discharged `OperationSafety` and
   `DataDomain` obligation carries a certificate `verify_obligation`
   accepted. Report the count per kind, with its denominator and the core's
   `Judged` / `Refused` split.
2. **Decisions keyed on text: 0** in the lane's code. Field and case
   identity come from `StructuralFieldId` / `StructuralCaseId`, resolved once.
   A field or case that typed trees hold only as a spelling is refused as
   `Unserved::Spelling`, and the repair goes upstream (standing rule:
   identity is fixed before elaboration). This covers the constant whose
   `EvaluatedConstant::Encoded(String)` is a text encoding: elaboration reads
   `materialized_initializer`, never the encoded text.
3. **Silent drops: 0.** Every refusal path returns `Checked`. The test
   `every_obligation_has_exactly_one_verdict` extends to the new kind. A new
   test asserts that every `PlaceRow` has a write or entry content reaching
   every read of it, or a refusal.
4. **Routes per obligation kind: 1.** One prover, one `verify_obligation`
   path; payload access and initialization are one structural judgment each.
5. **Lines added per construct.** Report lines added to
   `typed-trees-to-terminal-psi`, `terminal-psi-to-terminal-psi` and `terminal-check-tables`
   per row of the [in-scope table](#in). This is the number the design's
   stop rule is about: a later construct should cost less than the one
   before it. Route one's baseline is 7,618 lines for the three crates,
   tests included (`#2305`). No target number is set before the first lane
   measures one.

### The split of `typed-trees-to-terminal-psi`

The design calls for per-family modules after route one passes. On
`#2307`'s head `read.rs` is 2,110 lines and `lower.rs` 1,051, each holding
every family. The first commit of the lane moves code without changing
behavior. The test suite, with no test edited, is the check.

| Module | From | Holds |
| --- | --- | --- |
| `read/machine.rs` | `read.rs` | declaration and signature admission, formals, the subset gate |
| `read/types.rs` | `read.rs` | `scalar_type`, the `builtin_type_atom` route, literal landing; gains nominal data |
| `read/contracts.rs` | `read.rs` | `requires`, `ensures`, `crashes`, `result` identity |
| `read/statements.rs` | `read.rs` | locals, assignment, crash statements |
| `read/expressions.rs` | `read.rs` | scalar operators, short-circuit, slots |
| `read/calls.rs` | `read.rs` | callee resolution, applications, selections |
| `read/transitions.rs` | `read.rs` | arms, guards, dispatch subjects |
| `read/places.rs` (new) | | data types, field and case resolution to dense identities, record and case literals, member paths, patterns, constants |
| `lower/control.rs` | `lower.rs` | blocks, edges, short-circuit joins, returns |
| `lower/operations.rs` | `lower.rs` | scalar operations and their obligations |
| `lower/calls.rs` | `lower.rs` | invocations, crash continuations |
| `lower/places.rs` (new) | | roots, interning, the write log, construction, stores, `StructuralCase` |
| `outcome.rs`, `canonical.rs`, `body.rs` | unchanged | `body.rs` gains the place statement and expression kinds |

`terminal-psi-to-terminal-psi` gains `places.rs` for the read equation, the case
membership rows and the two structural judgments (initialization and payload
access). `prove.rs` does not change unless a kernel rule is missing, and a
missing rule is reported as a finding, not worked around.

