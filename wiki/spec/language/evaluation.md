# Semantic evaluation

Semantic evaluation computes constants, proofs, plans, and trait generators in
a hermetic target-semantic world. The source position requests evaluation;
the concrete invocation's complete normalized contract determines admission.
There is no `comptime`, `#run`, `const machine`, macro language, or
`is_build_time()` observation.

For an eligible runtime-capable machine, evaluation produces exactly the value
that target execution would produce with the same arguments, selected
conformances/providers, and target semantics. Evaluation changes when, not what.
Proof-only machines need no runtime representation but obey the same core value
semantics and cannot observe the compiler phase.

[Constants](constants.md) name pure values without storage identity.
[Build execution](../build/execution.md) instead orchestrates dependencies,
target selection, and staging through explicitly granted build-host services.
An external build observation reaches semantic evaluation only through staged
value/artifact custody, its recorded input and receipt, and compiler
materialization of an ordinary value. There is no ambient-host shortcut into
a proof, type, layout, or constant.

## Invocation admission

Specialize the selected machine's complete contract at its concrete arguments
and available facts. Admission requires all of the following:

- Ordinary checked termination.
- Empty runtime service reach.
- No possible suspension or blocking.
- No unhandled failure, trap, or abort route for the demanded value.
- No runtime authority acquisition or consumption.
- No escaping runtime mutation.
- Only proof/build-admissible trust and resource inputs.

