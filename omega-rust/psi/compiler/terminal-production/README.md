# Terminal production

Public contract: [Terminal Psi product](../../../../wiki/spec/terminal-psi/product.md).
This crate sequences production; it does not own another executable IR.

Enter [production.rs](src/production.rs). The sequence is
`checked-trees-to-lowered-psi` -> `lowered-psi-to-lowered-psi` ->
`lowered-psi-to-terminal-psi`. `LoweredPsi` belongs to representations.
Publication consumes the validated optimization-stage result, not an unoptimized
producer-private shortcut.

The product keeps checked boundary-operator application scope and selected
floating-point occurrences beside the canonical artifact. Callback custody is
an opaque owned sidecar: carrying it does not interpret placement or grant
registration, invocation, address, or lifetime authority. Native realization
must rejoin the exact source/build receipts under its own authority.

## Boundary source custody at the compiler handoff

Compiler Terminal handoffs restore boundary requirement calls through
`CheckedCompilation::terminal_production_trees`. The existing inverse-edit guards
validate selected adapter identities and operand graphs before restoration;
checked plans remain unchanged. Adapter edits are retained separately from
operator/FMA settlement because those transformations carry their own checked
plans. Full package source queries undo both batches in reverse settlement order.
This is a transitional handoff for the selected-dispatch tree, not permission to
serialize a selected provider as a source call or to weaken call-source custody.

## Source byte-view lowering

The [byte-view contract](../../../../wiki/spec/terminal-psi/byte_views.md)
is independent of the producer's indexing scheme. The source lowering keeps:

- Whole-parameter indexed reads as an empty projection path, distinct from a
  nominal field's nonempty path.
- Call-range endpoint facts keyed by state, statement, call ordinal, and dense
  structural argument ordinal.
- State-edge endpoint facts keyed by state, transition statement, and authored
  target argument position, not a dense structural ordinal.
- Length observations scoped to the selected path, restoring the incoming set
  before lowering a sibling.

The emitter must rejoin those coordinates to the exact expression and builtin
range before producing obligations. Helpers and trust-source identities must
follow that reconstruction; an observed length is not a checked bound.

## Service receipts

An erased `Service<R> in Bound` parameter is not an ordinary empty record.
Its source receipt binds typed parameter symbol and authored position, normalized
carrier and qualification, exact requirement, and selected-plan digest. The
raw producer checks typed custody; final compiler admission additionally rejoins
selection provenance and an ordered call/checked-operation bijection.

Attached and free helpers keep their actual attachment shape. Free helpers do
not invent an empty receiver. Whole-Service forwarding must rejoin both endpoint
receipts rather than infer authority from zero payload. Source support for
particular forwarding/control shapes is an implementation limit, not a new
meaning for Service or an alternate Terminal namespace.

## Borrowed-byte writer composition

The std writer's private helper calls its concrete provider's byte requirement
inside that selected closure. Do not inject another Service receiver into the
adapter or its state edges. General clients still use the ordinary bound Service
carrier; a private concrete call is not fabricated routed authority or permission
for arbitrary native calls.

The shared Unit catalog retains free/attached state graphs with immutable byte
views, scalar parameters, selected-edge subslices, and ordered calls. Preserve
source states, guards, successor operands, cleanup, and the authored
`Slice::Length` ranking rather than a writer-specific countdown. Unsupported
ranking witnesses reject instead of silently becoming unranked.

Rank evidence uses the tail descriptor's exact measured endpoint difference and
the positive-start subtraction-order proof. Zero start does not prove descent;
positive start does not waive bounds. Reuse a current path's exact length
observation so a fresh read cannot displace the guard-bound value. A sibling
observation is unavailable. See the
[byte-view contract](../../../../wiki/spec/terminal-psi/byte_views.md).

Scalar successor operands evaluate at their authored transition positions after
edge selection. Simultaneous scalar and structural block bindings preserve
their namespaces. A contiguous pre-call prefix can establish immutable scalars,
initialize mutable scalar storage, and assign checked branch-free expressions;
rejoin each expression to its statement/destination instead of treating mutable
storage as an immutable parameter. Interleaved writes after calls and initializers
requiring calls or short-circuit control need further producer support.

Reentered source entries use ordinary block parameters and a one-shot invocation
block; later arrivals do not reuse original invocation inputs. Descriptor
rebinding replaces only its own unrestricted view, preserving other aliases.
Fuel suspension resumes without duplicating output effects.

