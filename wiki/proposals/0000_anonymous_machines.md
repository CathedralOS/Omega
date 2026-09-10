# 0000: Anonymous machines and captured environments

Status: proposed; no anonymous-machine or lambda form is accepted. This includes
capture-free expressions, capture lists, inferred environment receivers, generic
literals, custom clauses, and state-bearing anonymous bodies. The body below is
a candidate design, not current language rules or an implementation mandate.

## Context and motivation

Named machines, ordinary data, and explicit static callback selection already
express the proposed behavior. Anonymous syntax would save environment naming,
construction, and body association at a use site. It adds ergonomics, not a new
execution model, proof capability, or source of authority. That convenience must
justify parser, elaboration, diagnostic, and maintenance costs.

The simple forms have a worked-out candidate shape; that does not constitute
acceptance. Reflection uses named callbacks and explicit contexts independently
of this proposal. No compiler implementation is claimed.

## Scope and alternatives

The minimum candidate is a capture-free typed body or an explicit capture record
associated with a statically selected body. Compare it with the equivalent named
machine and ordinary environment before extending it.

Three viable choices remain: no anonymous surface; a restricted simple-lambda
surface; or the full ordinary-machine surface below. The full form admits generic
constraints, authored contracts, states, and ranking evidence, but risks bringing
contract annotation back inside a machine. These are alternatives to decide,
not successive promised implementation phases.

Private-body effect inference is already an ordinary facility. It is not ambient
inheritance from a future caller. Reach/suspension/blocking are permissions;
termination and postconditions are guarantees which still require proof. A
contextual ceiling cannot establish a guarantee by inheritance. Callback-dependent
consumer contracts are not necessary to accept fixed-contract lambdas.

## Open questions and acceptance bar

