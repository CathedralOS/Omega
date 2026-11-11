# Structure-only elaboration

This crate turns frozen typed trees into an unsealed Terminal Psi module and
fills the [check tables](../../representations/terminal-psi/check-tables/README.md)
that describe it, for route one's scalar subset and for plain data: records
(literals, field reads, stores, whole copies) and sums (case literals, case
tests, dispatch and `match` with payload bindings). It is the second piece of
[check on the lowered form](../../../../../wiki/drafts/designs/check_on_lowered_form.md):
it proves nothing, reads no checker fact, and mints every obligation the
lowered form implies without discharging any. It is not wired into the
default compile and replaces nothing yet.

Start at [lib.rs](src/lib.rs) (`elaborate`). [read.rs](src/read.rs) resolves a
machine's typed trees into the closed body of [body.rs](src/body.rs);
[outcome.rs](src/outcome.rs) then works out, callees first, whether each
machine can complete normally and which crash routes it publishes;
[lower.rs](src/lower.rs) turns each body into blocks, values, operations,
edges and invocations. [canonical.rs](src/canonical.rs) holds the one route to
Terminal Psi's canonical form for every proposition the crate builds.

Both steps are split by construct family. `read.rs` holds the shared `Program`
and `Slot`, and each family is a module beside it:
[machine](src/read/machine.rs) (admission, signatures, the body driver),
[types](src/read/types.rs), [contracts](src/read/contracts.rs),
[statements](src/read/statements.rs), [expressions](src/read/expressions.rs),
[calls](src/read/calls.rs), [transitions](src/read/transitions.rs),
[places](src/read/places.rs) (data declarations, record and case literals,
member paths, case tests and patterns) and [subjects](src/read/subjects.rs)
(which declarations a compilation must judge besides those its entries call). `lower.rs` holds the `Lowering` state, the per-machine entry point and
`finish`, beside [control](src/lower/control.rs) (blocks, edges, arms, crashes,
returns), [operations](src/lower/operations.rs), [calls](src/lower/calls.rs)
and [places](src/lower/places.rs) (roots, projections, the write log, record
and case construction, field and payload reads, whole copies, the dispatch
terminator).

## The subset

Machines with one state and a complete body whose formals, locals and result
are builtin integers (`i8` to `u64`) or `bool`, using: scalar locals and
assignment to them; integer and Boolean literals; builtin `Exact` integer
arithmetic, bitwise operations, shifts and comparisons; `!`, `~`, `&&` and
`||`; direct calls of other such machines with `requires` clauses over their
formals and `ensures` clauses over their formals and `result`; `crashes Trap`
and `crashes Abort` clauses with or without route guards over the formals;
`crashes none`; an integer `!=` in a clause, read as `a < b or b < a`, the
form the kernel denotes a false integer equality by;
`crash Trap;` and `crash Abort;`; guarded transition arms; and returns. A call
of a machine that may return is an operation wherever it stands, and carries
the callee's crash routes when it publishes any. A call of a machine with no
normal completion is served only in terminal position (an arm target or a
call statement), where it becomes the block's no-normal invocation.
Everything else is refused with `RefusalReason::Unsupported` and a detail
naming the construct (`expression match`, `type reference`, `call no-normal
call inside an expression`, ...): a reference, slice, loop, recursion
through a cycle, generic, callable value, suspension, assembly, cast, float,
domain or service reach is never skipped, approximated or passed through. A
refusal anywhere in the compilation refuses the whole compilation.

### What one compilation elaborates

Every machine the check claims is lowered, not only the ones the entry calls.
A machine nothing calls still has a body that can overflow, break its own
`ensures` or call another machine without its `requires`; left out of the
module, it would be accepted with no obligation and no refusal.
[Validation subjects](../../../../../wiki/spec/build/validation_subjects.md)
sets the scope: "every declaration of the subject packages is valid, and
every declaration they depend on is valid", and the closure "starts from
every declaration of every subject, including unused ones".

`elaborate` therefore takes a `Compilation`: the Build's bound entries, the
declarations its selections name, and the subject package of the request.
The typed trees cannot supply the subject, because they hold declarations of
other packages too (the bundled library's among them) and the subject "is
decided by the request, not by where source lives". The roots are, in
`MachineId` order:

1. the entries, the first being the module's entry;
2. every machine the Build's selections name (a layout scope's entry, a
   selected provider), whichever package declares it: the closure includes
   "the obligations of the Build's selections";
3. every machine declared in a subject source file, in declaration order,
   including a specialization instance, whose symbol derives from its
   template's declaration;
4. every machine those call, in discovery order, whichever package declares
   it.

A selected data type is resolved as a record. A selection of anything else
(a trait, a requirement, an operator family) is refused as `declaration Build
selection that is not a machine or a data type`: the route does not judge a
provider against what it provides.

`read/subjects.rs` decides whose a declaration is from the source file of its
symbol: a toolchain file or a file of another admitted package is a
dependency, judged only when something reaches it; any other Product-scope
file is the subject's. A declaration with no source file is refused.

Subject declarations that are not machines are also roots. A `data`
declaration is resolved as a record, so one outside the record subset refuses
the compilation even when no machine names it. Every other kind (`const`,
`domain`, `operator`, `trait`, a conformance, a proposition, a measure, a
mathematical definition, a type alias, a wire schema, a machine token
binding) is refused as `declaration <kind>`: the route judges none of them
yet.

Two sets the specification's closure names are not roots, so the route's
verdict does not cover them and a router must not treat it as if it did.
The closure includes "the Build and preparation entries the compile
executes":

