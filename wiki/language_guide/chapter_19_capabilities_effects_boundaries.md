# Chapter 19: Capabilities, Reach, And Boundaries

A portable program should be able to say what it needs without naming a syscall,
DLL, or firmware table. Omega separates that requirement from the provider that
realizes it and the authority needed to invoke it.

Keep five questions separate:

| Question | Where the answer lives |
| --- | --- |
| Which services may this code reach? | The `reaches` ceiling. |
| May it park, block a worker, or crash? | Independent operational clauses. |
| What authority was it given? | Values with established qualifications and provenance. |
| Why trust the implementation? | Checked evidence or explicit provider admissions. |
| What resources and mechanism does it use? | Selected plans, resource contracts, and receiving policy. |

An empty service row does not prove termination or absence of mutation. A
capability does not override a reach ceiling. Selecting a provider does not
establish a runtime claim. The [effect specification](../spec/language/effects.md)
defines these distinctions precisely.

Examples illustrate the intended source model, not a claim that every target or
native route is implemented. Unsupported routes must reject rather than weaken
a contract. Implementation limits belong beside the
[native realization code](../../omega-rust/omega/compiler/native-realization/README.md).

## Boundary Surfaces

`boundary` marks an explicitly accounted crossing. Its declaration kind says
what crosses:

| Declaration | Concern |
| --- | --- |
| Boundary machine or operator | Control, calling, reach, and guarantees. |
| Boundary trait | Service requirements and provider realization. |
| Boundary data | Representation. |

A crossing can use checked Omega code or an admitted external realization.
The keyword alone does not grant trust or determine direction. Parameters,
results, and selected supply give those details.

Indexing, for example, keeps its caller obligation visible even when the
physical operation is compiler-provided:

```omega
boundary operator [] Slice::index<T>(items: &[T], index: u64) -> T
requires
    index < items.len;

boundary operator [..] Slice::range<T>(items: &[T], start: u64, end: u64) -> &[T]
requires
    start <= end && end <= items.len;
```

The caller proves the bounds. The requirement does not select its realization;
the target or an authorized build choice selects an admitted candidate. Users
need not know the private pointer or descriptor layout.

### Representation TCB and claim admission

An opaque representation answers “how are these bits carried?”, not “who may
create this value?” or “what may its holder do?”. An interrupt acknowledgement
can use a provider-selected carrier while remaining linear and consumable only
through its completion protocol.

Runtime by-value use needs one exact target-closed representation before ABI
planning. A reference-only opaque pointee need not expose its by-value shape;
proof-erased data needs none. The active consumer build selects a declared
candidate:

```omega
machine build(builder: &mut Build) {
    builder.select_representation<
        InterruptAcknowledgement,
        PicAckRepresentation
    >();
}
```

`PicAckRepresentation` names a conformance to the exact compiler-owned
representation trait. The compiler derives shape and movement rather than
accepting authored sizes or ABI classes. Compiler-owned primitives instead
derive their representation from pinned target semantics.

The initial relationship is inert: no independently invoked cleanup or disposable
debt may hide in the carrier, including inactive sum cases. Register and argument
copies of a linear opaque value move one semantic occurrence; they do not
duplicate its authority. A semantic copy needs a copyable opaque and a
structurally copyable inert carrier.

An invalid explicit selection rejects even if unused. A valid unused selection
remains visible to review but creates no by-value demand. Dependency builds do
not dictate the consumer's choice. Independently compiled by-value crossings
must agree on the exact representation; a state-migration proof cannot repair
an incompatible ABI in an existing caller.

See [opaque representations](../spec/build/opaque_representations.md) and
[representation review](../../omega-rust/omega/packages/review/evidence/README.md).
Opacity alone is not an accepted proposition or capability establishment.

## Boundary Traits

A boundary trait names a portable callable surface. Its requirements can be
realized by checked adapters, imported code, syscalls, firmware, or other
providers:

```omega
boundary trait Readable {
    machine read(
        path: [u8]::Utf8,
        out: &mut Vec<u8>
    ) -> ReadResult
      suspends;
}
```