The end-to-end writer control must cover empty/nonempty raw bytes, both newline
settings, exact output order, caller continuation, and suspension. Unguarded
head reads and unchanged tails reject. A source `requires bytes.len > 0` still
needs contract-level length observation and exact caller substitution; do not
inject body SSA values into parameter-only contracts or replace the view extent
with an unrelated length parameter. Native byte-view realization, natural-ranked
fixed bounds, general structural helper calls, and source-to-Psi correspondence
remain separate work; a Console-specific intrinsic would bypass this adapter.

## Multi-state control

The [control-flow contract](../../../../wiki/spec/terminal-psi/control_flow.md)
is broader than any one checked source family. The
[composed Unit producer](../../pipeline/typed-trees-to-checked-trees/src/flow/terminal_unit/composed_control.rs)
retains per-state operations, call coordinates, contracts, and service reach.
Admission and emission rejoin the original checked calls and publish complete
boundary/service/attachment catalogs; an unavailable child rejects the machine.
Provider-backed fields need exact attachment roots and a canonical requirement
set, not an invented runtime receiver. Closed anonymous-integer comparisons
become mathematical Boolean constants before downstream lowering.

Its first two-leaf boundary branch is not the general control limit: prefixed,
nested, dynamic, and state-graph families have separate admission. Claim-free
structural cleanup control remains distinct from effectful control; composing
them requires retained operations, successor bindings, and cleanup frontiers.
General custody joins, computed structural transfers, and mixed cleanup must
not inherit admission from a scalar/byte-view cyclic family.

Natural-cycle production retains the authored witness in
[ranking.rs](../../pipeline/checked-trees-to-lowered-psi/src/attached_unit/composed_control/state_graph/ranking.rs).
[control_cycle_proofs.rs](../../pipeline/checked-trees-to-lowered-psi/src/control_cycle_proofs.rs)
answers verifier-reconstructed questions. Proof-only recursive calls use their
own complete reachable closure; neither producer supplies semantic obligation
identities or substitutes a synthetic countdown for a view's extent.

## FloatMeaning correspondence

The [proof-value contract](../../../../wiki/spec/terminal-psi/mathematical_values.md)
defines Terminal's source classes. Source emission has a narrower implemented
frontier: direct top-level scalar parameters, the owning scalar result, and one
nonempty field/case path under a direct structural parameter have checked
artifact-relative correspondence. Fixed indexes, nested-state contracts, locals,
computed structural sources, and expression-to-Terminal operation/call mapping
still need producer work.

Direct-result reflexivity checks the authored expression, checked
equality/projection, owner, format, and sealed operation before erasing the
proof-only clause. It does not establish general callee or nested-result support.
Missing correspondence is an integration gap, not permission to invent a source
identity or a new meaning for the proof-value carrier.

## Structural access and stores

The [access contract](../../../../wiki/spec/terminal-psi/structural_access.md)
owns reference identity and non-observing writes. Current source support is
bounded: claim-free unrestricted field-path subloans, literal-indexed material
record receivers, and plain-record or whole-primitive scalar replacement.
Mutable-to-write-only attenuation preserves root access and independently
records the callee's weaker access. Shared projections preserve unrestricted
multiplicity and cannot originate from write-only roots.

Projection replay rejoins every field/index, array bound, type, and source
application. Dynamic/range projections, retained receiver aliases, deeper shared
array paths, and general reference-bearing or constrained data need separate
producer support. A stored pointer cannot be read merely to locate a write-only
receiver. Dynamic write summaries remain conservatively collection-wide.

Whole-root stores accept exactly typed literals and bounded fixed-integer/Boolean
scalar sources. Ordinary or selected fixed-integer call results retain their
checked call identity and durable result, including provider-plan correspondence
for selected calls. Plain-record stores share path/type reconstruction with
dynamic realizations; attached Unit bodies admit bounded single-store forms,
including one literal record-array index. General arithmetic locals, delayed
results, runtime IEEE sources, multi-write bodies, and richer indexed/aggregate
forms must not inherit admission from these cases.

Keep semantic scalar and structural ordinals distinct while preserving authored
argument order. Receiver `Self` resolves through the attachment and uses the
receiver write-frame root. Entry-bridge storage is a separate native obligation.
Artifact interpretation of broader store sequences is not evidence that source
production or native realization supports them.

## Partial ownership and cleanup

The [ownership contract](../../../../wiki/spec/terminal-psi/ownership.md) owns
claim maps, residual complements, and static cleanup order. Source production
admits bounded plain record/literal-array partial moves through ordinary Unit
disposers. Parameter, immutable-local, and anonymous result roots keep their
actual source establishment; do not synthesize a local for an anonymous producer
or reinterpret its result ordinal as a parameter.

