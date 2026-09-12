# Terminal production

Public contract: [Terminal Psi product](../../../../wiki/spec/terminal-psi/product.md).
This crate sequences production; it does not own another executable IR.
The [scalar computation and call map](scalar_calls.md) covers authored occurrence
replay, argument evaluation, and shared ordinary/composed call closures.

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

Concrete byte input uses the same exact satisfaction join in
`validation::exact_compiler_intrinsic_boundary_requirement`. Its zero-argument
result branch validates the closed byte-result declaration and matches the
requirement and realization's nominal result symbols. Source custody and reach
inference consume that join; the interpreter uses it before executing a bodyless
value callee. This establishes neither package acceptance nor native admission.
The concrete-byte-leaf line-loop regression exercises source-to-installation
composition; the bundled line-reader API/provider migration remains separate.

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

Closed-sum returning leaves can establish fresh claim-free affine boundary
results. The shared statement planner retains each authored local and call;
Terminal emission allocates distinct operation-owned places across states and
discards only the current leaf's results in reverse establishment order. Source
replay rejoins local statement, binding ordinal, boundary contract, and return
disposition. This leaf-only route does not admit later structural consumption
or nominal cleanup.

The general state graph inspects an owned, claim-free affine sum parameter or
a result of a boundary or ordinary graph call. Calls and effects retain their
authored order before and after result establishment. Ordinary successor
arguments can move a completed scalar-sum result into an exact owned state
parameter; the result binding rejoins its producing call and local declaration.
Source replay checks the subject, complete case roster, field paths, successor
positions, and cleanup provenance. A wildcard selects the remaining declared
cases. Case edges bind primitive payloads into staging blocks, then use ordinary
jumps for the remaining scalar arguments and borrowed views. The selected edge
disposes its affine subject after materializing those payloads.

Terminal receiving checks reconstruct producer dominance, exact nominal type,
and the live ownership frontier. An edge cannot transfer and discard the same
value or revive its old place. Decoded execution preserves the case and payload
through simultaneous state bindings and fuel suspension. Borrowed receiver
payload extraction, nested structural payload construction/transport,
loop-carried owned results, and whole nominal receiver replacement remain
separate dependencies. The `owned_case_state_transport` compiler test exercises
the filesystem's actual `ErrorKind` through calls and owned state dispatch.
The native [owned-state tests](../../../../tests/native-differential/tests/scalar_case_results/owned_state.rs)
exercise scalar-sum result transport through destination-owned homes, joins,
and dispatch using the ordinary publication pipeline. The complete filesystem
entry still requires the remaining source dependencies above.

Fresh closed scalar-payload sum expressions use that same owned-result namespace. An
ordered scalar `match` evaluates its subject once and constructs only the
selected case, transferring that owner into a continuation block parameter.
Nested selections, immutable locals, subsequent scalar statements, state
transfers, and returns retain the completed owner rather than reconstructing
the case or treating its tag as the source value. Constructor operands use the
ordinary scalar evaluation graph, once in authored field order and only on the
selected path. Membership borrows the completed local's actual place; it does
not reconstruct a selected case or encode a structural value as a scalar
parameter. Direct temporary membership shares the same constructor and operand
replay, with affine disposal after observation.

Independent source replay checks the exact constructor symbols, field meanings,
operand occurrences, ordered alternatives, and coverage before ordinary Terminal
ownership checking. The general state graph and ordinary scalar-completion
sequence share this value operation; an unavailable graph must not suppress a
complete ordinary sequence. Constrained fields stay on the existing proof-bearing
return route until general value establishment retains their range obligations.
This does not admit
joins of existing moved inputs, references, linear values, or nominal cleanup;
whole receiver-field replacement and selected floating comparisons remain
separate implementation dependencies.

Keep the complete filesystem fixture as the outer acceptance check. On macOS:

```sh
OMEGA_PASS_CANARY_FILTER=filesystem/windows_canonicalize_exit \
  mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail \
  -E 'test(=entry_and_abi::pass_canaries_compile)'
```

On PowerShell, set the same filter for the command and restore it afterward:

```powershell
$previousCanaryFilter = $env:OMEGA_PASS_CANARY_FILTER
try {
    $env:OMEGA_PASS_CANARY_FILTER = 'filesystem/windows_canonicalize_exit'
    mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=entry_and_abi::pass_canaries_compile)'
    if ($LASTEXITCODE -ne 0) { throw "Filesystem acceptance failed: $LASTEXITCODE" }
} finally {
    $env:OMEGA_PASS_CANARY_FILTER = $previousCanaryFilter
}
```