The caller proves input requirements. Checked realizations prove their
guarantees; opaque realizations require admitted guarantees. The boundary trait
contributes its own service identity automatically; additional `reaches` members
name other services. `suspends` separately permits parking.

Calling a statically selected window provider reaches its window service.
Whether its code came from a DLL is a provider fact. Calling an explicit runtime
loader additionally reaches `DynamicLibraryLoading` and still needs admission
for the loaded implementation. API visibility, crossings, and possible behavior
are different questions: `pub`, `boundary`, and `reaches` do not substitute for
one another.

## Service Bindings

A trait is an interface, not a runtime carrier. Local dynamic dispatch uses
`dyn Trait`; authority for a selected boundary service uses `Service<R> in Bound`:

```omega
data Application {
    logging: Service<LoggingService> in Bound;
}

machine Application::start(
    logging: Service<LoggingService> in Bound
) -> Application
{
    Application { logging }
}
```

Installation/publication establishes `Bound`; a record literal, zeroed storage,
or the existence of a provider cannot. A fused build may erase the carrier and
dispatch directly. An independently emitted call acquires one published provider
era and retains it through the call. Rebinding the slot does not rewrite every
service value.

The carrier is affine by default: move it or borrow it; duplicating authority
requires an authorized route. A session or other must-discharge obligation is a
separate linear value returned by the service. The public service receiver is
distinct from the provider's private state receiver.

### Local dynamic interfaces over bindings

A local dynamic descriptor is not a replaceable component interface. An ordinary
proxy can bridge the two:

```omega
data LoggingProxy {
    service: Service<LoggingService> in Bound;
}

ComponentLogger:
    LoggingProxy satisfies Logger
{
    machine write(&self, text: &[u8])
        reaches LoggingService
        suspends
    {
        suspend self.service.write(text);
    }
}
```

The local descriptor points to the proxy; the proxy calls through the service's
selected entry and provider lease. See [traits](chapter_14_traits.md) for local
dynamic values and [component publication](../spec/build/component_publication.md)
for service carriers, duplication, era pins, and replacement.

## Service Reach And Operational Clauses

`reaches` is a `+`-separated ceiling of boundary services. Parking, worker
blocking, and crashes use independent clauses; `terminates` is a positive
progress guarantee, not another may-effect.

At a call, possible parking and blocking are acknowledged independently:

```omega
operation();
suspend may_park();
block may_block_worker();
suspend block may_do_either();
```

These prefixes match the statically known envelope exactly. They do not force
a pause or change the ABI. A suspending call needs a continuation boundary and
cannot nest inside an argument or operator; a blocking-only call may nest.
[Concurrency](chapter_18_concurrency.md) explains the call-site restrictions.

## Service Identities And Inference

Services are exact boundary-trait identities, not a built-in numeric list or
names recognized by spelling. Inheritance adds parent services and `+` means
set union, never a choice between target providers.

Private checked bodies may omit clauses to request inference. Published surfaces
keep stable ceilings: omitted reach is empty, omitted suspension and blocking
forbid those possibilities, and omitted crash causes are forbidden. An explicitly
empty private `reaches` clause is a ceiling, not inference. Implementations may
refine requirements but cannot widen them after callers have compiled.

A checked in-memory `Readable` provider may avoid opaque trust and blocking;
it does not erase the abstract `Readable` reach. An empty service row says nothing
by itself about recoverable failure, mutation, authority use, or termination.
There is no subtraction or masking of services.

Installation-bound requirements have one restricted symbolic form:

```omega
pub boundary requirement InterruptAcknowledgement::complete(self)
reaches <= MachineControl + PortIo
requires
    self in InterruptAcknowledgement::Pending;
```