[Callback contract forwarding](../../OWNER_QUESTIONS.md#q2--callback-contract-forwarding)
remains an independent owner question for named generic consumers as well. No
effect projection, conditional call marker, or wholesale contract forwarding is
accepted by this proposal. The choice among no surface, simple forms, and full
anonymous contracts remains part of this proposal, not a pending owner decision.

Before acceptance, show a named-machine/environment baseline beside each proposed
form and price the actual boilerplate saved. Test capture-free invocation,
captured copies and loans, shared/exclusive/consuming access, and named callback
equivalence under one existing fixed requirement. Cover zero invocations of
linear captures, second-call rejection, ordered initializer failure and cleanup,
and trap abandonment without invented rollback. Repeated invocation must use
ordinary fresh reborrows; generic and heterogeneous cases must not require a new
contract system solely to support this syntax. Complex forms need a separate
argument beyond being possible to parse.

No implementation board item is authorized until an owner accepts a defined
scope. The remaining sections preserve the candidate details for that review.

## Source form

An anonymous expression has an optional generic list, optional capture list,
typed parameter list, optional result, ordinary machine clauses, and a braced
machine body. There is no `machine` introducer on the expression. Named
declarations retain it.

```omega
let above_threshold = [threshold = threshold](value: u32) -> bool {
    value > threshold
};

let identity = (value: u32) -> u32 { value };
let selected = above_threshold(sample);
```

Parameter types are explicit, except for an ordinary receiver spelling. An
omitted result is resultless. Captures are comma-separated binding initializers,
not implicitly collected free variables. Generic parameters precede captures:

```omega
let show = <Value>[output = &mut output](value: &Value)
where Value == u32 || Value == f32;
{
    transition {
        Value == u32 -> output.show_u32(value)
        Value == f32 -> output.show_f32(value)
    }
};
```

The output operations above are assumed declared library operations. Static
type branches follow [generic equality](../spec/language/generics.md#static-type-equality).
Bodies may contain ordinary states, transitions, contracts, and ranking
evidence. Captures remain the environment; state parameters carry values along
transitions. A body is not restricted to a single expression. Returning or
stopping in it does not return from the enclosing authoring machine.

Recognition is syntactic: the complete parameter-list, clause, and body form
distinguishes an anonymous expression from grouping or an array. Generic-list
recognition occurs at the head of this form, not by reinterpreting an ordinary
comparison according to expected types. A list or grouped expression alone is
not a callable. Parse implementation must cover nested expressions and contracts
without type-directed selection or repeatedly reparsing unbounded prefixes.

## Environment construction and scope

Each capture introduces a field whose type follows ordinary binding inference.
Its initializer is an expression in the construction scope, evaluated exactly
once in authored order. Copy, move, shared borrow, exclusive borrow, and
disjoint-subplace capture use ordinary [ownership](../spec/language/ownership.md). Construction
does not invoke the body. A copied integer snapshots its value; a reference
retains its referent and lifetime, not a snapshot of its contents.

Capture initializers do not implicitly acquire access to earlier environment
fields. The body resolves outer runtime locals only through declared captures.
Capture names introduce body bindings; conflicting parameter declarations obey
ordinary duplicate/shadowing rules. Static declarations and outer generic or
selected conformance binders retain ordinary lexical visibility and identity.
They are not ambient runtime captures. A capture-free body gains no global
runtime authority. Lexical access rights remain those of the authoring scope;
passing a private getter does not give its caller private field-access rights.

Explicit environment receivers use `&self`, `&mut self`, or owned `self`:

```omega
let show = [output = &mut output](&mut self, value: &u32) {
    self.output.show_u32(value);
};
```

Here self denotes the environment, not an enclosing machine's receiver.
Access to an outer receiver requires a capture such as `owner = &mut self`;
its initializer resolves self in the outer scope. Bare capture bindings and
explicit environment-field access denote the same captured fields. Neither
route bypasses field access or borrowing checks.

An erased capture uses ordinary [binding relevance](../spec/proofs/contracts.md#explicit-erased-bindings):
`[buffer = &buffer, evidence [erased] = evidence](...) { ... }`.
The ellipses stand for ordinary parameters and body. Evidence retains exact
subjects, validity, multiplicity, and provenance, but no runtime field, address,
or cleanup. It cannot justify another buffer, survive invalidating mutation, or
determine runtime data/control. Erasure does not discharge linear debt.

Recoverable failure during construction accounts for already acquired values
and loans through ordinary result, transfer, and cleanup rules. Unexecuted
initializers acquire nothing. A trap instead follows [crash rules](../spec/language/effects.md#guarded-crashes)
and the relevant execution-domain/survivor contract. Neither path invents
rollback; a trap does not promise recoverable cleanup.

## Invocation and receiver access

An explicit receiver declares the environment access required for invocation.
When omitted, infer the least demanding access sufficient for the complete
local body: shared, exclusive, or owned. Moving a capture out without restoring
the environment requires ownership. Moving a value into the environment alone
does not make invocation consuming. An explicit insufficient receiver rejects.

Generic callers check the declared callback contract, not a selected body's
private behavior. The three access forms are ordinary receiver contracts, not
new callable traits or purity categories. A shared callable can exercise held
authority; an exclusive callable can have a precondition permitting only one
successful call. No automatic conversion hierarchy follows from these forms.
Any receiver adapter must preserve the complete contract and resource custody.

```omega
let work = [job = move job]() -> WorkResult {
    Worker::run(move job)
};

let result = work();
```

This call consumes work. A second call rejects. Zero calls do not erase job's
obligations: disposing of an uncalled environment requires the captures' ordinary
authorized dispositions. Scope exit cannot silently abandon linear work.
All normal and recoverable outcomes account for remaining captures; crash and
process-exit outcomes obey their own abandonment contracts.

## Callable association and exact requirements

Both named and anonymous callbacks bind one statically selected body and its
ordinary environment argument. The anonymous value's checked association
identifies the body; it does not store a machine symbol as a runtime field,
convert one to a native address, or introduce a boxed function type.

Named contracts use exact trait machine requirements. An ordinary library may
declare the following fixed-ceiling visitor:

```omega
trait Visitor<Item, Context> {
    machine visit<'context, 'item>(
        context: &'context mut Context,
        item: &'item Item
    )
    terminates;
}
```

Ordinary omission rules make this requirement non-suspending, nonblocking,
crash-free, and empty-reach. It does not admit an arbitrary effectful callback
merely because the argument types match. A library authors a different ceiling
when its customer needs one.

A walker declares Item, Context, and Step in its generic list and constrains
Step with `where machine Step satisfies Visitor<Item, Context>::visit;`.
The clause establishes Step's binder kind and complete contract. An explicit
`machine Step` in the generic list is also consistent; no body or consumer-set
inference may invent a missing callable contract.

At an anonymous argument application, the checker binds Context to the actual
environment, selects the associated body, normalizes receiver syntax to its
explicit environment argument, and checks full refinement against the named
requirement. It records the exact satisfaction edge. Association chooses the
body; it does not prove satisfaction by itself. No ambient conformance search,
whole-trait evidence, or dynamic-dispatch eligibility is created.

An abbreviated call such as `visit_items(items, &mut show)` may recover the
static Step selection from that checked association when argument and declared
contract matching determine exactly one application. A named body plus explicit
context follows the same checking. Conflicting or undetermined selections need
explicit arguments, not a best-match search.

An inline structural callable contract remains available for a one-off use.
It uses the same [refinement judgment](../spec/language/machines.md#substitution), not a second
callable system. Named requirements retain their nominal identities even when
their signatures coincide. Neither a new function-typedef category nor a
reflective test that something is a machine replaces its callable contract.

## Generic families and invocation lifetimes

A generic callback is checked for all inputs admitted by its declared family.
A consumer proves each selected application satisfies those constraints; the
observed application set does not redefine the callback's public interface.
Type, value, machine, and conformance substitutions follow ordinary generic
identity. Explicit conformance selections remain explicit.

Invocation-lifetime binders belong to the callable requirement, not to its
captured environment. Matching must establish validity for every admissible
fresh invocation choice. It cannot choose one long lifetime spanning the
consumer and call that repeated-call compatibility. This rule applies equally
to named and anonymous static machine arguments; see
[generic requirement matching](../spec/language/generics.md#invocation-lifetime-families).

A sequential walker proves its index/projection valid, reborrows its context,
borrows the current item, invokes the callback, releases those invocation
loans, then advances. Its ordinary finite traversal argument and callback
termination promise justify termination. The next context borrow must remain
possible. Captured loans may outlive individual calls under their own contract.

A nonretaining callback cannot save a fresh item loan in a longer-lived context.
Rejection needs ordinary loan checking, not general authored outlives bounds.
Permitting retention instead needs an expressible, checked lifetime relationship
and compatibility with later traversal. General outlives syntax is not implied.

Heterogeneous [reflection](../spec/language/reflection.md) additionally requires independently
checked member selection and projection under its own contract.
Each selected field must satisfy the callback's generic requirements or reject
at that member; no silent skipping, type whitelist, or runtime type-to-generic
conversion follows. Mutable visits preserve disjointness, sequential reborrowing,
and active-case obligations. Early stop is an ordinary library result.

Reflection uses these same generic calls for compile-time selection policy and
runtime field operations. Selection receivers retain their separate completeness
and evaluation-result validation; callback-family checking alone establishes
neither exact selection nor permission to access the member.

## Operational contracts

Fixed callback ceilings and ordinary call-site acknowledgements apply now.
Possible suspension/blocking, reaches, failures, crashes, progress, mutation,
and resources remain independently checked. Preconditions hold at every call;
later calls use the previous post-state. Reentrancy or concurrent invocation is
not granted by receiver inference.

A general facility forwarding a callback's variable operational envelope is
unresolved under [callback contract forwarding](../../OWNER_QUESTIONS.md#q2--callback-contract-forwarding).
It is not implicit in lambda support. The question covers named generic machines
equally and does not block fixed-ceiling callbacks. Failure remains ordinary
result data; crash guards require substitution; repetition and overlap affect
resource composition. A callback contract cannot be copied wholesale onto its
consumer, and a private selected body cannot weaken the required envelope.

## Storage, identity, and evaluation

Capture types determine environment layout, multiplicity, cleanup, carried loans,
and carry demands. Environments use ordinary local/aggregate or explicitly
provisioned storage. Moves do not extend lifetimes, make pinned storage movable,
or enable self-referential construction.

Each literal occurrence has its own generated declaration identity. Repeated
execution of a closed occurrence shares that body identity but creates distinct
environment/resource occurrences. Capture values are not static specialization
keys. Retained identity uses declaration and substitution correspondence, not
source-text hashes, source offsets alone, or host addresses as type authority.

Anonymous environment types cannot be named directly in public signatures.
Use an explicit named wrapper and contract for public return/storage interfaces.
No opaque-result syntax or unrestricted existential packaging is introduced.
Runtime selection among different environments uses an explicit sum/wrapper or
a separately eligible, explicitly selected whole dynamic conformance. A static
generic family is not an unrestricted generic virtual method.

[Semantic evaluation](../spec/language/evaluation.md) may evaluate an eligible invocation with
eligible captured values. A statically known body does not make runtime captures
available during compilation. Erased material remains erased; returned constants
follow ordinary materialization and cannot retain evaluator references or raw
callable addresses. Runtime metadata tables and adapters retain their ordinary
storage, selection, and custody contracts; no global callable registry follows.

## Activation and foreign callbacks

A direct call stays in the current activation, even when it suspends or blocks.
Live captures/arguments retain ordinary carry, cancellation, loan, and stack
obligations. No implicit future, task, detach, or automatic joining is created.

An ordinary library task adapter may specialize an entry that receives and
invokes the environment, then use existing
[transactional task start](../spec/build/task_runtime.md#transactional-start).
Construction happens before submission. Dynamic rejection returns the original
environment and reservations or accounts for their authorized transfer; successful
start transfers execution/storage custody under the selected runtime contract.
Cancellation requests do not release captured loans or discharge linear tasks.
An exact adapter signature is separate library work, not a new TaskRuntime
primitive or permission to emit incomplete activation evidence.

[Foreign callbacks](../spec/build/private_callbacks.md) need a real context/lifetime
registration route to carry an environment. A native address alone carries none.
No-context APIs do not gain a fabricated argument or hidden global allocation.

## Compiler boundary

Psi owns syntax, anonymous declaration/environment identity, capture construction,
requirement checking, and ordinary data/call lowering before Terminal Psi.
Erased evidence and exact callable association remain auditable when runtime
layout omits them. Omega and targets retain realization/ABI responsibilities;
task plans remain build-owned companions. No parallel portable closure IR or
new continuation format is needed.
