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
by the selected profile enter the durable child projection. Inactive rows must
resolve their profile and slot ownership, but do not require checking or emission
support for that profile. Target-specific entry and calling-plan validation
belongs to the selected child; [implementation availability](configuration.md#target-recognition-and-implementation-availability)
is distinct from recognizing a binding. Unknown profiles or slots, a row
misattributing its slot to another profile, duplicate active bindings, or a
missing required slot reject with the relevant identity. Package/library products
bind no roots. An explicitly artifact-only application also binds no executable roots;
it must instead satisfy [required artifact completion](scoped_execution.md#staged-products-and-failure).
Runtime-installed slots may remain open until installation validates the
same shape, portable demands, target supply, authority, and lifecycle.

The `entry-slot-schema-resolved-by-bare-name` issue is answered by the exact
identity rule above. Resolve the target-owned schema, or the exact
consumer-accepted package/schema on a package-aware route. An unrelated
same-leaf declaration neither shadows that selection nor creates a second
candidate. The caller's package and a declaration's leaf spelling cannot
choose the schema. Selecting an alternate package contract requires the
existing explicit accepted binding; loading it is not selection. The remaining
lookup and canary repair is `ENTRY-SLOT-EXACT-SCHEMA-SELECTION` in `TASKS.md`.

## Source selection is not invocation

For an application named `entry_example`, Build selects
`windows_x86_64::ProgramEntry` with machine argument `start`, and
may separately select `TestConsole` for the exact Console requirement. Neither
selection invokes application code. The Build operations themselves follow the
ordinary static-machine argument rules; they have no receiver-syntax exemption.

The root-binding form is `Build::bind(builder, Target::Slot, Product::entry)`.
The first argument explicitly supplies the current activation's mutable Build
value; ordinary helpers may forward that value. The remaining operands select
the slot and entry rather than invoking either. There is no `roots` receiver
facet or implicit Build argument.

The entry selection identifies an exact static machine. Build selects that
declaration, not a data instance or an allocation request. An explicitly
installed entry contract determines its invocation arguments and the arrival
bridge must establish each one from actual backing. A mutable parameter alone
does not ask the bridge to construct its referent. The provider selection names a provider implementation;
its ordinary satisfiers determine the derived plan. A leaf needs `via` only for
payload not derivable from its exact declaration and target.

Only these designated toolchain operands resolve in the product context. They
do not require a host import of the selected implementation, and host `use`
exposure cannot redirect them. Ordinary same-named operations receive no exception.
[Product-reference selection](scoped_execution.md#selecting-product-declarations-without-executing-them)
defines lexical visibility, restricted helper delegation, exact source/target
identity, and rejection of generated-source or unfinished-layout cycles.

There is no special `main`, data-owned entry machine, uniquely visible export,
entry field, or ambient static discovery. Templates may write ordinary bindings. A test
harness name override carries no slot, signature, calling-plan, or storage
authority.

[Build-time test roots](testing.md#entry-contracts-and-ordinary-calls) reuse the
ordinary storage and explicit installed-argument rules below. Their runner roots are separate
from application target slots; registration does not install them in the
application or make their checked bodies trusted boundary declarations.
A native test harness instead binds an ordinary executable entry normally.

## Entry shape and arrival bridge

An entry shape names distinct physical and semantic arrival requirements, the
boundary schema and physical contract package that own its generated arrival
bridge and physical result map, visible parameters and semantic result. Named
decision: `ordinary-entry-storage`, ratified 2026-10-22. An entry has no special
storage or automatic context-construction mode. `NoneOrProvisionedZii` and
signature-scanned mutable-context provisioning are retired, not an optional
compatibility path. The installed contract identifies exact argument
occurrences separately from physical arrival arguments; no receiver flag,
machine name or data attachment participates. For example, a hosted Windows
`ProgramEntry` joins the `WindowsProcessEntry::enter` process arrival under the
`WindowsX86_64CallingPolicy` calling policy to the `ProgramStorageEntry::enter`
semantic continuation through the `WindowsX86_64Application` boundary schema's
generated bridge; the authored contract's `u32` return maps to the process
exit code, and no physical arrival parameters are exposed to source unless its
explicit installed contract declares and establishes them.

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

Application state has the same storage choices as other state:

- Ordinary stack locals use ordinary construction, lifetime and cleanup, with
	their complete activation demand checked by WCSU against actual stack supply.
	ZII is an initialization rule, not a special entry allocation mechanism.
- Ordinary static data uses its existing placement, lifetime and access rules.
	Static backing does not make mutable state ambient or freely shareable; an
	exclusive owner may lend it explicitly.
- An explicitly installed entry call may receive values or borrows, as with UEFI
	handoff. This is provisioning rather than a third storage class: the contract
	must establish backing, occurrence identity, qualifications and authority
	before invocation. Build selection does not execute the call or manufacture
	runtime capabilities.

Heap-backed state remains ordinary allocation/library use, not another entry
mechanism. A parameter not provided by the installed contract rejects; its
type or mutable-reference shape supplies no default object. Non-ZII state needs
ordinary explicit construction. Erased values obey the normal
[binding rules](../proofs/contracts.md#explicit-erased-bindings), not an
entry-only field-synthesis path. Boundary services remain exact provider slots
demanded by calls, never provisioned fields on an application record.

Storage used for application state or the active stack cannot also be forwarded
whole; retain the partition and pass only the exact disjoint residual. Generated
bridge code remains subject to portable demand checking. A freestanding schema
may explicitly expose image and initial-storage roots; hosted source sees
neither by default. Removing automatic context allocation does not remove the
target ABI startup bridge or weaken its custody obligations.

Ordinary owners perform cleanup on normal return; explicitly installed arguments
retain the caller/bridge's declared handback obligations. A crash retains its
crash frontier; a canonical [process exit](../language/process_exit.md) instead
retains the distinct external terminal outcome and abandonment contract.
Returning from the entry does not establish that other activations were settled:
the root/runtime contract must settle or legally transfer outstanding task and
resource custody before physical completion. The result map cannot conceal
abandonment behind the same exit syscall. Other roots, tasks, and
handlers obtain access only through explicit ordinary capability transfer,
borrowing, or synchronized sharing.
Hosted writable-image placement and freestanding storage partitioning must
preserve the same occurrence, root lineage, backing, and initial exclusive borrow.
Knowing an argument's size never creates a new physical root.

Each demanded static provider slot retains the exact source call occurrence,
closed requirement application, schema, target, and selected plan digest.
Missing, incompatible, ambiguous, or unauthorized supply rejects compilation;
no erased-field fallback or late source lookup is permitted. For selected Fused
slots, Terminal replay independently joins the call demand and plan before
erasing dispatch. Independent slots retain internal era identity without adding
a source context field.

Calling the same machine ordinarily supplies every argument explicitly and gains
no provisioning authority. Provider demands follow its checked call closure,
not a constructor attached to a data type. Remaining automatic entry-context
code and examples are migration debt under `EXPLICIT-ENTRY-STORAGE` in
[the execution board](../../../TASKS.md), not supported source semantics.

## Authority, identity, and resources

Selection is distinct from runtime permission and from an optional machine
annotation. For installation or an explicitly requested deployment-compatibility
check, the root's application/deployment manifest or installed contract supplies
the authoritative complete permission ceiling, and admission proves:

```text
transitive runtime demand ⊆ manifest/installation permission ceiling ⊆ target supply
```

Omitting a checked machine's `reaches` requests inference; it neither supplies
nor removes manifest permission. A declared service ceiling grants no runtime
authority. Omission from the manifest's complete set denies permission; absence
of a provider selection alone does not. Rejection preserves provenance to the dependency/declaration/
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
