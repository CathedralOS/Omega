# Terminal Psi operation vocabulary

> **Needs porting.** This document has not been consolidated or vetted for the
> current documentation structure. See the [migration index](../../README.md).

[Documentation index](../../../README.md) | [Pipeline](pipeline.md)

The [portable-product contract](../../../spec/terminal-psi/product.md),
[mathematical proof contracts](../../../spec/proofs/contracts.md),
[boundary-call and realization contract](../../../spec/terminal-psi/boundary_calls.md),
the [immutable byte-view vocabulary](../../../spec/terminal-psi/byte_views.md), and
[observations](../../../spec/terminal-psi/observations.md),
[encoding](../../../spec/terminal-psi/encoding.md),
[verification](../../../spec/terminal-psi/verification.md),
[calls and outcomes](../../../spec/terminal-psi/calls_and_outcomes.md),
[control flow and ranking](../../../spec/terminal-psi/control_flow.md),
[structural access and stores](../../../spec/terminal-psi/structural_access.md),
[loan resources and compatibility](../../../spec/terminal-psi/loans.md),
[structural claims and cleanup](../../../spec/terminal-psi/ownership.md),
[dynamic dispatch](../../../spec/terminal-psi/dynamic_dispatch.md),
[private callbacks](../../../spec/build/private_callbacks.md),
[mathematical proof values](../../../spec/terminal-psi/mathematical_values.md),
[integer certificates](../../../spec/terminal-psi/integer_certificates.md), and
[logical work](../../../spec/resources/logical_work.md),
[content custody](../../../spec/resources/content_custody.md),
[placed access](../../../spec/resources/placed_access.md), and
[component publication](../../../spec/build/component_publication.md) have moved to their
specification owners. This remaining reference carries operation-specific
meaning and validation details until their consolidation;
it does not redefine those migrated subjects.

Implementation entry maps live beside
[Terminal production](../../../../omega-rust/psi/compiler/terminal-production/README.md)
and [Terminal-to-abstract lowering](../../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md).
The [execution board](../../../../TASKS.md) owns unfinished implementation.
Current pre-release producers and consumers move together; stale artifacts reject.

## Terminal requirements

Terminal Psi is immutable and self-contained. It contains no arena handle that
requires `TypedTrees`, source syntax, the producer compiler, or instruction
selection to interpret its meaning. It contains:

- concrete machines and instantiated types;
- explicit typed blocks, block parameters, values, calls, transitions,
  continuations, and terminals;
- lowered predicates over the same stable value/place identities as execution;
- typed structural places, including ordinary and provider-backed roots plus
  field, dynamic-index, dereference, and range/subextent projection;
- explicit cleanup, transfer, conservation, invalidation, and boundary actions
  on edges, plus suspension plans on the exact incomplete calls they govern;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

The first quotient correspondence carrier is proof-only. `TerminalModule`
retains a strictly identity-ordered table for the narrow monomorphic, total,
direct faithful `define` certificate and the position-preserving direct
`lift` certificate backed by explicit `Congruence` and
`ForwardPreconditionTransport` evidence. The transport payload retains every
public-`Q`, representative-`P`, and congruence-legality fact's Left/Right
application side, authored source coordinate, and selected-theorem coordinate.
The codec serializes the complete
source-free certificate, including its canonically role-ordered theorem
evidence, and rederives its retained identity on decode. The role discriminant
precedes the selected application and role-specific payload in identity and
canonical bytes. Representation validation rejects missing, duplicate,
reversed, surplus, role/payload-mismatched, and unknown-tag evidence before it
independently reconstructs the theorem, correspondence, eligibility,
fact-major/source and theorem-coordinate order, exact congruence-`P`/transport-
`P` join, and direct-result shape. Format 53 / vocabulary 56 carry this
strengthened source-free contract. A nonempty table is still
rejected by execution validation, owns no machine or operation, and
does not authorize a representative call. The explicit producer attachment is
therefore a canonical-retention prerequisite, not executable quotient
lowering. A separate proof-only package-review row now covers the total, direct
`define` correspondence and the position-preserving direct transport-backed
`lift` by transactionally rederiving the complete source batch and retaining
the selected package's exact public callable, theorem/contract-fact
coordinates, relations, eligibility, and result coordinate. Package-review
schema 120 / row schema 78 / recovery schema 16 already encode the closed
transport kind and complete two-role payload, so no schema bump is required.
It is not ordinary checked package projection: quotient contract calls and
executable requests remain blanket-rejected, while two-argument lift, adapted,
literal, permuted, repeated, generic, private, and broader forms plus the full
package-review migration remain open.

Omega task activation applies the same authority split after checking.
`TaskRuntime::{start,try_start}` retains its compact specialization value only
as a report coordinate; provider planning derives a domain-separated strong
commitment over the exact checked TaskRuntime requirement and operation, exact
package-qualified target/entry signature including parameter modes, and target
machine-contract commitment. The task runtime receipt binding carries both
values but derives invocation identity from the strong commitment alone, so
compact equality never authorizes a different specialization.

## Direct scalar calls

The current `Call` operation names one canonical callee, carries positional
scalar arguments, carries exactly one caller obligation identity for each
published callee `requires` clause, and explicitly records the normalized
no-successor crash continuations that survive at that invocation. Validation
checks the complete signature, argument definedness and types, result type,
obligation arity, global obligation uniqueness, and crash-continuation
coverage. Verification substitutes the positional arguments into the callee
requirements and guarantees: requirements become caller proof obligations,
while verified guarantees enter the caller's normal-return semantic axioms.

The call verifier accepts exact unconditional and guarded routes from an
in-module callee. Terminal crash predicates retain canonical proposition terms,
not producer-authored identity bytes. The verifier substitutes every callee
parameter `ValueId` with the corresponding arbitrary caller-local argument,
reconstructs the surviving continuation set, and requires coverage by the
caller's published ceiling; an empty or untranslated set therefore cannot erase
a crash. Checked scalar contracts and body crash sites retain structured
predicate meaning through terminal lowering. Invocation-specific guarded call
rows now retain that same structure after substituting direct parameter and
caller-local scalar arguments. Checked scalar graphs also retain direct
call-valued bindings, their exact call coordinate, and positional scalar
argument plans. Source production composes the reachable in-module checked
scalar call closure, consumes each matching crash row, and emits `Call` with
parameter or computed direct-local substitutions intact. Calls stage
short-circuit scalar arguments left-to-right and Omega target lowering accepts
the resulting calls inside conditional control. A guarded staged call follows
the checked row's pinned target contract and substitutes its parameter-relative
routes with the exact terminal argument values; it never reverse-matches caller
expressions, which would be ambiguous for equal or overlapping arguments. A
selected boundary-operator result bound inside an attached Unit machine uses
the same scalar `Call` operation. Provider settlement supplies the exact
authored use, public requirement, strong `ProviderPlan` identity, and selected
machine/state application while Unit planning still owns the unrewritten
operator expression. Attached-Unit production admits only that selected scalar
graph and its ordinary scalar-call closure, emits the selected realization as
a distinct Terminal machine, and passes the call result through a
source-ordered prefix of immutable, branch-free scalar locals to later calls.
Each such local rejoins its exact checked scalar-expression fact; nested calls
and short-circuit control remain outside this lane.
The first native continuation is deliberately narrower than the semantic call
surface: attached Unit bodies may call service-free fixed 8/16/32/64-bit
integer functions with constants or prior scalar-call results. Target
selection retains the callee's exact ABI; assignment gives every result a
distinct durable Unit-frame home; native emission retains typed relocation and
argument/result intervals; object construction independently regenerates those
bytes; installation format 46 transports the validated records. This is
artifact consistency, not evidence that a human or model audited the code.
Before Terminal or native production, Omega independently resolves the
carrier's strong plan identity against its complete retained
`SelectedProviderPlanFacts` and requires the row's exact checked-adapter machine
and state. A second conforming machine with compatible signature, contract, and
reach cannot replace that selected row. Missing, duplicate, stale, or
coordinate-mismatched applications fail closed; neither planning nor
production scans conformances for a uniquely shaped realization.