Empty reach alone proves none of the other axes. Public trait requirements
establish the floor; an incompatible conformance rejects at its declaration.
[Package declaration-selection authority](../packages/boundaries.md#admit-before-execution)
also applies before evaluation, not only after the resulting value is used.
Permission to select a declaration does not establish its execution meaning.
An unresolved authored operator cannot be evaluated using the builtin meaning
of its token, including when reached through a helper machine.

Admission is invocation-sensitive. A trap-capable division can evaluate at
`divide(10, 2)` after proving the denominator nonzero. `divide(10, 0)` rejects
before execution and identifies the undischarged route and call chain. Reaching
a forbidden terminal route despite admission is a checker/accepted-assumption
consistency failure with a trace, not a normal machine result. Handled result
sums remain ordinary values.

The hermetic bridge creates a fresh owned interpreter value graph from every
materialized argument and a fresh machine instance. Only a recursive value
snapshot crosses back. Interpreter cells, references, and aliases into compiler,
other-invocation, or runtime state cannot escape. Local mutation, allocation,
temporary borrowing, and recursion are permitted implementation techniques
when their invocation satisfies the complete contract. The separate augmenting
build API deliberately returns mutated argument snapshots; that does not widen
the hermetic bridge.

Fixed arrays and recursively owned records may cross as values. A byte literal
in an exact fixed-array result copies its bytes into the array; a width mismatch
rejects rather than returning a slice into evaluator storage. Temporary slices
may exist inside evaluation. Dynamic owned sequences use the ordinary
collection model when const-evaluable, not a special compiler byte-blob type.
Structural equality and hashing retain their ordinary type meaning; constant
pool interning is an unobservable emission optimization.

The compiler derives `ConstEvaluable(T, value)` for a complete pure/copy-eligible
snapshot and `ConstMaterializable(value, layout)` for determined observable
runtime bits. Neither is user-satisfiable. The latter is a separate,
value-sensitive judgment described in [constant materialization](constants.md#materialization).
Canonical type/index identity imposes the additional rules below.

## Termination and sponsorship

There is only the ordinary `terminates` guarantee. A closed acyclic checked
body supplies it locally; a cyclic body proves `terminates by ...`; an open
or separately compiled contract declares `terminates;` when consumers may rely
on it. A terminating invocation can still be too expensive for its sponsor.

[Logical work](../resources/logical_work.md) defines the canonical Terminal
schedule and unobservable exhaustion. Resource admission uses either a certified
maximum fitting the compiler grant or deterministic metering against an explicit
sponsor budget. Reproducible published builds retain the certified ceiling or
the admitted budget and usage receipt. Peak live cells receive the same
treatment. Additional payload ceilings require a named compiler-owned account
whose complete allocation lifetime can be charged before entry and released
exactly; they do not promise generic temporary-memory or RSS bounds.

Only the root selects or raises/removes ceilings. Dependencies may report
expected usage, not grant themselves more. Evaluated code cannot inspect its
remaining budget, request increases, catch exhaustion, or branch on policy.
Raising a grant changes whether a result is obtained, never which result.
Exhaustion is a build-resource error, not divergence or a failed termination
proof. Scheduling and host speed cannot change canonical charges or aggregate
admission. Progress may name the invocation, sponsor, usage, and largest active
call path; displayed wall time has no accounting or admission force.

## Target-semantic capsule

The host evaluator consumes a sealed, compiler-owned, versioned target capsule
also consumed by target realization. Inputs include language/evaluator semantics,
required target primitive meanings, exact selected conformance/provider plans,
normalized implementations, and compiler-materialized arguments. The host's
filesystem, environment, clock, randomness, network, pointer width, floating
behavior, and process state are not inputs.

`build.omg` selects the capsule, not individual proof facts. Its closed typed
projection vocabulary belongs to the language/toolchain schema, not replaceable
providers or extensible package reflection. Packages derive values, propositions,
and plans; they cannot invent primitive observations or redefine carrier meaning.
There is no source-visible general `BuildWorld` or runtime reflection object.

Observations identify their subject explicitly: address bound for one address
space, endianness for one data layout, floating semantics for one format/mode,
or an entry guarantee for one target profile. A subject not fixed by target
closure rejects. Values and interpretation selectors use the same heterogeneous
typed mechanism; an endianness selector need not be encoded as an integer.
The source spelling of these projections remains unsettled; conceptual
`TargetSemantics::address_bound<addr>()` is not an approved API declaration.

Installed memory, devices, page geometry, and current CPU features instead come
from explicit providers, capabilities, installation, or admitted hardware input.
A profile guarantee describes a selected contract, not proof that physical
arrival satisfies it. The [UEFI entry profile](../build/uefi_entry.md) separates
its stack guarantee from invocation admission.

Floating operations use runtime `FloatSemantics`, including format rounding,
classification, conversion, square root, and fused versus unfused arithmetic.
Portable equality is equality of `FloatMeaning`; exact computed NaN payloads
need a proof or selected realization fixing them. Every selected realization
and semantic control state affecting the result participates in identity.

### Target-dependent applications

An observation can occupy any position available to an equivalent ordinary
constant, including array length, const-generic argument, proof expression,
further evaluation, and layout/calling-plan input. It cannot add/remove fields
or cases, change multiplicity, or splice declarations. A future integer-width
constructor would require its own admitted widths, identity, and lowering.

An unscoped constant or public type application may be target-polymorphic. Its
declaration retains one source identity; closed applications retain exact
type/const/machine substitutions and target dependencies. Before closure,
export a symbolic recipe, not a guessed concrete value. Different target
applications cannot share values or artifact identities merely because their
declaration paths match.

Target-dependent geometry normally belongs to validated plan size/alignment
and placed storage rather than a target-sized array, but the latter is not
forbidden. Genuinely different native field/case sets require distinct nominal
ABI schemas, privately selected by exact realizations behind one stable
portable requirement. Build orchestration cannot splice an existing schema.

Two distinct normalized dependencies propagate transitively:

| Dependency | Retained input |
| --- | --- |
| Observation application | Projection, subject, projection-semantics version, selected value or interpretation. |
| Selected-realization application | Requirement/slot, exact realization application, target scope, normalized contract/plan/binding identity. |

The second remains necessary for target-scoped code containing only literals
and no projection call. Derived values, proofs, plans, public signatures,
caches, and artifacts retain both. Folding preserves dependencies and their
target-closure receipt; verification reconstructs the projection or selected
realization instead of trusting a folded scalar.

Exact used dependencies are normative compatibility identity. A whole-capsule
and complete-realization-closure key is sound but conservatively rejects reuse.
Fine-grained replay may remove that key only after checking both dependency
kinds. Independently closed artifacts compose only when applications agree.
Adding, removing, or changing a published-signature dependency is a breaking
semantic-API revision; private changes alter target artifact identity and
require rebuilding/relinking without changing the public contract.

Dependency unions retain a compact origin DAG through aliases, constants,
generic applications, projections, and selected plans. A mismatch identifies
both producer/consumer closures and traces the mismatching type argument to
the introducing observation or realization.

## Canonical static identities

A value used as a canonical generic/domain index needs decidable structural
equality and one unique canonical encoding. Its value, not its evaluation
trace, enters identity. Generic, monomorphized, structural, indexed-qualification,
and constrained-call uses retain the same closed atom or const-binder identity.
Open symbolic indices remain normalized artifact data; arbitrary machine
evaluation does not participate in type equality.

Eligible atoms are integers, Booleans, fixed arrays, records, and cases, with
declaration order determining aggregate identity. Float/text values,
references, slices, dynamic identities, and boundary-opaque values are not
canonical generic atoms. Quotients and constrained records require a proved
canonical representative and required facts at use. Until quotient-backed
canonical rationals supply that evidence, an indexed `Rat` proves positive
denominator, cancelled signed coordinates, and gcd-reduced numerator magnitude
and denominator at the use site. These requirements are stronger than ordinary
evaluation or opaque runtime materialization.

## Result caching and published evidence

Semantic result identity is separate from accounting identity:

```text
ResultKey = normalized implementation closure + arguments
          + selected conformances/providers
          + target observation applications + selected realization applications
          + evaluator semantics version
UsageRecord = ResultKey + usage-schema version + canonical usage counts
PolicyCharge = interpret(UsageRecord, selected cost policy)
```

Changing accounting weights does not invalidate a semantically unchanged
result. Missing telemetry may require remeasurement without invalidating that
result. A cache hit replacing work in the current build graph receives its
recorded logical charge, so warm and cold builds make the same hard-ceiling
decision. Linking an already-built dependency does not charge its historical
production cost.

Published carrierless selected-conformance evidence is separate compilation,
not a local cache exception. A producer performs witness search; a consumer
projects the retained proposition and may cheaply verify its artifact/kernel
evidence without rerunning the search. Ordinary theorem contracts and named
witness/law bundles follow [proof contracts](../proofs/contracts.md); a cache
does not turn erased evidence into runtime values. The separate
[proof-search-cache proposal](../../proposals/0003_proof_search_cache.md) concerns
untrusted search reuse, not a replacement admission route.

## Trait bodies and generators

A trait machine body is its overridable default, without a `default` keyword.
[Semantic reflection](reflection.md) may inspect an explicitly selected type
under the lexical author's ordinary dependency and visibility authority; it is
not restricted to a conforming Self. A generic default does not inherit a
conformance author's or caller's private access. An owner can instead supply a
checked visitor/operation through an explicit requirement or conformance.
Owned TypeSchema descriptions and typed per-member calls follow the reflection
contract; they grant no runtime type-to-generic conversion, source splicing,
boundary authority, or exemption from evaluator admission.

Generated equality follows the current field set. Handwritten equality owns
its coverage and is not automatically substitutable equality; that requires
explicit quotient laws. Record destructuring is exhaustive: bind each field,
rename with `as`, or visibly waive it with `as _`. It is snapshot sugar for
ordinary field `let` bindings, with only copy-eligible implicit bindings and
no reference patterns or new binding modes. Larger fields require an explicit
borrow expression or waiver.

Body-shape coverage is not a requirable value proposition. A separate
trait-level coverage gate is deferred. Reflection visitation establishes its
declared member/case coverage through checked elaboration, not an arbitrary
body-coverage proposition or a separate generator-unroll keyword.

Reflection policy receivers are owned within an evaluation root. That root
freezes completed selections into an ordinary symbolic result snapshot; the
policy's mutable receiver does not escape or mutate compiler state out of band.
The [selection result contract](reflection.md#selection-evaluation-and-retained-results)
requires independent resolution, coverage, access, and requirement validation.
Choosing a static declaration is not manufacturing proof or a runtime callable.

The [implementation note](../../../omega-rust/psi/semantics/build-time-evaluation/README.md)
records the narrower currently supported admission and materialization paths.
