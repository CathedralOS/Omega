# RFC: Lambdas and captured environments through ordinary machines

Status: open design proposal. No anonymous-machine syntax, capture rule,
callable interface, generated conformance, or task adapter below is approved or
implemented by this RFC. Source examples are candidate sketches, not executed
tests. Existing named-machine, generic, ownership, and task contracts remain
unchanged. This proposal is not an owner decision or an execution-board task.

## Customers and recommendation

Library authors need callbacks for collection traversal, reflection walkers,
delayed predicates, and task submission. Callers should be able to write a small
operation at its use site, borrow or move its context deliberately, and compose
it with ordinary machines. A reflection design should consider those facilities
on their merits rather than treat the current compiler's named-only callback
surface as a permanent constraint.

Prefer anonymous ordinary machines with explicit environment construction as
the first candidate. Their generated environment is ordinary data; invocation
is an ordinary contracted machine call. Use the existing `machine` word and
braced body as the starting syntax direction, not new `lambda`, `async`, or
`closure` machine species. Capture-list punctuation and callable-binding syntax
remain open. Runtime erasure, allocation, and task activation are separate
operations, never automatic consequences of writing a lambda.

Compare three alternatives before adding machinery:

- Named machine plus explicit context: the semantic and implementation baseline.
  It can express walkers today at the design level, but makes small local
  callbacks repetitive and does not settle a generic per-field callable family.
- Noncapturing anonymous machine only: removes naming overhead but leaves users
  to thread every stateful context by hand. It is not sufficient evidence for
  the editor and task-submission customers.
- Anonymous machine plus ordinary captured environment: the recommended design
  candidate. It adds environment construction and callable binding, but should
  reuse ordinary data ownership, receiver access, and invocation contracts.

Lack of lambda syntax does not make walking impossible. Conversely, lambda
syntax alone does not make a heterogeneous reflected member into a typed value.
Both this proposal and the [reflection proposal](semantic_reflection.md) must
evaluate the generic callable and member-projection connection together before
either design is described as settled.

## Existing foundations and earlier async work