Free scalar return calls remain in their authored arms for computation planning.
Other supported guarded return calls use the existing arm-local continuation
normalization.
The continuation captures referenced state parameters and explicitly typed
primitive local values, including the current value of mutable local storage.
It evaluates the original scalar arguments and invokes the callee only after
the arm is selected. Repeated references share one captured value; argument
order, short-circuit scalar expressions, and partial arithmetic stay inside the
selected continuation. Local places, borrows, recasts, and nested calls are not
converted to value captures by this normalization. No additional Terminal
call or return representation is introduced.

Call-bearing scalar initializers, local assignments, state arguments, guards,
and returns retain arena-backed checked computation plans, separate from the
pure scalar expressions consumed by proof.
Their value leaves use the source state's checked scalar namespace; call nodes
retain exact flow-call handles and authored occurrence ordinals; conditional
nodes select one result; pure application templates consume only their computed
operands. Builtin Boolean composition, fixed-integer arithmetic/comparisons,
and nested free scalar calls lower to private typed blocks. Each completed
argument is carried before the next one starts, and only the selected `&&`/`||`
RHS executes. Trailing returns, unconditional value transitions, and both guarded
return arms use the same evaluation path, then deliver the completed result to
a private return block. No source state, source local, or new Terminal operation
is manufactured. Each root node retains the exact authored outer expression
handle, checked against its statement and destination role before expansion.
Invalid handles, cycles, duplicate roots/call occurrences, swapped arm roots,
and mismatched carriers or invocation coordinates reject before publication.

Pure scalar roots use the same authored-expression locator as computation roots.
Each pure plan requires one source-binding row and one expression row at its
state, statement, and destination role. Their expression handle, declaration
destination, primitive carrier, and ordered declaration namespace must rejoin
the authored source. The namespace contains scalar parameters followed by prior
initialized immutable primitive locals; mutable storage keeps its separate
symbol identity. Missing or conflicting rows, stale handles, reordered symbols,
and swapped continuation roles reject instead of selecting a matching row from
an ambiguous set. Shared structural and attached-Unit scalar-root lowering uses
this same check.
Direct scalar call bindings also rejoin the authored callee and result carrier,
and state transfers rejoin their selected target, including zero-argument calls
and transfers. A trait-operator realization consumes its existing source-bound
return only when it agrees with the selected realization plan; lowering does not
clone the checked program to insert a second return row.

Boundary and ordinary Unit calls retain source-bound scalar arguments through
their shared checked producer and all existing direct and composed lowering
paths. Each copied operation argument must agree with the unique checked plan,
authored operand handle, primitive formal, and caller declaration namespace.
The dense scalar callee ordinal is distinct from the authored argument index:
structural arguments advance the latter, and an implicit receiver does not.
Outer call coordinates and targets rejoin even when there are no scalar
arguments. Boundary source sites and exact signature or machine-parameter
requirements remain distinct from ordinary machine owners. A call through a
nominal machine parameter uses its exact boundary requirement's argument role,
not the ordinary Unit-call role of an unresolved callable. The checker and
lowerer share the existing compiler-intrinsic boundary-recognition policy;
lowering does not independently infer a compatible provider.
The retained call roots are statement calls and bare calls in immutable local
initializers or expression statements. Unit and boundary statement arguments
select either a pure expression or an exact checked computation root. Nested
scalar calls belong to those operand graphs, not additional statement operations.
The outer call keeps ordinal zero; nested preorder ordinals identify occurrences
and never determine execution order. Flow calls retain generational authored
expression handles, so lowering rejoins each nested target and operand root
without source-span lookup or a second call-ordinal traversal.
Nested calls may use a resolved data qualifier when the callee has no `self`
parameter. The qualifier must name that callee's exact attached data owner;
it is not a runtime receiver, and an arbitrary value receiver is not discarded.

Statement-call operands share the scalar evaluator's ordered private blocks.
Each completed scalar argument is retained before the next starts, including
pure Boolean short-circuit siblings. The caller's immutable scalar namespace
survives the completion block without gaining temporary argument slots. Structural
places remain in the enclosing Unit machine until the outer call commits their
transfers; normal cleanup follows existing ownership, while Trap and Abort have
no cleanup successor. Arithmetic obligations are finalized on the completed
selected Unit module, after borrowed-place and cleanup assembly, not on a
provisional closure used by another lowerer.

Single-state Unit bodies may finish with an authored Unit call expression,
with omitted or explicit Unit return annotations. Its computed scalar operands
use the same call-argument roles and evaluator as a semicolon call. Normal Unit
return follows the completed call; this does not imply stack-frame reuse or
tail-call optimization. Source normalization keeps the expression instead of
manufacturing an untyped result local. Validation requires the exact callee to
return Unit, and lowering rejoins the captured outer expression even when it
has no scalar operands. Value-returning callees cannot be silently discarded.
An affine-local prefix remains established in entry and declaration order while
operand evaluation branches. The verifier requires that exact establishment
sequence and checks live custody and reverse cleanup on each normal path; a
crash has no cleanup successor. This does not admit branch-local establishment.

Immutable scalar bare-call result initializers in a single-state Unit caller
also evaluate scalar operands through those blocks. Ordinary and boundary scalar
results may follow other scalar locals and Unit or boundary call statements.
The producer walks those statements in source order. Statement positions include
intervening calls; dense scalar-binding positions advance only when a local is
established. Pure primitive locals use the same positional distinction.

