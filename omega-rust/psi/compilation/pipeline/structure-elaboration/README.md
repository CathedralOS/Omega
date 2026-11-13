# Structure-only elaboration

This crate turns frozen typed trees into an unsealed Terminal Psi module and
fills the [check tables](../../representations/terminal-psi/check-tables/README.md)
that describe it, for route one's scalar subset and for plain data: records
(literals, field reads, stores, whole copies) and sums (case literals, case
tests, dispatch and `match` with payload bindings). It is the second piece of
[check on the lowered form](../../../../../wiki/drafts/designs/check_on_lowered_form.md):
it proves nothing, reads no checker fact, and mints every obligation the
lowered form implies without discharging any. It is not wired into the
default compile and replaces nothing yet. To run it and the discharge on a source
program without the Build or the old checker, see
[Running the checker on a source program](../lowered-discharge/README.md#running-the-checker-on-a-source-program).

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
member paths, case tests, case patterns and record patterns) and [subjects](src/read/subjects.rs)
(which declarations a compilation must judge besides those its entries call). `lower.rs` holds the `Lowering` state, the per-machine entry point and
`finish`, beside [control](src/lower/control.rs) (blocks, edges, arms, crashes,
returns), [operations](src/lower/operations.rs), [calls](src/lower/calls.rs)
and [places](src/lower/places.rs) (roots, projections, the write log, record
and case construction, field and payload reads, whole copies, the dispatch
terminator).

## The subset

