# Target slots and program entry

[Target selection](configuration.md) fixes one profile per child activation.
Its closed typed slots define what an installable artifact must bind; source
does not discover an entry by name.
[External-root admission](external_roots.md) governs installed liveness and
resource evidence after that selection.

## Slots

| Slot field | Contract |
| --- | --- |
| Identity and schema | Exact target-owned declaration and normalized schema. |
| Direction | `EnvironmentToProgram` external root or `ProgramToProvider` outbound service. |
| Binding shape | `ExactRequirement(requirement)`, `CompleteConformance(trait)`, or `EntryMachine(entry_shape)`. |
| Lifecycle | `BuildBound` or `RuntimeInstalled`. |
| Occurrences | Cardinality and required, optional, and reserved indices. |
| Installation authority | Authority needed to publish the bound occurrences. |

Direction, lifecycle, cardinality, and indexing are independent axes. Program
entry, reset/interrupt vectors, callbacks, and providers share the binding model
without becoming one undifferentiated kind. Binding shape comes from the slot,
not the trait's current requirement count. An exact-requirement consumer may
use that requirement's contract, not conformance identity or unrelated trait laws.

Every required build-bound slot must have exactly one selection. Only rows owned
by the selected profile enter the durable child projection. A row misattributing
its slot to another profile, duplicate active bindings, or a missing required
slot rejects with the exact slot identity. Package/library products bind no
roots. Runtime-installed slots may remain open until installation validates the
same shape, portable demands, target supply, authority, and lifecycle.

## Source selection is not invocation

```omega
machine build(builder: &mut Build) {
    builder.application("entry-example");
    builder.roots.bind(windows_x86_64::ProgramEntry, Application::start);
    builder.select_provider<windows_x86_64::Console, TestConsole>();
}
```

The entry selection identifies an exact free machine or, when the entry shape
allows it, a machine with one `&mut self` receiver. Build supplies neither that
receiver nor invocation arguments. The provider selection names a nominal type;
its ordinary satisfiers determine the derived plan. A leaf needs `via` only for
payload not derivable from its exact declaration and target.

There is no special `main`, `Main::run`, uniquely visible export, entry field,
or ambient static discovery. Templates may write ordinary bindings. A test
harness name override carries no slot, signature, calling-plan, or storage
authority.

## Entry shape and arrival bridge

An entry shape names distinct physical and semantic arrival requirements,
target-authored bootstrap adapter and physical result map, visible parameters,
semantic result, and receiver mode `None` or `ProvisionedZii`. For example, a
hosted Windows ProgramEntry may join WindowsProcessEntry under WindowsX86_64
calling policy to ProgramStorageEntry through WindowsProgramBootstrap and
WindowsProcessExitMap, exposing no ordinary parameters to source.

The platform calling plan validates the generated physical shell; the target's
entry shape validates the selected source signature. Resolve the slot first,
then validate source shape, then the applicable physical/semantic and storage
calling plans. Keep those facts joined through component-progress/provider
projection and their backend/storage consumers.

The environment supplies physical values under its requirement. The exact
target adapter validates them, installs scoped providers, establishes semantic
arrival, and supplies only schema-declared source arguments. Its result map
defines both pre-handoff rejection and normal-return physical outcomes.
The compiler does not invent meanings for platform handles. The combined bridge
derives crash, reach, write, work, stack/state, introduction, provisioning, and
provenance contracts and composes them with the application closure.

A free entry receives no implicit state. An attached entry gets one ZII-valid
receiver derived beneath an admitted root and borrowed for that activation;
it is not globally nameable. Provisioning is occurrence-local: the nominal data
type acquires no root authority or storage class. Storage used for the receiver
or active stack cannot also be forwarded whole; retain the partition and pass
only the exact disjoint residual. Generated bridge code remains subject to
portable demand checking. A freestanding schema may deliberately expose image
and initial-storage roots as parameters; hosted source sees neither by default.

The bridge runs ordinary receiver cleanup on normal return and accounts for
abandonment through the ordinary crash frontier otherwise. A non-ZII-valid
receiver rejects; a free entry may instead explicitly construct state from the
resources in its schema. Other roots, tasks, and handlers obtain access only
through explicit ordinary capability transfer, borrowing, or synchronized sharing.
Hosted writable-image placement and freestanding storage partitioning must
preserve the same occurrence, root lineage, backing, and initial exclusive borrow.
Knowing the receiver's size never creates a new physical root.

For selected Fused service fields, ZII bytes alone do not establish `Bound`.
Each direct `Service<R> in Bound` receiver field requires occurrence evidence
binding source signature/slot, receiver/attachment/field, carrier/qualification,
service schema, and selected plan digest. Terminal replay independently joins
the erased field and plan. This establishes the provisioned occurrence, not a
runtime-published slot, era handle, or Independent execution.

## Authority, identity, and resources

Selection is distinct from runtime permission. The authoritative build declares
one complete ceiling, and validation proves:

```text
transitive runtime demand ⊆ authored complete reach ceiling ⊆ target supply
```

Omission from that complete set denies authority; absence of a provider selection
alone does not. Rejection preserves provenance to the dependency/declaration/
selection introducing excess demand, including when only artifact evidence is
available.

Physical contracts bind exact target-contract source bytes with domain-separated
SHA-256 and retain the target role. Compact source/report fingerprints are not
identity. Source custody, accepted package/schema, contract identities, and
calling plans remain separate checks. Standalone UEFI compatibility requires
the byte-exact bundled contract; package-aware UEFI requires an exact
consumer-accepted ordinary-package binding, not origin or location inference.

[Installed content](../resources/content_custody.md#installed-introduction-schemas)
reuses these slots: only statically enumerable installed environment-to-program
parameter occurrences may introduce domain-authorized fresh roots. Ordinary
calls require existing lineage. Installation evidence is not a second source
authorization mechanism.

[WCSU](../resources/storage.md#stack-demand-and-backing) is checked against target
StackPlan supply. Ordinary build files do not choose stack size; a target-supply
override is deployment policy. Insufficient fixed supply reports the responsible
call path and requires a program change, not a nonexistent build knob.