Operand roots use the pre-initializer namespace; only successful outer-call
completion establishes the result. The evaluator retains earlier values across
successive operand graphs. The result operation owns the outer call, so no
duplicate whole-initializer computation is emitted. Source eligibility and
lowering rejoin the exact local initializer, result carrier, declaration namespace,
callee, static qualifier, and captured outer flow occurrence independently from
the nested operand occurrences. Boundary structural-result initializers share
the authored statement sequence and may follow scalar locals, scalar results,
ordinary calls, and earlier structural results. Scalar and structural result
bindings retain independent dense ordinals. Each boundary result is installed
only after its operands and provider complete; unused affine results are cleaned
in reverse production order on normal return. A later operand crash retains
earlier results without creating a cleanup successor. The exact authored local,
result carrier, and call occurrence rejoin independently of operand graphs.
Whole, owned, claim-free affine boundary results may move once into a later
ordinary Unit call, scalar-returning boundary wrapper, affine-result call, or
direct boundary call. Direct boundaries accept established ordinary or boundary
results across Unit, scalar, and structural return carriers, including nominal
boundary requirements. Successful boundary completion consumes the input before
establishing a replacement result; rejected or ill-typed provider responses
leave the input available for retry.
The call rejoins the exact authored local or temporary expression and transfer
event; only that result
loses caller cleanup. Boundary result signatures remain independently checked,
without deriving facts from provider implementations. Unrestricted results do
not enter this affine move route. Named immutable, whole, plain-owned affine
results may also supply shared reads to ordinary Unit calls, scalar boundary
wrappers, and direct Unit or scalar boundary calls. The exact authored `&local`
and captured read access rejoin the producer binding; a read neither transfers
ownership nor removes cleanup. Repeated reads retain the same value identity
until one final owned move or reverse-order caller cleanup. Independent frontier
verification requires the intact owner to be live at every read and rejects a
call that both borrows and moves that result. The shared parameter is unrestricted;
the produced value and its caller-owned custody remain affine. Provider refusal
leaves that custody available for retry, and a later operand crash adds no cleanup
successor. An ordinary Unit call may also read a whole anonymous result
through its sole `&producer(...)` argument. The source owner retains the real
expression root: owned affine establishment at the producer, an unrestricted
shared loan at the consumer, then affine cleanup at that consumer's dying
continuation. Lowering independently rejoins those events, their provenance,
source order, and exact call identities. No source local is synthesized; the
existing result binding remains live through the read. A checked
`CallContinuationCleanup` entry immediately after the consumer names its exact
call coordinate and ordered affine discard rows. Lowering emits an ordinary
`Jump` with cleanup to the next evaluation block, forwarding every live scalar
binding. The temporary is neither retained until caller return nor represented
as another authored call. Each such source statement has exactly two captured
calls, with an ordinary or boundary producer. Earlier locals, later statements,
and successive producers in separate statements share the ordinary schedule;
each temporary dies before the next statement runs. Multiple arguments or
producers in one call, other consumer return carriers, mutable/write-only or projected result
operands, self consumers, linear result claims, and sum-payload inspection remain
separate work.
Anonymous boundary structural results use the same argument schedule as
ordinary affine producers. Static requirements, bodyless declarations, and
caller-owned nominal requirements retain their exact source target separately
from the resolved boundary declaration. Each temporary is established after
successful provider completion and either transfers exactly once to its enclosing
consumer or retains ownership through the bounded shared-read continuation above.
Provider refusal preserves completed operands for retry; a later
operand crash preserves earlier effects without a cleanup successor.

Scalar-returning boundary callers also use this evaluator for their existing
two-statement body: an immutable boundary-result initializer followed by that
local's return. Nested operand calls use shared scalar-helper identities; they
do not create synthetic Unit bodies or additional source statements. The root
retains its checked crash contract and whole-root claims while arguments execute.
Only successful boundary completion settles those claims and supplies the
returned scalar. Source validation rejoins the initializer, its captured call,
the declared result carrier, and the returned local independently of operand
graphs. The emitted root and scalar helpers retain their exact source owners.

Ordinary Unit closures also retain these boundary-return bodies as callees with
immutable scalar values and structural parameters. The body remains a boundary-return
plan, not a manufactured scalar graph. Its attachment, bodyless boundary,
service ceiling, nested scalar helpers, result, and crash contract join the same
module before identities and proofs are finalized. Repeated calls share the
same callee; identical boundary declarations from distinct checked owners
coalesce, while conflicting declarations reject. The caller keeps its own root
service reach. Nested operand calls may select further parameterized wrappers
in this shared Unit catalog. Scalar formals retain authored source positions
separately from their dense value ordinals; the returned local follows all scalar
formals. Parameter ranges and supported entry predicates use the existing checked
scalar contract and canonical conjunction lowering. Each ordinary call must prove
that conjunction against its evaluated actuals before entering the wrapper.
Missing predicates and range rows reject rather than disappear. Named-root
lowering also retains mixed scalar/structural signatures, including whole-root
claim settlement and scalar-only entry relations. Structural qualifications
remain in the structural signature, not scalar proof slots. Direct checked scalar
calls retain structural arguments and exact claim transfers, emitting the existing
`CallStructuralScalar` operation. Their source parameter identities, projection
paths, and transfer events rejoin the authored call; scalar result production
does not discard structural custody. Callee claims use machine-local dense IDs,
independent of the module-wide value and operation identity ranges.
Scalar-only computed edges still reject structural signatures, even when another
direct call has retained that same callee in the module. Previously established
affine structural results use their existing result owner and single-consumer
cleanup rules; this does not introduce linear result claims. Provider rejection
retains custody for retry, and an operand crash invokes neither the boundary nor
cleanup. Constructed local arguments reuse the existing leading empty-record
prefix or single-i64-field record establishment. The source declaration,
constructor value, and exact establishment/transfer events remain authoritative;
scalar result ordinals do not count construction locals. Only transferred locals
lose caller cleanup, and unused locals retain reverse declaration order.
The verifier shares the ordinary Unit local/result policy and still rejects
borrowed or projected locals, missing establishments, and duplicate disposal.
Nested whole affine result expressions use the same authored argument schedule
as ordinary Unit and structural calls. The wrapper's returned scalar extends
the source binding namespace only after successful completion; private staged
operands do not become locals or displace earlier bindings.
Wider construction carriers, structural-observation requirements, broader result
guarantees, and mixed structural-field crash predicates remain separate work.

Composed-control boundary leaves use the same evaluator, including the existing
three-state, prefixed, nested acyclic, dynamic-result continuation, and closed-sum
payload routes. The producer partitions exact outer flow calls from nested
operand occurrences; the outer boundary remains one operation. Private blocks
execute only within the selected leaf, carry completed scalar arguments once,
and preserve the leaf's original value namespace for subsequent boundary calls.
Whole-root linear settlement retains its claim until the boundary succeeds;
scalar operands do not change structural argument or receipt positions.
The selected scalar helper closure retains exact source targets, callee
requirements, and crash contracts, and its identities are disjoint from Unit and
dynamic-realization machines. Operation proofs are completed on the assembled
module. Closed-sum payload execution still requires interpreter case inspection;
operand support does not supply that missing runtime carrier.

Ordinary Unit-call leaves in the three-state, prefixed, and nested acyclic
control routes, dynamic-result continuations, and closed-sum payload continuations
use that evaluator too. A control-state Unit-call prefix completes before its
guard; the guard and successors use the retained source values after operand
evaluation. The callee's parameters, statements, transitive calls, providers,
and normal cleanup come
from the same complete Unit-body lowering as standalone Unit machines.
Free callees keep no attachment; a static data qualifier does not manufacture
a receiver.

Unit crash ceilings may combine Boolean and fixed-integer scalar parameters
with the supported structural member predicates. Scalar parameters use dense
primitive positions; structural predicate roots retain authored positions, which
lowering explicitly maps to the emitted structural places. The runtime signature
still uses dense structural positions. Free and hosted mixed-signature Unit
functions share the same parameter collector; free functions gain no attachment
or receiver. Ordinary calls instantiate the checked parameter-relative routes
with the exact completed scalar arguments and structural places, including equal
and reordered operands. Composed internal calls retain their scalar-only argument
restriction. The independent verifier
substitutes scalar values as well as structural places in the callee contract
and requires the exact invocation routes and caller-ceiling coverage. A true
guard permits a crash; it does not execute one. Explicit crashes in the invoked
body still preserve preceding effects and have no normal continuation.
Coverage recognizes a block parameter as a forwarded machine formal only when
every incoming scalar edge supplies that same formal with the same type.
Unknown or conflicting inputs, structural payloads, and cycles without an
established origin do not establish that equality. This affects only the
caller-ceiling comparison; the invocation still retains its actual argument IDs.
Crash-route comparisons normalize conjunction/disjunction order and duplicate
leaves after substitution, including when two formals receive one actual value.
This comparison normalization does not change the codec's canonical wire order.

