# Native realization

This coordinator consumes canonical Terminal Psi and explicit realization
inputs. Its public contracts are [boundary realization](../../../../wiki/spec/terminal-psi/boundary_calls.md),
[private callbacks](../../../../wiki/spec/build/private_callbacks.md), and
[component publication](../../../../wiki/spec/build/component_publication.md).
Start at [lib.rs](src/lib.rs).

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

## Callback custody boundaries

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