Whole structural-result claim maps have broader codec/verifier support than the
source producer's one-claim slice. Literal indexed linear transfers and
claim-free affine residual cleanup have separate admission rules. General
projected contracts/content partitions, mixed dying roots, reference fields,
cases, dynamic indexes, nominal destruction of partial roots, and arbitrary
construction-local partial moves do not follow from either case.

One final projected temporary uses return cleanup. Continuing Unit bodies retain
their exact `CallContinuationCleanup` and residual Jump, including an empty
complement. The bounded form has one ordinary/boundary producer and one owned
argument per consumer, with exact empty effect-free Unit disposers. Mixed
argument effects/producers and non-Unit consumers need further support.

The current construction-prefix parser in
[calls.rs](../../pipeline/typed-trees-to-checked-trees/src/flow/terminal_unit/calls.rs)
admits empty, unqualified, claim-free affine elements and fixed-array lengths
through 26, with literal establishment of the all-but-last prefix. The general
reverse-index cleanup rule is not limited to that bound. Wider or dynamic plans
remain implementation work, not a sequence of new language cases.

Nominal cleanup is bounded to root-only Unit machines with eligible affine
records, empty drops or receiver-independent helper-call bodies, and supported
Boolean-field prerequisites. Rejoin target clauses to each owned place; a caller
fact at another position is not evidence. Conditional whole-parameter cleanup
has its own bounded structural-control producer. It is not admission of nominal,
projected, claim-bearing, or mixed-root cleanup on arbitrary control-flow edges.

## Scalar returns with cleanup

Source production retains the authored scalar/structural parameter partition:
separate dense namespaces, disjoint and complete authored-position maps, and
source-ordered immutable primitive bindings. Every initializer and return rejoins
its checked expression and statement coordinate. Structural custody is never a
scalar parameter; cleanup refers to actual structural places.

Short-circuit locals and returns become explicit decision blocks. Some bounded
families distribute later work into leaves; shared-convergence families instead
bind a typed Boolean parameter and use one cleanup tail. Both preserve selective
evaluation, dominance, and the complete cleanup stream on each normal return.
Neither construction implies general effectful locals, arbitrary multi-state
custody, or mixed projected/nominal cleanup support.

The nominal subset composes no-code roots and eligible cleanup targets in reverse
authored order, retaining contextual target premises against each exact root.
Scalar values and the return link must survive executable cleanup. The bounded
shared tail accepts supported parameter/constant Boolean trees, an optional exact
direct Boolean field, and classified integer-comparison leaves. Its field case
requires a remaining Boolean parameter; nested/second fields, field-only trees,
and arbitrary call/effect mixtures need further support.

The proof producer enters through
[nonzero_divisor_certificate.rs](../../pipeline/checked-trees-to-lowered-psi/src/nonzero_divisor_certificate.rs),
despite that file's narrower historical name. It consumes machine requirements
and independently reconstructed pre-operation facts, then emits kernel-checked
certificates for canonical integer goals. The operation's own later result
equation is unavailable. Source sufficient-form classification cannot substitute
for this certificate or the receiving artifact verifier.

## Structural results and suspension

The [call/outcome contract](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
owns result identity, argument order, cleanup, and park/resume semantics.
Source production still admits structural results through bounded families:
whole linear passthrough and claim-free owned-affine producers, their ordinary
call closure, and explicit temporary-result transfer/cleanup schedules.

Use one stored-owned type classifier before referent-oriented normalization.
Reference/slice fields, erased carriers, projected linear obligations, and field
qualifications cannot acquire a plain-owned route by losing their storage role.
Whole-result support does not imply general projected, borrowed, qualified, or
linear result support. Native eligibility is separately checked by lowering;
historical one-fragment publication routes are not architectural alternatives.

One statement/argument sequencer preserves authored positions across scalar
locals, structural initializers, nested producers, and calls. Scalar and
structural binding ordinals remain separate from statement coordinates and
private argument slots. Source correspondence must reject replacement by a
different same-typed live result even if Terminal ownership alone permits it.

The first suspension retention path covers receiver-free direct scalar calls
with checked primitive liveness and empty claim rosters. Receiver/threaded-local,
persistent, structural, claim-bearing, Unit, boundary, and dynamic frontiers
need further producer support. Preserve checked places and claims when adding
them; do not infer liveness from Terminal block shape.

Current site/plan validation catches an individually missing or changed plan,
but paired rows cannot detect coordinated deletion of both. Bind an independently
established possibly-suspending crossing roster into the call-side contract before
claiming arbitrary rewritten modules are demand-complete. This remains part of
the task/activation work on the [execution board](../../../../TASKS.md).