- **The Build program in `build.omg`.** It is read under the Build's
  dependency scope, and the Build activation checks it with the current
  checker before it runs and before these trees exist.
- **The bound target's entry-contract source**
  (`source/library/std/targets/<target>/entry.omg`), whose machines plan the
  entry boundary. The current checker keeps that whole source as a subject
  (`check_product_source`); here it is a toolchain file, so its machines are
  judged only if something reaches them.

Which check judges these two when the route governs a Product compilation is
undecided. Making them roots today would refuse every program with a bound
entry, since neither is inside the lowered subset.

### Generic machines

One kind of generic machine is read, as an experiment for
[the checker's generic bodies](../lowered-discharge/README.md#generic-bodies-experiment):
a machine whose only generic parameters are implementation parameters
(`Selected satisfies Step`), calling members through them
(`Selected::apply(x)`). It is read in two forms.

- **Its specializations**, by `elaborate`. Stage 04 closes each static
  application `advance<Double>` by generating a machine for it, so the
  typed trees already hold one closed machine per application. A call of
  `advance<Double>` targets that generated machine and is read as a direct
  call; inside it, the call through the parameter targets the selected
  member and is read as a direct call of that member. Each such call is
  recorded in `Elaboration::selections` with the parameter, the trait, the
  requirement and the member, all symbols. Only a declaration whose
  generic parameters are all implementation parameters has its generated
  machines read; an application with a type or value argument is still
  `call static application arguments`, and a member named through the
  implementation itself (`Double::apply(x)`) is still `call static
  requirement dispatch`.
- **The generic check form**, by `elaborate_generic`. The declared machine
  is read with its parameters rigid: a call through one names the
  requirement, which becomes a machine with a signature, a contract and no
  body (`Elaboration::requirements`, `lower_generic`). It lowers to a
  machine with no block. A generic form is never sealed, encoded or run.

`elaborate_witness` builds the statement that an implementation refines a
requirement: the requirement's contract on a body that is one call of the
implementation. `template_of` gives the declaration a generated machine came
from. A type, value or callable parameter is still refused as `machine
generic parameters` (findings 29 to 32).

### Records

A formal, a local, a result, a field and an argument may also be a
copy-eligible plain record: a `data` declaration marked `[copy]` whose fields
are builtin integers, `bool` or other such records. On such values the subset
serves:

- the record literal `R { a: x, b: y }`, the field read `r.a` and `r.a.b`,
  and copying a record value (`let b = a;`, passing it by value, returning
  it, moving it into a field of a literal);
- a store into a field of a `mut` local, `r.a = x` and `r.a.b = x`, and the
  replacement of a record field by a record, `l.from = p`;
- whole assignment to a `mut` local, `r = R { ... }` and `r = other;`;
- a local declared without an initializer: `let mut r: R;` holds the zero
  value of `R`, and `let mut x: u32;` holds zero;
- a `requires` on the declaration, over `self` and its fields
  ([Data `requires`](#data-requires)).

`[copy]` is required, not inferred
([ownership](../../../../../wiki/spec/language/ownership.md): "A record still
requires an explicit `[copy]` declaration; copyable fields do not change the
affine default."). Plain data without it is affine, moves once, and belongs
to the ownership lane (`type data with custody`).

Formals are immutable here. A `mut` formal is refused where the signature is
read (`parameter mutable parameter`), and a store through a `&mut` formal
belongs to the references lane (`type reference`).

### Sums

A `data` declaration marked `[copy]` whose members are all cases is a sum,
served wherever a record is: as a formal, a local, a result, a field or
payload field, and an argument. A payload field is a builtin integer,
`bool` or other such data. On sums the subset serves:

- the case literal `S::C { p: x }`, `S::C {}` and `S::C`, and copying a sum
  value;
- the case test `s in S::C`, and `k == Kind::A` against a payload-free case,
  which the typed trees hold as the same test;
- `transition s { S::A { p } -> ..  S::B -> ..  _ -> .. }`, with payload
  bindings, renames (`p as q`), waived fields (`p as _`) and `..`;
- value-position `match` over a sum named by a slot or a path beneath one,
  with scalar arms;
- a local of sum type declared without an initializer, which holds the
  first declared case with a zeroed payload.

Whether a payload may be named where it is named is not decided here. It is
the checker's payload judgment
([lowered-discharge](../lowered-discharge/README.md#payload-access)), and so
is the completeness of every dispatch it is handed.

The rest of the places foundation lane
(`wiki/drafts/designs/places_foundation_lane.md`) is refused by name until
the slice that serves it:

| Construct | Refusal detail |
| --- | --- |
| a `requires` on a case | `type data requires` |
| a data `requires` clause that calls a machine, compares Booleans with `!=`, does arithmetic, or names something other than `self`, its fields and literals | `contract clause that is not a comparison or conjunction`, `contract ordering of Booleans`, `contract arithmetic clause`, `contract term other than a field of the value or a literal` |
| a constrained field type or a named domain | `type constrained` |
| data that is not `[copy]`, or has a cleanup selection | `type data with custody` |
| data with common fields beside its cases | `type mixed data` |
| generic, quotient or opaque data, data with a carry policy, an erased field, retired field identities or no field | `type generic data`, `type quotient data`, `type data without a checked shape`, `type data with a carry policy`, `type data with an erased field`, `type data with retired field identities`, `type data without fields` |
| a store into, or assignment to, a binding that is not `mut` | `statement store into an immutable binding`, `statement assignment to an immutable binding` |
| a store into a field of a value that is not a local, or into an indexed place | `statement store into a value that is not a local`, `statement assignment to a projected place` |
| `let x;` with neither a type nor an initializer | `statement uninitialized local without a declared type` |
| `let { a, b } = r;` | `statement record pattern binding` (finding 20) |
| a directly authored payload member, `s.at` and `s.at = x` | `spelling member without a field symbol` (finding 19) |
| a field of a data payload binding, `inner.x` | `spelling member without a field symbol`; from a closed body, `expression field read beneath a case payload` |
| a case arm with a guard, a pattern field tested against a value, a pattern that binds its case, a second arm for one case, a wildcard no case reaches | `transition case arm with a guard`, `transition pattern field tested against a value`, `transition pattern that binds its case`, `transition case arm for a case an earlier arm selects`, `transition wildcard arm that no case reaches` |
| `crash` in a case arm | `transition crash in a case arm` (finding 21) |
| `match` on a scalar or with value patterns, on a subject that is not a place, with a data or Unit result | `expression match on a scalar subject`, `expression match arm on a value pattern`, `expression match on a subject that is not a place`, `expression match with a data result`, `expression match with a Unit arm` |
| a `match` arm with a guard or after the wildcard | `transition case arm with a guard`, `expression match arm after the wildcard` |
| `s in S::C` whose right side names no case | `expression case membership` |
| a pattern that names fewer than all payload fields without `..` | `typing pattern misses a payload field` |
| a clause that names a record formal whole, an `ensures` or crash guard that projects a formal, and any clause that projects `result` | `contract record formal named whole in a clause`, `contract clause that projects a formal or result`, `contract result of a record machine` |
| `==` and `!=` between records or between two sum values | `expression operator on a record` |
| a field or literal type the typed trees hold only as a spelling | `spelling member without a field symbol`, `spelling record literal without a type symbol`, `spelling record literal field without a symbol`, `spelling field without a symbol` |

A named constant never reaches this crate as a name: stage 04 and build-time
evaluation replace each use with the constant's materialized initializer, so a
scalar constant arrives as an integer literal (as it did in route one) and a
record constant as a record literal, and each is read as that literal. A name
that still resolves to a constant is refused as `expression const`.

## How identity is fixed

- **Types** are scalar only through `SymbolTable::builtin_type_atom` on the
  resolved symbol of a `Named` type: the compiler-installed root slot. The
  typed trees' `PrimitiveType::from_name` classifies by spelling and is not
  used. A `Named` type with an invalid symbol is refused as a spelling; a
  declaration spelled `u32` is a different symbol and is refused as nominal.
  Integer literals take their type from the landing stage 04 recorded.
- **Record types** are the `StructuralTypeId` minted for a `SymbolKind::Data`
  symbol's declaration the first time a type names it, a nested record before
  its owner. Nothing here reads a type's name, so data declared in another
  package would take the same route through its symbol and declaration; no
  program with such a type has been run through it yet.
- **Fields** are a position in that declaration and a `StructuralFieldId`
  minted with it, unique in the module. A literal's field and a member are
  matched to the declaration by the field symbol stage 02 recorded
  (`TableStructLiteralField::field_symbol`,
  `TableMemberExpression::member_symbol`); one without a symbol is refused as
  a spelling.
- **Cases** are a position among the declaration's cases and a
  `StructuralCaseId` minted with it, unique in the module. A case literal, a
  pattern head and the right operand of a case test are matched to the
  declaration by the case symbol; a payload field by its field symbol, as a
  field is. A case of another type than its subject's is refused
  ([structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#case-observation):
  "Its case identity must belong to the exact selected sum type; matching
  case spellings in another type supply no authority."). A member's
  `case_variant` is a spelling and is never read.
- **Text identities** exist in one place only. Terminal Psi's own type and
  field declarations carry an interned text
  (`StructuralTypeDeclaration::identity`), and `StructuralLeafCopy` names a
  path segment by that interned index. The reader takes the texts from the
  typed trees' own renderers (`normalized_nominal_type_identity`,
  `DataField::member_identity`), lowering copies them into the module, and
  nothing in this crate or the checker compares one.
- **Names** are slots only when the path has one member and a valid symbol
  equal to a formal's or local's symbol. A name without a symbol is refused.
- **`result`** in a contract is the result pseudo-value only through the name
  path's `reserved_result_of`, the contract owner that stage 02 records where
  it decides the name is the reserved result (`symbols.rs`,
  `assign_contract_call_symbols`). The owner must be the machine's one state,
  the clause an `ensures`, and the machine scalar. A name spelled `result`
  with no recorded owner is refused as a spelling. The pseudo-value is the
  value right after the scalar formals, with `ValueOrigin::MachineResult`, so
  the module's `TerminalMachineResult::Scalar`, its `ensures` propositions
  and the value table name one identity.
- **Formals** travel in Terminal Psi's two lanes. The `k`-th scalar formal is
  value `k + 1`, skipping record formals, and that is how a clause names it; a
  record formal is a root place and has no value identity.
- **Callees** are `MachineId`s reached from a `SymbolKind::Machine` symbol.
- No table here is keyed by text; text appears only in `Refusal::detail`.

## Evaluation order and control

From [the evaluation schedule](../../../../../wiki/spec/language/expressions.md#evaluation-schedule):
strict binary operands left then right, call arguments left to right, each
operation emitted after its operands, so block order is evaluation order.
`a > b` lowers to `IntegerLessThan(b, a)` over already evaluated operands;
`a != b` is `BooleanNot(IntegerEqual(a, b))`.

`left && right` ends the current block in a `Conditional` on the left value.
The true edge enters a block that evaluates `right` and jumps to a join with
it; the false edge goes straight to the join carrying the left value, which
is the result there. The join's one Boolean block parameter is the result.
`||` swaps the two edges.

A crash is a `Crash` terminator with its cause and its site guard: the exact
incoming conjunction of branch decisions on the path to it, each the branch
value equal to the Boolean its edge took ([crash](../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#crash):
"The guard retains the exact incoming conjunction"). Canonical consequences
used as route witnesses are proof information and are not added here. A call
of a machine with no normal completion is an `InvokeNoNormal` terminator whose
crash continuations are the callee's published routes with each formal
replaced by the actual at its position; nothing is dropped or simplified.
Both edges go to `Destination::NoNormal`. A call of a machine that may return
and also publishes crash routes stays a `Call` or `CallUnit` operation and
carries the same substituted routes as its crash continuations: the block
goes on after it, so the crash site is the operation
(`CrashSite::Operation`), not the block. Their dispositions are empty: the
subset mints no conserved obligation, and if one existed the edge is refused
rather than given a guessed fate (a crash abandons; `Fate::Abandoned`).

What a machine publishes ([guarded crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes)):
an authored `crashes` clause is its ceiling; a public body with none publishes
no route; a private body with none publishes each cause its body may reach,
from a `crash` statement or a callee's route, as the alternative `Truth`.
`Truth` is the widest route of that cause, so inference never hides a crash a
caller must cover; a narrower guard needs proof.

An authored `crashes none` is not an omitted clause. It leaves no contract
row, so the reader takes it from `Machine::declares_no_crash_route`, the fact
stage 01 records where it parses the clause and stages 02 and 03 copy. Such a
machine publishes no route and nothing is inferred for it, whether or not its
body can reach a crash by structure. effects.md says an explicit empty value
"promises the empty value and rejects any body or selection that violates
it": each crash site the body reaches owes coverage by no route, which
[lowered-discharge](../lowered-discharge/README.md) proves only where the
site is unreachable (`requires d != 0` against a `d == 0` guard) and rejects
everywhere else.

Stage 01 desugars `transition subject { arms }` into a `DispatchSubject`
local, evaluated once, followed by one `Transition` statement per arm
(`01_tokens-to-syntax-trees/src/bodies_and_transitions.rs`,
`parse_transition_block_handles`). Each guarded arm ends the current block in
a `Conditional`: true enters the arm, which completes the machine; false
continues with the next statement. An unguarded arm completes the machine and
must be last. An arm's target is a value, a terminal call (a call whose
result completes the machine, [state contracts](../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms):
lowered as a call followed by a return) or Unit.

Stage 01 writes dispatch over a sum the same way: a local that holds the
subject, then one transition per arm guarded by `subject in case`, with every
use of a payload binding replaced by the payload member of the subject it
names. The reader takes the run of case arms over one subject, and the
unguarded arm after them as the wildcard, as one dispatch. Lowering ends the
block in one `StructuralCase` on the subject local's root, with one edge per
declared case in declaration order, built from the declaration's case table:
to the arm that names the case, else to the wildcard arm's block, which the
uncovered cases share. A declared case with neither is refused there with
reason `Unestablished`
([patterns](../../../../../wiki/spec/language/patterns.md): "Every dispatch
must cover the admitted subject. Missing runtime coverage is a compile
error, not an implicit trap or fallthrough."). Each scalar payload field of
an arm's case is one parameter of the arm's block
([control flow](../../../../../wiki/spec/terminal-psi/control_flow.md):
"Selected payload fields become scalar parameters on that case's edge before
cleanup."), and a payload member inside the arm is that parameter. A data
payload field is no parameter; using it copies it out with a
`StructuralLeafCopy` whose path is the case and the field.

Value-position `match` is the same terminator on the place the subject
names, with no copy: each arm evaluates its value in its own block and jumps
to a join whose one parameter is the result. An arm is an expression, so it
rebinds no slot and stores nothing, and the result is the only thing that
differs across the arrivals.

Scalar locals are SSA values: a binding or assignment names the value the
slot holds from then on. Every transition arm completes the machine, so the
only joins are the short-circuit joins and the join of a `match`. A machine with no record has no structural
place, and its place, root and write tables are empty because the form
implies none.

## Places, roots and the write log

A record value is storage, so lowering a record fills three tables in step
with the Terminal Psi operations that create the storage
([lower/places.rs](src/lower/places.rs)).

**Roots.** One Terminal `PlaceId`, one `PlaceRow { origin: Root }` and one
`roots` entry for each record formal (`StructuralPlaceKind::Parameter`), each
operation that produces a record (`OperationResult`: a literal, a whole copy,
a call with a record result) and a record machine's result place (`Result`).
A record local has no place of its own: its slot holds the root its
initializer, or the last whole value assigned to it, produced. A local
declared without an initializer is the one local with a home of its own: the
root of the operation that reserves its storage. `PlaceId`s are dense per
machine, like every identity here.

**Projections.** `r.a` is the row `(r, Segment::Field(a))`, found by binary
search of the parent's sorted `children` and pushed only when absent, so a
path is one `ExactPlaceId` however often it is used.

**Writes.** Each row says what one operation stored and where, at
`ProgramPoint::AfterOperation` of that operation. Lowering never asks what a
place holds: answering that needs the graph, and the checker is the one that
walks it.

| Operation | What is logged |
| --- | --- |
| `EstablishRecord` | each scalar field `Content::Value(operand)`; each record field one `Content::Overwritten` on that field's place |
| `EstablishScalarCase`, `EstablishStructuralCase` | one `Content::Overwritten` on the root, then each scalar payload field `Content::Value(operand)` and each data payload field one `Content::Overwritten`, on the place `Case` then `Field` |
| `StructuralScalarFieldStore`, `WriteOnlyPrimitiveStore` | `Content::Value(stored)` on exactly the leaf it names |
| `StoreStructuralField` (a record into a field) | one `Content::Overwritten` on that field's place |
| `StructuralLeafCopy` | one `Content::Overwritten` on its fresh root |
| a call with a record result | one `Content::Overwritten` on the root: the callee's values have no identity here |
| `EstablishZeroedStructural`, `EstablishPrimitiveLocal` of a local with no initializer | nothing |

No row says which case a root was established as: `Content` has no such
form. The root's row says it has new contents, and the checker reads the
case from the establishing operation at that row.

`Content` has no variant for "what that other place held". So a whole copy,
a record operand and a record stored into a field log only that the
destination has new contents; the operation at the row's point names the
source, and the checker resolves each leaf through it with the same walk it
uses for a read. A sibling of a stored leaf gets no row and keeps its
contents, and no row is ever edited, so what was known about an old value
stays known.

What a formal holds at entry is not a write. It is one value per scalar leaf
with `ValueOrigin::EntryContent { place }`, minted the first time an
operation names the leaf or a place above it and shared by every later use.
No value is invented.

**Stores.** `r.a.b = x` evaluates `x` and emits one
`StructuralScalarFieldStore` whose path is the carrier `a` and whose field is
the leaf `b`; a plain field has no declared range, so `range_obligation` is
`None`. `l.from = p` constructs the value first (a literal is its own root, a
local is copied whole) and replaces the field with one
`StoreStructuralField`, never with member stores
([structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary):
"A whole record assignment constructs its operands before replacing the
destination through the ordinary structural replacement operation ... Root,
field, and indexed destinations do not change construction semantics or
decompose the assignment into member stores"). Whole assignment to a local,
`r = R { ... }`, constructs the value and then the local names that root:
this is what the current pipeline emits for a root destination, and it is
the same rebinding a scalar assignment does. Stores go straight into the
root a local names ("A whole, unqualified, claim-free owned record home
(entry, block parameter, or completed establishment/call result) supplies
write authority directly"); the current pipeline first moves a `mut` local
into a block parameter, which this subset has no join to need.

**Locals with no initializer.** `let mut r: R;` emits
`EstablishZeroedStructural` and `let mut x: u32;` a zero constant and
`EstablishPrimitiveLocal`, the operations the current pipeline emits, so the
module runs.

A data local is zero-established. Its declaration logs one row for the
root's new contents and, on each scalar leaf of the zero value, a constant
zero of the leaf's type, emitted just before the establishment. The zero
value of a sum is its first declared case with a zeroed payload
([data and literals](../../../../../wiki/spec/language/data_and_literals.md):
"The ordinary zero representation of a sum selects the first declared case
and a recursively zeroed payload."), so only that case's payload leaves get a
row. The current pipeline emits no constant for a data home; the constants
are what gives each leaf's content a value identity the module contains, the
way its own scalar home's constant does. The establishment owes the
`requires` clauses of the type and of every data-typed place of the zero
value, and the checker decides from them whether the storage is an
established value or the type is gated
([lowered-discharge](../lowered-discharge/README.md#zero-establishment)).

A scalar local is zero-constructed by the same rule
(`local-without-initializer`): `let mut x: u32; f(x)` reads zero, as the
current checker accepts. Lowering logs `Content::Value(zero)` on the scalar
home at its `EstablishPrimitiveLocal`, naming the zero constant emitted just
before it
([dependent values](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization):
"If zero satisfies the default domain, the value is zero-constructible.
Otherwise the type is gated: construction or checked qualification must
establish the domain before observation."). Every scalar type the reader
accepts (`i8`..`u64`, `bool`) admits zero, and a qualified or ranged scalar
type is refused as `type constrained`, so a scalar home owes no domain
clause and no gated scalar occurs in this subset. The sentence "An
unestablished local cannot be read"
([structural access](../../../../../wiki/spec/terminal-psi/structural_access.md))
governs declared shared view locals, which this route does not serve. Such a
scalar local is a storage home, written by `WriteOnlyPrimitiveStore` and read
by `PrimitiveScalarRead`; every other scalar local stays an SSA value.

<a id="data-requires"></a>
**Data `requires`.** A declaration's `requires` is the type's default domain
([dependent values](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization):
"A data declaration's field qualifications and `requires` clauses define its
default domain"). The reader reads each clause with the clause reader the
machine contracts use (`read/contracts.rs`), with `self`, its fields named
bare, and member paths beneath them in scope, and with two forms a machine
contract does not have: `self in S::C` and `||`. A clause is kept in
`RecordType::requires` in the form Terminal Psi gives a clause over storage:
a field is a field term over one root that stands for `self` and the dense
field and case identities down to the field, and a case test is a
case-membership proposition on such a path. Nothing proves or knows a clause
in that form. Each type's declaration in the module states its clauses
(`StructuralTypeDeclaration::requires`), so they are encoded with the module
and part of its identity; the current pipeline proves them on checked trees
and its declarations state none. `Elaboration::domains` restates them by the
type's identity for the checker, which refuses a table that differs from the
declarations.

Lowering mints one `DataDomain` obligation per owed clause, at the point
before the operation that owes it
([data and literals](../../../../../wiki/spec/language/data_and_literals.md):
"Construction and every restoring write prove the owning default-domain
clauses."):

| Operation | Clauses owed, in the order `DataDomain::clause` numbers them |
| --- | --- |
| `EstablishRecord`, `EstablishScalarCase`, `EstablishStructuralCase` | the constructed type's |
| `EstablishZeroedStructural` | the type's, then those of each data-typed field of the zero value in declaration order, parent first |
| `StructuralScalarFieldStore`, `StoreStructuralField` | those of the type that declares the written field, then of each type that contains it, outward to the root |

A record stored whole into a field owes nothing for its own type: it was
established where it was constructed. The goal of each obligation is built
by the checker, not here.

**The Terminal Psi forms.** Operands of a literal are evaluated in authored
order and the one `EstablishRecord` follows them with its roster in
declaration order. A scalar field read is `ScalarStructuralField` (integers)
or `BooleanStructuralField` (`bool`), naming the whole root, the carrier path
by `StructuralFieldId` and the final field; neither holds text. A record used
as a value is a root nothing else names: a literal or call is its own root, a
local, formal or member path is copied whole by `StructuralLeafCopy`. A call
takes each record argument whole and owned
(`CallStructuralScalar`, `CallStructuralWithScalarArguments`, `CallUnit`) and
is followed by a `Jump` into a continuation block whose `cleanup_actions`
retire the consumed homes. A record result returns through
`ReturnStructural`.

**Cases.** A case literal evaluates its payload operands in authored order
and then establishes once: `EstablishScalarCase` with no field for a case
without payload, `EstablishStructuralCase` with the payload roster in
declaration order otherwise, which are the operations the current pipeline
emits. `s in S::C` is one `StructuralCaseMembership` on the sum in place. A
scalar payload member outside a dispatch on its place is a
`ScalarStructuralField` whose carrier ends in the one `Case` step.

**Module declarations.** A module that names a record declares, once for the
module, what the codec and the interpreter require: `structural_types` (one
`Record` per record type, `copyable`, `Packed`, each field `Scalar` or
`Structural`; one `Sum` per sum type with its cases and their payload
fields), `identities` (the interned type, field and case texts), and
`native_layouts` (one natural row per record and none per sum, with no
policy, no realization and absent nominal identities). Each scalar type some local is homed in is
declared after the records as a `PrimitiveScalar` type, with the identity the
typed trees render for it. A scalar module with no such local declares none
of them.

The record flags (`copyable`, `no_code_affine_cleanup`, `descriptor_cleanup`,
no linear claim) are the ones the current pipeline emits for the same plain
`[copy]` record, checked by decoding its artifact. One column differs: the
current pipeline sets `inline_carrier` on each scalar field to the primitive
structural type of that scalar, and this route leaves it `None`. The
interpreter loads and runs both.

## Obligations

| Kind | Where | Created at |
| --- | --- | --- |
| `OperationSafety` | each `Exact` add, subtract, multiply, divide, remainder, shift (every primitive whose `integer_policy_bridge` has formation conditions) | the point before the operation |
| `CallRequirement` | one per callee `requires` clause per call, clause = position in the callee's canonical `requires` | the point before the call |
| `PublishedGuarantee` | one per `ensures` clause, the identity `ContractClause` carries | the entry block |
| `NormalGuarantee` | one per `ensures` clause per normal exit edge, naming its `PublishedGuarantee` | that edge |
| `ArrivalInvariant` | one per edge into a block with parameters (only short-circuit joins have them); the predicate is the checker's | that edge |
| `CrashCoverage` | one per crash site and cause. `CrashSite::Block`: a `Crash` terminator (its one cause), or an `InvokeNoNormal` terminator (each cause among its continuations). `CrashSite::Operation`: a `Call` or `CallUnit` whose callee may return and publishes crash routes (each cause among its continuations) | the point before the terminator or the call |

## Source correspondence

`MachineSources` holds one `SourceOrigin` per block, operation, edge and
obligation, indexed by the same identity and minted in lockstep with it.
An origin is the authored span, or `Unpositioned` with the reason
(`MachineEntry`, `ShortCircuitRight`, `ShortCircuitJoin`,
`DispatchContinuation`, `ImplicitReturn`, `ArgumentRetirement`, or
`SpanAbsent` when the typed tree's span is empty). It never borrows a neighbour's position.
`LoweredSourceCustody` was not used: it records call occurrences only and
depends on `checked-trees`.

## What is reused from the current pipeline

Pure helpers only: `numerics::integer_policy::integer_policy_bridge` (which
operations are partial), the Terminal Psi data types,
`terminal_codec::canonical_proposition_order_key` and
`canonical_scalar_term_order_key` (Terminal Psi's canonical order). Nothing
from Psi 05, validation, checked trees or Psi 06 is used or depended on.

## Findings

Each is a gap between this route and the contract it builds against; none was
worked around by adding to `terminal-check-tables`.

1. **Inferred routes are `Truth` per cause.** A narrower guard would be over
   body values, not formals, and stating it over formals is proof work.
2. **One `ensures` identity per clause, one obligation per exit.**
   `MachineContract::ensures` holds a `ContractClause` with one `ObligationId`
   per clause (`terminal-psi/src/proof.rs`), while control flow requires every
   normal return to establish each guarantee. Lowering mints one
   `PublishedGuarantee` per clause, which the `ContractClause` carries, and one
   `NormalGuarantee` per (normal return edge, clause), which names it through
   its `published` field. The published clause holds when every return
   edge's guarantee holds.
3. **No summaries.** `Summary::requires` and `result_relation` are
   `PropositionId`s into "the module's proposition table", but Terminal Psi
   keeps contract propositions inline in `MachineContract` and its only
   `PropositionId` table (`proposition_declarations`) is named propositions
   with a `String` name. `CheckTables::summaries` is left empty; published
   crash routes live in `MachineContract::crash_routes` only.
4. **`requires` is a canonical set in Terminal Psi.** The codec requires
   `MachineContract::requires` to strictly increase in canonical byte order,
   so clauses are sorted and a duplicate clause is refused. Clause positions
   in `CallRequirement` are canonical positions, not authored ones.
5. **The machine declaration is left `None`.** `CheckedMachineDeclaration`
   holds identity-table text; producing it is identity work this route does
   not do. The codec and the interpreter accept the module without it.
6. **A value arm's test is stored twice.** For `true -> X` on a captured
   subject, stage 03 keeps a `When(subject == true)` guard and, as a separate
   expression copy, a non-structural pattern whose `guard` and `condition` are
   one handle. The reader evaluates the guard once and refuses the arm unless
   the copy reads to the same computation (`same_computation`, over slots,
   literal values, operators and callee identities).
7. **Calls name the callee's entry state.** `target_symbol` and transition
   paths hold the `State` symbol; the machine is that symbol's parent, and the
   reader checks that the machine's one state is exactly that symbol. Every
   authored call also carries an authored selection occurrence and a static
   application whose registered operation is that state; both are accepted
   only when they resolve to exactly the callee and apply nothing.
8. **Stage 04 leaves some literals unlanded.** An integer literal with no
   landing takes the type its context expects (initializer, assignment,
   argument, return, arm result, or the other operand), per
    [literals and expected types](../../../../../wiki/spec/language/domains.md#literals-and-expected-types);
    with no expected type it is refused.
9. **Every machine carries a reach row.** A machine with no reach holds
    `ServiceReachRowTable::EMPTY_ROW`, not `NULL`; both are accepted, any other
    row is refused.
10. **A field read is `ScalarStructuralField`, not `PrimitiveScalarRead`.**
    The lane plan's table names `PrimitiveScalarRead`; the specification
    gives a static field path the other form
    ([structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary):
    "A carrier with neither a runtime element nor a reference crossing keeps
    the canonical `ScalarStructuralField` or `BooleanStructuralField` read."),
    and it is what the current pipeline emits. It is also the form with no
    text in it: its path is `CanonicalStructuralPathSegment::Field(StructuralFieldId)`.
11. **`StructuralLeafCopy` of a member path needs the interned text index.**
    Its path is `StructuralPathSegment::Field(TerminalIdentityIndex)`.
    Lowering fills it from the field's own declaration row and compares
    nothing. The checker needs the copy's source place, which no table row
    can hold, and gets it by asking the lowered form to resolve the path
    (`terminal_semantics::runtime_structural_field`); it compares no
    identity itself.
12. **The decoder requires a native layout row per record.** A module with a
    record type and an empty `native_layouts` encodes and then fails to load
    ("native layout catalog differs from the complete ordered record
    inventory"). Lowering adds the natural row. Its nominal native identities
    are absent: producing them is the native layout review's identity work,
    which this route does not do.
13. **The write log cannot say "copied from that place".** `Content` holds a
    value, a reference, unknown contents or a hole, and `WriteRow` has no
    source column. A whole copy therefore logs `Overwritten` and the checker
    reads the source from the operation. A `Content` variant naming the
    source `ExactPlaceId` would let the log stand alone; this lane adds no
    such variant and reports the gap instead.
14. **A return does not write the result place.** A record machine's result
    place is a root with no log entry. The slice that states `result.field`
    clauses logs the returned leaves on the return edge.
15. **Record constants are served as literals, with no constant route.** The
    typed trees hold each use as the materialized initializer, so there is
    nothing left to refuse or to resolve.
16. **A transition's payload bindings are block parameters, not reads.** The
    lane plan's table gives "payload reads on the selected edge". The
    specification gives a root sum's payload the other form
    ([control flow](../../../../../wiki/spec/terminal-psi/control_flow.md):
    "Selected payload fields become scalar parameters on that case's edge
    before cleanup."; structural access: "producers read a root sum's
    payload as parameters of its dispatch block"), and it is what the
    current pipeline emits for `transition`.
17. **Value `match` uses the dispatch terminator; the current pipeline does
    not.** For `match` the current pipeline emits a chain of
    `StructuralCaseMembership` tests and conditionals with payload reads
    whose carrier ends in the case. This route emits one `StructuralCase`,
    as the lane plan asks ("The same terminator; arms join with the result
    as a block parameter"). Both forms run in the interpreter.
18. **Where coverage is rejected today.** A `transition` over a sum that
    misses a case never reaches this crate: stage 03 rejects it
    (`equality_and_exhaustiveness.rs`, "match over `S` does not cover").
    A `match` that misses a case, a pattern that names fewer than all
    payload fields, and a partial record pattern are rejected by
    `validation` (`validate_match_coverage`, `validate_for_subject`,
    `validate_destructure_exhaustiveness`), which runs after the typed trees
    this route reads are frozen. So this route decides the first two itself.
19. **A directly authored payload member has no field symbol.** `s.at` and
    `s.at = x` on a sum reach the typed trees as a member with a name and an
    invalid `member_symbol`; so does a field of a data payload binding,
    `inner.x`. Resolving one would mean comparing the name with the payload
    fields' names, so each is refused as a spelling. The payload members
    this route reads are the ones stage 01 substitutes for pattern bindings,
    which carry the field symbol. Serving the rest needs the symbol recorded
    where the member is typed.
20. **A record pattern's field roster exists only as text.**
    `let { x, y as _ } = p;` reaches the typed trees as a marker local named
    `__destructure#x#y` followed by one ordinary field-read local per bound
    field. Which fields the pattern mentions is in the marker's name and
    nowhere else, and the current checker splits and compares it by spelling.
    [Patterns](../../../../../wiki/spec/language/patterns.md#irrefutable-data-bindings)
    requires the pattern to name every field, and accepting the marker
    without checking that would accept `let { x } = p;`. It stays refused
    until the marker carries its fields' symbols.
21. **A crash inside a case arm has no exact site guard.** The guard is the
    conjunction of the branch decisions before the crash, each a value equal
    to a Boolean. Which case a dispatch selected is not a value, so the arm
    is refused instead of given a wider guard.
22. **Mixed data is refused.** A case literal of a type with common fields
    beside its cases names none of them (`Token::Mark { kind: 2 }`), so what
    they hold after construction is a rule no slice of this lane lowers yet.
23. **`ValueType::Record` and `RecordType` name records and sums.** A sum is
    the same row with `cases` instead of `fields`; the names were kept so
    the slices before this one read unchanged.
24. **Terminal Psi carries no data `requires`.** For
    `data R requires low <= high`, the current pipeline's module holds the
    construction and the store with `range_obligation: None` and no clause
    anywhere: it proves the domain on checked trees. The clauses therefore
    travel in `Elaboration::domains`, a field of this crate's result, not in
    the module and not in `terminal-check-tables`.
25. **No field in this subset has a `range_obligation`.** The slot is set
    for a field of bounded or refined type (`StructuralFieldType::BoundedInteger`,
    `RefinedScalar`), which a constrained field type or a named domain
    produces. A plain integer field under a data `requires`, one-sided or
    two-sided, lowers with `range_obligation: None` in the current pipeline,
    so this slice mints no `OperationSafety` for a field.
26. **Invariant windows mint nothing new here.**
    [Dependent values](../../../../../wiki/spec/language/dependent_values.md#invariant-windows):
    "A write known to preserve the default domain keeps it established.
    Otherwise the write opens a window on the place ... The domain must be
    proved again at the next consumption point". The store's or zero
    establishment's own `DataDomain` row is the window's identity, and
    discharge finds the consumption points from the lowered form
    ([lowered-discharge](../lowered-discharge/README.md#invariant-windows)).
    `ObligationKind::DomainDebt` is not minted: it names a
    `DomainSemanticId`, which a data `requires` clause does not have, and a
    conserved row would need a disposition on every edge, which
    `no_successor_dispositions` refuses today.
27. **An entry `requires` may project a record formal; nothing may project
    `result` yet.** `requires p.first == 7` is read as a field term rooted
    at the formal's place (the `k`-th record formal is place `k + 1`), the
    form the module's contract carries; the checker states it over the
    formal's entry content inside the machine and over the actual's leaf at
    each call. `ensures result.x <= 9`, `ensures result in S::C`, a formal's
    field in an `ensures` and in a crash route guard are still refused as
    `contract clause that projects a formal or result`: they need the
    returned leaves logged on the return edge (finding 14) and a result
    leaf stated as a value in the caller, which this slice does not do.
29. **A machine generated for a static application keeps its declaration's
    identities.** Stage 04 generates one machine per application of a
    generic machine; its state symbol's `generated_from` is the declared
    machine. The call's authored selection and binding still name the
    declaration, and the generated machine's `ensures` clause still records
    `result` against the declaration, because it shares the declaration's
    clause trees. The reader accepts each where the symbol is the callee's
    `generated_from` and refuses any other mismatch. Nothing is matched by
    name.
30. **A trait requirement is a bare signature.** The typed trees hold a
    requirement as a `StateSignature`, not as a machine, so the reader has a
    second entry to the one contract reader. An authored `crashes none`
    leaves no contract row on either; stages 01 to 03 record it as
    `StateSignature::declares_no_crash_route`, as they do on `Machine`, and
    the reader takes it as `DeclaredNone`.
    [Generics](../../../../../wiki/spec/language/generics.md#requirements-and-conformance-evidence)
    gives the two spellings different meanings ("omitted `crashes` defers to
    its published routes"; "`crashes none` admitting only crash-free
    members"). Both publish no route in the generic form, so the generic
    check decides nothing about crashes through the call either way. The
    current checker refuses a member that is not crash-free where it
    satisfies a `crashes none` requirement; the deferral is not yet carried
    to generic callers
    (`tests/omega/pending/traits/omitted_requirement_crashes_reach_generic_caller`).
31. **The declared machine's call through a parameter names a child of the
    parameter.** In the generic declaration `Selected::apply(x)` targets a
    state symbol under the parameter's symbol, with no requirement recorded
    on the call. Which requirement it stands for is the typed trees' own
    relation, `TypedTrees::evidence_requirement_declaration` (position among
    the parameter's children against the trait's requirements); the reader
    calls it and compares no name.
32. **Terminal Psi cannot name an unselected requirement.** A call operation
    holds a `MachineId`, and a `TerminalMachine` is blocks. The generic form
    gives the requirement a `MachineId` and a machine with an empty block
    list, which no validator, codec or interpreter may be handed. That keeps
    the caller's obligations on the ordinary call rows; a later lane that
    wants requirements out of `module.machines` needs a callee row for one.