Machines with one state and a complete body whose formals, locals and result
are builtin integers (`i8` to `u64`) or `bool`, using: scalar locals and
assignment to them; integer and Boolean literals; wholly anonymous `+`, `-`,
`*` and `/` trees over unsuffixed literals, evaluated as exact rationals (so
`7 / 2` is `7/2`, not 3) and lowered as one literal when the completed value
is an integer of the context's type reached with no fractional intermediate,
any other such tree (fractional anywhere, out of range, or under an operator
with no anonymous meaning) being refused; builtin `Exact` integer
arithmetic, bitwise operations, shifts and comparisons; integers of type
`T in Wrapping` and `T in Saturating` with their arithmetic
([Arithmetic policies](#arithmetic-policies)); `!`, `~`, `&&` and
`||`; `as` between two integer carriers ([Casts](#casts)); direct calls of other such machines with `requires` clauses over their
formals and `ensures` clauses over their formals and `result`; `crashes Trap`
and `crashes Abort` clauses with or without route guards over the formals;
`crashes none`; an integer `!=` in a clause, read as `a < b or b < a`, the
form the kernel denotes a false integer equality by; a `requires` or
`ensures` clause that compares mathematical integers
([Mathematical integers in a clause](#mathematical-integers-in-a-clause));
`crash Trap;` and `crash Abort;`; guarded transition arms; and returns. A call
of a machine that may return is an operation wherever it stands, and carries
the callee's crash routes when it publishes any. A call of a machine with no
normal completion is served only in terminal position (an arm target or a
call statement), where it becomes the block's no-normal invocation.
Everything else is refused with `RefusalReason::Unsupported` and a detail
naming the construct (`expression match`, `type reference`, `call no-normal
call inside an expression`, ...): a reference outside [References](#references), slice, generic, callable value, suspension, assembly, float,
domain, cast that names a domain, or service reach is never skipped, approximated or passed through. A
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

Three kinds of generic parameter are read, as an experiment for
[the checker's generic bodies](../lowered-discharge/README.md#generic-bodies-experiment):
an implementation parameter (`Selected satisfies Step`), through which the
body calls members (`Selected::apply(x)`), a value parameter (`const
Units: u64`), which the body and its contract name like a formal, and an
opaque type parameter (`T`), whose values the body can pass on. A machine
whose generic parameters are all of these kinds is read in two forms.

- **Its specializations**, by `elaborate`. Stage 04 closes each static
  application `advance<Double>` by generating a machine for it, so the
  typed trees already hold one closed machine per application. A call of
  `advance<Double>` targets that generated machine and is read as a direct
  call; inside it, the call through the parameter targets the selected
  member and is read as a direct call of that member. Each such call is
  recorded in `Elaboration::selections` with the parameter, the trait, the
  requirement and the member, all symbols. A machine generated for a
  value argument (`clamp<7>`) has an integer literal wherever its
  declaration named the binder, so it reads as any closed machine. Only a
  declaration whose generic parameters are all implementation or value
  parameters has its generated machines read; an application with a type
  argument is still `call static application arguments`, and a member named
  through the implementation itself (`Double::apply(x)`) is still `call
  static requirement dispatch`.
- **The generic check form**, by `elaborate_generic`. The declared machine
  is read with its parameters rigid. A call through an implementation
  parameter names the requirement, which becomes a machine with a
  signature, a contract and no body (`Elaboration::requirements`,
  `lower_generic`); it lowers to a machine with no block. A value parameter
  becomes a scalar formal after the authored ones, of the binder's carrier
  and with no definition (`Elaboration::value_parameters`): a name in the
  body or in a clause resolves to it by the binder's symbol, and all that is
  known of it is what the `requires` say. Only an integer carrier is
  served. A generic form is never sealed, encoded or run.

**A requirement with value parameters** (`trait Bounded { machine
value<const Units: u64>(input: u64) -> u64 requires input <= Units ...; }`)
is read at the literals one call binds them to: `Implementation::value<7>(x)`
is held to the contract with 7 where the clauses name `Units`
(`requirement_signature`, `ClauseScope::bound`). The literals are the call
site's own static arguments, each of which must be exactly one integer
literal (`StaticMachineArgument::const_literal`); a requirement is one
machine per tuple of them (`Elaboration::value_arguments`). In a generated
machine the same call targets the machine stage 04 generated from the
selected member for those arguments, and its selection records the literals
(`Selected::value_arguments`). Stage 04 also records the requirement's
signature at those arguments beside the dispatch
(`StaticRequirementDispatch::contract_signature`); the reader states that
contract itself and does not read the record, which only Psi 05's helpers
can resolve.

**A type parameter** of the generic check form's entry is rigid: a formal
or result of type `T` has `ValueType::Rigid`, which is neither a scalar nor a
record, so every operator, comparison, field and literal rule refuses it.
What each type parameter in scope stands for is kept beside the machine
being read (`Program::scope`), found by the binder's symbol. A call of
another generic machine at the entry's own type parameters,
`forward<T>(item, limit)`, is read from the bindings of the call's static
application: the callee becomes one machine per tuple of type arguments,
with its signature and contract at those types and no body
(`Program::application`, `Elaboration::applications`). A value of `T` may be
named once in a body unless `T` is declared `[copy]`; a `[linear]`
parameter, a local of type `T` and a call that returns a `T` are refused. A
machine stage 04 generated for a type argument is a closed machine and
reads as one, as far as its clauses and body are in the subset.

**A generated machine of a finite type family** keeps its type equations.
Stage 04 does not fold them: the machine generated from `threshold<T>
requires T == u32 || T == u64 { transition T == u32 { true -> (37u32) _ ->
(73u64) } }` for `u32` still holds `requires u32 == u32 || u32 == u64`, a
local bound to `u32 == u32`, both arms, and an arm that returns a `u64`
from a machine whose result is `u32`. Such an equation is static
([generics.md](../../../../../wiki/spec/language/generics.md#static-type-equality):
"a branch with established `T == u32` may use that equality to check its
operations and values as u32. On leaving the branch, only facts common to
its surviving predecessors remain"), so the reader decides it
(`static_truth`): two closed builtin types are equal exactly when their
symbols name the same compiler-installed atom, and a local bound to such a
truth, and `==`, `&&`, `||` and `!` over those, are decided with it. A
`requires` or `ensures` clause decided as true adds no clause, and one
decided as false is refused. An arm whose guard is decided as false is the
branch of another type: it is left unread, because it is typed under an
equation that is false here. An arm decided as true is taken
unconditionally, and the arms after it are left unread. A lone Boolean
literal decides nothing this way: only a type equation makes a condition
static. An equation with a rigid type parameter, a data type or an
application on either side is not decided, so the opaque generic form of
such a machine is refused (`expression type expression`).

**The family itself** is read once per alternative it lists, by
`elaborate_alternatives`: `family_alternatives` finds the one `requires`
clause that is a disjunction of `T == X` with `X` a builtin scalar type,
and each check reads the declaration as its entry with `T` fixed to one `X`
(`Reading::Alternative`). A type equation naming `T` is then decided as if
stage 04 had substituted it. Inside a machine contract clause a type
equation guards what follows it (generics.md: "Conjuncts are read left to
right; a disjunct whose type equation is false for the instantiation holds
vacuously"): a conjunct decided true is dropped, and a disjunct a type
equation decides false (`guarded_false`) contributes nothing and is not
read, so a conjunct typed only under another alternative is never typed.
What remains must be a clause the reader serves; a clause decided false
outright is refused.

`elaborate_witness` builds the statement that an implementation refines a
requirement: the requirement's contract, at the given value arguments, on a
body that is one call of the implementation. `template_of` gives the
declaration a generated machine came from. A callable parameter, a
runtime-capable value binder (`Count: u32` without `const`), a value
parameter of a carrier other than an integer, and a type parameter with a
declared type equation are still refused as `machine generic parameters`
or by the parameter's own refusal (findings 29 to 32).

A contract clause that is the literal `true` adds no clause: it asks and
promises nothing, as an absent `requires` or `ensures` does.

### Casts

`operand as T` is read when both the operand's type and `T` are builtin
integers and neither carries a domain
([domains](../../../../../wiki/spec/language/domains.md#carrier-casts): "The
operand of a carrier cast must carry no domain."). Lowering emits the one
exact conversion the two carriers determine: `IntegerWiden` when every
operand value is representable in `T` (a wider carrier of the same sign, or
unsigned to a wider signed one), and otherwise `IntegerExactCast` with an
`OperationSafety` obligation ("exact widening, or narrowing with
representability proved"). A cast to the operand's own carrier converts
nothing and is its operand.

The reader decides from what the typed trees record, never from the cast's
spelling: a recast (`CastForm::Recast`), a cast with a domain symbol, a cast
into `Trapping`, a cast that changes the carrier of a policy-qualified
integer ([Arithmetic policies](#arithmetic-policies) serves the one that
keeps it), and an operand with no carrier of its own (an anonymous
literal or rational expression, which the
[numeric landing rules](../../../../../wiki/spec/language/numeric_values.md#fractional-intermediate-diagnostics)
land) are each refused as `Unsupported`.

### Arithmetic policies

An integer formal, local, result or field may be of type `T in Wrapping` or
`T in Saturating`, for a builtin integer `T`. Such a type is a different
type from `T` (`ValueType::Qualified`), stored and compared as `T`
([numeric values](../../../../../wiki/spec/language/numeric_values.md#overflow-policies):
"Adding and removing a policy domain ... change future operations, not
existing payloads"). Every typing rule compares the whole type, so a
destination takes only a value whose type carries exactly its policy
([domains](../../../../../wiki/spec/language/domains.md#literals-and-expected-types):
"the supplied expression's static type must carry exactly the domains the
destination declares").

- **Operators.** `+`, `-`, `*`, `/`, `%`, `<<` and `>>` over two operands of
  one policy, or over one such operand and a literal, are that policy's
  operation and their result carries the policy
  ([policy domains](../../../../../wiki/spec/language/numeric_values.md#policy-domains)).
  Lowering names the operation, `WrappingIntegerAdd`, `SaturatingIntegerAdd`
  and so on. A Wrapping or Saturating add, subtract and multiply is total
  and owes nothing; a divide and a remainder owe their divisor under either
  policy ("An overflow policy does not invent a result for undefined
  operands such as an integer zero divisor"); a Wrapping shift reduces its
  count and is total. A shift's count keeps its own plain type. The
  comparisons compare the stored values and return `bool`.
- **Casts.** `x as T in Wrapping` and `w as T` add and remove the policy.
  The cast is its operand (`ExpressionKind::Requalify`): no operation is
  emitted, and the operand keeps whatever operations its own type chose.
  When `x` is a plain integer of another carrier, the carrier is converted
  first by the one exact conversion of [Casts](#casts), with its
  `OperationSafety` obligation when it narrows: a policy never makes `as`
  truncate or clamp
  ([domains](../../../../../wiki/spec/language/domains.md#domain-casts):
  "`as` never calls user code, rounds, saturates, traps"). A literal operand
  lands in the carrier the cast names, so `5 as u32 in Wrapping` is the
  Wrapping constant.
- **Suffixed operators.** Stage 04 rewrites `a +% b` to
  `((a as T in Wrapping) + (b as T in Wrapping)) as T` and `a +| b`
  likewise, so both are read as the casts and the operation above.
- **Contracts.** A clause names a policy-qualified formal, field or result
  as its stored value.

The checker takes each operation's obligation and result equation from
Terminal semantics, so a policy operation is judged as itself: nothing here
tells it that a Wrapping sum is the mathematical sum, and it has no rule that
bounds one.

Refused by name:

| Construct | Refusal detail |
| --- | --- |
| `T in Trapping`, whose every operation is its own `Trap` site, and a cast into it | `type constrained by Trapping`, `expression cast into Trapping` |
| a type constrained by a declared domain, alone or beside a policy | `type constrained by a domain that is not one arithmetic policy` |
| a policy on `bool` or a float | `type arithmetic policy on a type that is not an integer` |
| an operator over a policy-qualified operand and a plain binding: open owner question `policy-operator-plain-operand` | `expression operator between a policy-qualified operand and a plain operand that is not a literal` |
| operands under two different policies | `typing operands under two arithmetic policies` |
| `&`, `\|`, `^` and `~` over a policy-qualified operand, which the policy catalog has no row for | `expression bitwise operator on a policy-qualified operand`, `typing unary operand type` |
| a Saturating shift, which owes the Exact count proof under an operation the lowering does not emit | `expression saturating shift` |
| a shift whose count is policy-qualified | `expression shift by a policy-qualified count` |
| a cast that changes the carrier of an integer that already carries a policy ("Rejected: each of these changes the carrier of a qualified value") | `expression cast that changes the carrier of a policy-qualified integer` |
| a literal or other plain value where a policy-qualified one is declared: `let x: u32 in Wrapping = 5;`, an argument, a field initializer, a result ("An expected type selects the carrier in which an anonymous literal lands. It never inserts a domain cast") | `typing local initializer without the arithmetic policy of its local`, `typing argument types`, `typing record literal field type`, `typing arm value type`, `typing final value type`, `typing assigned value type` |
| a policy-qualified local declared without an initializer, `&T in Wrapping`, a loan of such a place, an `[erased]` one, and a value `match` that yields one | `statement uninitialized local of a policy-qualified type`, `parameter reference to a policy-qualified integer`, `call loan of a policy-qualified integer`, `parameter erased parameter of a policy-qualified type`, `expression match with a policy-qualified result` |

### Mathematical integers in a clause

A machine's `requires` or `ensures` may compare mathematical integers:
`requires embed(rest) + embed(count) * 10 <= 99`. `embed` takes a runtime
integer into the unbounded integers
([proof contracts](../../../../../wiki/spec/proofs/contracts.md#mathematical-values-and-quantification):
"Runtime integer/address values enter mathematical integer arithmetic through
the total `embed` projection, retaining exact carrier range"), where `+`, `-`
and `*` cannot overflow
([mathematical values](../../../../../wiki/spec/terminal-psi/mathematical_values.md#integer-terms):
"These terms denote unbounded mathematical integers and cannot overflow"). So
such a clause computes nothing in a carrier and owes no formation condition,
which is what sets it apart from carrier arithmetic in a clause
(`requires a + b <= 99`, which "discharge[s] representability before
formation" and is refused until a slice states that obligation).

A comparison is mathematical when an `embed` stands on either side beneath
binary operators only (`mathematical`). `embed` is the builtin by the symbol
stage 02 resolved the call to, never by its spelling. Each side is read by
`mathematical_term`:

- `embed(t)`, where `t` is any term a clause of that kind may already name:
  a formal, a field of a record formal or a borrowed formal in an entry
  `requires`, `result` in an `ensures`, a bound value parameter, a typed
  literal. The embedding keeps `t`'s carrier (`IntegerMathTerm::MathValue`
  for a value, `Embed` for a field of a place).
- an anonymous integer literal, or a wholly anonymous tree whose exact value
  is integral ("An anonymous integral final value can land in mathematical
  `Int` or `Nat` without fractional-history acknowledgment");
- `+`, `-` and `*` of those.

`<=`, `<`, `==`, `>=`, `>` become `IntegerMathLessOrEqual`,
`IntegerMathLessThan` and `IntegerMathEqual` (operands in the vocabulary's
order, as the codec requires); `!=` is `a < b or b < a`, as for carrier
integers. A clause's text for diagnostics keeps each embedded formal open, so
a call's statement reads `embed(a) + embed(b) * 10 <= 99` over its own
arguments.

| Construct | Refusal detail |
| --- | --- |
| carrier arithmetic in a clause term, `requires a + b <= 99` | `contract carrier arithmetic in a clause term` |
| a suffixed operator in a clause term, `a +% b` | `contract policy operator in a clause term` |
| a cast, or a call other than `embed`, in a clause term | `contract cast in a clause term`, `contract call in a clause term` |
| `embed` that is not a side of a comparison or beneath its arithmetic, `embed(embed(a))` | `contract embed outside a mathematical comparison` |
| `embed` of a Boolean, or with other than one argument | `contract embed of a value that is not an integer`, `contract embed of other than one value` |
| `/` or `%` between mathematical integers (rational division, or the named quotient and remainder) | `contract mathematical division in a clause` |
| a shift or bitwise operator, or a suffixed operator, between mathematical integers | `contract mathematical operator other than add, subtract and multiply`, `contract policy operator in a mathematical term` |
| a runtime value or typed literal that is not embedded, `embed(a) + b` or `embed(a) <= 99u8` | `contract runtime value in a mathematical term without embed`, `contract typed literal in a mathematical term without embed` |
| a cast or a call inside a mathematical term, `embed(a) as Nat`, `integer::quotient(..)` | `contract cast in a mathematical term`, `contract call in a mathematical term` |
| a literal past 64 bits, or an anonymous tree whose value is fractional | `contract mathematical literal beyond 64 bits`, `contract fractional anonymous value in a mathematical term` |
| the comparison in a crash route guard or in a data `requires`, each restated by a substitution that knows only scalar comparisons | `contract mathematical comparison in a crash route guard`, `contract mathematical comparison in a data requires` |
| the comparison in the `requires` of a member of a call cycle, which becomes a loop head's invariant | `contract mathematical comparison required on a call cycle` |

### Erased scalar bindings

A scalar formal or local may be `[erased]`
([proof contracts](../../../../../wiki/spec/proofs/contracts.md#explicit-erased-bindings):
"The binding remains in checking and semantic identity but contributes no
runtime field, address, read, or cleanup. It may supply proof computation or
static authorization; it cannot determine runtime data or control."). This is
the first slice of the
[proofs, evidence and quotients lane](../../../../../wiki/drafts/designs/proofs_evidence_lane.md).

- **An erased formal** has a value identity, so `requires` and `ensures`
  clauses name it like any formal, and no runtime lane: Terminal Psi declares
  it in `MachineContract::erased_scalar_formals`, not in `parameters` or
  `parameter_order`
  ([encoding](../../../../../wiki/spec/terminal-psi/encoding.md): "Erased
  formals occupy no tag."). Its identity comes after the result
  pseudo-value; `Signature::formal_values` is the one numbering the reader
  and lowering share.
- **The argument for an erased formal** is a term, not an operand
  (contracts.md: "Construction supplies the erased term"). It goes into the
  call's `erased_arguments` as a `ScalarTerm` and no operation computes it.
- **An erased local**, `let w [erased]: T = term;`, names its term
  (`Statement::Witness`) and emits nothing.

A term is a literal or a scalar binding, relevant or erased: the forms with
no formation condition. Anything that computes has one (contracts.md, "Total
arithmetic": "Exact operations discharge representability before formation"),
and no operation of the lowered form would own it, so it is refused.

| Construct | Refusal detail |
| --- | --- |
| an erased formal or local of a data type (Terminal Psi's proof-term lane, `erased_proof_formals`) | `parameter erased parameter of a data type`, `statement erased local of a data type`, `expression erased term of a data type` |
| a term that computes, or reads a local declared without an initializer | `expression erased term other than a literal or a binding`, `expression erased term that reads storage` |
| an erased local with no initializer | `statement erased local without an initializer` |
| a runtime expression that reads an erased binding | `expression runtime use of an erased binding` (finding 34) |
| a crash route guard that names an erased formal | `contract route guard names a value that is not a formal` |

### Loops

A loop in Omega is a machine whose transition arm calls it again, directly
or through other machines. The subset serves such a call cycle between
scalar machines when every call between its members stands in terminal
position (an arm's target, or the body's final value) and no member promises
termination: `machine spin(n: u32) -> u32 { transition n > 0 { true ->
spin(n - 1) false -> 0 } }`, and the same through two or more machines.
[termination](../../../../../wiki/spec/language/termination.md#calls-and-productive-loops):
"A productive terminal-transfer cycle without a termination promise may run
forever and owes no rank". [Call cycles as loops](#call-cycles-as-loops)
gives the lowered form.

The rest of the loops lane
(`wiki/drafts/designs/loops_recursion_termination_lane.md`) is refused by
name until the slice that serves it:

| Construct | Refusal detail |
| --- | --- |
| a call of a member whose result the caller goes on to use, `1 + f(n)` | `call recursion outside a terminal position` |
| a `terminates by` witness, on any machine | `contract ranking witness` |
| a bare `terminates` on a machine that lies on a cycle or calls into one | `contract termination promise over a call cycle` |
| `terminates` under a progress premise | `contract termination under a progress premise` |
| `ensures` on a member | `contract ensures on a call cycle` |
| a `crashes Cause` clause with a route guard on a member | `contract guarded crash route on a call cycle` |
| a record or sum formal, result, local or expression in a member, or a local declared without an initializer | `machine storage on a call cycle` |
| an `[erased]` formal or local in a member | `machine erased binding on a call cycle` |
| a formal, result or local of a type parameter in a member | `machine value of a type parameter on a call cycle` |

A bare `terminates` on a machine that reaches no cycle is served: every
machine it can reach is acyclic and inside the subset, which is the
specification's "A private acyclic body needs no annotation and derives a
local `Terminates` summary".

### Records

A formal, a local, a result, a field and an argument may also be a
plain record whose fields are builtin integers, `bool` or other such records.
A `[copy]` declaration permits copying; ordinary data remains affine. On such values the subset
serves:

- the record literal `R { a: x, b: y }`, the field read `r.a` and `r.a.b`,
  and copying a record value (`let b = a;`, passing it by value, returning
  it, moving it into a field of a literal);
- a store into a field of a `mut` local, `r.a = x` and `r.a.b = x`, and the
  replacement of a record field by a record, `l.from = p`;
- whole assignment to a `mut` local, `r = R { ... }` and `r = other;`;
- a local declared without an initializer: `let mut r: R;` holds the zero
  value of `R`, and `let mut x: u32;` holds zero;
- the record pattern `let { a, b as c, d as _ } = r;` over a place: each
  bound field is the ordinary local that follows the pattern's marker, a
  field read of the subject, and the marker itself lowers to nothing. The
  pattern is served when its roster names every field of the subject's
  record ([patterns](../../../../../wiki/spec/language/patterns.md#irrefutable-data-bindings):
  "A pattern that can fail rejects; a binding introduces no hidden branch,
  trap, fallback or runtime assertion."). One that omits a field is refused
  as `statement record pattern that omits a field`;
- a `requires` on the declaration, over `self` and its fields
  ([Data `requires`](#data-requires)).

Data with no field and no case, `data Token {}`, is a record whose roster is
empty. A value of it is established by `Token {}` or by a local declared
without an initializer, copied or moved by the same rule as any record, and
is a field, an argument and the referent of a reference like one. It has no
leaf, so nothing reads from it and no write log row speaks of it; that a
value which is not `[copy]` moves once is the custody replay's judgment
(`terminal_semantics::plain_affine`), which works on places, not leaves.

`[copy]` is required, not inferred
([ownership](../../../../../wiki/spec/language/ownership.md): "A record still
requires an explicit `[copy]` declaration; copyable fields do not change the
affine default."). Plain data without it transfers once. Its exact moved
regions become unavailable and disjoint siblings remain readable. Whole owned
transport consumes its source home. A projected move leaves its owner allocated;
normal continuations dispose the residual regions before retiring that home.
Named affine locals reserve storage distinct from their initializer or call
result. This first ownership slice requires no-code cleanup and no conserved
claim: nominal cleanup hooks, linear data, reference-bearing fields, borrowed
extraction and incompatible affine frontiers at joins are refused.

A declaration may state lifetime parameters, `data Outer<'a>`, and a field
may apply such data at them, `inner: Inner<'a>`. A lifetime argument selects
nothing a value is made of
([lifetimes](../../../../../wiki/spec/language/lifetimes.md#binders-and-source-applications):
"Lifetime arguments are erased regions, separate from runtime generic arity,
layout and monomorphization."), so the declaration is one record type
whatever lifetimes a use applies it at, and a formal, local or result may
name it bare. What a lifetime relates is a stored view ("A lifetime relates a
borrowed result or stored view to storage that remains valid for its use").
The one view a field may store is a byte view, `name: &'a [u8]`, and its type
is keyed by the region it names under the declaration that binds it, so a
view of any other region, an elided one included, is another type: it cannot
be stored in the field or returned as it (`typing assigned value type`).
Naming the data bare therefore relates nothing. The source record
correspondence a product keeps is the declaration's, with no lifetime
argument in it: it keeps the type occurrence the record was read from, bare
or applied at lifetimes, and keeps none for a declaration with no type
parameter that the program declares and never names as a type, because the
declaration alone identifies that record. Any other reference field is refused as `type reference`.
The reader checks
the declaration and each application itself, because the stage that did is
being removed ("They must match the declaration's lifetime arity and use
binders declared in the lexical owner. Duplicate binders and undeclared tags
reject."). The owner of the lifetimes an application names is the typed
trees' answer: the declaration they retained for that occurrence of the type,
as stage 03 does for the type of a literal nested in another literal, and
otherwise the data declaration whose member is being read. A machine is read
under no owner, since one that declares lifetime parameters is refused
(`machine generic parameters`).

A `mut` formal that holds its own value (`mut x: u32`, `mut p: R`) is the
callee's own storage, initialised from the argument, and is written as a
`mut` local is ([ownership](../../../../../wiki/spec/language/ownership.md#owned-local-mutability):
"the recipient may bind its own value mutably"). Nothing a caller sees
changes: the signature, the contract and the argument are those of the same
formal without `mut`.

- A scalar is assigned by rebinding its slot to the assigned value. The
  formal's entry value keeps its identity, so an entry `requires` stays a
  fact about that value and proves nothing about the formal after
  `x = x + 1`
  ([fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md#entry-facts-and-crash-coverage):
  "Current body values are not entry snapshots").
- A record is stored into by field in the formal's own parameter place,
  which holds the copy the call passed; the store logs the new content on
  that exact field, and its siblings keep what they held at entry. A whole
  assignment rebinds the slot to a fresh root.
- `&mut x` and `&mut p.a` lend the formal as owned storage
  (`Lender::Owner`), as they lend a `mut` local.

Refused by name:

| Construct | Refusal detail |
| --- | --- |
| an `ensures` clause or a crash route guard that names a `mut` formal: whether the name means the argument or what the body last assigned is the open owner question `mutable-parameter-contract-name` | `contract clause that names a mutable parameter outside requires` |
| a `mut` formal of a type parameter, whose replaced value no slice disposes of | `parameter mutable parameter of a type parameter` |
| a `mut` formal marked `[erased]` | `parameter mutable erased parameter` |
| an assignment to, or a store into, a formal that is not `mut` | `statement assignment to an immutable binding`, `statement store into an immutable binding` |

A `mut` formal of plain data without `[copy]` is refused as that data is
anywhere (`type data with custody`). A formal passed by reference is
[References](#references).

### References

The first slice of the references and loans lane: a formal passed by `&T`
or `&mut T`, for a builtin scalar or plain record `T`, and the loans a direct
call forms for it.

- **A borrowed formal** is read as its referent type with the reference
  beside it (`Signature::borrowed`). It is a place whatever its referent: a
  root `Parameter` place declared with the reference's access
  (`SharedBorrow`, `MutableBorrow`), holding the caller's storage for the
  activation ([ownership](../../../../../wiki/spec/language/ownership.md#borrows-and-aliases):
  "Borrowed arguments continue to denote the original storage, not a
  caller-side snapshot chosen because the referent is small."). A scalar
  referent is a place of its primitive type, read with
  `PrimitiveScalarRead` and stored with `WriteOnlyPrimitiveStore`, as a
  home is; a record referent is read and stored by field, as a record
  formal is. Only `&mut` lets the body store into it. The formals that are
  places, record formals and borrowed formals in position order, are the
  machine's first places, so a clause names the `k`-th of them as place
  `k + 1`. An entry `requires` may name a borrowed scalar formal (`x < 10`)
  or a field of a borrowed record (`cell.value > 0`): what the storage holds
  at entry. This is what the current pipeline emits for such a formal.
- **A loan** is the argument for a borrowed formal: `&place` or `&mut
  place` with the formal's access, the place a slot or a field path beneath
  one (`ExpressionKind::Borrow`), or a borrowed formal of the caller named
  whole, which reborrows its referent (`set(value)`). The call passes the
  lent place itself as a structural argument with the loan's access, the
  invocation row names that exact place, and one `Loan` obligation is
  minted at the point before the call: captured place, access, and lender
  (`Lender::Owner` for owned storage, `Lender::Incoming` beneath a borrowed
  formal). [loans](../../../../../wiki/spec/terminal-psi/loans.md): "Loan
  formation captures a place, not a live selector expression." A scalar
  local that is an SSA value gets a home holding its current value first
  (`EstablishPrimitiveLocal`, as the current pipeline does for a local it
  lends), and the slot names that home from then on.
- **When the call returns**, the loan has ended: each is formed for one call
  and returns with it, so no loan is live on any edge. An exclusive loan
  logs `Overwritten` on the lent place at the call's normal continuation:
  the callee may have stored anything beneath it, and nothing else changes,
  so a sibling field keeps what it held
  ([exact projected-path scoping](../../../../../AGENTS.md)). A shared loan
  logs nothing.

Whether two loans of one call may both be formed, and whether a reborrow
asks for more than its parent holds, is the checker's judgment
([lowered-discharge](../lowered-discharge/README.md#loans)). The reader
refuses, by name, what the slice does not serve:

| Construct | Refusal detail |
| --- | --- |
| `&write T`, a reference with a named lifetime, an `[erased]` reference, `mut` on a shared reference, a reference of a requirement, a referent of a type parameter | `parameter write-only reference parameter`, `parameter reference parameter with a named lifetime`, `parameter erased reference parameter`, `parameter mutable binding of a shared reference`, `parameter reference parameter of a requirement`, `parameter reference to a value of a type parameter` |
| a referent of data with `requires`, or holding such data | `parameter reference to data with requires` |
| a reference local, a reference result or field, a reference anywhere but a formal | `type reference` |
| a borrow anywhere but the argument for a borrowed formal | `expression borrow` |
| an argument for a borrowed formal that is not a borrow, a borrow with another access than its formal's, of a value that is not a place, of a case payload | `call argument for a borrowed formal that is not a borrow`, `call borrow whose access differs from its formal's`, `call borrow of a value that is not a place`, `call borrow of a case payload` |
| `&mut` of a binding that is not `mut` (an exclusive reborrow of a shared reference is the checker's conflict) | `call exclusive loan of an immutable binding` |
| another argument of the same call that names a lent slot (`f(&mut a, a)`): the lowered form forms every loan at the call, so it does not say that the read runs while the loan is live | `call argument that names storage another argument lends` |
| a call that never returns and lends storage | `call no-normal call that lends storage` |
| a loan that would give an SSA local a home inside the right of `&&` or `||` or a value `match` arm, so on some paths only | `call loan of a value local on some paths only` |
| `let r = reference;` without a declared type, and whole assignment through a record reference | `statement local bound to a reference without a declared type`, `statement whole assignment through a reference` |
| an `ensures` or crash guard that names a borrowed formal: what it holds at the return is a version no slice states yet | `contract clause that names a borrowed formal outside requires`, `contract clause that projects a formal or result` |
| a borrowed formal on a call cycle | `machine storage on a call cycle` |

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
| a field type constrained by a declared domain or by `Trapping` | `type constrained by a domain that is not one arithmetic policy`, `type constrained by Trapping` |
| linear data, or data with a cleanup selection | `type data with custody` |
| data with common fields beside its cases | `type mixed data` |
| a lifetime binder stated twice, a lifetime application with another count of arguments than its declaration's binders or with a tag its owner does not declare, a field that names lifetime-parameterised data without its arguments | `typing data that declares a lifetime twice`, `typing lifetime argument count differs from the data declaration's`, `typing lifetime argument its owner does not declare`, `typing data field that elides a lifetime argument` |
| data with type, const or machine parameters, an instance of such data, quotient or opaque data, data with a carry policy, an erased field or retired field identities | `type generic data`, `type quotient data`, `type data without a checked shape`, `type data with a carry policy`, `type data with an erased field`, `type data with retired field identities` |
| a store into, or assignment to, a binding that is not `mut` | `statement store into an immutable binding`, `statement assignment to an immutable binding` |
| a store into a field of a value that is not a local, or into an indexed place | `statement store into a value that is not a local`, `statement assignment to a projected place` |
| `let x;` with neither a type nor an initializer | `statement uninitialized local without a declared type` |
| a record pattern that omits a field, or whose subject is not a place | `statement record pattern that omits a field`, `statement record pattern over a value that is not a place` |
| a store into a payload member, `s.at = x` | `statement store into a case payload` |
| a payload member whose spelling two cases share | `spelling member without a field symbol` (finding 19) |
| a field of a data payload binding, `inner.x` | `expression field read beneath a case payload` |
| a case arm with a guard, a pattern field tested against a value, a pattern that binds its case, a second arm for one case, a wildcard no case reaches | `transition case arm with a guard`, `transition pattern field tested against a value`, `transition pattern that binds its case`, `transition case arm for a case an earlier arm selects`, `transition wildcard arm that no case reaches` |
| `crash` in a case arm | `transition crash in a case arm` (finding 21) |
| `match` on a scalar or with value patterns, on a subject that is not a place, with a data or Unit result | `expression match on a scalar subject`, `expression match arm on a value pattern`, `expression match on a subject that is not a place`, `expression match with a data result`, `expression match with a Unit arm` |
| a `match` arm with a guard or after the wildcard | `transition case arm with a guard`, `expression match arm after the wildcard` |
| `s in S::C` whose right side names no case | `expression case membership` |
| a pattern that names fewer than all payload fields without `..` | `typing pattern misses a payload field` |
| a clause that names a record formal whole, an `ensures` or crash guard that projects a formal, and any clause that projects `result` | `contract record formal named whole in a clause`, `contract clause that projects a formal or result`, `contract result of a record machine` |
| `==` and `!=` between records or between two sum values | `expression operator on a record` |
| a field, literal type or pattern roster the typed trees hold only as a spelling | `spelling member without a field symbol`, `spelling record literal without a type symbol`, `spelling record literal field without a symbol`, `spelling field without a symbol`, `spelling record pattern without a resolved subject`, `spelling record pattern field without a symbol` |

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
- **Record patterns** are covered by the roster on the pattern's marker
  local (`TableLocalData::pattern_fields`, `pattern_subject`), which typed
  preparation resolves against the subject's declared record: the marker's
  record symbol must be the declaration of the subject's own record type, and
  each row's field symbol one of its fields. A row without a symbol, or a
  roster resolved against another record, is refused; no row's spelling is
  read.
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
- **Formals** travel in Terminal Psi's two lanes. The `k`-th relevant scalar
  formal is value `k + 1`, skipping record and erased formals, and that is
  how a clause names it; a record formal is a root place and has no value
  identity. An erased scalar formal is a value after the result pseudo-value
  and travels in neither lane.
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

## Call cycles as loops

[components.rs](src/components.rs) finds the call components of the bodies
read: the strongly connected sets of the call graph. Nesting plays no part
([control flow](../../../../../wiki/spec/terminal-psi/control_flow.md):
"Source nesting does not bound the component or justify any edge."). Each
machine still gets its own Terminal machine, and a machine on a cycle holds
its whole component in its own graph
(control flow: "Constant-stack terminal cycles may become block backedges"):

- **The entry** holds the machine's formals as machine parameters and jumps
  to the machine's own head with them.
- **One head block per member**, whose block parameters are that member's
  scalar formals. The member's body is lowered once beneath it with its
  slots bound to those parameters, so nothing in a loop names a machine
  formal.
- **A terminal call of a member** evaluates its arguments left to right and
  jumps to that member's head with them. It is no call operation and gets no
  invocation row: the lowered form has no call there.

A component of `k` machines is therefore lowered `k` times, once per
machine. Nothing is shared between those graphs.

**`requires` at a head.** A member's `requires` clauses, each formal
replaced by the head's parameter, are the head's one row in the module's
`scalar_block_invariants`, a canonical conjunction when there are several
(control flow: "A scalar block invariant is a semantic row identifying a
machine, destination block, scalar predicate, and one obligation for every
actual incoming edge."). Every edge into the head mints one
`ArrivalInvariant` obligation at that edge and one arrival in the row: the
entry's jump, and each jump back. That is
[state contracts](../../../../../wiki/spec/language/state_contracts.md#bindings-and-arrivals)
on the lowered form: "Machine `requires` applies at every invocation,
including terminal and recursive calls." A member with no `requires` has no
row and its head owes nothing. The predicate is the authored one; nothing
here infers a loop invariant.

The entry is never a head (control flow: "The machine entry cannot carry a
row: invocation supplies the machine's declared preconditions rather than an
explicit control-flow edge"), so a machine's own `requires` reach its loop
only by being proved on the entry's jump.

**Outcomes.** Whether a machine completes normally and which causes it may
crash with is the least fixed point over the call graph
([outcome.rs](src/outcome.rs)): every machine starts as never completing,
and the bodies are walked until nothing changes. A machine whose every path
calls it again never completes, and a call of it is an `InvokeNoNormal`
terminator (control flow: "An infinite path has no normal return").

No `ranked_scc` is written: these cycles promise nothing.

## Places, roots and the write log

A record value is storage, so lowering a record fills three tables in step
with the Terminal Psi operations that create the storage
([lower/places.rs](src/lower/places.rs)).

**Roots.** One Terminal `PlaceId`, one `PlaceRow { origin: Root }` and one
`roots` entry for each record formal (`StructuralPlaceKind::Parameter`), each
operation that produces a record (`OperationResult`: a literal, a whole copy,
a call with a record result) and a record machine's result place (`Result`).
An affine named local reserves its own home and receives its initializer
through a checked whole store; a continuation retires the consumed initializer.
Copy locals may retain their initializer's unrestricted root. A local declared
without an initializer has the home its zero-establishing operation produces. `PlaceId`s are dense per
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
accepts (`i8`..`u64`, `bool`) admits zero, a scalar type under a declared
domain is refused, and a policy-qualified local without an initializer is
refused, so a scalar home owes no domain
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
no linear claim) retain authored copy permission and the admitted plain
storage's no-code disposal authority. Copyable fields never promote an affine
declaration to `[copy]`. One column differs: the
current pipeline sets `inline_carrier` on each scalar field to the primitive
structural type of that scalar, and this route leaves it `None`. The
interpreter loads and runs both.

## Obligations

| Kind | Where | Created at |
| --- | --- | --- |
| `OperationSafety` | each `Exact` add, subtract, multiply, divide, remainder, shift, each Wrapping or Saturating divide and remainder (every primitive whose `integer_policy_bridge` has formation conditions), and each cast that is not a widening | the point before the operation |
| `CallRequirement` | one per callee `requires` clause per call, clause = position in the callee's canonical `requires` | the point before the call |
| `PublishedGuarantee` | one per `ensures` clause, the identity `ContractClause` carries | the entry block |
| `NormalGuarantee` | one per `ensures` clause per normal exit edge, naming its `PublishedGuarantee` | that edge |
| `ArrivalInvariant` | one per edge into a block with parameters: a short-circuit join, whose predicate the checker infers, or a loop head, whose predicate is its member's `requires` | that edge |
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

`Elaboration::names` ([names.rs](src/names.rs)) is what diagnostics call
those constructs, because the lowered form carries identities and spans but
no binder names. The reader records, per machine, its name (a template's for
a generated specialization) and its state's, its formals' names, the
authored text of each `requires` clause (in canonical position) and
`ensures` clause with each formal left open, the binder each value
expression's nodes land in (a local's name, or the return value) keyed by
their spans, each call's argument text keyed by the call's span, and each
exit's statement position keyed by its span (with the position of a unit
body's fall-off exit). All of
it comes from the typed trees' `Identifier`s and
`ExpressionTable::display_name`, never from source text, and nothing in
lowering or the checker decides anything from it. `lower_bodies` leaves it
empty.

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
19. **A directly authored payload member carries its field symbol.** `s.at`
    and `s.at = x` on a sum used to reach the typed trees as a member with a
    name and an invalid `member_symbol`, and so did a field of a data payload
    binding, `inner.x`. Stage 03 now records the payload field's symbol (a
    `SymbolKind::Field` under the case's `SymbolKind::Variant`) where it
    completes a member against its receiver's declared type, when no common
    field and exactly one case declares that spelling, and types a field of
    a payload binding through the binding's own field symbol. So this route
    reads `s.at` as the payload member it is, and the lowered form's payload
    access judgment decides whether the sum holds that case there. A spelling
    two cases share still arrives without a symbol and is refused as one;
    the member's `case_variant` stays unset and is not needed.
20. **A record pattern carries its field roster as symbols.**
    `let { x, y as _ } = p;` reaches the typed trees as a marker local
    followed by one ordinary field-read local per bound field. The marker
    used to name its fields only in its own name (`__destructure#x#y`), which
    the current checker split and compared by spelling. It now carries a
    roster (`TableLocalData::pattern_fields`, one row per named field, bound
    or waived) that typed preparation resolves against the subject's declared
    record (`pattern_subject`), and `validate_destructure_exhaustiveness`
    and this route both decide coverage from those symbols.
    [Patterns](../../../../../wiki/spec/language/patterns.md#irrefutable-data-bindings)
    requires the pattern to name every field, so a roster that omits one is
    refused here as it is rejected there.
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
    satisfies a `crashes none` requirement, and gives each application of a
    deferring requirement the selected member's routes
    (`tests/omega/fail/traits/omitted_requirement_crashes_reach_generic_caller`).
    This route hands those programs back (conformances are outside it).
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

33. **A resolved call is not a permitted call.** The typed trees hold a call
    into a dependency's private machine fully resolved; the current checker
    rejects it later, where a product selects the callee. The reader
    therefore checks visibility itself: a callee that is not public must be
    declared in the caller's package, by the source file each symbol was
    declared in, with a generated machine placed where its declaration is
    (`visible_to`). Any other call is refused as `call private machine of
    another package`, so the current checker decides the compilation
    (`tests/fixtures/fail/build/executed_dependency_private_call`).
34. **A runtime read of an erased binding falls back; it is not yet rejected
    here.** The current checker rejects `let total: i32 = bound;` for an
    erased `bound` ("erased parameter `bound` has no runtime value"). That is
    a verdict about the program, and this route has no refusal reason for it,
    so the reader refuses the closure as `Unsupported` and the current
    checker decides. The slice that owns the rejection adds the reason.
35. **The current pipeline cannot lower an erased local supplied as an erased
    term.** `let witness [erased]: i32 = limit; keep(n, witness)` passes the
    current checker and then fails in Psi 06 ("scalar read has no authored
    binding"). This route lowers it: the local names the term. The witness is
    `tests/omega/pending/relevance/erased_local_forwarded_to_erased_formal`.
36. **A recursive call records its selection as the entry state.** A call of
    another machine carries an authored selection that resolves to the
    symbol the call names. A call of the machine being read names the
    machine and its selection resolves to that machine's one state. The
    reader accepts a selection when both symbols enter the same machine
    (`entered_machine`: a machine, or the one state of one), comparing
    symbols only.
37. **A ranking witness is refused wherever it stands.** The typed trees
    hold its subjects twice, as rendered text
    (`RankingWitness::subjects`) and as typed expressions
    (`RankingExpressionCustody`). This slice reads neither and refuses the
    witness, on an acyclic machine too, because whether a witness with
    nothing to rank is an error is the ranking slice's to decide. That slice
    reads the custody and never the text.
38. **A final value counts as terminal position.** Stage 01 leaves a body's
    final expression as an expression, and the reader makes it the
    unguarded arm that completes the machine, so `machine f(n: u32) -> u32
    { f(n) }` is a jump back like `transition { _ -> f(n) }`. Whether the
    current checker treats the two alike was not measured.
