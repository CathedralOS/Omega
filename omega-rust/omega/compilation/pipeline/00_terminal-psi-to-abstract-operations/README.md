# Terminal Psi to abstract operations

Public contracts: [Terminal product](../../../../../wiki/spec/terminal-psi/product.md)
and [boundary realization](../../../../../wiki/spec/terminal-psi/boundary_calls.md).

This stage lowers checked Psi for native execution. The build's checked selection
identifies which functions need target support here; raw artifact requests without
that record keep full-module checking. Unused library helpers do not demand
unavailable target operations through the selected route.

The execution contract requires host-authorized producer trust, not just
canonical decoding. Public ingress still needs that enforcement under
`PSI-CLAIM-ADMISSION`; the cache configuration's authorization meaning is the
open [native-cache-producer-authorization decision](../../../../../OWNER_QUESTIONS.md#native-cache-producer-authorization).

The build's starting function, implementations and callbacks are joined before
lowering. The [shared input owner](../../representations/abstract-operations/verified-input/src/lib.rs)
keeps that checked record with the lowered program. Later stages read the same
record without depending on this earlier stage. The entry map is [lib.rs](src/lib.rs),
which preserves the established public names through reexports.

- The [shared proposal owner](../../representations/abstract-operations/verified-input/src/native_proposal.rs)
  checks the chosen target, program entry, provider rows and callback occurrences
  against the canonical artifact before publishing its immutable native proposal.
  It does not compile source or grant native authority. Target identity comes
  from the validated entry; no build-evaluator object or duplicate entry receipt
  crosses this boundary. Providers, occurrences and boundary applications remain
  complete groups. Application metadata belongs to the backend target owner;
  package permissions and behavior exclusions remain independent inputs.
  Selected exclusions map to the artifact's local service IDs without reopening
  a typed or checked program or calling a source resolver.
- The [shared execution owner](../../representations/abstract-operations/verified-input/src/native_execution.rs)
  follows calls from the proposal's entry, implementations and callbacks. It
  validates component replacements before excluding their local implementations.
  The [scope walk](../../representations/abstract-operations/verified-input/src/native_execution_scope.rs)
  also retains cleanup, installed-interrupt and code-layout requirements, plus
  functions referenced by dynamic dispatch tables. Reached calls can require
  additional direct or target-family implementations; demand completion checks
  those additions and binds the final choices after completion. Original build
  choices remain distinct from that completed selection.
- [retained_terminal.rs](src/retained_terminal.rs) orchestrates lowering and
  native provider preparation using the shared proposal and execution records.
  It joins package permissions and prepares callbacks, imports and builtins for
  admission. Derived import evidence remains invocation-owned through admission
  and emission; the compiler forwards this stage's request rather than defining
  a second set of its fields. The [codec](src/retained_terminal_codec.rs) persists
  source-free derivation inputs. Recovery passes them through the same shared
  proposal operation without reopening source. The in-memory retained product
  itself does not encode a separate native authority record.
- [native_execution_scope.rs](src/native_execution_scope.rs): the complete native entrance.
  Canonical input preparation, provider settlement and installation, and
  mechanism/exclusion review produce one admitted input before Omega 01.
  [Provider admission](../../realization/provider-admission/src/provider_admission.rs) derives each demanded
  leaf's mechanism and admits builtins through the receiving policy and target
  catalog; it reads the bindings the proposal derived instead of rejoining them. Omega 02 consumes these facts;
  neither it nor the compiler reconstructs the admission sequence.
- [artifact_admission.rs](src/artifact_admission.rs): canonical preparation,
  optional obligation-ledger replay, and distinct ordinary/optimizer/native
  admission. Every admitted result retains the exact access-view input roster.
  Extraction into a consumer without roster custody rejects nonempty inputs;
  this check follows verification and lowering, not canonical decode.
  [Native preparation](src/artifact_admission.rs) receives the complete checked
  selection from the shared owner, verifies its artifact, entry, target and
  original choices, applies selected geometry, then lowers those functions.
  Raw requests without that record keep full-module checking and retain no
  selected scope. There is no partially lowered native input followed by a
  public pruning step. Ordinary/no-normal calls,
  cleanup, callbacks, native metadata and dynamic-table address roots retain
  their existing demand and per-block diagnostic order. Explicit artifact
  inspection and independent ownership validation still cover the full catalog.
- The admitted handoff to Omega 01 is `VerifiedPsiOptimizationInput` in [verified-input](../../representations/abstract-operations/verified-input/src/lib.rs); Omega 01 builds its optimization unit from it.
- `provider_installation`: exact selected-adapter and installation custody.
- [shared callback settlement](../../representations/abstract-operations/verified-input/src/callback_settlement.rs): rejoin a produced
  callback's Terminal interface to its exact selected calling application.
  Native placement reads each reached boundary's canonical calling telescope:
  semantic/native-only argument identities and order, nominal callback binders,
  static-machine ordinals, and synchronous-invocation flags. Target calling
  conventions supply the direct function-pointer geometry. A proposed calling
  application's duplicate telescope must agree with that declaration; it cannot
  override it. Ordinary value layouts and private inline-slot layouts still
  arrive prepared and remain a separate source-free preparation gap.
  Placement replay, requirement identity, scalar/structural ABI matching and
  the private channel's import exclusion live here, beside program-entry
  settlement. The coordinator supplies the artifact, placement and calling-plan
  roster; no checked-compilation wrapper or new pipeline stage is involved.
  Its [registrar-import settlement](../../representations/abstract-operations/verified-input/src/callback_settlement.rs)
  joins each ordered callback occurrence to the registrar's separate outbound
  plan and selected import row. The retained-Terminal and direct-native routes
  share this operation; exact operation multiplicity and check-only absence of
  an import remain explicit.
- [Shared boundary settlement](../../representations/abstract-operations/verified-input/src/settlement.rs)
  binds calls and comparisons to implementations already chosen by the build.
  It selects from canonical requirements and checked body candidates using the
  target's Build rows. Intrinsic executions arrive already established by
  provider review. No checked application scope, source declaration lookup,
  const/type reconstruction or intrinsic semantic replay enters this operation.
  The proposal retains the selected coverage and opaque ABI applications;
  native admission consumes them without a second source join.
  Also traverses Terminal
  comparisons and consumes the exact executions already selected by boundary
  coverage. A sorted plan-digest index joins the target plans without source
  uses, checked handles or Lowered Psi occurrence records. Integer comparisons
  retain their selected semantic operation, not another lowering emission
  triple. Ordinary builtin integer comparisons need no provider row.
- `lowering`: one operation-by-operation walk for scalar, Unit, and structural
  results. Optimization selection does not change this projection.

[Module lowering](src/lowering.rs) checks entry and structural block bindings,
then calls [machine lowering](src/lowering.rs) directly.
That owner prepares block/value rosters, visits blocks in order, dispatches
[operations](src/lowering.rs) and
[terminators](src/lowering.rs), and assembles the abstract
function. Scalar, Unit and structural results share this traversal; there is no
separate forwarding or ordinary-machine route to navigate.

No earlier program, StateGraph, or caller-created module substitutes for the
canonical artifact. Unsupported vocabulary rejects at admission or lowering;
provider installation is not permission to drop an unsupported result.

Native admission returns `AdmittedNativeArtifact`: one abstract program and
its checked optimizer context. It decodes and reconstructs proofs once, lowers
through ordinary native authority, then checks optimizer eligibility while moving
the already-verified state. These admission gates remain distinct; optimizer-only
input cannot be promoted into native authority or paired with an unrelated plan.
Optimizer and native custody borrow the roster from their retained canonical
module, without a second mutable roster. A native consumer lacking placement
support extracts `VerifiedNativeArtifactInput` only after an empty-roster check.

Image-writer storage must not be relabeled as compiler-authored object data.
For example, Mach-O replay derives aligned eight-byte lazy-binding pointer slots
from exact imports/relocations. Installation's text/immutable-data projection
covers compiler-authored prefixes; the complete image is separately bound and
replayed. A projection is not a claim that every image byte came from an object.

Installation replay keeps candidate and caller result places distinct and
preserves complete argument paths, claim sources, and residual complements.
Provider scalar signatures are compared against Terminal declarations rather
than copied into a second wire schema. Result/conformance admission alone
does not implement native structural-result storage.

The next stages operate on the same representation sequence for empty and
selected optimization. Historical direct-assignment and special one-fragment
publication routes are not architectural alternatives. Machine-effect facts
must still reach final call placement, frame/return-address preservation,
relocations, bytes, and independently validated publication.

## Structural results and residual cleanup

Primitive array construction retains its complete structural result and ordered
scalar leaves in `EstablishScalarArray`. Ordinary, optimizer, and native artifact
admission use this same projection, including empty recursive dimensions. Current-IR
validation independently checks shape, leaf type/availability, and owned call/return
custody; scalar rewrites retain every leaf occurrence. Native storage and direct
result fragments use the downstream ordinary aggregate graph; its
[instruction-selection owner](../03_target-operations-to-selected-instructions/README.md#ordinary-selected-control-flow)
retains the remaining owned-argument, empty-value and indirect-result limits.

Verified branches, calls, and crash exits retain operation identities and
ownership transfers regardless of result category. Local establishments are
ordinary operations, not hidden inside a structural return. The
[ownership contract](../../../../../wiki/spec/terminal-psi/ownership.md) requires
replay of actual moves and maximal residual subtrees independently of producer
rows. Parameter/result sources and unrelated live roots remain distinct.

This stage retaining an operation does not establish its native implementation.
The next stages accept Unit, scalar, and structural functions through one graph.
Executable cleanup, claim-bearing continuations, projected qualifications,
and installed-provider calls require their complete
ordinary operation and proof joins; missing support rejects. No flat
whole-function or no-code structural-return alternative supplies that support.

## Native calls and storage

Each callee has its own ABI. Native argument transport must preserve source
values before filling register or outgoing-stack destinations. Physical replay
checks the outgoing area, alignment, return-link preservation, source places,
field widths, calls, and relocations. An aggregate's padding is not observable
payload, and a borrowed referent must not become a private value copy.

The [target stage](../02_abstract-operations-to-target-operations/README.md) and
[instruction-selection stage](../03_target-operations-to-selected-instructions/README.md)
own their actual operation coverage. Admission here, source production, and
successful optimization are not native-publication claims.

## Ranked native admission

Contract: [control flow and ranking](../../../../../wiki/spec/terminal-psi/control_flow.md).
[artifact_admission.rs](src/artifact_admission.rs) routes natural-ranked modules through ordinary
Terminal verification and abstract lowering. That verifier checks the exact
grouped control-cycle evidence, including slice-decrease proofs; the route does
not require or manufacture a fixed-work ceiling. The abstract graph retains its
cyclic edges and structural bindings, and the separately verified optimizer input
retains the canonical Terminal module and checked producer facts. Native artifact
admission alone does not establish downstream optimization, target lowering, or
publication support for these cycles.

Whole plain owned structural block arrivals retain exact source/destination
types, access, multiplicity, and ordered bindings. Independent current ownership
replay consumes all old owned roots before establishing destinations, including
swaps and self-loops. Whole Linear arrivals rehome each live claim to its exact
destination while preserving the claim identity and typed path; they neither
create replacement claims nor copy a source obligation. Ordinary call transfer
rejoins that current home, including terminal calls on conditional and guarded
arms. Alternative arms inherit the same claim identity; only the selected
invocation transfers it, and each path independently accounts for its owner.
Affine cleanup follows reverse establishment order
derived from dominance, not serialized block IDs. Retained frontier snapshots
independently rejoin cleanup and rebinding. Conditional claim-lineage changes,
claim-bearing result joins, and broader qualified or projected Linear transfers
remain outside this bounded admission. Exact owner-designated scalar-record
cleanup retains its consumed claim roster through an ordinary hook invocation;
no-code discard remains Affine-only. Erased or nested Linear child disposition
is not covered by the parent hook. Plain owned payloads use the ordinary native
graph. Integer and Boolean record-field observations retain their actual
parameter, construction-result, or block-arrival place; current
ownership and dominance checks reject unavailable sources. Field-only carrier
paths retain canonical declaration-local IDs through the abstract graph.
Independent native and image replay reconstruct each child declaration and
offset from the original root; the same leaf ID in another record cannot replace
that path. Direct reads use an empty path. Native local and block-arrival reads
use the existing structural home and primitive load path, including bounded
integer leaves. Their declarations and independently reconstructed construction
obligations remain retained; a read's physical width/sign comes from that exact
carrier. Bounded stores still reject without an invariant-establishment proof.
Indexed/case carriers and broader structural call transport remain separate
consumer dependencies. The hosted `declared_range_inference_hosted_entry_runs_natively`
canary executes bounded signed, full-width unsigned and nested record reads.

Initialized primitive locals retain their exact operation-result place and typed
initializer. Local stores remain distinct from incoming-parameter stores; fresh
reads retain their source place and new scalar result. Current-IR checking
reconstructs primitive type, access, claim-free unrestricted storage, dominating
establishment, and nonescape. Optimization cannot treat a read as its initializer
or an earlier observation across mutation. The ordinary native graph realizes
fixed 8/16/32/64-bit integer, Boolean, and IEEE binary32/binary64 locals and their
borrowed calls, with exact-width source reads and unchanged scalar payload bits.

Unranked modules take the same ordinary verification and abstract route without
a progress claim. Scalar cycles proceed through the shared
[target control graph](../02_abstract-operations-to-target-operations/README.md)
and native physical pipeline. Exact verified-source custody, not the presence
of ranking metadata, authorizes downstream cyclic safety checking.

The older unsigned-countdown native custody is unsupported. Admission rejects
it explicitly rather than erasing its proof requirements or falling back to
ordinary verification. There is no dedicated countdown target, selected, or
machine-code carrier. Natural ranking and its grouped certificates remain
checked on the ordinary path.

The [projected-receiver contract](../../../../../wiki/spec/language/termination.md#ranked-callees-on-projected-receivers)
still requires composed argument references, call/return, cleanup, callee measure
checking, and resource evidence. A graph representation alone does not establish
those guarantees; unsupported transfers reject until their ordinary operation
and proof joins are implemented.

## Dynamic dispatch

The following describes the legacy selector implementation pending replacement
by [runtime machine values and some packages](../../../../../wiki/drafts/designs/machine_values_some.md).
Source `dyn` and whole-trait runtime tables are retired. Preserve the argument,
contract, result and custody behavior below while migrating the representation;
these implementation notes do not authorize the old source form.

The [dynamic-dispatch contract](../../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)
owns the replacement descriptor and package evidence. The legacy implementation
selects named implementations with separately supplied data arguments. `SelectImplementation` retains its exact table and a scalar
result home; `CallDynamic` becomes an ordinary abstract call with a dynamic
target, separately retaining selector, interface, slot and value arguments.
The u64 is only the opaque physical carrier, not permission to substitute an
integer literal or direct machine. There is no instance/table descriptor,
synthetic receiver or transparent-hop native fallback.

The abstract plan retains the complete interface/table catalog. An invoked
slot's requirement owns the argument ABI; no implementation supplies a
representative signature. Ordinary scalar flow carries forwarding, rebinding,
storage and joins, independently of borrowed data custody. Executable table
rows remain address-retention roots, while invoked-slot reachability alone
contributes authority and effects. Changing an unselected row still changes
identity.

The current public slot vocabulary carries Unit or scalar results and ordinary
structural formals. It does not carry structural result origins, qualification
preconditions or ingress claim schemas. Missing metadata rejects rather than
being inferred from private implementation bodies. Native lowering, register
allocation and image publication consume these same targets through their
ordinary operation owners; table rows without executable callables keep their
zero trap slots.
