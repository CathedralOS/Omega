# Calls and outcomes

[Verification](verification.md) reconstructs call obligations and outcome guards.
[Observations](observations.md) defines which outcomes an execution exposes.
[Boundary realization](boundary_calls.md) owns provider selection and settlement.

## Normal results

A machine declares exactly one normal result form:

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
