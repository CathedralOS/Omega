# Calls and outcomes

[Verification](verification.md) reconstructs call obligations and outcome guards.
[Observations](observations.md) defines which outcomes an execution exposes.
[Boundary realization](boundary_calls.md) owns provider selection and settlement.

## Normal results

A machine admitting normal return declares exactly one normal result form:

| Form | Semantic result |
| --- | --- |
| Unit | No runtime value, result `ValueId`, structural result place, or return-equality axiom. |
| Scalar | One typed value with a stable result pseudo-value. |
| Structural | An exact structural type, multiplicity, qualifications, and result place. |

Unit contracts may refer to parameters, not an absent result. A Unit call is
not a scalar call whose result is ignored. `ReturnUnit` is a normal exit;
it charges its one Terminal edge.

`ReturnStructural` transfers a live source place and its exact ordered claim
set to the declared result place. The verifier checks type, multiplicity,
qualifications, claims, and content correspondence. Content-identity facts arise
only at the validated return edge. Scalar and Unit machines cannot declare
structural result places.

A structural call has a distinct operation-result place. Argument transfer and
returned-claim transfer are separate records: returned callee claims continue
caller custody, rather than establish new claims. The result and its claims
enter the caller frontier only after successful completion. A callee crash
creates no result.

Whole-value identity is not determined by ABI fragment width. Source and result
places remain distinct through lowering, assignment, emission, and installation.
A result's qualifications and claims are semantic metadata, not additional ABI
words.

### Scalar qualifications

A scalar declaration retains both its payload carrier and its normalized
qualification set. The empty set is bare. A matching payload carrier alone does
not make signatures interchangeable: ordinary successor bindings, calls and
returns preserve the complete qualification set. A call's qualified result is
available only on normal completion.

Predicate-free, route-free scalar domain declarations can be carried in the
closed scalar qualification catalog. Canonical declarations, normalized sets,
and exact introduction edges participate in semantic identity. An introduction
names the machine, edge, argument position, source value and fresh destination
parameter. It may add same-carrier membership; it cannot remove an existing
atom, alter payload bits, or qualify the original source retroactively. Every
incoming edge is checked independently. A result annotation, ordinary constant,
load or arithmetic operation cannot establish membership by itself.

These declarations do not encode predicate proofs or routed authority. Those
theories require their own evidence vocabulary rather than an assertion that
they are obligation-free. Qualification adds no runtime tag, storage or operation;
the ordinary edge transports the payload. Host scalar arguments alone do not
establish a qualified entry contract. Current interpreter entry therefore rejects
qualified host inputs until an evidence-bearing input interface exists.

### Primitive array construction

`EstablishScalarArray { elements }` establishes one complete owned structural
value. Its exact result type is a fixed array, recursively containing fixed
arrays or a primitive scalar. Operands are the already evaluated scalar leaves
in row-major index order. Their count must equal the product of all dimensions,
and every operand must dominate the operation and have the exact leaf carrier.
Construction performs no conversion, arithmetic, or floating-point rounding.

Dimensions remain in the structural type even when any dimension is zero.
An empty payload does not erase its element carrier or inner dimensions. Type
declarations must still exist and be acyclic below an empty dimension; an empty
array cannot hide an unknown, recursive, or nonprimitive element shape.

The result is unrestricted, unqualified, and claim-free. Construction establishes
neither borrowed storage nor content or qualification evidence. Formation and
operand-availability checks decide this total operation; it introduces no new
proof obligation or authorized admission. The ordinary structural frontier and
return rules still check result availability and exact producer/consumer custody.
It may occur among other ordered operations, not only at a machine's return.

Execution retains the actual typed leaf payload, not just an opaque place
identity. One logical operation unit is charged before the complete array is
established; insufficient fuel establishes nothing. A later return charges its
own edge. Suspension or calls must preserve an already completed construction
without replaying it. Consumers that cannot carry the payload through an argument,
result, transfer, or native realization reject that use explicitly; recognizing
the structural type alone does not establish executable support.

## Call contracts

A call names its exact callee and positional actuals. Each published callee
`requires` clause has one distinct caller obligation at the same position.
Validation checks the complete scalar/structural signature, defined operands,
result, transfer rows, obligation arity and uniqueness, and surviving crash
continuations. Structural qualifications do not occupy scalar proof slots.

