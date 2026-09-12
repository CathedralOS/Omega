# Terminal Psi to abstract operations

Public contracts: [Terminal product](../../../../wiki/spec/terminal-psi/product.md)
and [boundary realization](../../../../wiki/spec/terminal-psi/boundary_calls.md).

The entry map is [lib.rs](src/lib.rs). Responsibilities are separate modules:

- `artifact`: canonical admission and replay.
- `optimization`: verified optimization-unit construction and proof questions.
- `provider_installation`: exact selected-adapter and installation custody.
- `lowering`: one operation-by-operation walk for scalar, Unit, and structural
  results. Optimization selection does not change this projection.

No checked tree, StateGraph, or caller-created module substitutes for the
canonical artifact. Unsupported vocabulary rejects at admission or lowering;
provider installation is not permission to drop an unsupported result.

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
[instruction-selection owner](../target-operations-to-selected-instructions/README.md#ordinary-selected-control-flow)
retains the remaining owned-argument, empty-value and indirect-result limits.

Verified branches, calls, and crash exits retain operation identities and
ownership transfers regardless of result category. Local establishments are
ordinary operations, not hidden inside a structural return. The
[ownership contract](../../../../wiki/spec/terminal-psi/ownership.md) requires
replay of actual moves and maximal residual subtrees independently of producer
rows. Parameter/result sources and unrelated live roots remain distinct.

This stage retaining an operation does not establish its native implementation.
The next stages accept Unit, scalar, and structural functions through one graph.
Executable cleanup, claim-bearing continuations, projected qualifications,
installed-provider calls, and descriptor forwarding require their complete
ordinary operation and proof joins; missing support rejects. No flat
whole-function or no-code structural-return alternative supplies that support.

## Native calls and storage

Each callee has its own ABI. Native argument transport must preserve source
values before filling register or outgoing-stack destinations. Physical replay
checks the outgoing area, alignment, return-link preservation, source places,
field widths, calls, and relocations. An aggregate's padding is not observable
payload, and a borrowed referent must not become a private value copy.

The [target stage](../abstract-operations-to-target-operations/README.md) and
[instruction-selection stage](../target-operations-to-selected-instructions/README.md)
own their actual operation coverage. Admission here, source production, and
successful optimization are not native-publication claims.

## Ranked native admission

Contract: [control flow and ranking](../../../../wiki/spec/terminal-psi/control_flow.md).
[native.rs](src/artifact/native.rs) routes natural-ranked modules through ordinary
Terminal verification and abstract lowering. That verifier checks the exact
grouped control-cycle evidence, including slice-decrease proofs; the route does
not require or manufacture a fixed-work ceiling. The abstract graph retains its
cyclic edges and structural bindings, and the separately verified optimizer input
retains the canonical Terminal module and proof bundle. Native artifact admission
alone does not establish downstream optimization, target lowering, or publication
support for these cycles.

Whole plain owned structural block arrivals retain exact source/destination
types, access, multiplicity, and ordered bindings. Independent current ownership
replay consumes all old affine roots before establishing destinations, including
swaps and self-loops; cleanup follows reverse establishment order derived from
dominance, not serialized block IDs. Retained frontier snapshots independently
rejoin cleanup and rebinding. Claims, qualifications, projected transfers, and
linear arrivals remain outside this bounded admission. Plain owned payloads use
the ordinary native graph. Integer and Boolean record-field observations retain
their actual parameter, construction-result, or block-arrival place; current
ownership and dominance checks reject unavailable sources. Field identity and
scalar type are reconstructed from that source's exact declaration. Native
local and block-arrival reads use the existing structural home and primitive
load path; broader projected reads and structural call transport remain separate
consumer dependencies.

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
[target control graph](../abstract-operations-to-target-operations/README.md)
and native physical pipeline. Exact verified-source custody, not the presence
of ranking metadata, authorizes downstream cyclic safety checking.

The older unsigned-countdown native custody is unsupported. Admission rejects
it explicitly rather than erasing its proof requirements or falling back to
ordinary verification. There is no dedicated countdown target, selected, or
machine-code carrier. Natural ranking and its grouped certificates remain
checked on the ordinary path.

The [projected-receiver contract](../../../../wiki/spec/language/termination.md#ranked-callees-on-projected-receivers)
still requires composed argument references, call/return, cleanup, callee measure
checking, and resource evidence. A graph representation alone does not establish
those guarantees; unsupported transfers reject until their ordinary operation
and proof joins are implemented.

## Dynamic dispatch

The [dynamic-dispatch contract](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)
owns selection identity, forwarding roles, and complete tables. Reconstruction
starts in [dynamic_dispatch.rs](src/lowering/machine/operation/calls/dynamic_dispatch.rs).
Retain whole closed applications, not just the selected callable. Changing an
unselected row must still change identity and fail stale replay.

Descriptor transport and checked selection do not establish native indirect-call
execution. Immutable-table calls still require the exact descriptor, table, slot,
relocation, call/return, and resource joins on the common graph. Missing joins
reject; there is no direct-selection or transparent-hop native fallback.
