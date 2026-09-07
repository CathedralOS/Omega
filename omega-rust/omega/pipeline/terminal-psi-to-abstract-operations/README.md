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
