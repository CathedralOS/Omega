# Native realization

This coordinator consumes canonical Terminal Psi and explicit realization
inputs. Its public contracts are [boundary realization](../../../../wiki/spec/terminal-psi/boundary_calls.md),
[private callbacks](../../../../wiki/spec/build/private_callbacks.md), and
[component publication](../../../../wiki/spec/build/component_publication.md).
Start at [lib.rs](src/lib.rs), then follow [realization](src/realization/native_artifact.rs):
validate scope and entry, obtain the abstract input, admit providers, emit the
object, and assemble the image. One `NativeRealizationRequest` carries the image
request, optional checked scope and optional prepared input alongside target and
provider evidence. Those inputs do not select alternate API entrypoints.

The result distinguishes direct and dynamic ELF artifacts; extracting a direct
artifact cannot grant dynamic output installation authority. Every rejection
returns the exact image request. Program-entry and callback-custody adapters
retain their additional owned evidence and use the same realization operation.

## Multi-target reuse

[Target selection](../../../../wiki/spec/build/configuration.md) defines child
identity and isolation. The compiler batch route prepares each child's canonical
Terminal artifact independently. [PreparedNativeRealizationInput](src/realization/input.rs)
shares target-neutral decoding, proof admission, and abstract-input lowering
only for equal complete `TerminalArtifactIdentity`, exact `AdmissionProfile`,
and exact `PostTerminalOptimizationSelections`; it rechecks that key on use.
An optimized/unoptimized Boolean alone is not its key.

Target, entry/calling plans, provider/external settlements, authority policies,
callbacks, FMA admission, physical evidence, and machine/image lowering remain
child-local. This reuse is an implementation optimization, not review, proof,
or audit evidence. Equal Terminal artifacts may enter different ISA lowerers;
different target-selected root artifacts must remain separate.

## Program-entry settlement

[Entry roots](../../../../wiki/spec/build/entry_roots.md) owns the source/arrival
contract. [entry_settlement](src/entry_settlement/mod.rs) independently replays
target, source signature, calling/storage plans, canonical artifact, exact
Terminal entry, and service establishment before issuing the validated carrier.
It does not call the Psi receipt producer to validate that producer's output.

[service_establishment.rs](src/entry_settlement/service_establishment.rs) joins
selected Fused service evidence to the Terminal receiver's attachment and exact
erased fields, then to the selected provider plans. Semantic receiver identity
and retained Terminal attachment identity are separate facts and need not have
the same spelling. The [entry controls](src/tests/native_realization/entry_settlement.rs)
cover this join; it is not evidence of runtime slot publication or Independent
execution support.

## Callback custody boundaries

Retained-product callback and checked-scope composition must close through
actual native publication, not a zero-payload layout or provider-selection check.
Widening scalar-provider forwarding additionally requires preserved incoming
ABI homes, complete call/relocation replay, and a genuinely reachable authored
entry. Use rooted provider and callback controls retaining complete argument,
result, and resource custody; metadata-only receipt tests do not close the route.

[callback_custody.rs](src/realization/callback_custody.rs) returns the caller's
opaque companion by value on both success and rejection. That wrapper does not
admit, lower, fingerprint, or interpret its contents.

Native callback arguments separately enter the realization request. Their
[target-side carrier](../../pipeline/abstract-operations-to-target-operations/src/model.rs)
binds a Terminal operation, placement index, private function, native parameter
application, registrar plan/context, and application commitment. Target lowering
validates the one-slot relation. The commitment remains producer provenance:
the reduced tuple cannot reconstruct the complete authored telescope or prove
the source-site-to-operation mapping. Independent authentication and replayable
source correspondence remain required before claiming complete publication
custody; see the callback work on the [execution board](../../../../TASKS.md).

The bounded direct-parameter route retains one callback on the normalized-import
path with fixed-integer semantic arguments/results and complete register or
stack placement. Field destinations and multiple callbacks need further work.
The checked callback body is a separate canonical artifact, with its own local
machine namespace. Lowering must bind private code, native argument ordinal,
relocation, and final executable region without inventing a Terminal operand.

The direct route supplies the declared native parameter, not a source-specific
host operation or compiler-inserted callback storage object. Function/image
evidence does not establish registration or lifetime. Generic runtime registration
primitives in [component-publication](../../backend/runtime/component-publication/src/callback_registration.rs)
are separate from connecting an authored registrar result to capacity, leases,
source registration, cleanup, and retry. Their existence alone does not close
that end-to-end path.