The verifier substitutes actual value/place identities into requirements before
importing guarantees. Clause positions and child order survive substitution,
even when actuals are equal or reordered. Never reverse-match caller expressions
or compare uninstantiated formal names. A callee's guarantees cannot prove its
own invocation's requirements; the argument's arithmetic obligations precede the
call obligation.

Normal guarantees refer to the actual result and appropriate parameter versions.
They apply only on normal return; external exit neither establishes them nor
necessarily violates them. A no-normal-result terminal transfer publishes no
fictional result or normal-return guarantee.
The reserved result occurrence belongs to its contract owner, not any local or
parameter spelled `result`. Declared requirements and published crash routes
describe invocation entry. Reassigned storage and earlier immutable snapshots
are distinct values; current reads cannot impersonate entry facts.

Surviving crash routes are derived from the pinned callee contract using exact
actuals. A simplified caller condition is proof information, not a replacement
callee interface. Empty or untranslated continuation rows cannot erase a crash.
A true route permits that crash; it does not execute one. An unconditional
no-return callee crash needs no extra caller branch or fabricated crash event.

## Argument and result ordering

Operands evaluate in authored formal-position order, with nested producers
completing before their enclosing invocation. Mixed scalar and structural
operands share that order; they do not form two separately evaluated lists.
Short-circuit control remains selective. A crashing operand prevents later
operands and the enclosing call from executing.

Completed scalar operands and structural results retain their identities across
evaluation and suspension. Private staging coordinates are not source binding
ordinals. Source lowering must rejoin the authored occurrence and exact
producer/consumer relationship, even when substituting another same-typed value
would satisfy Terminal type or ownership checks.

A moved structural result leaves no caller disposal; its receiver owns later
transfer or cleanup. An unconsumed affine result requires caller cleanup.
Multiple unconsumed results are disposed in reverse production order before
older live parameters. Use before production, duplicate moves, forged producers,
and cleanup of an already transferred result reject.

For a nested whole-result operand, distinguish the actual transfer schedule:

| Use | Required ownership relation |
| --- | --- |
| Owned transfer | Transfer the temporary exactly once to its enclosing call; no caller-return disposal. |
| Shared read | Keep the owner live through the consumer, then dispose it on the exact normal continuation. |
| Partial transfer | Transfer the selected subtree and dispose its exact residual complement at the dying continuation. |

A shared read does not turn the owned result into a returned reference.
Projected results require retained storage, loans, claim transfers, and exact
residual cleanup. Admitting a result signature alone establishes none of those.

## Scalar evaluation and convergence

Scalar values and structural places retain separate namespaces with complete
authored argument-position correspondence. Initializers execute in source order;
their values must dominate every use. Short-circuit expressions evaluate only
the selected operands. A convergence parameter represents the selected result,
not permission to execute both branches or change structural ownership.

Every normal-return path performs its complete required cleanup, whether lowering
duplicates that cleanup on leaves or shares a tail. Sharing must preserve result
and referent identity, effects, proof premises, and cleanup order. Each preceding
exact arithmetic operation remains independently justified; a safe final result
does not establish safe intermediates.

## Normal cleanup

The [ownership contract](ownership.md) defines claim paths, partial moves,
residual complements, and nominal cleanup eligibility.

The normal return carries an exact ordered cleanup-action stream, empty when
nothing remains live. Actions distinguish:

- Whole-root no-code disposal.
- Typed residual disposal.
- Executable nominal cleanup.

The verifier reconstructs the complete live frontier and reverse declaration
order, checking each nominal target and obligation. Return evaluation charges
the edge and materializes the result before cleanup commits. Nominal bodies run
resumably; budget exhaustion must neither replay an action nor partially commit
the exit. Logical-work analysis composes their invocations.

Lowering and publication preserve action order and call ownership. A no-code
disposition produces no target instruction. Register-held results alone are not
persistent storage for a later projected use.

## External process exit

The canonical [process-exit requirement](../language/process_exit.md) carries
an explicit closed external completion kind and no normal result. Its invocation
is a terminal transfer retaining the exact requirement and ordered arguments,
with no normal successor, return postconditions, or implicit cleanup.
The verifier reconstructs its terminal-external observation independently.

An ordinary helper that conditionally exits still has a normal continuation
when it returns. The verifier checks the returning branch's result and cleanup
without joining a fictional frontier from the exit branch. Reach ceilings
propagate exit permission; they are neither guaranteed exit nor outcome guards.