| Owner | Existing contract | Addition still needed |
| --- | --- | --- |
| [Machines](../spec/language/machines.md) | One contracted transition-system meaning, including suspension and checked generated bodies. | Anonymous declaration identity and an expression that constructs its environment. |
| [Static machine binders](../spec/language/generics.md#static-machine-binder-categories) | Authored structural or nominal callable contracts; selected calls become direct calls. No runtime callable or inferred capture. | Binding an anonymous body and, separately, a runtime environment; generic callback families. |
| [Ownership](../spec/language/ownership.md) and [loans](../spec/terminal-psi/loans.md) | Moves, captured places, compatible subloans, and linear debt. | Deriving and checking those obligations for a captured environment. |
| [Dynamic dispatch](../spec/terminal-psi/dynamic_dispatch.md) | Explicit named whole-conformance selection and borrowed instance/table custody. | A specified route from an anonymous callable to an eligible named callable interface. |
| [Semantic evaluation](../spec/language/evaluation.md) | Invocation-sensitive hermetic evaluation and pure result snapshots. | Eligibility and retained identity of anonymous bodies and captured values. |
| [Task activation](../spec/build/task_runtime.md) | Static entry machine plus explicit arguments, transactional start, accountable activation storage, and linear settlement. | A library adapter that submits a callable environment as those arguments. |

The async design already separates a direct call from
`runtime.start<Worker::run>(job)`, and specifies rejected-start ownership return,
fixed nonmoving activation stacks, cancellation safe points, and loans across
suspension. It explicitly does not infer captures or transform a function into
a future. The [concurrency guide](../language_guide/chapter_18_concurrency.md)
teaches that same shape. Those are reusable requirements, not evidence that a
lambda or closure lab has already passed.

The repository history records this task shape in the language-decision and
runtime-contract documentation changes (`a3175c3f0d`, `3bc2c50d3a`). A targeted
search of the available local session index did not recover an earlier
lambda-specific experiment. This is a limit on the recovered evidence, not a
claim that no such discussion occurred elsewhere.

## Candidate source and conceptual lowering

An explicit captured threshold illustrates a reusable shared callable:

```omega
let above_threshold = machine [threshold = threshold](value: u32) -> bool {
    value > threshold
};

let selected = above_threshold(sample);
```

The left-hand threshold names an environment field; the right-hand threshold
is an ordinary capture initializer evaluated when the lambda is constructed.
This copy-eligible integer snapshots that value, not later changes to its source.
The body does not execute during construction. Names, parameters, and body
syntax here are provisional; a lambda body may have ordinary states, transitions,
contracts, and ranking evidence rather than being restricted to one expression.

Its conceptual lowering is:

```text
Environment { threshold: u32 }
invoke(environment: &Environment, value: u32) -> bool:
    value > environment.threshold

above_threshold = Environment { threshold: threshold }
selected = invoke(&above_threshold, sample)
```

The generated names are explanatory, not new public types or a stable ABI.
The body identity is statically selected; the threshold can be runtime data.
This requires no per-value code generation, stored native code pointer, heap
allocation, or globally registered callable. Noncapturing machines have an empty
environment and may be eligible for static-machine binding without a runtime
environment argument.

A mutable captured output is a different environment and receiver contract:

```omega
let show = machine [output = &mut output](value: &u32) {
    output.show_u32(value);
};

visit_items(items, &mut show);
```

Assume a declared library output operation and a walker that borrows its callback
mutably and sequentially. Creating show establishes the captured exclusive loan;
calling it reborrows the captured output. The source output remains unavailable
for conflicting access until that loan is legitimately released. Mutably
borrowing show does not grant additional access to items.

A consuming callable moves an owned argument into one invocation:

```omega
let work = machine [job = move job]() -> WorkResult {
    Worker::run(move job)
};

let result = work();
```

The candidate rule makes this call consume work because it moves job out of the
environment. Calling it twice rejects. Dropping work without calling it is not
permission to abandon job: the environment retains every ordinary linear debt.
The operation must return, settle, or transfer its remaining captured resources
on every permitted outcome under the ordinary contracts.

## Capture and invocation contracts

Prefer an explicit list of capture initializers for the first candidate. Copying,
moving, shared borrowing, and exclusive borrowing use ordinary expression rules,
not an independent capture ownership system. Capture initializers run once in
authored order. If construction can fail partway, already acquired values and
loans require ordinary accounted cleanup or ownership return. No hidden rollback
or silent resource loss is introduced.

The body resolves runtime outer locals only through declared captures and its
parameters. Static declarations, type parameters, and explicitly selected
conformances obey ordinary lexical visibility and dependency rules; they are not
ambient runtime captures. Capturing self is an explicit choice to borrow, move,
or copy an eligible receiver. Capturing a reference preserves its referent and
lifetime; it does not snapshot the referent's contents. Capture-free bodies do
not acquire global runtime authority merely by lacking an environment.

An inferred-capture alternative should be compared for ergonomics, especially
place-precise captures and diagnostics. It is not selected here. An explicit
first candidate must still support capturing a disjoint subplace rather than
unnecessarily locking the entire containing record. Dependent capture types and
retained facts follow the actual captured value/place identities.

The following are semantic categories, not proposed new trait keywords:

| Environment receiver | Meaning |
| --- | --- |
| Shared borrow | Invoke without requiring exclusive access to the environment. |
| Exclusive borrow | Invoke with exclusive access; captured state may be updated. |
| Owned value | Consume the environment and account for all remaining captures. |

These modes describe environment access, not purity or an invocation-count proof.
A shared callable may have permitted effects through an authorized shared
capability. An exclusive callable may require state facts that permit only one
successful invocation. A consuming callable cannot be used by a walker promising
arbitrarily many visits unless it explicitly returns a valid next environment.
Any adapting conformance between modes must satisfy the complete contract and
cleanup obligations; no universal conversion hierarchy is assumed.

Local anonymous bodies may infer a checked receiver requirement and private
operational summary from their use of captures. A generic library still authors
its callable contract. Expected type/parameter information can check or elaborate
the literal, but it cannot invent a missing public requirement from current
callers. Reuse ordinary signature inference only where specified; do not silently
extend it to all higher-order contracts.

The full contract includes preconditions, results and post-state, service reach,
mutation, suspension, blocking, failure/crash routes, progress, resource ceilings,
and custody. A walker invokes under its declared callback requirement; a selected
body must refine it. If a general walker forwards the callback's operational
envelope, that polymorphic envelope needs an explicit checking rule. Lambdas do
not supply universal effect or progress polymorphism just by being anonymous.

Call-site `suspend` and `block` acknowledgements remain required by the selected
call's envelope. Repeated calls must establish their preconditions after each
previous post-state. Reentrancy, simultaneous calls, and cancellation are not
granted by a convenient call operator or inferred receiver category.

## Generic lambdas and reflection walkers

A heterogeneous walker needs one callback body that can be checked at several
field types while borrowing the same environment. Noncapturing lambdas alone
would not resolve that customer. Consider this candidate spelling:

```omega
let show_field = machine<Value> [output = &mut output](
    field: FieldInfo,
    value: &Value
)
where Value == u32 || Value == f32;
{
    transition {
        Value == u32 -> output.show_u32(field.name, value)
        Value == f32 -> output.show_f32(field.name, value)
    }
};

reflect::visit_fields<Player>(player, &mut show_field);
```

This candidate passes an ordinary runtime environment, whose concrete callable
family is known statically. It is not an unrestricted generic method in a dynamic
trait table. The walker would elaborate operations equivalent to:

```text
invoke<u32>(&mut show_field, health_description, &player.health)
invoke<f32>(&mut show_field, speed_description, &player.speed)
```

Compilation selects each member and instantiates its callable application.
Execution supplies the player and uses the same output environment for each
visit; the compiler neither copies the environment per field nor executes output
while traversing the schema. The callback does not need a closed compiler list
of supported types: a more general callback can require an explicitly selected
library conformance per member. An unsupported selected member must diagnose its
path and failed callback obligation rather than silently skip it.

The callable-family contract remains open: compare an explicitly selected
callable conformance on the generated environment with a structural body-and-
environment binder. Existing generic rules do not automatically express a binder
universally callable at every demanded member type. Specify which type, value,
machine, and conformance parameters are bound, where their constraints are
checked, and whether a requirement quantifies over all admitted types or an
explicit selected schema's members. Do not infer a public interface from the
accidental set of instantiations in one program.

Each visit must receive a valid field subloan with the required lifetime. A
nonretaining callback can be checked for each fresh visit lifetime; retaining
field references instead requires an explicit outlives relationship to the
environment and compatibility with subsequent traversal. Do not accidentally
require a static lifetime or permit a short-lived field reference to escape into
a longer-lived closure. Mutable walkers additionally need disjointness or
sequential reborrowing and active-case preservation.

Ordinary walker libraries own traversal order, recursive descent, pruning, and
early-stop outcomes. A callback may return a library result such as Continue or
Stop rather than a new control-flow keyword. Stopping returns control through
ordinary calls and releases/accounts for current loans; it is not a nonlocal
return from the lambda's enclosing authoring machine. Recursive schema/code
derivation must reuse the same closed callable family instead of generating an
infinite sequence of new lambda identities. Runtime cyclic graphs need a separate
visited-object policy and resource/progress contract.

Anonymous bodies keep the lexical access rights of their authoring scope; the
walker does not inherit them. An owner can deliberately author a private getter
inside a callback and pass that operation without exporting raw storage. Generic
member projection still must satisfy reflection's access rules. A closure can
delegate an authorized operation; a descriptive field key cannot impersonate it.

## Storage, return, and dynamic selection

An environment's actual captures determine size, layout, multiplicity, cleanup,
loan provenance, and carry demands. Store it locally, inline in an ordinary
aggregate, or in explicitly provisioned storage as applicable. Captures that
borrow cannot outlive their sources. Moving the environment does not lengthen
those lifetimes or make pinned storage movable. Self-referential environment
construction is not implicitly supported by capture syntax.

Different literal occurrences have distinct generated identities even if their
text matches. Repeated executions of one closed occurrence share code identity
but create separate environment/resource occurrences. Captured runtime values
are not static generic identity. Retained artifacts need stable declaration and
substitution correspondence, not source-text hashes or host addresses as type
authority.

Returning or naming an anonymous environment in a public signature requires a
specified result-type route. An existing explicit named wrapper/conformance is
the baseline; an inferred private result or a future opaque result form must
be evaluated separately. This RFC does not smuggle in associated types,
unrestricted existential packaging, or an implicit boxed function type.

Runtime selection between distinct closure types similarly needs an explicit
sum, wrapper, or eligible dynamic interface. Existing borrowed dynamic dispatch
can be a target only through a defined exact conformance selection. Anonymous
bodies cannot bypass the named whole-map rule. Returning a consuming owned
erased callable additionally needs ownership, storage, and destruction contracts;
the current borrowed dynamic surface is not evidence for that capability.

Generic lambdas can specialize at compile time without being dynamically
dispatchable over arbitrary future type arguments. A runtime reflection table
retains closed adapters for its chosen members and concrete entry shapes; it
does not materialize a universal generic function or a JIT.

## Direct suspension and task submission

An anonymous ordinary machine may suspend or block under exactly the same
contracts as a named machine. Calling it directly stays in the current
activation. Its live environment and callback arguments participate in ordinary
carry, loan, cancellation, and stack analysis at each crossing. There is no
future transformation, separately addressable continuation, or automatic spawn.

Task submission is a different operation. A candidate ordinary library adapter
could use the consuming work closure above as follows:

```text
try_start_callable(runtime, move work)
    specializes an entry machine for this callable/environment
    forwards to runtime.try_start<GeneratedEntry::run>(move work)

GeneratedEntry::run(work):
    consume-invoke work

outcomes:
    Started(task)
    Rejected(work, reason)
```

These are conceptual signatures, not new TaskRuntime primitives. The generated
entry supplies the exact existing start requirement and complete activation
description. Environment construction occurs in the caller before submission;
it does not spend task capacity or execute the body. Known incompatibility
rejects statically. Dynamic rejection returns the closure with its captures and
every passed reservation/lease, or accounts for transfer to another authorized
owner, under [transactional start](../spec/build/task_runtime.md#transactional-start).

On success, the provider owns execution custody and compatible storage. The
environment and activation state are separate from the returned linear Task
claim. Captured borrowed data must remain valid for all possible execution and
settlement; a cancel request or apparent inline completion cannot by itself
release that relationship. CPU/thread restrictions and suspension-forbidden
captures retain their [carry demands](../spec/resources/carry.md). A task owning
captured linear resources must settle/transfer them through its actual normal
and cancellation paths; cancellation never means dropping the closure unseen.

The same operation can be expressed manually as a named entry plus a payload
record. The lambda proposal should reduce authoring overhead without changing
that lifecycle. No async-specific lambda mode, hidden detach, automatic joining
cleanup, or guarantee of cancellation response is added.

[Foreign callbacks](../spec/build/private_callbacks.md) remain separate. A native
address does not carry a closure environment. APIs accepting context tokens may
use checked explicit environment registration with lifetime and quiescence
custody. APIs with no context route need an independently justified adapter;
lambda syntax cannot fabricate a native ABI parameter or a global storage slot.

## Compile-time machines and tables

Metadata filtering, string-name lookup, and table construction are ordinary
library algorithms whether supplied as named machines or anonymous callbacks.
Their eligible concrete invocations may run under semantic evaluation. Temporary
environment/collection storage is accounted to that evaluation; runtime tables
use ordinary [constant materialization](../spec/language/constants.md#materialization).
No reflection-specific allocator, mathematical sequence conversion, or global
type registry follows from lambda support.

A runtime capture cannot be read during compilation merely because its body
identity is static. A lambda whose complete invocation is hermetically eligible
may be evaluated with eligible captured values. Returning callable identity and
an environment as a constant still requires a defined identity/materialization
contract; ordinary snapshot support does not permit returning an evaluator
reference, compiler handle, runtime authority, or raw callable address.

Compile-time code may prepare runtime lookup data such as name bytes and entry
indices. A library linear scan, sorted table, or generated hash table can resolve
a runtime name. The selected entry then uses a retained checked adapter with an
actual instance and contract. Lambda syntax may make those adapters easier to
author, but does not itself specify their runtime retention, erased calling shape,
or storage ownership. Lookup failure remains ordinary data.

## Compiler ownership and cost

Psi owns anonymous-body binding, environment types, capture construction,
callable-family checking, loans, effects, and lowering to ordinary calls and
data before Terminal Psi. Omega and target backends retain their existing
realization and ABI responsibilities. TaskRuntime planning retains its existing
build-owned companion; a lambda is not a new portable continuation format.

Prefer generated ordinary records, machine declarations, and explicitly selected
conformances over a parallel closure IR or universal callable object. This still
adds real checking and retained-identity obligations. Name those obligations
rather than call all closures syntax sugar. Neither absence of a new keyword
nor a handwritten lowering example establishes implementation feasibility or
zero compile/runtime cost.

## Design lab

No experiment has run. Compare each accepted example with a named-machine and
explicit-environment implementation, including failure paths.

| Case | Discriminating acceptance |
| --- | --- |
| Noncapturing literal | Same checked call semantics as a named machine, no runtime code pointer required. |
| Copied threshold | Later source mutation changes neither captured value nor code specialization identity. |
| Mutable output walker | Visits share one environment; source output rejects conflicting access until loan release. |
| Captured disjoint subplace | Permit compatible sibling access when ordinary loan evidence proves it. |
| Consuming job callback | A second call and silent loss of captured linear debt reject. |
| Failure during capture construction | Every acquired resource and loan is accounted for; no invented rollback. |
| Partial callback precondition | Repeated invocation proves each next precondition from the prior post-state. |
| Numeric reflection walker and serializer | Generic literals and named callbacks use the same typed member mechanism, without format-specific compiler operations. |
| User-defined field type | Explicit conformance selects behavior; no compiler whitelist. |
| Escaping field borrow | Reject a short visit loan retained in a longer-lived environment; permit only declared valid outlives/custody relationships. |
| Early stop or callback failure | Ordinary result propagation preserves the environment, current loans, and declared partial-output behavior. |
| Recursive derivation and cyclic values | Finite callable-family reuse during compilation; separate runtime traversal policy. |
| Owner-authored private getter | Delegate only the authored operation, not private structural projection authority to a walker. |
| Direct suspending callback | Remain in the same activation with ordinary carry checks and call markers. |
| Rejected task start | Return the original captured environment and reservations intact or prove their authorized transfer. |
| Successful task and cancellation | Storage/capture lifetimes survive until actual settlement; cancel request alone releases nothing. |
| Runtime closure selection | Explicit sum or eligible conformance preserves actual environment and envelope; no silent boxing. |
| Compile-time metadata filter | Eligible ordinary computation works; runtime captures and evaluator references cannot escape staging. |
| Runtime name lookup | Materialized name/index table dispatches only to retained checked adapters. |
| Native callback with no context | Reject unsupported environment attachment; no fabricated ABI argument. |

## Decisions before implementation

The shape to test is ordinary machine plus ordinary environment, not a settled
source feature. Resolve these contracts before claiming a stable design:

1. Anonymous syntax, explicit versus inferred captures, evaluation order, and
   declaration/environment identity through specialization and publication.
2. Shared/exclusive/owned invocation binding and local inference, including how
   public callable requirements state receiver, state, and operational contracts.
3. Generic callable families, explicit conformance arguments, per-visit lifetime
   quantification, and any required effect/progress-envelope forwarding rule.
4. Storage and return-type routes, and exact conformance generation/selection
   for borrowed runtime erasure. Owned erasure requires additional design.
5. The task adapter's exact signature, rejection ownership, crossing evidence,
   and resource accounting through success and cancellation.

The first proposed experiment should pair a stateful collection walker with the
generic reflection inspector/serializer, then exercise consuming task submission
and rejection. Do not declare the walker design sufficient from a noncapturing
lambda alone, or task support sufficient from static activation metadata alone.
Use expected checked behavior and resource/lifetime evidence, not syntax
convenience or the currently implemented subset, to choose between alternatives.