Use `cargo` in place of `mbx` when the wrapper is unavailable. This fixture
cross-compiles its Windows target on macOS; it does not execute Windows code.

The same graph retains its normal result explicitly: Unit or a closed,
claim-free affine/unrestricted scalar sum. Free and attached constructors and
returning state leaves retain each selected field's authored occurrence and
independently checked scalar expression. Admission rejoins nominal result/case,
the complete field roster, and operand identities before emission evaluates
fields once in authored order and emits `EstablishScalarCase` in declaration
order. The construction place and machine result place remain distinct.
Borrowed-view arguments and ordinary scalar state transfers use the existing
graph bindings, including loops before a returning leaf. Internal scalar-case
calls share the ordinary closure's predeclared signatures. This source call
route currently requires an empty callee precondition list. Selected
case edges dispose the returned affine root using the existing checked cleanup
evidence. Bounded fields receive independent declaration-range obligations;
missing proofs reject artifact production. This proof-bearing return plan still
requires pure constructor operands; the general value operation above handles
call-bearing plain scalar fields. Structural payloads and loop-carried owned sum
results remain separate dependencies. Ordinary native scalar-sum construction,
return/call transport, and selected-case observation retain full-width payloads
through installation replay for Linux x64/ARM64 and macOS ARM64. Windows
indirect aggregate returns remain a realization dependency; see
`tests/native-differential/tests/scalar_case_results.rs` at the repository root.

Closed integer field restrictions retain their exact carrier and inclusive
bounds through the checked catalog and Terminal declaration. The selected case
introduces those bounds on its copied scalar payload, allowing
`ByteRead::Byte(value: i32 [0..=255])` to prove an exact `as u8` conversion through
ordinary scalar forwarding. A successor parameter annotation is not an
independent hypothesis. Native providers must establish every promised value;
byte-input realization rejects restrictions excluding either 0 or 255.

Source range normalization uses the existing closed-expression evaluator's i64
window. Unsupported or unevaluated bounds reject instead of becoming unrestricted
scalars; floating/address restrictions and non-Exact arithmetic/range combinations
are not admitted by this producer. Restricted field stores and scalar-record
construction remain fenced until written-value obligations are retained.
Opaque interpreter inputs/results cannot establish these restrictions from type
identity alone. The authored `terminal_byte_views/read_line.omg` native fixture
composes the byte leaf, guarded writes, and payload-bearing line outcomes through
installation replay. It does not establish the bundled library's provider/API
migration or the complete `cli_mvp` entry path.

The general state-graph path also retains one persistent unrestricted mutable
record receiver. It shares ordinary Unit statement construction for ordered
field writes and whole-receiver Unit calls. Direct relevant integer/Boolean
field reads occur at their expression positions; successor states keep the
original invocation place rather than copied field values. Scalar expressions
use the existing checked arithmetic and call-argument evaluation paths.

Ordinary Unit helpers and boundaries receive immutable byte literals through
the same structural argument lane as whole borrowed views. State-graph calls
establish each literal at its authored argument position on the selected path;
mixed scalar operands retain their original bindings across short-circuit blocks.
Source replay checks each literal's bytes, type, access, and call coordinate.
Backedges reexecute the same establishment, and neither an earlier iteration nor
a sibling branch authorizes premature use. Projected byte-field operands and
owned argument transfers need separate producer support on this literal path.

Guarded Unit bodies can write `out[index] = byte` through an exact unrestricted
mutable byte-view parameter. Explicit state arguments transfer that view; they
do not implicitly capture an entry parameter or preserve a second exclusive
name. Terminal retains a fresh same-view length and an independently checked
bounds obligation. Artifact interpretation changes the caller's original
field-backed bytes without resizing the field or changing untouched bytes.
Ordinary field-to-view calls currently admit field-only paths from an
unrestricted mutable record parameter; indexed owner paths remain fenced.
Fresh per-iteration guards support mutable fill loops and exact cursor increments.
Completed ordinary byte-view calls preserve the caller's reference carrier, so
it can be passed again; they do not restore a still-live descendant loan or
preserve arbitrary content facts. Whole mutable views now reach ordinary native
indexed stores, including before scalar-sum line outcomes. Lending constructed
owned arrays as mutable backing and general multi-arrival invariants remain separate dependencies;
the writer primitive alone does not establish the shared line-reader contract.

