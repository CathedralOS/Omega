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