Exit abandons the bound domain, including outstanding linear obligations,
without successful-disposition receipts. Retained local frontier evidence is
only a lower bound; survivor guarantees require their crossing evidence. Normal
entry return must still settle or legally transfer task/resource custody under
the root/runtime contract, even when physical completion uses an exit syscall.

## Suspension

Suspension is an interprocedural state of an ordinary call, not a Terminal CFG
terminator. A possibly suspending call has one local completion continuation.
While parked:

- The operation is incomplete and its result is not established.
- Later operations have not executed.
- No return, crash, cleanup, or affine-disposition edge commits.

Parking transfers scheduling custody of the still-live activation. It creates no
source-visible continuation value. Resumption continues the same incomplete call;
normal completion then establishes the result and reaches its continuation.
Budget suspension likewise preserves the completed prefix without repeating a
paid invocation or transfer.

A suspension site binds the exact call, checked crossing, callee, and commitment
to the complete ordered live frontier. Its plan retains each value/place,
type, storage role, exact live claims, and carry requirements for suspension,
CPU, host thread, and address stability. Validation checks site/plan bijection,
target, frontier commitment, membership, visibility/provenance, order, counts,
types, and policies. Missing, duplicate, redirected, or understated rows reject.

Completeness requires an independently established possibly-suspending crossing
roster. Matching a producer's site and plan is not enough: deleting both must not
erase a required crossing. A scalar/block shape alone cannot establish checked
liveness or claim custody.

Terminal retains provider-independent carry demand. The runtime activation
companion joins it to CPU/thread preservation, exact stack demand and lease,
runtime provider, and preservation evidence. Those selections do not enter
target-neutral Terminal semantics or weaken its frontier.

`request_cancel()` introduces no parked-state exit and retains the external
`Task<T>` claim. It changes the eventual ordinary outcome at a checked safe
point; `finish(self)` consumes the claim. Crash remains a no-successor outcome.

Preserving a parked frontier proves safety, not finite response. Response
analysis may report `NoFiniteGuarantee` at the responsible edge and its cyclic
component. Bounded-response and termination profiles reject that result unless
accepted finite-wait evidence closes it.

### Task activation identity

Task-runtime `start`/`try_start` selection is build-owned. Its strong,
domain-separated specialization commitment binds the exact checked requirement
and operation, package-qualified target/entry signature and parameter modes,
and target machine contract. A compact specialization value is a report
coordinate only. Runtime receipt invocation identity uses the strong commitment;
compact equality cannot authorize another specialization. These selections travel
through the [build-owned companion](product.md#build-owned-companion), not by
changing target-neutral call meaning.

## Crash

`Trap` and `Abort` are closed causes on distinct no-successor terminators.
A crash is not a normal transition or a missing cleanup list.

Each site carries its canonical guard set, same-cause published route coverage,
and a statically known local frontier lower bound. The guard retains the exact
incoming conjunction and any sound canonical consequences used as route
witnesses. A derived consequence does not replace the exact path identity.

Published buckets are fingerprinted and normalized by cause. Each contains a
canonical disjunction of predicates over the same lowered values and places as
executable Terminal Psi; an unconditional clause contains `Truth`.

The verifier proves every site guard from independent entry/CFG facts and checks
same-cause coverage: the published route is `Truth`, or the guard set contains a
canonical predicate from that route. Call composition substitutes arguments and
caller path facts. Disproving every route removes that cause's edge from the
caller's semantic frontier.

A callee-body fact is available only when the body belongs to the same
fingerprinted verification unit. Otherwise use the imported published ceiling
and its certificate.

The frontier lower bound reports obligations definitely live at the site.
It does not enumerate the dynamically abandoned frontier, certify unlisted
state or external effects, or authorize survivors. Restart needs separate
closed-custody, recovery, external-reset, and target-isolation evidence.

The interpreter returns a distinct crash outcome with cause and semantic site,
not ordinary result data. Build-time evaluation rejects an invocation with a
surviving crash route; an invocation that disproves every route remains admissible.
Native lowering preserves reachable no-successor leaves. A physical check may
remain even when a caller proves its semantic edge unreachable, unless valid
specialization removes it.

Crash obligations are semantic and fingerprinted; their proof derivations remain
replaceable certificate material.
