# Terminal Psi to abstract operations

Public contracts: [Terminal product](../../../../wiki/spec/terminal-psi/product.md)
and [boundary realization](../../../../wiki/spec/terminal-psi/boundary_calls.md).

The entry map is [lib.rs](src/lib.rs). Responsibilities are separate modules:

- `artifact`: canonical admission and replay.
- `optimization`: verified optimization-unit construction and proof questions.
- `provider_installation`: exact selected-adapter and installation custody.
- `lowering`: verified-machine lowering, with ordinary scalar/Unit and structural
  families separated.

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

The [ownership contract](../../../../wiki/spec/terminal-psi/ownership.md) requires
replay of actual moves and maximal residual subtrees, independently of producer
rows. Parameter/result sources and unrelated live roots remain distinct.
Current native continuation support is an adjacent acyclic Unit chain; broader
cycles, computed scalar bindings, projected boundary results, and mixed cleanup
schedules need complete downstream support rather than erased edges.

Ordinary identity-result producers and projected disposers retain separate result
homes under the admitted direct integer-class aggregate ABI. Split register
fragments preserve order, register, offset, and exact width; Microsoft x64's
indirect structural-return form remains outside this lane. Odd-sized fragments
cannot read or write alignment padding. Eight-byte home alignment does not
increase logical extent. Staging, copying, and result stores must preserve
source registers and indirect bases, and independently replay all intervals.

Continuations retain real edges and ordered cleanup, including a zero-byte record
for an empty complement. Final-return records contain only owners still live
there. Entry-origin scalar homes must survive calls and staging; source-to-target
validation rejoins authored bindings, while later byte replay cannot recover an
unretained authored alias or block map. Object/image/installation replay checks
the retained source/result, call/edge sequence, layout, homes, and partition.

Root length/stride metadata belongs to array roots, not record roots whose paths
happen to traverse arrays. ABI copies materialize only the owned subtree being
transferred; this is not a borrowed-referent copy. No-code residual cleanup emits
neither instructions nor liveness-dependent loops.

## Scalar call realization

Each callee selects its own ABI. Evaluate arguments into disjoint durable homes
before filling register or outgoing-stack destinations. Emission includes the
complete outgoing area (including Microsoft x64 shadow space), x86 alignment,
and AArch64 link preservation. Typed relocations bind exact Psi operations and
callees; conditional emission preserves live inputs and rebases independently
encoded arm/condition relocations into final function order.

Selected-operator Unit continuations currently have narrower admission than the
call vocabulary: fixed-native integer results and supported ordinary scalar
closures, with exact selected-plan/adapter identity replay. The structural-operand
subset consumes a permutation of whole claim-free affine roots with separate
scalar operands; its hosted empty-record path has Linux x86-64/AArch64 native
coverage. Nontrivial layouts, claims, services, projections, borrowed operands,
and wider control do not follow from that case. Artifact replay is consistency
evidence, not a claim that a human audited the emitted program.

## Scalar cleanup tails

Native scalar-return lowering preserves the result across executable cleanup and,
on AArch64, preserves the return link. Physical replay checks the exact frame,
stores, loads, calls, stack ceiling, and result lifetime on every path. No-code
cleanup positions remain semantically ordered without inventing target calls.

A shared Boolean convergence tail retains source-ordered decisions, joins from
nonfinal leaves, final-leaf fallthrough, and one physical cleanup tail. A direct
field read binds the source place and field identity to its native interval;
replay reconstructs field type/offset, source home, live stack depth, and exact
load/normalization bytes. Opaque field identity is not layout authority.
Admission of a comparison or field leaf is not general nested structural access
or arbitrary cleanup composition.

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

Only the legacy unsigned-countdown carrier selects
[ranked_native.rs](src/artifact/ranked_native.rs). This specialized entrance admits
the entry machine's exact unsigned countdown and ceiling. Native and fixed-fuel
verification run independently. Unsupported countdowns still reject rather than
falling back to ordinary admission. This is not admission of an ordinary call to
a countdown-ranked callee.
The [projected-receiver contract](../../../../wiki/spec/language/termination.md#ranked-callees-on-projected-receivers)
needs composed argument references, call/return, cleanup, callee measure checking,
and resource evidence; removing an entry guard or widening parameter count is
not that implementation.

The bounded structural frontier has one affine-owned place or a persistent
mutable receiver. Borrowed receivers preserve their original referent, reference
multiplicity, empty owned frontier, and `BorrowedReference` ABI shape. Primitive
arrays retain exact lengths/element types rather than synthetic record leaves.
Assignment and replay derive shape and target pointer placement from declarations;
only the target-prescribed rank register is admitted.

`EstablishPrimitiveLocal` and `PrimitiveScalarRead` have verified interpreter
semantics but no native storage realization yet. Ordinary operation routing
rejects both with operation-specific errors; it cannot replace a borrowed
referent with an SSA snapshot. Native local allocation, lifetime, load/store,
and call-observation controls remain required.

The retained countdown projection contains canonical semantics/proof bytes,
fixed-fuel fields, header/backedge frontiers, graph, ABI, and type closure. Object
replay decodes the proof again, reconstructs graph/frontiers and physical body,
and rejoins coordinates to the verified module. Coherent substitutions and
stripped records reject. The specialized Linux x86-64/AArch64 body retains exact
four-operation/five-edge work attribution; it does not insert runtime accounting.
Ordinary selected-instruction support, provider installation, and mixed work do
not follow from this specialized route. It must not become a second architectural
pipeline as broader ranked lowering is implemented.

## Dynamic dispatch

The [dynamic-dispatch contract](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)
owns selection identity, forwarding roles, and complete tables. Reconstruction
starts in [dynamic_dispatch.rs](src/lowering/machine/operation/calls/dynamic_dispatch.rs).
Retain whole closed applications, not just the selected callable. Changing an
unselected row must still change identity and fail stale replay.

Source/native support remains bounded around direct selections, a rebound
descriptor, and a transparent parameter hop. The immutable-table native route
does not establish arbitrary multi-block continuations, mutable initialized
data, or BSS. Unit and scalar forwarding retain different result storage needs.

The mutation-bearing source subset admits bounded ordered literal primitive-field
stores through a mutable receiver before an independent field return. Direct
and finite named-record paths are distinct from general indexed/case paths,
computed values, repeated destinations, and arbitrary body reorderings. Its
native integer/Boolean stores and scalar results do not imply general IEEE or
aggregate support. Recheck the relevant producer and receiving admission before
extending a shape; successful descriptor transport alone is not executable
reference preservation.
