# Places foundation lane

Staging plan, 2026-11-10, for the first lane after route one of
[check on the lowered form](check_on_lowered_form.md). When this was written
nothing here was built, and the lane was to start once route one passed its
gate. It started before that:
[what landed](check_on_lowered_form.md#started-before-the-gate-paused-and-resumed)
is recorded there, with the gate that now holds further lanes.

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

On a 608-fixture slice probed at `#2307`'s head, the lane removes the first
refusal of 178 programs. Only 16 to 20 of those newly enter the subset (an
estimate), because most of the rest also borrow, and borrowing is the next
lane's work. The lane is worth doing for its tables, not for the rows it
moves on its own. [The slice](#the-corpus-slice) gives the counts and labels
the estimates.

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
(`terminal-psi-to-terminal-psi/src/graph.rs`). It is not a proposition, so it makes no
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

## The corpus slice

The probe ran on `#2307`'s head (`e10fbc6cc`). It ran
`typed_trees_to_terminal_psi_corpus`, built with
`CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler --example typed_trees_to_terminal_psi_corpus`,
on this cloud VM (Linux x86_64, 4 CPUs), in three shards over a list shuffled
with a fixed seed. All 608 fixtures completed in about 48 minutes. The probe
reports only the *first* refusal of each closure.

The slice has two parts:

- **All** 506 fixtures in `pass/{cases,constants,data,expressions,locals,patterns,structs,structural,types}`
  and `run/`. These are the directories where records, cases and constants
  live.
- **A sample** of 102 from `pass/facts`: every 38th fixture by name, out of
  3,885.

| Probe stage | Fixtures |
| --- | ---: |
| refused | 477 |
| no program entry | 67 |
| current checker failed (so no typed trees) | 52 |
| ran (already inside route one) | 11 |
| panicked | 1 (`run/`, in the fixture harness) |

First refusals, top ten:

| Refusal | Fixtures | In this lane |
| --- | ---: | --- |
| `machine service reach` | 183 | no; 170 of them import `omega_language_std::console` |
| `type nominal type` | 117 | yes |
| `statement uninitialized local` | 51 | yes, but see below |
| `type constrained` | 26 | no (domains) |
| `type fixed array` | 25 | no (collections) |
| `call receiver call` | 10 | no |
| `type float` | 8 | no |
| `expression cast` | 7 | no |
| `type reference` | 6 | no |
| `call receiver, requirement or coercion call` | 5 | no |

The lane removes six refusals: `type nominal type`,
`statement uninitialized local`, `expression match`, `expression member`,
`expression projected name path` and `expression record literal`. Those six
are the first refusal of **178** fixtures (117, 51, 4, 4, 1 and 1). That is
how many reach a *later* refusal once the lane lands, not how many enter.

How many enter is an **estimate**, because the probe stops at the first
refusal. Each of the 178 sources was screened by text, as a measurement only,
for constructs outside route one plus this lane. 157 contain at least one.
The counts overlap: a borrow or reference type 136, a bracket (array, index or
slice) 84, other out-of-lane keywords 55, a generic application 52, `as` 35,
a string literal 32, a float type 11. The remaining **21** were then read one
by one:

| Verdict by reading | Fixtures |
| --- | ---: |
| inside route one plus this lane | 16 |
| needs a rule the lane must confirm: a machine call inside a data `requires` (`pass/facts/case_where_equality_data_predicate_whole_replacement_restored_exit`), integer literal patterns (`pass/patterns/named_value_before_negative_integer_pattern_exit`), a zero-established data local (`pass/data/prefix_case_pure_sum_retirement`), data from another package (`pass/facts/nominal_declaration_owner_through_local`) | 4 |
| outside: mutual recursion (`run/contracts/cyclic_mixed_conjunction`) | 1 |

So **16 to 20 fixtures of this slice newly enter the subset**. The 16 read
as inside are 7 in `pass/`, 2 in `run/` and 7 in the `pass/facts` sample;
the 4 to confirm are 2 in `pass/` and 2 in `pass/facts`. Even 20 is an upper
bound, because a program that passes the screen can still hit a later
refusal. Scaled from the sample (7 to 9 hits in 102, times 38), about 270 to
340 of `pass/facts`' 3,885 fixtures would enter. That is a counted estimate
with wide error, not a measurement.

What the numbers say about the lane:

- **Places alone move few rows.** Of the 178 programs whose first refusal
  this lane removes, 136 also borrow, and every one of the 51 refused as
  `statement uninitialized local` also uses something out of lane. Most
  initialize the local through `&write` or pass it as `&mut` entry storage.
  The lane still serves initialization, because it owns the write log, but
  that refusal will not turn into entering rows until the references lane
  lands. The lane's value is the tables the next five lanes build on. Its
  agreement sample is small, and the lane says so rather than claiming
  coverage.
- **`machine service reach` is the largest single refusal**, at 183 of 608,
  and no lane in the post-gate ordering names it before the second group's
  `host`. Any program that prints, including most `run/` rows, is blocked
  there whatever this lane does. This is recorded for the coordinator. It is
  not in scope here.
- **Fail rows were not probed.** At this head, a program the current checker
  rejects never reaches elaboration, so its stage is `check-failed`; `#2312`
  adds that seam. The must-reject rows below were chosen by reading.

### Must-reject rows that guard the lane

Each row is in the ledger as `rejected expected`. Each uses no reference,
loop or array, so it lies inside route one plus this lane and must be
rejected by the new route for the right reason. None binds a `ProgramEntry`,
so each needs the per-machine elaboration root that `#2312` leaves open
(see [dependencies](#dependencies)).

| Row | What it guards | Must fail as |
| --- | --- | --- |
| `fail/data/constructor_stale_local_bounds` | a record literal reads a local after a reassignment; the domain obligation must bind the reaching value (`7`), not the initializer (`1`) | `DataDomain` unproved |
| `fail/cases/constructor_subject_range_unproved` | case construction under a data `requires` that tests the case and bounds its payload; the payload is an unconstrained formal | `DataDomain` unproved |
| `fail/cases/constructor_subject_range_neighbor_not_evidence` | a literal on a sibling field is no evidence for the bounded field | `DataDomain` unproved |
| `fail/constants/owned_record_argument_requires` | a call `requires packet.first == 7` on a record that a constant's initializer builds with `first = 8` | today build-time evaluation rejects it (`constant invocation of read failed checking`). If the lane elaborates the authored initializer, the new route must reject it as `CallRequirement` unproved through the entry content of a structural formal; if build-time evaluation still rejects first, it stays rejected there. The lane must say which happened |
| `fail/data/record_pattern_missing_field` | a record pattern must name every field | refused before the lane (Psi 04); listed so the lane does not accept it by reading only named fields. Its machine takes `&mut Main`, so the new route refuses it as out of scope until the references lane |

`fail/data/struct_literal_duplicate_field_rejected`,
`fail/data/mixed_record_literal`, `fail/data/match_nonexhaustive_cases` and
`fail/structural/foreign_case_record_member` are rejected by stages 01 to 04,
or by a declaration check, before any checker runs. They stay rejected
whichever checker governs. Rows that guard the same rules through a
reference (`fail/data/constructor_prior_operand_write`,
`fail/data/constructor_distinct_field_versions`,
`fail/cases/constructor_subject_range_replaced_value`,
`fail/cases/nested_payload_read_after_case_replacement`) belong to the
references lane.

The corpus lacks four must-reject programs inside this lane. None goes under
`facts/`. The lane adds at most one focused pair under
`tests/omega/{pass,fail}/places/`, and lists the rest here for the lanes that
own them:

- a payload read of an owned local after a whole replacement by another case
  (the owned twin of `nested_payload_read_after_case_replacement`);
- a field store that breaks the data `requires` (`r.count = 9` with
  `requires count <= 5`), then returns `r`;
- a read of a local of gated type (or a view local) declared without an
  initializer that is assigned on only one arm of a `transition`; a plain
  scalar local is zero-constructed and its read is accepted;
- a data `requires` that relates two fields (`low <= high`), broken by a
  store to one field, where the other field's value comes only from the
  write log.

## Measures and acceptance

The lane is done when each of these holds, measured as route one measures
them (`route_one_comparison`, `typed_trees_to_terminal_psi_corpus`).

1. **Agreement.** Every corpus row whose constructs lie within route one
   plus this lane is checked by both routes. Every disagreement is listed and
   classified: new route cannot prove a true obligation, new route accepts
   something false, or current checker wrong. None is averaged. Target: zero
   "new route accepts something false"; every must-reject row below rejected.
2. **Kernel-checked evidence.** Every discharged `OperationSafety` and
   `DataDomain` obligation carries a certificate `verify_obligation`
   accepted. Report the count per kind, with its denominator and the core's
   `Judged` / `Refused` split.
3. **Decisions keyed on text: 0** in the lane's code. Field and case
   identity come from `StructuralFieldId` / `StructuralCaseId`, resolved once.
   A field or case that typed trees hold only as a spelling is refused as
   `Unserved::Spelling`, and the repair goes upstream (standing rule:
   identity is fixed before elaboration). This covers the constant whose
   `EvaluatedConstant::Encoded(String)` is a text encoding: elaboration reads
   `materialized_initializer`, never the encoded text.
4. **Silent drops: 0.** Every refusal path returns `Checked`. The test
   `every_obligation_has_exactly_one_verdict` extends to the new kind. A new
   test asserts that every `PlaceRow` has a write or entry content reaching
   every read of it, or a refusal.
5. **Routes per obligation kind: 1.** One prover, one `verify_obligation`
   path; payload access and initialization are one structural judgment each.
6. **Lines added per construct.** Report lines added to
   `typed-trees-to-terminal-psi`, `terminal-psi-to-terminal-psi` and `terminal-check-tables`
   per row of the [in-scope table](#in). This is the number the design's
   stop rule is about: a later construct should cost less than the one
   before it. Route one's baseline is 7,618 lines for the three crates,
   tests included (`#2305`). No target number is set before the first lane
   measures one.
7. **Check time per program**, both routes, as `#2305` reports it.

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

## Gate run, 2026-11-11

The lane's five slices are on `main` (`#2328`, `#2332`, `#2337`, `#2343`,
`#2362`, with `#2356` for one justification). Run against the corpus slice
above, they bring almost no corpus program to a verdict: 25 programs now
lower whole, 5 are judged, and the 4 the two checkers agree on have no
obligation at all. The tables and judgments are exercised by the programs
written for each slice, not yet by the corpus.

The run: `route_one_comparison` on `main` at `e125b2fc3`, built with
`CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler --example route_one_comparison`,
over 619 fixtures in two shards on a cloud VM (Linux x86_64, 4 CPUs), 87
minutes. The slice is the same selection as above and has grown from 608: 516
in the nine `pass/` directories and `run/`, and 103 from `pass/facts` (every
38th by name). No fixture panicked. The raw lines are in
[places_foundation_lane_gate.tsv](places_foundation_lane_gate.tsv).

| Current checker | New route | Fixtures |
| --- | --- | ---: |
| accepted | outside: elaboration refuses a construct, so the current checker decides | 479 |
| accepted | no program entry | 68 |
| rejected | not reached (a `pass/` or `run/` row that is red today) | 47 |
| accepted | lowered whole, then discharge refuses the module | 20 |
| accepted | accepted | 4 |
| accepted | rejected | 1 |

**Agreement.** Of the 5 judged programs, 4 agree. The disagreement is
`run/calls/scalar_contract_route_exit`: "requires clause 1 of the called
machine: no proof found from the facts available here (the bounds do not
decide the comparison)", at bytes 744..756. It is the prover gap `#2305`
recorded, so its class is "new route cannot prove a true obligation". None is
"new route accepts something false". The 4 agreeing programs have no
obligation and no use of storage.

**Kernel-checked evidence on the corpus.** 7 certificate obligations, all in
that one program: `OperationSafety` 3 of 3, `CallRequirement` 2 of 3,
`NormalGuarantee` 1 of 1, each judged by the kernel core, none refused by
it; 1 `PublishedGuarantee` held. No `DataDomain` obligation is minted on the
corpus. On the programs written for the slices: `DataDomain` 20 of 23,
`CallRequirement` 8 of 8, all judged (`#2362`).

**First refusals** among the 479 the route does not serve:

| Refusal | Fixtures |
| --- | ---: |
| `machine service reach` | 192 |
| `type data with custody` | 111 |
| `type fixed array` | 50 |
| `type constrained` | 34 |
| `type float` | 11 |
| `parameter mutable parameter` | 9 |
| `call receiver, requirement or coercion call` | 8 |
| `type reference` | 8 |
| `expression cast` | 7 |
| `type generic data` | 6 |
| other refusals, 5 or fewer fixtures each | 43 |

The six refusals this plan set out to remove (`type nominal type`,
`statement uninitialized local`, `expression match`, `expression member`,
`expression projected name path`, `expression record literal`) are the first
refusal of no fixture now.

What the numbers say:

- **Corpus data is not `[copy]`.** The lane serves plain copyable data only,
  as scoped, and 111 fixtures stop at `type data with custody`: a record
  without `[copy]` is affine
  ([ownership](../../spec/language/ownership.md): "A record still requires an
  explicit `[copy]` declaration; copyable fields do not change the affine
  default"). That is the ownership lane's work and the largest refusal next
  to this lane. The estimate above of 16 to 20 entering programs did not
  hold: it was made by screening sources for borrows and brackets and missed
  the affine default.
- **20 programs lower whole and get no verdict**, for two reasons that
  predate the lane: discharge refuses a no-normal invocation terminator
  ("invocation outside an operation", 15) and a callee clause that names a
  value that is not a formal (5). Closing those two would quadruple the
  judged sample.
- **`machine service reach` is unchanged** at 192: every program that prints.
- **Rejections are not measured here.** The slice has no `fail/` fixture, and
  the 47 rejected rows are red `pass/` and `run/` rows the route never
  reached.

Other measures. Decisions keyed on text: 0. Silent drops: 0. Routes per
obligation kind: 1; initialization, payload access and dispatch coverage are
one structural judgment each over one reaching walk. Check time: the current
checker took 9,306.7 s over the 619 fixtures (at most 42.1 s for one; it
includes discovery, review and Build execution); the new route took 42 ms
over the 25 it lowered (at most 14 ms; elaboration and discharge only). The
two are not the same work.

Lines added per slice outside tests and READMEs (`typed-trees-to-terminal-psi`,
`terminal-psi-to-terminal-psi`, `terminal-check-tables`):

| Slice | Lines |
| --- | --- |
| records, field reads, copies (`#2332`) | 1,955 / 441 / 0 |
| stores, whole assignment, initialization (`#2337`) | 565 / 805 / 11 |
| sums, dispatch, payload access (`#2343`) | 1,730 / 661 / 18 |
| data `requires`, zero-established locals (`#2362`) | 510 / 1,200 / 26 |

The cost is not falling: sums and data `requires` each cost about what
records did. The design's stop rule asks for a later construct to cost less
than the one before it; on this count the lane does not show that yet.

Not done from this plan's table: a record pattern and a directly authored
payload member (the typed trees hold their fields as text; an upstream repair
is in progress) and clauses over `result`. Invariant windows on data
`requires` are judged since this run: a store not shown to keep a clause
opens a window that each consumption point must close, and a gated local is
established field by field
([terminal-psi-to-terminal-psi](../../../omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/README.md#invariant-windows)).

## Gate re-run, 2026-11-12

Sixteen of the 619 fixtures now reach a verdict, where five did, and twelve
are accepted by both checkers, where four were. The four the route rejects
and the current checker accepts are all a result bound of a division or a
shift, which waits for the kernel rules admitted under the decision
[`kernel-rule-admission`](../../spec/proofs/foundation.md#admitting-an-arithmetic-rule); none is a
program the route accepts wrongly.

The run: the same 619 fixtures as [the first run](#gate-run-2026-11-11),
with `route_one_comparison` built at `b743adba3` (the commit `#2505` landed
as `51e8c45c7`) with `CARGO_PROFILE_DEV_OPT_LEVEL=1`, one process per
fixture, two at a time, `timeout 300 nice -n 19`, on a cloud container
(Linux x86_64, 4 CPUs, 15 GB) that was also building the compiler. No
fixture panicked or timed out. The raw lines are in
[places_foundation_lane_gate_2026-11-12.tsv](places_foundation_lane_gate_2026-11-12.tsv).
The harness now reports a module the discharge refuses as `outside`, so the
first run's "lowered whole, then discharge refuses" row has no counterpart
here.

| Current checker | New route | First run | This run |
| --- | --- | ---: | ---: |
| accepted | outside: a construct is refused, so the current checker decides | 499 | 502 |
| accepted | no program entry | 68 | 68 |
| rejected or stopped | not judged (a `pass/` or `run/` row that is red today) | 47 | 33 |
| accepted | accepted | 4 | 12 |
| accepted | rejected | 1 | 4 |

The first run's 499 is its 479 refused at lowering and its 20 refused at
discharge. Of this run's 33, the route refuses 31 at lowering, one binds no
entry, and one stops before the typed trees.

**Agreement.** Of the 16 judged programs, 12 agree. The four disagreements,
each a true obligation the prover has no rule to prove:

| Fixture | Unproved |
| --- | --- |
| `pass/facts/signed_shift_left_bound_through_call_result_exit` | a call's `requires` over a shifted value, and the subtraction after it |
| `pass/facts/unsigned_shift_left_bound_through_other_state_exit` | the add after a shift |
| `run/contracts/scalar_crash_route_exit` | `ensures result <= value` of `value / 2` |
| `run/contracts/scalar_crash_route_trap` | the same clause |

`run/calls/scalar_contract_route_exit`, the first run's one disagreement,
is now accepted. The twelve accepted programs are
`pass/constants/target_layout_derived_ground_guarantee_exit`,
`pass/data/prefix_case_contextual_case_field`,
`pass/facts/abi_multi_arg_spill_through_call_result_exit`,
`pass/facts/case_payload_transport_through_local_exit`,
`pass/facts/live_across_two_calls_preservation_bound_through_call_result_exit`,
`pass/facts/nested_three_level_record_field_bound_through_local_exit`,
`pass/facts/recorded_build_role/other_machine`,
`pass/locals/runtime_unrestricted_record_rebinding_exit`,
`pass/patterns/named_value_before_negative_integer_pattern_exit`,
`pass/types/storage_address_core_name_shadowed_exit`,
`run/arithmetic/anonymous_comparison_with_typed_operators` and
`run/calls/scalar_contract_route_exit`.

**Kernel-checked evidence on the corpus.** 123 obligations in the 16 judged
programs. Of the 110 that take a certificate, 105 are discharged and judged
by the kernel core: `CrashCoverage` 38 of 38, `OperationSafety` 24 of 26,
`ArrivalInvariant` 16 of 16, `CallRequirement` 13 of 14, `NormalGuarantee`
12 of 14, `DataDomain` 2 of 2. Of the 13 `PublishedGuarantee` rows, 11 hold
and 2 fail with their return. 24 uses of storage, all established; no
invariant window opens.

**First refusals** among the 502:

| Refusal | Fixtures |
| --- | ---: |
| `machine service reach` | 199 |
| `parameter mutable parameter` | 72 |
| `type reference` | 50 |
| `machine generic parameters` | 32 |
| `declaration Build selection that is not a machine or a data type` | 20 |
| `type data with custody` | 18 |
| `type constrained` | 16 |
| `machine attached data` | 16 |
| `contract contracts on a state` | 15 |
| `type float` | 11 |
| `type fixed array` | 7 |
| `expression cast` | 6 |
| `declaration const` | 6 |
| other refusals, 4 or fewer fixtures each | 34 |

`type data with custody` falls from 111 to 18 and `type fixed array` from 50
to 7, because the route now roots every declaration of the subject and
refuses at the first one it cannot lower, which is more often a machine than
a type. The order of this table is therefore not comparable with the first
run's. The 32 `machine generic parameters` were measured before the opaque
type parameter work (`#2517`).

Check time: the current checker took 8,406.7 s over the 619 fixtures (at
most 91.4 s for one, on a loaded machine); the route took 285 ms over the 16
it judged (at most 141 ms). The two are not the same work.

## Dependencies

| PR | What this lane needs from it | If it does not land |
| --- | --- | --- |
| `#2307` crash routes, `result` identity | `ValueOrigin::MachineResult` and the reserved-result route. `result.field` clauses project from it. The `crashes none` handling is in the same files the split moves. | Split after it lands. Starting earlier means rebasing a 2,000-line move. |
| `#2305` relations, branch facts, comparison harness | `Justification::BranchCondition` (case edges reuse it), `MachineTables::propositions`, `PublishedGuarantee`, `route_one_comparison`. `DataDomain` goals relating two fields (`before == after`, `low <= high`) need the relation search. | Blocking: agreement cannot be measured without the harness. |
| `#2312` typed frontier on rejected programs | `check_keeping_typed_frontier`, so the must-reject rows below reach the new route. Most `fail/` rows bind no `ProgramEntry`; the lane needs an elaboration root per rejected machine, which `#2312` leaves open. | Blocking for the must-reject half of agreement. |
| `#2309` saved-proof re-verification | Not needed. | None. |
| `#2295` evidence seals (base of `#2305`) | `OperationSafety` seals extend to field-range obligations without change. | Comes with `#2305`. |

The lane starts when route one's gate is recorded as passed in
[check on the lowered form](check_on_lowered_form.md). If the gate fails, the
record is withdrawn and this plan is moot.

## Worker brief

Paste this into the implementing session once route one's gate is recorded
as passed and `#2305` and `#2307` have landed:

> Follow AGENTS.md and `/advance`. Lane `psi`, gate `RC-SOURCE-SEMANTICS`.
> Implement the places foundation lane of the lowered-form checker exactly as
> `wiki/drafts/designs/places_foundation_lane.md` specifies; read it and
> `check_on_lowered_form.md` first. First commit: split
> `typed-trees-to-terminal-psi` into the per-family modules in that record's table,
> moving code only, with every existing test unchanged and passing. Then, in
> `typed-trees-to-terminal-psi`, admit copy-eligible plain `data` types, record and
> case literals, field and payload reads and writes on owned roots, case
> tests, `match` and `transition` over sums, record patterns, locals declared
> without an initializer, and named constants (from `materialized_initializer`,
> never the encoded text). Fill `places`, `roots` and `writes` in
> `terminal-check-tables`. Add exactly one obligation kind, `DataDomain`, and
> one refusal reason, `Unestablished`; reuse `OperationSafety` for every
> `range_obligation`. In `terminal-psi-to-terminal-psi`, add the read equation from the
> write log, case membership as `BranchCondition` rows, and the two structural
> judgments (initialization, payload access); every certificate goes through
> the existing prover and `verify_obligation`. Knowledge names values, never
> places. No table, map or decision keyed by text, and no step that drops
> without a refusal. Everything listed as out is refused `Unsupported` with a
> detail. If a fix would need a second route, carrier or per-place branch,
> stop and write a cluster report (stop rule 6). Report: the probe counts on
> the 608-fixture slice in that record, rerun at your head; both-checker
> agreement over every row inside route one plus this lane, with each
> disagreement classified; kernel-checked evidence per obligation kind; lines
> added per construct; and the must-reject rows listed there, each rejected
> for the reason given. Add at most one positive/negative fixture pair under
> `tests/omega/{pass,fail}/places/`, none under `facts/`. Commit before
> recording corpus rows. Open a draft PR and stop.