Owned primitive-array values use `EstablishScalarArray`: already evaluated scalar
leaves initialize the exact recursive fixed-array type, including empty dimensions.
The ordinary effect sequence retains immutable array locals and structural return
alongside scalar stores and calls. Source replay checks the declaration, selected
constant indices, contextual numeric landings, and exact returned binding. The
decoded interpreter returns the actual payload and preserves it across suspended
calls. Primitive leaves use the existing pure scalar and computation owners,
including ordered calls and selective Boolean evaluation. Completed leaves survive
later control joins in private scalar slots; they never enter the authored local
namespace. Source replay checks literal and selected operation meaning in addition
to the shared read/call custody. Ordinary calls return array payloads through that
same sequence, either directly or into immutable locals. Their closure retains
each complete callee body; source replay checks the exact result binding and
call occurrence, not merely a compatible array type. Nested scalar arguments
and borrowed primitive-local operands retain the shared evaluation schedule.
Whole owned unrestricted array parameters and immutable constructor/call-result
locals also feed ordinary calls, including nested call results. Returning a
parameter retains its exact structural slot, separately from operation-result
ordinals. Replay checks authored positions and source identities; repeated
unrestricted actuals do not acquire affine disposal obligations.
Direct array-literal operands retain the enclosing call occurrence and authored
formal position on their constructor and scalar-element bindings. The shared
argument schedule interleaves constructors with scalar actuals and nested
structural calls; no synthetic call ordinal or source local is introduced.
Empty constructors still rejoin their exact owner and recursive type. Scalar
locals use the same selective evaluator, preserving earlier values across joins.
The ordered body can complete with its actual checked scalar expression, including
comparisons after array construction and calls. Completion uses ordinary scalar
establishment with the exact Return-role source binding. A final name, including
the immutable local/name pair introduced by existing source normalization, reuses
its existing value without repeating the call or charging another operation.
This ordered scalar completion does not yet carry authored
scalar contracts or result refinements; existing scalar-only contract lowering
remains available for bodies it can fully represent.
An ordered body can also complete through a conditional with two scalar returns.
The prefix establishes its arrays and scalar snapshots once; the existing scalar
evaluator then selects the guard and return under their exact authored roles.
Immutable array-local operands retain declaration symbols until the ordered
sequence supplies their actual result places. Counting earlier declarations is
not a substitute: nested calls and argument arrays share that result namespace.
Receiving replay checks the whole prefix, both return coordinates, fallback
coverage, and the local's source/type/borrow occurrence. This adds no Terminal
operation or array-specific control emitter. Array transfers to authored states,
repeated establishment in cycles, and ownership-bearing control completions
remain separate value/storage work.
The source and independently decoded/fuel-resumed probe is
`cargo nextest run -p checked-trees-to-lowered-psi --test scalar_array_source scalar_array_local_control_keeps_prefix_effects_and_call_result_storage --no-fail-fast`.
The CLI entry is
`cargo run -p omega -- inspect-terminal --machine selected --target macos_arm64 tests/omega/pass/collections/array_local_control/main.omg`.
Native acceptance is
`cargo nextest run -p omega-native-differential-test --test scalar_array_results array_local_control_preserves_selected_returns_and_prefix_effects --no-fail-fast`.
It publishes on all four native targets and executes both branches over every
byte input on a matching Linux/macOS host; other runtime legs explicitly skip.
The comparison probes are
`cargo run -p omega -- inspect-terminal --machine scalar_comparison --target macos_arm64 tests/omega/pass/collections/owned_array_scalar_comparisons/main.omg`
and the same command with `--machine array_comparison`. These publish Terminal
Psi. The native `scalar_array_results` suite publishes both entries on all four
targets and checks their exact Boolean scalar/array payloads on supported hosts.
Scalar computation calls retain array-valued actuals in expression-owned structural
slots. The shared evaluator completes each array's scalar leaves, establishes its
real structural result, then evaluates the next authored formal. Empty arrays use
the same constructor step without a fabricated scalar result. Boolean selection
and Match arms retain construction on their selected path. Source replay rejoins
the exact call, formal, recursive type, leaf expressions, and indexing selections.
The verifier checks unrestricted array payload
availability through producer dominance and same-block order, separately from
the exact affine/linear ownership frontier. Branch-local unused arrays therefore
permit scalar continuation joins; use outside their dominating scope still rejects.
Repeated array establishment in cycles and block-parameter payload transport remain
unsupported. The source probe is
`cargo run -p omega -- inspect-terminal --machine computation_row tests/omega/pass/modules/module_array_constant_indices/main.omg`;
decoded execution returns `[42u8, 9u8]` for input `42u8`. The
[`computation argument tests`](../../pipeline/checked-trees-to-lowered-psi/tests/scalar_array_source/computation_arguments.rs)
exercise nested/empty arrays, mixed formal effects, selective construction,
source substitutions, and fuel suspension without replay.
Transitive scalar callees retain this same ordered operation sequence. The
producer prunes callers against a stable roster of complete body candidates;
`checked-trees-to-lowered-psi/src/attached_unit/call_catalog.rs` then closes
operation, scalar-helper and provider dependencies before assigning identities,
including Unit statements retained inside scalar graphs.
Each operation body uses the existing ordered emitter once. A helper's local
storage requires the structural call frame even with a scalar-only signature;
the module entry names the selected source, not whichever helper was allocated
first. The companion probe is
`cargo run -p omega -- inspect-terminal --machine transitive_computation_row tests/omega/pass/modules/module_array_constant_indices/main.omg`.
The decoded regression fixtures return `[42u8, 9u8]` for input `42u8`. The
[`operation-body callee tests`](../../pipeline/checked-trees-to-lowered-psi/tests/scalar_array_source/operation_body_callees.rs)
retain scalar/Unit/array entries, mixed helper dependencies, empty arrays,
local write order, one-unit fuel resumption, and exact source/contract rejection.
Array state transfers, borrowed/projected payloads, and boundary-provider array
payloads still need their complete value/storage paths. Native constructors and
direct array results now use the ordinary aggregate graph; see the
[native transport owner](../../../omega/pipeline/target-operations-to-selected-instructions/README.md#ordinary-selected-control-flow).
The transitive example also reaches native owned-argument and incoming-result
transport through those shared homes; the native differential
`scalar_array_results` test loads this complete source, publishes all four
target artifacts and executes on matching supported hosts. Ordinary array result
locals retain the regular free-machine signature even without scalar parameters;
their presence does not imply a selected-operator affine signature.
Floating literals, parameters, locals, and ordinary calls retain binary32/binary64
format and payload through the same array path. Closed constant-row projections
check every sibling's format before selecting leaves; source replay rejects changed
bits or formats. Floating arithmetic still requires its own selected execution.
The native array ABI transports integer aggregate fragments, not foreign C
homogeneous-floating aggregates; scalar floating parameters retain their float-bank
placement. The `scalar_array_results` regressions observe raw bits, including
signed zero, subnormals, infinities, and NaN payloads, through owned-array calls.
Calls combining scalar floating arguments with an array result retain exact
mixed-bank constraints and integer result fragments. The complete `selected`
caller in `scalar_array_results/floating.rs` covers publication and native bits,
not just its separate construction and owned-array forwarding entries.
General slice-backed `.len` operands require retained view formation and bounds
evidence; endpoint subtraction alone cannot justify eliminating the view operation.

Provider-field calls retain the same exact attachment requirement roots as
ordinary Unit bodies, including across backedges and interleaved field writes.
Each machine retains only its direct boundary requirements; an ordinary callee
owns its own roots even when it borrows the caller's receiver. Roots are not
runtime operands. The complete state write frame includes reachable successor
effects and is replayed through the shared frame resolver, while each body's
ordered operations are checked separately.
Provider calls also rejoin their authored receiver root, field, and carrier;
unchanged target or requirement sets cannot justify a substituted source receiver.

The source publication/reload regression is
`cargo nextest run -p compiler --test cyclic_receiver_execution --no-fail-fast`.
It checks computed guards, effectful helper calls, provider-field attachment
requirements, caller-visible updates, and every fuel suspension point without
a termination claim. Canonical interpreter
tests additionally cover projected receiver calls. Whole plain-owned scalar loops
use the [scalar graph route](scalar_calls.md#guarded-primitive-reference-operand).
General owned cyclic custody,
ordinary projected source helpers, indexed/aggregate mutation, guarded crashes, and native
realization remain separate dependencies; this does not make `print_squares`
an executable native product.

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

## Structural predicate production

Contract: [structural predicates](../../../../wiki/spec/terminal-psi/structural_predicates.md).
Current production admits bounded acyclic relevant record/sum expansion, including
an enclosing record chain around one mixed common-field/case occurrence. This is
not arbitrary recursive depth, multiple mixed siblings, a mixed value inside
another mixed shape, or projected runtime sum support. Preserve every prefix
identity; extending depth is implementation work, not another equality rule.

Supported leaves are Boolean, fixed integer, IEEE, and byte-sequence carriers.
Address and erased leaf equality, all-erased records, written equality bodies,
text literals/direct text inequality, and general sum-bearing projected calls
remain outside this source family. A predicate may be serialized even though the
interpreter lacks the runtime sum inspection needed to execute its source use.

Whole-root/all-field-projected integer predicates preserve selected bitwise and
arithmetic terms. Runtime divisor/count and Exact value bounds need complete
checked requirement packages and independent caller proofs. Case-payload numeric
paths, imported crash capsules, and ownership/content effects do not acquire
support from these scalar-predicate paths. Source rejection must not erase a
retained contract or reinterpret unsupported operands as another scalar type.

## Structural access and stores

The [access contract](../../../../wiki/spec/terminal-psi/structural_access.md)
owns reference identity and non-observing writes. Current source support is
bounded: claim-free unrestricted field-path subloans, literal-indexed material
record receivers, and plain-record or whole-primitive scalar replacement.
Mutable-to-write-only attenuation preserves root access and independently
records the callee's weaker access. Shared projections preserve unrestricted
multiplicity and cannot originate from write-only roots.

Whole borrowed sum parameters support runtime `in` observations through ordinary
scalar completion, including Boolean locals and Boolean composition. Checked
source replay retains the authored subject and case; Terminal records a
non-consuming `StructuralCaseMembership` operation. It does not grant a durable
fact about later mutable contents. Canonical interpretation also observes
established scalar-case values without consuming their payload or owner. Native
realization currently follows the existing scalar-field sum layout; mixed
common-field sums remain outside that native layout route. Direct constructions
and immutable locals use the ordinary structural-value sequence described above,
with scalar operands evaluated once and membership observing the completed owner.

Ordinary Unit helpers retain borrowed `self` when checked scalar operands read
its fields, including through computed arguments. Attachment metadata cannot
supply the referent. Helpers without runtime receiver reads keep their existing
erasure eligibility. The `receiver_call_source::cyclic` tests observe projected
unranked and Natural-ranked callees' writes after return and across every
interpreter fuel pause. Shared Unit graphs retain the existing guarded unsigned
countdown judgment for `u8`, `u16`, `u32`, and `u64` using the same natural-rank
subject carrier as `Slice::Length`; private operand-evaluation edges preserve
the rank while authored cyclic transfers strictly decrease it. The existing
single-state cyclic-component restriction and unsupported explicit rank ranges
remain; this does not introduce a fixed-work ceiling. Native projected record
loops use ordinary borrowed-reference signatures and field stores, including
loop-carried integer sources. The `terminal_psi_indexed_receivers::cyclic_receivers`
regressions publish ranked/unranked callers on four hosted targets, retain the
full callee stack demand, and check original backing storage after return on
the supported execution host.

Projection replay rejoins every field/index, array bound, type, and source
application. Nonescaping alias prefixes with immutable bindings to mutable or
write-only references capture whole roots or static field/literal-index
projections. Mutable parents permit
mutable or write-only children; write-only parents permit only write-only
children. Their captured prefix precedes each
receiver suffix exactly once. Erasure replays every immediate parent, exact
formation and lifetime, and receiver use. Nested exclusive chains retain the
existing direct-root handoff evidence: every parent's final use forms its single
child, and the leaf ends at state exit. The call subloan retains the original
root, path, and attenuated
access; erasing local names does not authorize skipping intermediate custody.
Dynamic/range projections, escaping receiver aliases, early nested closure,
restored-parent uses, deeper shared array paths, and general reference-bearing
or constrained data need separate producer support. A stored pointer cannot be
read merely to locate a write-only receiver.
Dynamic write summaries remain conservatively collection-wide.

Nonempty fixed-array storage shapes retain integer elements with an explicit
arithmetic policy, including receiver fields such as `[u64 in Wrapping; 16]`.
The complete array identity retains the policy while its element uses the
integer payload layout. This is a storage projection: source assignments and
selected arithmetic still owe their exact policy correspondence, and implicit
policy erasure remains invalid. Range and nominal qualifications need their own
retained evidence; the arithmetic-policy classifier does not admit them.
Constant array construction keeps its separate complete-type eligibility checks.
This shape admission composes with existing receiver-field operations and calls;
indexed reads and writes of policy-qualified integer arrays still need their
executable projection path.

Run the source-to-canonical receiver controls, including nested alias erasure,
exact source/loan tampering, and fuel-boundary interpretation, with:

```sh
cargo nextest run -p checked-trees-to-lowered-psi --test receiver_call_source --no-fail-fast --no-tests fail
```

For four-target publication and supported-host caller-storage observations:

```sh
cargo nextest run -p omega-native-differential-test --test terminal_psi_indexed_receivers --no-fail-fast --no-tests fail -E 'test(nested_aliases::) | test(projected_aliases::) | test(mutable_aliases::)'
```

Cross-publication is not runtime coverage on the other targets.

Whole-root and plain-record field stores accept exactly typed IEEE literals and
runtime parameters alongside bounded fixed-integer/Boolean scalar sources.
IEEE replacement retains its format and payload without observing the destination;
it does not authorize unselected floating computation. Ordinary or selected
fixed-integer call results retain their
checked call identity and durable result, including provider-plan correspondence
for selected calls. Plain-record stores share path/type reconstruction with
dynamic realizations. Attached Unit bodies retain ordered direct scalar-field
store sequences and branch-free computed values, including direct integer field
observations. Bounded projected single-store forms include one literal record-array
index. Ordinary Unit primitive locals use initialized referents and current
storage reads through the [shared call producer](scalar_calls.md#unit-and-boundary-integration);
immutable snapshots survive borrowed mutations. Ordinary Unit helpers retain
the authored local actual and its exact checked borrow occurrence. Readable
primitive parameters can initialize a distinct local; write-only inputs cannot.
Native establishment/read realization supports fixed 8/16/32/64-bit integer,
Boolean, and IEEE binary32/binary64 locals through the ordinary graph.
Delayed results, computed IEEE arithmetic,
short-circuit store values, and richer indexed/aggregate forms need further
producer support.

Bounded-owned byte fields accept literal replacement through the same ordered
assignment and exact write-frame route. Source predicates are discharged before
erasure; the Terminal operation independently proves the exact source length fits
the destination capacity. Literal establishment stays at the assignment's authored
position. Artifact interpretation replaces live bytes and live length on the
original referent, including empty and shorter values across ordinary calls.
Runtime-indexed byte replacement retains checked index and value operands in
authored order, followed by a current field-length observation and the
independently reconstructed bounds obligation. Source encoding predicates remain
checked before erasure. Artifact interpretation preserves caller-visible writes,
live length, and sibling fields. Native byte-field operations are not realized;
the consumer explicitly rejects whole replacement, field length, and indexed
replacement before projection. This support does not make the unchanged
`print_squares` sample runnable.

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

An explicitly discarded plain-owned structural result from a boundary or ordinary
call retains its real result place and exact source call. It does not acquire a
synthetic local or a Unit signature. Affine disposal belongs to the immediate normal continuation,
before the next statement; source replay independently requires that edge even
if a rewritten plan instead claims return-time disposal. Named results and
nested argument temporaries retain their existing custody routes. Result-producing
graph calls use the ordinary call-input and requirement/crash handling, including
literal byte views and projected mutable fixed buffers. Receiver retention joins
ordinary and graph callers before closure pruning, independently of result category;
the receiver remains a loan of the caller's original storage. A graph caller uses
the same projected transfer validation as a straight-line caller, not a whole-root
type comparison or a caller-state-count restriction. This source support does
not extend provider-candidate admission beyond its separately
implemented body families.

Checked provider discovery reuses the ordinary/composed call closure, including
state graphs constructing plain affine scalar-sum results. It retains their
helper and boundary dependencies to a fixed point; it does not turn the caller's
boundary operation into a selected direct call. Competing ordinary, composed,
and affine-identity plans reject. Source admission checks the graph and exact
result, and canonical verification independently checks the conformance signature
and service refinement. Provider selection and native entry provisioning remain
separate obligations. See the
[composed-provider regressions](../../pipeline/checked-trees-to-lowered-psi/src/tests/composed_provider_candidates.rs).

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
