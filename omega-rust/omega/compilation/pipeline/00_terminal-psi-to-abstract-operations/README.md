# Terminal Psi to abstract operations

Public contracts: [Terminal product](../../../../../wiki/spec/terminal-psi/product.md)
and [boundary realization](../../../../../wiki/spec/terminal-psi/boundary_calls.md).

The entry map is [lib.rs](src/lib.rs). Responsibilities are separate modules:

- [retained_terminal.rs](src/retained_terminal.rs): the retained Terminal product
  and its target-constrained native proposal. This owner joins canonical
  semantics, entry custody, provider rows, callbacks, comparisons and exclusions
  once, while the proposal is derived; the product has no codec and never
  leaves the process, so neither re-entry nor admission replays those joins
  ([verification placement](../../../../pipeline.md#verification-placement)).
  Compiler reports only retain and expose its products. Build selection closes
  into the existing validated native entry settlement before retention, so no
  build-evaluator object or duplicate entry receipt crosses that boundary.
  Providers, occurrences and boundary applications cross as complete units;
  boundary coverage is retained without decomposing and rebuilding its rows.
  Target identity comes from the validated entry. Application metadata belongs
  to the backend target owner and is shared with native reports; application
  intent remains independent of the image subsystem. Package permissions and
  behavior exclusions remain independent inputs, never inferred from providers.
  Its [proposal constructor](src/retained_terminal/proposal.rs) owns the full
  producing sequence: entry settlement, canonical decode, provider demands,
  callback and comparison joins, boundary coverage and registrar imports.
  The compiler supplies completed callback artifacts and borrowed selections;
  the constructor checks their roster and exact placement joins. Compiler
  builtins are bound to their selected plan and Terminal boundary here, once.
  It does not call compilation or grant native authority. Checked selections
  remain transitional inputs until provider selection itself moves behind
  Terminal Psi.
  Its [native provider preparation](src/retained_terminal/native_providers.rs)
  joins package permissions and prepares callbacks, imports and builtins in
  one operation. Derived import evidence remains invocation-owned through
  admission and emission; the compiler forwards the stage's admission request
  without defining a second copy of its custody fields.
- [native_admission.rs](src/native_admission.rs): the complete native entrance.
  Canonical input preparation, provider settlement and installation, and
  mechanism/exclusion review produce one admitted input before stage 01.
  [Provider admission](src/provider_admission/mod.rs) derives each demanded
  leaf's mechanism and admits builtins through the receiving policy and target
  catalog; it reads the bindings the proposal derived instead of rejoining them. Stage 02 consumes these facts;
  neither it nor the compiler reconstructs the admission sequence.
- [artifact_admission.rs](src/artifact_admission.rs): canonical preparation,
  optional obligation-ledger replay, and distinct ordinary/optimizer/native
  admission. Every admitted result retains the exact placed-view input roster.
  Extraction into a consumer without roster custody rejects nonempty inputs;
  this check follows verification and lowering, not canonical decode.
- `optimization`: verified optimization-unit construction and proof questions.
- `provider_installation`: exact selected-adapter and installation custody.
- [callback_settlement.rs](src/callback_settlement.rs): rejoin a produced
  callback's Terminal interface to its exact selected calling application.
  Placement replay, requirement identity, scalar/structural ABI matching and
  the private channel's import exclusion live here, beside program-entry
  settlement. The coordinator supplies the artifact, placement and calling-plan
  roster; no checked-compilation wrapper or new pipeline stage is involved.
  Its [registrar-import settlement](src/callback_settlement/registrar_imports.rs)
  joins each ordered callback occurrence to the registrar's separate outbound
  plan and selected import row. The retained-Terminal and direct-native routes
  share this operation; exact operation multiplicity and check-only absence of
  an import remain explicit.
- [boundary_application_settlement.rs](src/boundary_application_settlement.rs):
  join the sealed artifact-bound Psi scope to canonical demand identities, exact
  checked-body or intrinsic realizations, and opaque ABI application custody.
  The coordinator supplies borrowed selection inputs; the stage owns role
  selection, const/type identity projection and retained-provenance replay.
  Const application values arrive already admitted by Psi checking. The
  proposal retains the coverage it settles; native admission consumes it
  without replaying it against the sealed scope.
- [comparison_settlement.rs](src/comparison_settlement.rs): join emitted IEEE
  and integer comparisons to their exact authored application, selected plan,
  retained provenance and rederived intrinsic execution. The native proposal
  derives these records once and retains them. The existing
  checked-build route still supplies source-side selection evidence; these
  source-free sidecars grant no native authority and do not bypass canonical
  admission. Integer emission order/negation and diagnostic ordering are retained.
- `lowering`: one operation-by-operation walk for scalar, Unit, and structural
  results. Optimization selection does not change this projection.

[Module lowering](src/lowering/mod.rs) checks entry and structural block bindings,
then calls [machine lowering](src/lowering/machine.rs) directly.
That owner prepares block/value rosters, visits blocks in order, dispatches
[operations](src/lowering/machine/operation/mod.rs) and
[terminators](src/lowering/machine/terminator.rs), and assembles the abstract
function. Scalar, Unit and structural results share this traversal; there is no
separate forwarding or ordinary-machine route to navigate.

No checked tree, StateGraph, or caller-created module substitutes for the
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
[native.rs](src/artifact_admission/native.rs) routes natural-ranked modules through ordinary
Terminal verification and abstract lowering. That verifier checks the exact
grouped control-cycle evidence, including slice-decrease proofs; the route does
not require or manufacture a fixed-work ceiling. The abstract graph retains its
cyclic edges and structural bindings, and the separately verified optimizer input
retains the canonical Terminal module and proof bundle. Native artifact admission
alone does not establish downstream optimization, target lowering, or publication
support for these cycles.

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

The [dynamic-dispatch contract](../../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)
owns selection identity, forwarding roles, and complete tables. Its governing
model is runtime selection of named implementations with separately supplied
data arguments. This stage has no dynamic lowering: the retired instance/table
descriptor route was deleted, and the named-implementation selector route
under `DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION` replaces it. Retain whole
closed applications, not just the selected callable. Changing an unselected row
must still change identity and fail stale replay. Missing joins reject; there
is no direct-selection or transparent-hop native fallback.
