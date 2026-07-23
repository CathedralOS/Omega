# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-23.

## 1. What is the runtime and object-safety contract for `dyn Trait`?

Closed-world call-site specialization currently makes `&dyn Trait` parameters
execute correctly when every concrete receiver is known at its call site. It
cannot represent a runtime-varying trait value stored in data, passed across a
component boundary, or rebound to one of several satisfiers. The language guide
explicitly leaves the runtime representation and boundary legality open, while
the remaining task requires descriptors that preserve satisfier identity.

Decide:

- whether the stable value is a two-word `{instance, table}` pair whose table
  identity names the satisfier, or carries a separate sealed satisfier/contract
  identity (or component/endpoint handle);
- which trait signatures are object-safe, especially `Self` outside the
  receiver, unbound trait parameters, value returns, generic requirements,
  effects, capabilities, and boundary machines;
- whether `dyn Trait` may be owned/stored directly or only borrowed, and how
  lifetime, mutability, drop, migration, and hot-swap pinning travel with it;
- who emits, owns, versions, validates, and updates machine tables, including
  the ABI identity used across separately built components; and
- how named satisfier selection and third-party named-only conformances are
  encoded and checked at coercion.

Recommendation: use a sealed descriptor whose logical identity is
`{instance, satisfier_contract}` and let a validated target-specific table be a
private realization of that contract. Initially admit only borrowed receivers,
fully bound trait parameters, and requirements whose nonreceiver
parameters/results do not mention `Self`; require declared effect/capability
ceilings at every dynamic slot. This keeps the public model independent of raw
table addresses and leaves room for loader-controlled table replacement.

## 2. What is automatic cleanup's graph-edge and partial-value contract?

Omega already records affine StateExit events and rejects non-empty `drop`
bodies so cleanup cannot silently disappear. Executing those bodies is not just
an instruction-selection task: the language has graph states rather than
lexical scopes, while the current guide still labels exact cleanup syntax and
field order provisional.

Decide:

- which outgoing edges run automatic cleanup (explicit transition, terminal
  return, natural state completion, trap/failure, and synthesized call
  continuation), and exactly where cleanup occurs relative to argument moves,
  guard evaluation, result materialization, and the target handoff;
- the deterministic order for locals, by-value parameters, the owning value's
  `drop` machine, remaining fields, nested aggregates, and conditional sum
  payloads, including partially moved values;
- whether the reserved `Type::drop(&mut self)` body is inlined onto every edge,
  lowered as an ordinary state call with a continuation, or represented by a
  distinct checked cleanup plan, and how recursion/re-entry is constrained;
- how `requires`, `ensures`, effects, boundary reaches, and the settled
  infallible/non-suspending rule are checked and instantiated at each implicit
  cleanup site; and
- what proof artifact distinguishes a trivial affine discard from executed
  cleanup and demonstrates that every live cleanup obligation is transferred or
  discharged exactly once.

Recommendation: synthesize an explicit checked cleanup-edge plan before
backend selection. On each normal outgoing edge, move target arguments first in
the semantic plan, then clean the remaining live locals in reverse creation
order, by-value parameters in reverse declaration order, invoke the owner's
cleanup body, and finally clean remaining fields in reverse declaration order.
Reject cleanup on nuclear traps, fallible/suspending drop bodies, recursive drop
cycles, and any partially moved shape the plan cannot enumerate. Treat this as
one ownership subsystem rather than special-casing calls in instruction
selection.

## 3. What is a composite linear value's resource frontier?

Omega requires structural linearity: a record, live sum payload, array, or
generic container cannot erase a contained linear obligation. The current
whole-place checker can conserve one obligation through a composite, but it
deliberately rejects extracting one field from a multi-resource linear record.
Accepting that program requires a semantic decomposition rule, not merely
recording a field segment: two independently established fields must retain two
origins, and the remainder must stay live after either field moves.

Decide:

- whether `[linear]` on a composite denotes one nominal claim, the frontier of
  its contained linear claims, or a nominal claim in addition to those
  contained claims;
- whether constructing a composite automatically merges field claims, merely
  nests them, or requires an explicit resource operation, and the inverse rule
  for field extraction/destructuring;
- whether a by-value whole-composite consumer discharges every live component,
  only a nominal claim, or must expose an outcome mapping for each component;
- how alternative sum payloads, repeated array elements, generic substitution,
  and partially moved records identify their live component set at joins; and
- which stable identity extends `PermissionProvenance` so multiple components
  established at the same state-entry or statement source cannot collapse into
  one apparent origin.

Recommendation: define a value's permission state as a path-indexed resource
frontier. A nominal linear leaf contributes one claim; a composite with linear
children carries those child claims at canonical field/index paths without
minting an extra claim unless the declaration explicitly opts into a distinct
nominal protocol. Whole-value moves preserve the frontier, field moves transfer
the selected subtree and leave siblings live, and whole-value consumers must
account for every live frontier entry. Give each establishment an event-local
origin identity rather than using source location alone. Defer dynamic-index
owned extraction until the index/disjointness proof can name a unique element.

## 4. How is quantified convergence packaged as a quotient relation?

The checked construction corpus now proves rational closeness transitivity and
its pointwise sequence form for arbitrary precision and indices. That is not
yet the proposition required by `data Real = CauchySeq %
converges_together`. A Cauchy certificate has the logical shape "there exists a
modulus such that, for every positive precision and every pair of later
indices, the samples are close"; heterogeneous convergence has the same
existential/universal shape. Current machine parameters quantify only across a
theorem declaration. They cannot package an existential static-machine witness
and its universal proof as a value or as the checked pure binary `bool`
relation the quotient validator requires.

Decide:

- whether the general source surface is a proof-only proposition/certificate
  type, explicit quantifiers, or an existential package of static machine
  witnesses plus checked theorem schemas;
- whether a sequence's modulus and Cauchy proof participate in
  `CauchySeq<...>` family identity, remain erased evidence attached to one
  representative, or use a separate normalized proposition identity;
- how `converges_together<A, B>(a, b)` binds or receives its joint modulus and
  proof while remaining the binary relation shape required by quotient
  formation;
- how reflexivity, symmetry, and transitivity compose existential witnesses
  without a compiler-known Cauchy rule, and how their certificates are exposed
  to the existing quotient equivalence checker; and
- which termination, universe, coherence, and separate-compilation rules keep
  quantified certificates ordinary checked Omega declarations rather than a
  hidden trusted logic.

Recommendation: add one general proof-only quantified-certificate mechanism,
not Real-specific syntax. It should existentially package erased static-machine
witnesses with checked universal theorem schemas, give the resulting
proposition a normalized identity, and let quotient relations consume that
proposition plus ordinary equivalence witnesses. Keep all moduli and proof
machines out of runtime layout. Do not admit an always-true executable relation,
an implicit compiler quantifier, or a boundary axiom as a temporary Real
implementation: each would change or assume the semantics the construction is
supposed to prove.