The selected provider's exact row must fit the bound and be resolved throughout
the installation closure. It cannot escape unresolved through an ordinary
callable package or component interface. Equal reach sets do not prove provider
identity or acknowledgement lineage. See the
[effect identity rules](../spec/language/effects.md#published-identity-and-installation-rows).

### Declared service reach and installed terminal authority

Source review asks which abstract service is reachable. Receiving policy also
asks what the selected implementation can physically do. A filesystem provider
cannot justify a process-execution syscall merely by satisfying a filesystem
requirement.

The receiver compares the service/schema's permitted authority with the selected
mechanism and argument contract's exercised authority. Every demanded mechanism
needs one classification, including explicit empty cases; unknown is not empty.
A function named `open_read` proves no flag restriction. Narrowing needs retained
checked constraints on the actual arguments.

Filesystem policy distinguishes content read/write, metadata query/mutation,
directory enumeration, and namespace mutation. Requesting or arming deletion
is namespace mutation. Ordinary release incidentally completing another actor's
already-requested deletion is different; deferred-deletion behavior attached to
the released handle is not exempt. The exact mechanism and accepted release
contract decide the classification, not the name `close`.

An empty dangerous-authority set does not hide the call or prove purity, lifecycle
safety, trust, or object confinement. Raw integer descriptors cannot prove “writes
only files opened for writing.” That needs unforgeable, checked object authority.
[Permission policy](../spec/build/permissions.md) owns the complete rules.

## Synchronous Boundary Invocation

Transitive service reach is not a call graph. `invokes` describes a binding that
may be entered synchronously before the current invocation returns. Bodyful
machines infer direct invocations; bodyless requirements declare them, and
omission permits none. An invoked handler contributes its service and selected
operational envelope to the current invocation.

Durable registration instead establishes a future external root. Creating it
adds no synchronous edge. A registrar that also calls the handler on its current
chain must declare that possibility. The realized synchronous graph must be
acyclic: adding a wrapper does not break a cycle, while a queue or genuine new
activation can. Root admission separately checks the concrete future handler.
See [callback lifetime](../spec/build/private_callbacks.md#registration-and-lifetime).

A nonreturning native operation is not automatically successful external
termination. `never`, a Unit `exit_process` declaration, or an exit syscall cannot
distinguish success from crash or divergence. The semantic distinction is settled,
but its authored completion declaration and effect-identity ownership remain
[undetermined](../spec/terminal-psi/observations.md#successful-external-termination-source-form-undetermined).

## Capabilities And Authority Flow

Two packages can both reach `Writable` yet hold different power. One writes using
authority supplied by its caller; another prompts for fresh authority. The reach
row alone does not distinguish them.

Authority uses ordinary values qualified by owner-defined domains:

```omega
data Folder {
}

domain Folder::Writable
established by Desktop::choose_folder;

boundary trait Desktop {
    machine choose_folder(prompt: &[u8]::Utf8) -> Folder::Writable;
}

boundary trait Writable {
    machine write_bytes(folder: Folder, path: &[u8]::Path, bytes: &[u8])
    requires
        folder in Folder::Writable;
}
```

This is a protocol sketch; the actual runtime fields and provider are
package-owned. Reconstructing fields does not establish `Writable`. A caller must
already carry the qualification or obtain it through the authorized route and
matching admitted invocation.

Authority-flow reports distinguish accepting, using, deriving, storing,
returning, releasing, and acquiring authority. Opening a narrower handle within
a caller-supplied folder derives authority; prompting for a new folder acquires
it. Thus a report can distinguish:

```text
caller-supplied cache:
  accepts/uses Folder::Writable; acquires none; reaches Writable

prompted cache:
  acquires Folder::Writable via Desktop::choose_folder;
  reaches Desktop + Writable
```

Policy may allow the first and reject the second. Neither report replaces provider
trust or object-level confinement. The
[authority specification](../spec/resources/authority.md) defines establishment,
provenance, and the distinction between promises and actual receipts.

### Package builds

`build.omg` runs with compiler-issued, package-scoped facets, not ambient
filesystem, network, process, secret, or package-acceptance power. `BuildSource`
observes the verified source snapshot, `BuildOutput` writes to its sponsored
staging tree and publishes explicit generated-source handoffs, and `BuildLog`
records observations. Helpers inherit no extra authority; their effects compose
into the root.

The resolver retrieves dependencies before their code runs. Its transport and
credential authority does not pass to dependency builds. Generated source also
inherits no build authority: ordinary runtime contracts apply to it. Release
observations must be hermetic or replayable; volatile development inputs do not
establish a source-rebuildable release. See [build execution](../spec/build/execution.md).

Package acceptance is a root policy decision over compiler-derived findings,
not proof that a human audited or understood the source. A signature identifies
key control; a certificate proves only its checked claim.
[Package review](../spec/packages/review.md) keeps these evidence roles separate.

### Origin and custody

Origin records where a claim began. Custody records the owner that currently
gives it meaning and must remain available for reclamation. A transparent
`Transaction { slot: u64 }` can still depend on the provider era interpreting
that slot. Moves and returns preserve custody. Transfer requires a named
receiver's checked acknowledgement or admitted receipt; a written postcondition
does not create it. Old-era sessions can safely delay replacement until consumed
or transferred. See [returned-value custody](../spec/build/component_publication.md#returned-values-and-custody).

## Host Providers

The target package declares checked adapters and irreducible native leaves.
The compiler derives a complete candidate plan from their explicit satisfaction
relationships. Build selects a candidate, not authored plan rows:

```omega
machine build(builder: &mut Build) {
    builder.select_provider<selected_target::Console, TestConsole>();
}
```

The target supplies ordinary defaults; an authorized slot owner can override
one. Selection, structural validation, semantic admission, and runtime invocation
are separate steps. A selected provider must cover its required surface and
apply to the target.

A generic requirement is normally one selected schema, not a slot per type or
const argument. Final composition closes every reachable application and checks
its realization. A symbolic demand describes what must be supplied; it is not
coverage. Even future universal checked-body evidence could not provide concrete
ABI layouts or emitted code.

Equal closed applications may share semantic evidence, but every surviving
executable boundary occurrence needs its own replayed physical realization.
Eliminating an occurrence requires verified optimization, not a missing report.
See [provider selection](../spec/build/provider_selection.md) and
[boundary realization](../spec/terminal-psi/boundary_calls.md). These are required
guarantees, not blanket claims of generic or native implementation support.

## Freestanding Targets And Hardware Facts

A freestanding target has no operating-system host below it. Its providers may
guarantee hardware behavior: a mapping becomes active, an MMIO access reaches
a device, or an interrupt mask remains in force until restoration. Those accepted
facts remain explicit trust inputs. [Inline assembly](chapter_23_inline_assembly.md)
is an implementation mechanism, not a bypass around their contracts.

Physical arrival is distinct from semantic authority. UEFI supplies an image
handle and system-table pointer; neither is an owned `Extent`. A target-owned
bootstrap obtains admitted image correspondence, establishes separate storage,
accounts for stack and receiver storage, crosses the semantic entry once, and
calls the source continuation with only its declared values.

A returning firmware application and an OS-loader handoff have different
lifecycles. Handoff must account for retry, live Boot Services, final exit, and
surviving storage explicitly; it cannot reinterpret failure as success or loop
without a bound. [Entry roots](../spec/build/entry_roots.md) and
[entry stacks](../spec/resources/entry_stacks.md) own the arrival rules.

Callbacks and interrupts are external roots even without an ordinary Omega
caller. Installation includes their service, trust, state, stack, and custody
demands so they cannot hide behavior outside the ordinary call graph.

### Admitted executable installation

Omega has no general operation converting arbitrary bytes to code. Installation
borrows an admitted immutable artifact and consumes authority over one
destination. Writable placement is frozen, exact final bytes and footprint are
validated, and the provider establishes permissions and instruction-fetch
visibility before entry. The artifact is reusable; destination authority is not.
Replacement also needs quiescence before retirement. Indirect transfers retain
their own entry-reference and descriptor contracts. See
[executable installation](../spec/build/executable_installation.md).

### Build policy and privileged reach

A hosted profile can reject privileged reach; a kernel profile may grant a
small admitted provider set. Approval still supplies neither a resource value
nor its operation-specific qualification. An interrupt-table value does not
authorize installation, and installation authority does not construct the table.
Checked wrappers and hardware roots remain visible to policy.
[Hardware materialization](../spec/build/hardware_materialization.md) and
[interrupt obligations](../spec/build/interrupt_obligations.md) define generic
crossings; OS tables, drivers, and lifecycle protocols remain package-owned.

## Views, domains, and foreign shapes

Foreign APIs use ordinary types, qualifications, and contracts. Text can be
owned, growable, or borrowed bytes with an encoding domain:

```omega
boundary trait CConsole {
    machine write(text: &[u8]::Utf8 & NoNul)
    reaches
        CConsole;
}
```

Length comes from the descriptor. Sequence-wide facts such as UTF-8 validity or
absence of NUL are established once and carried while valid; each use need not
rescan the bytes. [Domains](chapter_8_domains.md) explains establishment.

A semantic view does not choose a foreign ABI. If the public contract leaves
representation choices, the leaf states the counterparty's actual shape and a
checked adapter converts it. Retaining a pointer after return requires backing
custody in a linear protocol, not a bare borrow. `addr` and `Ptr<T>` are inert
tokens; memory access needs an authority-bearing view.

Write-only access lends an already valid region without permission to inspect
its old contents. It is not vacant-storage initialization. The outcome states
the modified prefix and preserves the untouched suffix; opaque compliance needs
admission or enforcement. See [foreign storage](../spec/build/foreign_storage.md)
and [layout/ABI](chapter_20_memory_layout_abi.md).

## Boundary evidence and authority values

A boundary guarantee establishes routed qualification only for the exact subject
and requirement authorized by the domain, with the selected invocation's receipt.
Calling a checked adapter directly does not import that receipt. Predicates can
prove geometry; they cannot manufacture external provenance.

`Extent { base, length }` describes a range, but reconstructing those fields
does not recreate a granted extent. Splitting granted storage consumes the
parent and proves its content equals the separated children while preserving
backing and lineage. Matching byte counts alone prove neither fresh exclusive
issuance nor valid recomposition.

[Authority](../spec/resources/authority.md),
[content custody](../spec/resources/content_custody.md), and
[extents](../spec/resources/extents.md) distinguish installed local roots, provider
issuance, geometry, conservation, and external correspondence. Boundary arguments
must already carry their required qualifications; the call does not establish
them again. Joins and optimization cannot union incompatible histories to invent
authority.

## Boundary Realization Catalog

Checked adapters handle sequences, argument reshaping, caching, and other policy.
A bodyless leaf uses `via` only for a payload its declaration, signature, and
target cannot derive. Binding cases describe an object-format import, a syscall
number, or a validated native-table field.

An import locator keeps its coordinates together: PE name/ordinal, ELF
object/symbol/version, or Mach-O install-name/symbol. They are foreign bytes,
not Omega names or provider keys. Firmware calls name validated fields, not
authored numeric slots. The requirement independently owns the calling plan;
the locator cannot reselect it.

Compiler intrinsics use exact declaration, signature, target, and sealed catalog
identity, not an empty `via` payload or authored intrinsic string. Changing a
foreign binding changes dependent artifact identity and requires relinking and
fresh admission. Build cannot rewrite a selected provider's evaluated locator.
[Foreign bindings](../spec/build/foreign_bindings.md) owns the complete vocabulary,
lifetime applications, and validation.

## Blocking Boundaries

A pipe read can wait for data or closure; a driver can wait for an interrupt.
`blocks` permits occupying the worker but does not prove that either event will
happen. Positive progress needs its own admitted premises. Without finite-wait
evidence, response analysis cannot claim a finite bound.

A pinned UI executor may make a blocking native call. A blocking-executor package
may instead use queues and completion claims. Neither strategy permits killing
an in-process native worker while its custody remains live; bounded recovery
from a genuine hang requires suitable isolation. See
[foreign operational coherence](../spec/build/foreign_bindings.md#operational-coherence)
and [response bounds](../spec/resources/logical_work.md#response-and-physical-time).

## Host vs Standard Library

Typical layering is:

```text
application
  -> optional checked library adapters
    -> boundary requirement
      -> selected provider and calling plan
        -> imported symbol, syscall, or firmware operation
```

The standard library is optional ordinary package code, not a privileged
namespace. Target providers can live in it or separate packages. Static versus
dynamic linkage is different from checked versus admitted supply. Application
authors normally use portable interfaces; provider authors own the target-specific
binding and explicitly expand the boundary base.

## Foreign callbacks through platform adapters

A foreign callback is selected by exact nominal satisfaction, not signature
coincidence or a conveniently named visible machine. This sketch assumes the
named `MicrosoftX64Policy` conformance is in scope:

```omega
boundary trait WindowProcedure:
    Calling<MicrosoftX64, MicrosoftX64Policy>
{
    machine call(
        window: HWnd,
        message: u32,
        word: WParam,
        long: LParam,
    ) -> LResult;
}

boundary machine ApplicationWindow::dispatch(
    window: HWnd,
    message: u32,
    word: WParam,
    long: LParam,
) -> LResult
    satisfies WindowProcedure::call
{
    ...
}
```

The requirement supplies the signature, contract, and inbound calling/state
plan. The registrar's outbound plan is separate. Its static machine binder names
the exact requirement; a signature-free path denoting several overloads rejects.

A direct callback destination is declared at its actual native parameter position:

```omega
machine install<machine Handler>(
    hook: HookKind,
    native callback procedure from Handler,
    module: ModuleHandle,
    thread: ThreadId,
) -> Registration
where machine Handler satisfies HookProcedure::call;

install<ApplicationHook>(hook, module, thread)
```

The call omits `procedure`: no runtime function-pointer value is passed. The
compiler privately realizes the callback and relocation. Calling policy places
that declared entry; it cannot invent a trailing argument or retarget the binder.

A nested destination instead comes from a layout explicitly citing a named
`PrivateCallbackSlot<Requirement>` conformance. Layout owns the offset; the
calling plan maps the binder to the declared private place. Merely declaring
a conformance changes no layout, and private slots are not readable source
fields. [Private callbacks](../spec/build/private_callbacks.md) defines both forms
and their independent occurrence/placement checks.

Code emission is not registration. Build selects realization and resources;
ordinary runtime control calls the registrar. Success establishes a future root
and moves live-registration capacity into the linear registration. Failure creates
no root and preserves that authority. Teardown unregisters and establishes
quiescence before releasing code/component leases; retry preserves outstanding
custody. Capacity counts live registrations, not emitted thunks.

Per-instance state uses an explicit context token, checked generational handle,
or package-owned stable state. No implicit closure environment accompanies the
native address. Retained caller storage follows ordinary foreign-storage rules.

A platform adapter can answer restricted synchronous queries and queue ordinary
notifications for a new activation. It still respects the acyclic `invokes` graph,
executor affinity, and [callback entry-stack contract](../spec/resources/entry_stacks.md#foreign-callback-entry).
A native declaration alone proves no absence of opaque re-entry.

The [native implementation note](../../omega-rust/omega/compiler/native-realization/README.md#callback-custody-boundaries)
separates current materialization support from complete authored-use, registration,
and lifetime custody. A retained callback companion or installed-entry record
alone does not establish that protocol.

## Build Artifacts

Reports keep service reach, authority flow, provider trust, representation,
resources, and executable scope separate. Exact requirement and realization
identities matter; names and compact coordinates explain evidence rather than
granting admission. A transitive provider change can matter even when reach
stays the same.

An opaque binary loaded in-process remains trusted code behind a checked wrapper.
Its known-entry list may be incomplete because it can load or generate code
outside Omega's mediation. An isolated provider has a parent endpoint and a
separately evaluated child scope; isolation, termination, fault containment,
and resource bounds remain independent guarantees.

[Provider trust](../spec/build/provider_selection.md#executable-trust-and-containment)
defines manifest and runtime-admission rules. The useful audit question is not
just “was this package accepted?”, but “which exact claims, capabilities,
realizations, and resource obligations does this artifact depend on?”