Proof-gated mixed Unit crash arithmetic retains integer comparison requirements
over scalar formals, structural members, and literals. The closure allocates its
real scalar formal declarations before lowering contracts, and the machine bodies
reuse those declarations. Each ordinary call substitutes its completed scalar
arguments into the callee requirements before checking predicate totality, then
rebases structural roots to the actual places. Requirement slots keep the callee's
canonical order even if reordered or equal arguments change the substituted terms.
Each call slot becomes an obligation finalized after complete Unit or cleanup
assembly. The shared integer certificate producer consumes the independently
reconstructed pre-call requirements and facts; the callee's own guarantees cannot
prove its preconditions. The independent verifier reconstructs scalar and
structural substitution, including the exact ordering of equality operands.
Reversed equalities carry explicit symmetry certificates instead of silently
reordering the proof goal or its premise.
Pure immutable Exact arithmetic arguments retain their completed result IDs in
the call requirements. Complete supported integer entry-requirement packages
remain present even when the caller's crash ceiling is unconditional or absent.
Nominal cleanup retains ownership of its contextual caller and target contracts;
the shared Unit closure does not interpret those provisional namespaces as final
parameters.
Ordinary Unit requirements also retain Boolean parameters and exact structural
Boolean members, constants, negation, equality, conjunction, and disjunction.
Scalar and Unit contracts share bounded Boolean polarity conversion; each owner
supplies its own parameter/member namespace and integer-bound encoding. Unit
integer comparisons keep their original strict or inclusive spelling. Scalar
and structural actual substitution preserves requirement slots and child order;
reordered or shared arguments do not recanonicalize the reconstructed obligation.
These requirements use the same independently checked pre-call certificates.
Case analysis projects nested disjunctions from conjunctions with explicit
elimination proofs; it does not turn a conjunct into an uncited entry assumption.
This does not establish totality from integer bounds hidden inside logical
connectives or extend requirements to unsupported arithmetic operands.
Computed Boolean actuals retain their executed result IDs. Nonliteral Boolean
operation rows declare an equation plus positive and negative polarity
implications. Reconstruction derives each implication only from that operation's
typed denotation, using checked Boolean conversion with no caller hypotheses.
The original equation remains first; both implications follow before any later
call obligation is captured. The producer cites these implications explicitly,
proves their premises, and transports the result through exact equality proofs.
The separate private crash-path walk retains the original operation equations,
without adding these redundant auxiliary implications to its bounded path
copies. It still retains authored guarantees; ordinary operation and call proof
reconstruction independently checks the complete fact set.
Implication search is bounded to 4,096 steps and depth 64; a cycle supplies no
premise. Neither an authored call requirement nor a callee guarantee can supply
the missing operation meaning.
Source evaluation checks the selected operator meaning in the expression's
owning machine. Substituting a callee formal switches to caller ownership once;
the caller's argument does not inherit a callee specialization's operator
selection. Closed Boolean equality uses that checked meaning, not the spelling
of a selected user-defined comparison.
Bounds guards likewise recover the actual collection-length carrier and the
carrier of a supported builtin integer computation before excluding unrelated
operator candidates. Collection length requires an exact structural receiver;
a nominal field named `len` retains its declared type. Computed operands require
builtin inner-operation meaning and compatible known operand types. Anonymous
literals and unresolved operands remain independent wildcard candidates, not
copies of a sibling's type. Neither type recovery nor a visible comparison
declaration supplies a bounds proof or changes the selected operation.
Call preconditions cannot fall back to comparing an uninstantiated callee name
with a caller fact: identically named formals do not establish anything about
the actual argument.
Arithmetic formation obligations precede the call
obligation; the shared proof producer uses reconstructed operation equations and
caller facts rather than replacing the argument with its authored expression or
manufacturing an exact caller premise. Source mathematical substitution does not
erase the runtime computation or its independently verified overflow obligation.
Division nonzero and signed overflow bounds, and Exact shift count and value
bounds, remain required. This does not add mixed projected-argument partial cleanup.

The supported composed-control leaves accept either a semicolon call or a final
Unit call expression. This includes boundary leaves after dynamic scalar-result
dispatch and the final boundary call in a closed-sum payload leaf. Only the
selected leaf evaluates its operands and invokes its callee. A trailing expression
retains its exact state, statement coordinate, and captured outer call even for
pure or zero operands; it is not rewritten into a discard or another state's
call. Calls before transitions still require statement syntax. Result, signature,
structural-custody, and control-topology restrictions are unchanged.
Provider-field receivers rejoin the exact attached storage declaration through
the existing inherited-field mapping; missing or conflicting receiver stamps
cannot be recovered from their spelling during publication.

The composed root and its Unit callees select one type, boundary, service,
and helper catalog before assigning identities. Scalar helpers shared by root
operands and callee bodies have one machine identity. The root graph consumes the
shared identity counters and is inserted before proof finalization; independently
lowered modules are not concatenated or relocated. Complete Unit-body lowering
replaces the former parameterless-only composed target emitter.
The exact source-to-machine map survives dispatch, so callee suspension,
conformance, and float-source metadata are selected with their actual owners
rather than treating the composed root as the entire source closure.

Ordinary callers can also invoke composed Unit bodies.
Both checked body catalogs are pruned together to a complete call graph; a
missing transitive body removes upstream callers from either catalog. Lowering
borrows each body's actual entry signature and operations, allocates shared
machine/header identities, then emits each authored body once. Composed callees
and roots share emission and return normally to their callers; states are not
converted into synthetic machines. Nested ordinary and
composed calls share scalar helper identities and proof obligations. Whole-root
linear arguments retain their call transfer, state-entry claim aliases, and
selected boundary settlement. Calls from composed leaves still require
scalar-only targets without runtime entry requirements. Claim-free graphs with
scalar parameters and shared byte views use general state traversal
for both free and attached bodies, including branches, joins, and empty states.
Qualification, owned-frontier, implicit-receiver, and closed-sum cases retain their
specialized routes. Once the general route is selected, a failed source check
does not fall back to a shape matcher. Scalar declaration/assignment prefixes
reuse the shared local-storage namespace. Branch-free scalar successor operands
are evaluated only after their edge is selected, then passed simultaneously to
the target state. Selected-edge subslices retain their exact operation-result
descriptors through later states and repeated loop iterations, including
descriptor-selecting joins. The same route retains the supported authored
`Slice::Length` witness and its natural-ranked component evidence; see
[borrowed-byte writer composition](#borrowed-byte-writer-composition).

Closed-sum continuations retain the structural-result boundary and its exact
case/payload transfer independently of their ordinary callees. Calls within a
selected payload leaf preserve that leaf's scalar namespace for subsequent
operations. Root control blocks, values, operations, edges, and storage places
use the shared closure's identity counters. The root's provider requirements
cover its entry boundary and leaf boundaries together; callee requirements
remain owned by their complete Unit bodies.

Direct, rebound, stored, and forwarded dynamic-result continuations retain that
same ordinary call closure. Shared Unit and scalar identities are assigned
before dynamic realizations and forwarding helpers; already-emitted calls are
not renamed. The caller's blocks, values, places, edges, and descriptor
establishment operation use the shared counters. Dynamic root conformance rows
remain separate from selected callee applications, with exact source owners
retained for the whole published module. Only the selected leaf runs its operand
computations and ordinary body. An unused provider-backed field remains in the
root attachment without inventing a direct boundary requirement from a callee's
independent call. Producer, codec, and verifier each require exact equality
between the root's actual direct boundary calls and its attachment requirements;
neither transitive reach nor a retained field supplies a missing requirement.

Structural returned calls still need connections to the shared evaluator.
Structural arguments on composed internal calls,
mixed runtime requirement proofs for call-bearing arguments, other arithmetic
policies, and mutable snapshots,
and caller-ceiling proofs for computed-argument
routes remain implementation work.
Existing control-state signature restrictions remain; operand evaluation does
not itself add general scalar state-argument transport or computed dispatch guards.

Computed Boolean guards complete before either branch destination starts. A
private dispatch block consumes the completed Boolean and retains the source
value prefix for the selected branch. Guard roots rejoin the exact authored
`When` expression and Boolean carrier. A single-subject Boolean dispatch with
exactly opposite literal arms tests the subject once and treats its final arm
as fall-through, in either authored arm order. Independent anonymous guards are
not combined merely because their call expressions look alike. Longer computed
dispatches still need explicit shared-subject planning.
Existing lone-call comparison hoists still produce a direct-call binding and
pure guard; retained nested or multi-call guards use computation roots.
Publication also checks the authored fallback: a later independent `When`
cannot reuse an unconditional edge from an earlier checked dispatch.
Fall-through proof contexts retain failed earlier guards, including their
Boolean polarity. Inductive contract and ranking judgments use those facts
for the exact selected edge; range propagation still invalidates facts affected
by guard effects or intervening writes.

An authored standalone `crash Trap;` or `crash Abort;` may be the fallback
after a guarded scalar return or state transfer. The checked destination keeps
the exact statement coordinate; lowering rejoins its authored cause, live
terminal target, absence of a continuation, and checked crash-site evidence.
Only the selected fallback enters a private block with the existing no-successor
crash terminator and explicit frontier lower bound. A computed guard completes
first, so its own crash precedes the fallback and a successful ordinary arm
never executes the crash. This adds no inline crash-arm syntax or new Terminal
operation.

Crash-route coverage includes stable consequences of earlier failed guards in
the same source state. These predicates refer to exact immutable entry-parameter
symbols, not same-spelled locals or parameters of another state. Boolean
projection can retain `!flag` from a failed `flag || effect()` guard without
treating the call result as a parameter fact. Unrelated local writes cannot
invalidate the retained entry snapshots; mutable-storage route transport
requires its own value evidence.

Immutable and mutable initializers and local assignments in free scalar machines
use that same evaluation path. Each RHS completes before the following statement;
private continuation blocks carry prior values and the new result. A computed
assignment reads the old storage value throughout its RHS and updates the current
storage position only after that value completes, without changing earlier
immutable snapshots. Assignment roots rejoin the exact authored RHS and prior
mutable local destination, including its declared carrier. Initializer roots are
checked against the authored local's initializer, carrier, mutability, and exact
destination identity or immutable binding ordinal. Pure initializers and simple
immutable calls with pure arguments retain their existing flat binding path.
Exact-cast facts for assignment right-hand sides are collected against the
pre-write value environment, before invalidating the destination's old facts.

Owned mutable Boolean and fixed-integer parameters in free scalar machines
use the same current-storage mapping. Checked state graphs retain arena-backed
entry rows naming the authored parameter ordinal, symbol, and carrier. Lowering
matches each row to that exact state's mutable declaration before initializing
storage from the incoming operand. Immutable peers keep their authored ordinal
slots, but a body read of a mutable formal cannot use an immutable entry alias.
Each transition initializes its target state's storage from the delivered
values; no entry-machine or sibling-state binding is recovered by spelling.
Copies established before reassignment remain separate values. A postcondition
mentioning a mutable formal still describes its final value, not an implicit
snapshot of the caller's earlier argument. Declared requirements and published
crash conditions are separate entry contracts: their checked predicates retain
incoming parameter values, while executable guards read current storage.
An incoming edge predicate over mutable owned scalar storage is not evidence
about an invocation-entry crash route. Direct-site and call-route coverage
exclude that predicate until its entry-value origin is retained independently.
Checked coverage separately uses exact Boolean machine/entry-state requirements
as invocation-snapshot hypotheses. Named-state requirements do not become
ambient entry facts. Requirement names must agree both with their exact formal
symbols and with the canonical entry ordinal; body reassignment does not change
those hypotheses. All-crash scalar graphs retain their checked requirements
instead of dropping the contract because no normal return exists.
The strict Boolean-formal entry reader also accepts attached and mixed
signatures. It rejoins an attachment through one live nominal symbol and checks
its retained name; machine and owner generic/lifetime binders remain excluded.
The entry state and every parameter must retain their exact live symbol owner,
with no duplicate parameter symbols or names. Each observed leaf is one exact
non-const, non-receiver owned Boolean formal, not a field or borrowed Boolean.
Unread structural operands and receivers retain authored positions for crash
identities; checked scalar operands count only preceding primitive parameters.
Mutable formal reads in this contract namespace rebind to that scalar entry
position, independently of the execution support for the enclosing body.

Structural entry hypotheses use a separate strict entry-predicate reader, not the
closed-scalar result-contract fallback. Plain Boolean field paths rejoin each
selection through its retained live field symbol, exact nominal receiver, and
declared field type. The root retains its authored parameter ordinal; nested
selections cannot borrow a same-spelled field from another owner. Declared
requirements describe invocation entry for owned, shared-borrow, and
mutable-borrow roots. This does not admit current body reads as snapshots.
Authored parameter names must agree with the complete ordered entry namespace
before canonical crash identity encoding. Numeric operands retain their separate
totality owner rather than becoming Boolean-field evidence.
The explicit receiver rejoins the producer's exact machine-owned `Self` alias
to the attached nominal declaration and original field identity. Guarded Unit
crash contracts retain that receiver even when the executable body alone could
omit it; contextual cleanup requirements keep their receipt-bound environment.
Runtime field lookup shares the structural nominal resolver and preserves
explicit numbered field identities.

Fixed-integer entry comparisons over direct parameters and plain field paths
reuse the checked integer-contract owner for literal landing, same-carrier
operands, and selected operator meaning.
Their strict preflight checks the live expression and formal symbols, names,
builtin type identities, and every type chain used to count dense scalar slots.
Field operands retain authored structural root positions and exact field paths,
including numbered identities and the explicit receiver; each selected leaf
must be a fixed-integer carrier. Write-only roots provide no readable field
hypothesis. Shared comparison construction does not widen the separate scalar
contract reader's namespace. Mutable formals and fields still denote
invocation-entry operands. Comparisons are total
even on Wrapping, Saturating, or Trapping-qualified inputs; this admits no
arithmetic, casts, calls, or body values as new hypotheses.
The separate Boolean-only result-contract fallback remains unchanged.
The source regression command is `cargo nextest run -p checked-trees-to-lowered-psi
--test entry_requirement_crash_coverage --no-fail-fast`. Structural entry cases
serialize and independently verify their retained requirements and unchanged
callee continuations; they do not execute arbitrary host records as entry proofs.

Numeric requirement and crash encodings retain their existing distinct forms.
The verifier constructs an `IntegerOrderDiscreteness` certificate to connect an
adjacent inclusive entry bound to its strict crash predicate. Each scoped entry
alternative must prove a published alternative; no disjunct becomes an ambient
fact. The existing kernel checks the certificate and its exact premises under
the shared search and depth limits. No numeric normalization or proof rule is
added. Caller requirement proofs may also reuse an exact strict-order
certificate with `IntegerOrderWeakening` to discharge the original inclusive
goal, preserving all endpoint-equality citations.

Independent call-ceiling validation proves the union of a same-cause bucket's
published alternatives from caller requirements. It reuses the checked Boolean
predicate-denotation conversion and certificate search used for direct sites,
but supplies only entry requirements, never CFG facts or current body values.
One union goal shares the 4,096-step conversion and search limits across the
bucket; proof depth is bounded to 64. Exact callee continuations remain unchanged.
This also permits a disjunctive entry requirement to cover separate published
alternatives without claiming either alternative holds by itself.
Common consequences of nested alternatives use kernel-checked disjunction
elimination: each ordered branch proves the same goal with only its exact
alternative added as a local assumption. The assumption is discharged before
the next branch. Conjunction projections and case branches share the existing
search budget and depth limit; semantic axioms never occupy requirement slots.

Checked source coverage recognizes equivalent negation and Boolean-literal
equality wrappers only after verifying builtin meaning and the exact entry
formal/symbol and canonical ordinal on both requirements and authored routes.
It retains the original published predicate identity as a proved consequence.
Strict entry-only consequence extraction unions conjunction facts and intersects
facts from every disjunct, preserving polarity through negation and literal
Boolean equality. Published compound routes must be established completely from
those retained facts. The shared runtime-guard reader is unchanged; current body
storage and selected authored operators cannot supply entry hypotheses.
Common-consequence intersection and route assembly share 4,096 work units and
depth 64; exhausted work is not retried with a fresh budget for another route.
Negated scalar conjunctions and disjunctions lower through logical De Morgan
propositions, including Boolean-literal equality wrappers, not eager scalar
operations. Scalar and structural crash predicates also admit equality between
compound Boolean predicates: equality combines the two equal-polarity branches,
and inequality combines the two opposite-polarity branches. Contract and crash
lowering share that logical constructor while retaining their separate atomic
denotations and namespaces. The whole crash predicate has one 4,096-unit logical
input/expansion budget and depth 64, including all branches and top-level
conjuncts. Unnormalized scalar constant connective children still reject; atomic
Boolean and numeric crash encodings remain unchanged. Structural composition
retains its existing constant-child handling and atom-specific field, IEEE,
byte-sequence, sum-equality, and case-membership readers. Equality can combine
those proposition-only leaves without inventing scalar denotations. Existing
negation of a proposition-only atom remains implication to falsehood, not a
complemented numeric comparison. Every expanded operand retains the original
runtime requirements and structural root/path checks. This logical budget does
not replace arithmetic-subtree, structural-path, or sum-case checks.

Boolean implication beyond structural common consequences, exact entry crash
hypotheses beyond Boolean/fixed-integer direct parameters and plain field paths,
and arithmetic-expression or float entry coverage remain
implementation work. Case-qualified payload paths need their case identity in
the canonical crash predicate before they can supply exact entry hypotheses.

Direct crash-site validation independently proves every asserted guard from
invocation-entry requirements and facts reconstructed before that terminator.
The private reconstruction retains raw branch-value polarity and bounded path
alternatives through CFG joins, including later selection on computed Boolean
block parameters. Every path must establish the guard or a kernel-checked
contradiction; site guards and producer evidence never become premises.
Each machine's traversal has independent limits of 4,096 block visits and
4,096 generated or copied facts; exhausting either rejects without dropping
unvisited paths.
Bounded certificate search uses exact assumptions, semantic axioms,
conjunctions, disjunction introduction and elimination, and equality rules.
The proof owner checks both the certificate and fixed Boolean/integer
predicate-denotation conversions; these private conversions add no serialized
proof rule or obligation identity.
Crash-only raw branch facts stay out of ordinary obligation reconstruction.

Ranked-site proofs use only entry requirements until all-path invariant custody
is available. Unversioned structural observations need exact shared-borrow
parameter roots to supply entry facts; current reads on owned or mutable roots
cannot impersonate their entry snapshots. Their raw scalar read values still
support executable branch predicates. Broader mutable-origin transport remains
implementation work, not a language-design decision.

Resolver operand preprocessing cannot move indexed reads or cast-wrapped calls
out of guarded transition targets or selective Boolean right operands. These
operands remain behind their original selection boundary for checked lowering.
Ordinary returns, initializers, and assignment right-hand sides also retain
cast-wrapped free calls in operand order: a later cast cannot hoist its call ahead
of an earlier operand. Assignment target/indexed-read normalization is unchanged;
computed projected writes still need their own destination plans.

Computed call arguments bind the pinned callee's parameter-relative crash
routes to the actual argument values, using the same route substitution as
ordinary staged calls. The checker row still supplies exact target-contract
identity; Terminal verification independently reconstructs route coverage.
Declared crash ceilings may remain even when every crashing branch is skipped.
Direct scalar calls, including literal arguments, bind those same pinned callee
routes to the emitted argument values. Source-side simplification of a caller's
crash conditions is proof information; it does not replace the callee's published
route interface in a call operation.

Integer applications share operation construction with pure checked expressions.
Each operand retains its own arithmetic policy; a resolved callee's declared
result type supplies its carrier and policy, not the enclosing destination.
Comparison normalization may reverse completed values for `>` or `>=` but never
reverses their evaluation. Widening and occurrence-proved exact casts use the
same pure templates after evaluating their operand once. Cast range obligations
must still be reconstructed by independent Terminal verification.

Fixed-integer result/literal postconditions, including conjunctions and
disjunctions, retain predicates over the actual Terminal machine result.
Normal-return bounds can discharge nested exact casts. Source validation joins
the reserved result occurrence to its exact contract owner and admits only
builtin numeric comparisons; a parameter named `result` is not the returned
value. Strict integer literal bounds normalize to equivalent inclusive bounds
only when the adjacent endpoint is representable. Publication reconstructs both
callee postcondition and caller cast obligations, with certificates over the
emitted return equations and instantiated call guarantees. A literal tautology
does not stand in for a result guarantee, and a changed return or weakened
guarantee cannot reuse its serialized evidence.

Integer contract predicates also name exact immutable entry parameters. Their
checked positions follow the complete scalar parameter order, with the result
slot present only in postconditions. Explicit requirements and enforced literal
parameter ranges become one canonical conjunction; calls substitute the actual
evaluated argument values and prove that requirement before assuming a result
guarantee. Equality operand order comes from the codec, with explicit symmetry
evidence when proof traversal needs the other direction. Computed argument bounds
cross a returned-value equality or order relation through checked substitution
or transitivity, including case analysis over disjoined guarantees. Named-state
forwarding retains exact immutable entry origins and rejects ambiguous joins,
including backedges to the entry state spelled through its state or machine name.

Free scalar requirements also retain Boolean entry predicates, including
negation, equality, conjunction, and disjunction. Mutable formals use their
incoming values, not later body storage. The scalar fallback requires exact
entry-parameter symbols and builtin operators; spelling cannot recover a
missing or foreign operand identity. Boolean polarity lowers into logical
propositions over the exact entry operands; literal wrappers and negation do
not require evaluating a separate Boolean expression inside call proof search.
The existing literal contract encoding and mutable-formal postcondition fence
remain unchanged. Source calls prove these requirements before executing the
callee; this does not add runtime requirement checks to host-supplied interpreter
entry arguments. Nested body-local and mutable-storage operands cannot alias
entry namespace positions in a retained contract predicate.
Compound equality expansion has a 4,096-step per-clause lowering budget;
exhaustion rejects before allocating an unbounded proposition tree.

Remaining numeric policies and selected operator calls, longer computed dispatches,
borrowed/projected operands and writes, and named runtime proof outputs still
need execution-plan extensions. Nonliteral contract arithmetic and result bounds
that need caller-specific snapshots beyond immutable scalar formal comparisons
still need complete transport. Source interval projection uses only immutable
formal declarations and builtin required bounds, never a reread of caller
storage. A call's carrier alone never proves a partial conversion.
The older flat guarded-argument and shared match-subject normalization remains
on uncovered paths and must be retired as those paths move to checked computation
planning; this work remains in `STATE-LOCAL-VALUE-FRONTIER`.

Structural-operand Unit composition is a distinct checked operation, not a
widened scalar `Call`. One free or hosted Unit body may bind a primitive result
from one selected operator whose structural operands are an exact permutation
of its whole, claim-free owned affine parameters and whose remaining operands
are checked scalar expressions. Checked planning rejoins the authored use,
strong provider plan, exact specialized realization, contract, result, and
empty service reach. Terminal production emits that realization as a separate
machine and the use as `CallStructuralScalar`; scalar values and source-ordered
structural roots remain separate argument rows, while consumed roots are absent
from the Unit return discard list. Production rechecks the same custody and
rejects missing, mistyped, unavailable, duplicate, reordered, projected,
substituted, or borrowed operands. The Unit and realization share one exact
structural-type catalog. The hosted fixed-width integer/empty affine-record
subset continues through native, object, image, and installation custody on
Linux x86-64 and AArch64. Structural results, claims, content evidence,
services, projections, borrows, nontrivial layouts, and wider control flow
remain later slices.

A nonempty path to a relevant Boolean field of a record parameter retains every
canonical structural-field identity and rebases across structural Unit calls.
For a field- or literal-fixed-index-projected structural argument, the caller's
canonical argument path is prepended to the callee's parameter-relative Boolean
path. Canonical predicate segments distinguish verifier-owned field identities
from exact array indices. The verifier independently traverses both declared
record and fixed-array paths, requires in-bounds indices, structural
intermediates, and a Boolean leaf, and rejects absent, erased, truncated,
mistyped, out-of-bounds, or redirected paths. Built-in Boolean equality,
inequality, negation, and conjunction may compose multiple relevant member
paths and literals; every nested path is independently traversed and rebased.
Equality, inequality, and ordered comparisons also accept same-typed relevant
fixed-integer member paths; terminal terms retain both the canonical path and
the exact integer type, and the verifier checks that annotation against the
declared leaf. Built-in fixed-integer `&`, `|`, `^`, and `~` compose the same
typed member terms without an arithmetic proof obligation; overloaded forms and
the distinct address carrier remain outside this bounded structural slice.
Checked production applies that address fence both to direct member predicates
and to whole-record leaf expansion; the source contract may remain in checked
identity, but Terminal lowering receives no portable scalar term and rejects it.
Whole-root and all-field-projected structural calls reconstruct those predicates
across the callee boundary by prepending the caller's canonical argument path to
every callee-relative integer-member path, including operands nested beneath
bitwise terms. The verifier repeats that substitution independently and rejects
a redirected continuation even when the redirected path reaches another valid
same-typed leaf. A built-in Boolean
disjunction retains two distinct canonically ordered proposition branches; each
branch may contain the same accepted Boolean or integer-member predicate forms.
Production and independent verification recursively rebase every branch across
whole-root and all-field-projected calls. Both codecs retain the proposition;
the semantic codec rejects nested, duplicate, or noncanonically ordered
disjunction rows. Whole-record equality does not add an opaque aggregate term:
for two same-typed `Equatable` parameters, checked production retains the
language-defined inline field expansion. A finite nonempty tree containing only
relevant Boolean, fixed-integer, IEEE `f32`/`f64`, and supported byte-sequence
leaves becomes one flat canonical conjunction. Float leaves use an atomic
format-annotated IEEE `==`
proposition rather than mathematical `Equal`, preserving NaN non-reflexivity and
signed-zero equality. Direct float-field `!=` uses the same atomic proposition
with an explicit comparison kind, preserving the complementary IEEE result
without a second verifier family. Whole-record float `!=` reuses the canonical
equality conjunction as the premise of `P -> Falsehood`, so aggregate float
negation adds neither a duplicate leaf family nor De Morgan permutations. Each
leaf keeps its left and right parameter root;
call verification independently substitutes both roots and rejects redirecting
either operand even when the replacement path is otherwise valid and
same-typed; float leaves additionally require the exact declared format.
Byte-sequence leaves use a separate atomic content-equality proposition over
two nonempty structural paths. Terminal structural identity distinguishes a
borrowed view from bounded owned storage and retains the bounded carrier's exact
capacity, but does not expose a native pointer/length descriptor. Equality is
defined only by equal live lengths and equal live byte prefixes: pointer
identity, capacity, and bytes beyond the live length are irrelevant. The
verifier independently requires both resolved leaves to have byte-sequence
carrier types, and call substitution rebases both roots. The bounded slice
admits field-to-field whole-record equality for `&[u8] in Domain` and
`[u8; N] in Domain`; text literals and direct text `!=` remain fenced. The
current semantic codec, proof-bundle codec, and installation record encode this
vocabulary. A genuinely zero-member record instead normalizes equality to the
existing Boolean `true` term; inequality uses the existing negation, and calls,
codecs, verification, fixed fuel, and interpretation reuse that carrier. An
all-erased record is not empty and remains fenced. Payload-less sums retain
their closed case roster as exact Terminal structural case identities. Equality
is the canonical flat conjunction of both case-membership implications for
each case; inequality is that complete equality proposition implying
falsehood. The verifier resolves each subject and case independently.
Payload-bearing pure sums additionally retain each exact case-payload field.
For direct relevant Boolean, fixed-integer, IEEE, and byte-sequence payload
leaves, equality is a canonical disjunction whose arm for each case conjoins
membership of both roots in that case with the exact payload-leaf equalities;
inequality is that complete disjunction implying falsehood. A case path uses an
exact case identity followed by its exact payload-field identity, and the
verifier and codecs reject unknown or redirected identities. One relevant
acyclic record or pure-sum tree directly held by a case-payload field also
expands its supported leaves transitively. Those paths retain every exact
alternating case, payload field, enclosing record field, and leaf identity in
order through nested sums, and whole-root calls independently rebase both
operands. Direct whole-root mixed shapes retain both common fields and a closed
case roster. Their equality is one canonical conjunction: supported
common-field leaf equalities in declaration order followed by one
source-ordered disjunction whose arms contain matching membership for both
roots and the selected case's supported payload-leaf equalities. Inequality is
that complete equality proposition implying falsehood. Whole-root Unit calls
independently rebase both operands, while codec format 33 / vocabulary 35,
verifier, fixed-fuel, interpreter, and installation format 40 preserve and
replay the exact common-field, case, and payload-field identities. One bounded
nested form is also included: a whole-root acyclic record may contain exactly
one relevant direct field whose type is the existing mixed shape. Every mixed
common-field, case-membership, and payload-leaf path retains that enclosing
field as its first segment, and whole-root Unit-call rebasing preserves it on
both operands. No new proposition or format is added; independent structural-
path replay rejects a substituted enclosing field. The next bounded form
admits exactly two enclosing relevant record fields. Every path carries both
field identities before the sole mixed
occurrence; equality, inequality, whole-root Unit-call rebasing, codecs,
verification, fixed fuel, and interpretation replay that exact ordered chain,
and mutation of either field rejects independently. The following bounded form
admits exactly three enclosing relevant record fields. Equality, inequality,
whole-root Unit-call rebasing, codecs, verification, fixed fuel, and
interpretation retain all three exact field identities before the sole mixed
occurrence, and mutation of any prefix rejects independently. A fourth bounded
form admits exactly four enclosing relevant record fields and replays the same
complete ordered path through equality, inequality, whole-root call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A fifth bounded form admits exactly five enclosing relevant record
fields and replays the same complete ordered path. A sixth bounded form admits
exactly six enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A seventh bounded form admits exactly seven enclosing relevant record
fields and replays the same complete ordered path through whole-root equality,
inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. An eighth bounded form admits
exactly eight enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A ninth bounded form admits exactly nine enclosing relevant record
fields and replays the same complete ordered path through whole-root equality,
inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A tenth bounded form admits
exactly ten enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. An eleventh bounded form admits exactly eleven enclosing relevant
record fields and replays the same complete ordered path through whole-root
equality, inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A twelfth bounded form admits
exactly twelve enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A thirteenth bounded form admits exactly thirteen enclosing relevant
record fields and replays the same complete ordered path through whole-root
equality, inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A fourteenth bounded form
admits exactly fourteen enclosing relevant record fields and replays the same
complete ordered path through whole-root equality, inequality, Unit-call
rebasing, codecs, verification, fixed fuel, interpretation, and independent
prefix mutation. Fifteen or more enclosing fields, mixed values below case
payloads or another mixed shape, two mixed sibling fields, direct projected
mixed comparisons, recursive cycles, address and erased payload equality,
written `equals` bodies, and runtime sum layout remain outside this bounded
terminal slice. When an acyclic relevant record field reaches a payload-bearing
sum, the same sum proposition is retained below that field path, and independent
verification preserves the complete `Field -> Case -> Field` identity chain.
Direct source-call rebasing
through a sum-bearing projection remains fenced with runtime sum projection and
cleanup.
Arithmetic over
same-typed relevant fixed-integer members accepts Exact addition, subtraction,
and multiplication: each member or fixed-integer-literal operand retains its
exact checked carrier, nested operations remain typed `ExactIntegerAdd`,
`ExactIntegerSubtract`, or `ExactIntegerMultiply` terms, and whole-root or
all-field-projected calls rebase every member leaf recursively. The verifier
independently repeats that substitution and validates every declared leaf and
arithmetic-node type; both codecs preserve the nested term. Policy-selected
fixed-integer members also accept the total Wrapping and Saturating forms of
addition, subtraction, and multiplication. The terminal term retains the exact
selected behavior, and projected calls, codecs, verification, fixed fuel, and
interpretation preserve it without an overflow obligation. Wrapping left and
right shifts are likewise retained as total structural terms: the value's
carrier and the independently typed integer count remain distinct, and the
language-defined Euclidean count reduction survives projected calls, codecs,
verification, fixed fuel, and interpretation without a count obligation. Exact
right shifts accept a self-proving in-range literal count or a complete retained
package proving a runtime count nonnegative and below the shifted carrier width.
Exact left shifts require the same count evidence plus carrier-tight value bounds
at the greatest possible count; a zero count or a compile-known value that shifts
safely is self-proving. The producer canonically orders the complete requirement
package, and projected calls rebase one exact obligation per requirement.
Independent verification reconstructs the count and overflow checks and rejects
missing or weakened evidence. Direct Trapping arithmetic remains forbidden in
predicate terms. An explicit fixed-integer or address `embed` instead lowers to
an unbounded proof-`Int` term carrying the source carrier identity and exact
derived range; an explicit same-carrier `as` lowers to Exact arithmetic and
retains its discharged representability obligations. Wrapping and Saturating
predicate nodes retain their distinct total denotations. Terminal Psi never
creates a proof-side Trapping node or a predicate-generated crash effect.
Executable Trapping operations separately retain their compiler-owned
primitive trap predicate and path-conditioned crash site. Verification checks
that denotation against the primitive catalog and proves the derived guard is
covered by the authored same-cause route disjunction.
Exact division and remainder accept a
same-carrier literal divisor only when it is nonzero and cannot trigger signed
`MIN / -1` overflow. Wrapping and Saturating division and remainder accept any
same-carrier nonzero literal, including signed `-1`: their selected policy
defines the `MIN / -1` result, while division by zero remains illegal.
A whole-root structural Unit closure may instead name a runtime integer-member
divisor. For Exact operations, each machine's complete bounded `requires`
package must prove one of the verifier-owned totality shapes: `1 <= divisor`,
`divisor <= -2`, or the joint signed bounds `divisor <= -1` and
`MIN + 1 <= dividend`. For Wrapping or Saturating operations the corresponding
package need only prove the divisor nonzero through `1 <= divisor`,
`divisor <= -2`, or `divisor <= -1`. Checked plans retain those packages without
source handles and terminal Psi publishes the exact requirements. Every direct
or all-field-projected structural call carries one exact obligation per callee
requirement; the producer rebases the target place through the caller's
canonical field prefix, cites the matching caller assumption, and emits a
replaceable certificate. Independent verification reconstructs that prefix,
repeats the rebasing, and checks the assumption index before codec or
interpretation. Removing evidence or weakening or redirecting a bound rejects.
Case-payload paths and imported crash capsules remain fail-closed.
Structural/content contracts reject because custody effects require their own
vertical slice rather than an ordinary scalar flag.

The interpreter uses owned call frames and charges the call before entering the
callee. Sponsor exhaustion in the callee resumes without replaying that paid
call. A callee crash escapes as the original no-successor crash site and uses
that callee edge's fuel charge; call composition records the surviving route
without fabricating or double-charging another executable crash. Validation
rejects recursive call graphs until terminal Psi can carry and verify the
required tail-position and ranking evidence. Fixed-fuel derivation includes
separate acyclic callee return/crash bounds: caller tails compose only with
normal returns, while callee crash paths terminate at their own edge. It retains
its own cycle rejection as defense in depth.

Omega selects each callee's native calling plan and evaluates arguments into
disjoint frame spills before filling their ABI homes. Assignment retains
explicit register or outgoing-stack destinations. Emission materializes the
complete outgoing area, including Microsoft x64 shadow space, preserves x86
call alignment and the AArch64 link register, and emits a typed internal-call
relocation tied to the exact Psi operation and callee. Conditional-control
emission preserves live entry registers across condition calls and rebases
relocations from independently encoded conditions and arms into final function
order.

An unconditional crash continuation requires no caller-side machine-code
branch: the verified internal call reaches the emitted callee crash leaf and
cannot return along that execution. Omega still resolves the typed call
relocation and preserves the callee leaf; it does not reinterpret a crash as a
scalar result.
