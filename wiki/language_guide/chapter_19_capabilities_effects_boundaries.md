# Chapter 19: Capabilities, Reach, And Boundaries

Omega models host and compiler boundaries explicitly.

> **Service and operational contract model
> ([effects_authority_and_observation.md](../design_briefs/effects_authority_and_observation.md)).**
> `reaches` contains boundary-service reach only. Independent `suspends`,
> `blocks`, and guarded `crashes` clauses publish operational may-ceilings;
> `terminates` remains a
> separate positive progress guarantee. Authority remains capability values,
> trust remains provider receipts, recoverable failure remains sums, mutation
> remains ownership, and resource bounds remain dependent contracts. Lowercase
> implementation vocabulary does not collapse these semantic axes.

The outside world is not one thing. Linux may expose raw syscall numbers,
Darwin normally routes process IO through `libSystem`, Windows imports APIs
from DLLs such as `Kernel32.dll`, Wasm imports host functions, and embedded
targets may jump through firmware tables. The shared concept is not "Unix
syscall." The shared concept is an imported boundary whose implementation is
not Omega code.

## Boundary Surfaces

`boundary` marks a declaration that participates in an audited crossing. The
declared kind identifies what crosses:

| Declaration | Crossing concern |
|---|---|
| boundary machine/operator | control, calling, service reach, and guarantees |
| boundary trait | service requirement and provider realization |
| boundary data | representation |

Supply determines how the crossing is justified: a checked body, requirement,
selected provider, compiler lowering, or accepted declaration. Direction comes
from parameter/result use and provider selection rather than from the
`boundary` modifier.

Core operators keep their public contracts visible while primitive lowering
stays behind the compiler/runtime boundary. Fixed surface tokens resolve to
these named declarations without hiding their signatures or proof obligations.
The literal fixed token appears immediately after `operator`. The declaration
states no provider selection:

```omega
boundary operator [] Slice::index<T>(items: &[T], index: u64) -> T
requires
    index < items.len;

boundary operator [..] Slice::range<T>(items: &[T], start: u64, end: u64) -> &[T]
requires
    start <= end && end <= items.len;
```

Working interpretation:

- `requires` remains a proof obligation for callers.
- `boundary operator` publishes the contract that every selected realization
  must satisfy; it does not name or choose that realization.
- The boundary report records boundary operators, library/authority boundary
  clauses, target policies, and accepted policies.
- Realizations use the same checked-body or bodyless `boundary machine ...
  satisfies ...` supply forms as every other boundary requirement; `via`
  appears only for a binding payload. The target profile or an
  authorized `build.omg` slot override selects one admitted candidate.

Users can inspect `Slice::index` and its proof contract without depending on the
private descriptor, pointer, or code-generation mechanism used after proof.

### Representation TCB and claim admission

A claim-free `boundary data` declaration is representation evidence, not by
itself a proposition, capability establishment, provider guarantee, or service
reach grant. It can still enlarge the code/ABI trusted computing base because
an external realization determines some or all of its representation.

Opacity does not imply one representation source. The source is an independent
part of the declaration's closed application:

- compiler primitives such as `Ptr<T>` derive their runtime shape from pinned
  target semantics, while provider/compiler operations retain the separate
  authority to mint valid pointer occurrences;
- provider-owned runtime values such as `InterruptAcknowledgement` close through
  one selected checked or admitted opaque-representation application;
- a boundary datum used only behind a reference, such as `EfiSystemTable`, does
  not require a by-value pointee representation; and
- proof-erased data such as the temporary `Real` carrier has no runtime
  representation to close.

Representation demand is therefore lazy. A runtime-relevant by-value crossing
requires one exact target-closed application before calling-policy evaluation.
Reference-only and proof-erased uses are complete without one. Absence rejects
only when a use demands closure. An authored selection is nevertheless checked
when selected: duplicate, conflicting, stale, shape-invalid, or lifecycle-
invalid selections reject even if no later by-value use emits a demand row.

Packages declare provider-owned candidates through the ordinary named-
conformance model rather than a new `represents` keyword. Conceptually:

```omega
pub trait OpaqueRepresentation<Opaque> { }

PicAckRepresentation:
    PicAckCarrier satisfies
        OpaqueRepresentation<InterruptAcknowledgement>;

machine build(builder: &mut Build) {
    builder.select_representation<
        InterruptAcknowledgement,
        PicAckRepresentation
    >();
}
```

The compiler-owned trait is recognized by exact identity; a package-authored
lookalike grants nothing. The conformance is inert until selected. Its concrete
subject is load-bearing: the compiler derives the closed shape and physical
movement plan from `PicAckCarrier` rather than accepting authored sizes,
alignments, ABI classes, or numeric representation IDs. The selected v1
lifecycle disposition is explicitly `Inert`. Across every field, array element
type, and sum payload, the closed carrier may contain no independently invoked
cleanup or disposable obligation. The opaque declaration remains the sole
source of semantic multiplicity and discharge. A foreign carrier may supply
the same relationship only through a disclosed admission, not by asserting
that it is cleanup-free. Compiler-sealed generic families such as `Ptr<T>`
resolve from target semantics without package selection.

For affine and linear opaque values, backend copies used for registers, spills,
arguments, returns, and aggregates relocate or place one semantic occurrence;
they do not copy the value. A checked copy of a copyable opaque may create a new
occurrence only when its carrier is itself structurally copyable and inert.
Resource-owning carriers require a separate, versioned lifecycle relationship;
ordinary carrier `drop` is never silently imported through the empty trait.
The compiler enforces this with an exact selected-application receipt derived
from the final build selection and complete carrier graph. An ordinary Psi
entrance cannot assert or synthesize the receipt.

Representation, minting, and authority remain independent. A representation
application states how bits carry a value. A minting route states who may create
a valid occurrence. A domain fact states what one occurrence permits. None
implies either of the others.

The opaque declaration, not its carrier, owns multiplicity and terminal
discharge. A `[linear]` acknowledgement remains legally consumable only through
its named protocol. Its selected carrier may define byte movement and storage
finalization performed inside that valid consumption, but cannot make the value
copyable, droppable, or otherwise create another terminal path.

Package evidence reports availability and demand separately. Producer-owned
availability binds the package-qualified opaque declaration and its ordinary
public conformance/carrier surface. It says that the candidate exists; it does
not claim that the producer accepted any consumer's build selection.

A selecting consumer owns the demand row for each actual runtime by-value use.
That row binds the exact boundary requirement application, opaque declaration,
named conformance or compiler-owned target-semantics source, concrete carrier,
target, closed shape graph, physical movement, role-tagged lifecycle
disposition, representation version and evidence origin, closed-conformance
commitment, and complete boundary-calling-plan commitment. A foreign row
rejoins the producer's exact reviewed declarations and selected immutable source instance.
Checked carrier derivation is recheckable evidence rather than an admission.

Selection is compilation-activation policy. At most one application may be
selected for an opaque declaration in one activation, even when the selection
is never used. An unused selection emits no demand row because it creates no
ABI dependency; a copyable selection still emits its target-independent,
audit-recommended selected copy receipt owned by the selecting package, even
when the opaque declaration belongs to a dependency. Its authored occurrence
remains in build-source custody and it still excludes a second selection. The
receipt binds the schema, named conformance application, `Inert` lifecycle, and
copy disposition, but does not claim target movement. `Unbound` remains a complete
producer report state where no runtime by-value use demands a shape, and
becomes an error only when an active use requires closure.

Dependency build selections are not inherited by a consumer. Dependency
compilation supplies its compiler-issued generated-source bundle, while the
consumer's one authoritative build selects the representation for the active
combined compilation. Earlier package-as-root demand rows remain historical
review evidence and are not unified across a source closure.

The selecting build machine and source occurrence are audit provenance rather
than ABI identity. Agreement uses the exact application and calling-plan
commitments; names, compact report fingerprints, source locations, lockfile
strings, and prose do not establish it. Every producer and consumer of one
runtime value uses the active compilation's same application. Future
independently compiled artifacts must compare the same strong commitments at
each actual by-value composition edge; disjoint artifacts need not agree.

The same exact descriptor is a replacement-facing contract row. An inline
provider may replace independently only when the descriptor matches exactly. A
stable indirect handle may keep its public descriptor while changing backing,
but outstanding non-copy handles pin the era that interprets them. A descriptor
change expands the replacement cohort to every fused producer and consumer,
requires their rebuild, or rejects; a state-migration theorem cannot repair an
ABI mismatch in already-compiled callers.

Initial introduction or material change strongly recommends code/ABI audit but
does not, by opacity alone, create a blocking trust-claim admission. Unchanged
representation rows remain visible without demanding a recurring blanket
approval. A deployment profile may elevate an exact compiler-owned mechanism
to blocking policy when that mechanism is intrinsically dangerous.

Independent facts retain their independent consequences:

- adding an accepted proposition, boundary guarantee, authority
  establishment, provider guarantee, or executable mechanism creates its own
  blocking admission row;
- a breaking public representation/API change may block compatibility policy
  without being mislabeled as an accepted theorem; and
- derived dangerous authority remains subject to dangerous-authority review
  even when it passes through an opaque value.

The absence of a `reaches` row does not make an opaque representation invisible,
and opacity cannot hide authority derived from the operations that consume or
produce it.

## Boundary Traits

A boundary trait names callable behavior whose realization is selected at a
crossing. It is still a trait: callers see machine signatures, requirements,
guarantees, and service reach. The selected realization may be checked Omega code or
an implementation accepted through a host package, target binding, firmware
surface, dynamic loader, or other provider edge.

Static import and runtime loading are distinct. Calling a statically selected
window provider reaches `WindowSystem`; the build's provider graph records that
its implementation came from a DLL. Calling a runtime loader additionally
reaches `DynamicLibraryLoading`, and the loaded realization still requires
provider admission. Reach is complete for every trait: checked bodies infer it,
bodyless surfaces publish it, and callers inherit it. Deployment policy, rather
than a per-trait opt-in, decides which entries are critical.

`boundary` is not a synonym for "reaches a service." These are separate axes:

- `reaches` names what externally visible behavior class can happen.
- `pub` names what package-owned declarations belong to its API surface.
- `boundary` names the crossing whose supply, contracts, and receipts are
  represented explicitly.

Ordinary Omega code can reach services by calling lower boundary surfaces. It
is still proved Omega code. An accepted provider supplies guarantees through a
receipt when its implementation is unavailable as checked Omega source.

```omega
boundary trait Readable {
    machine read(
        path: [u8]::Utf8,
        out: &mut Vec<u8>
    ) -> ReadResult
      suspends;
}
```

Working interpretation:

- `boundary trait` means the machines describe behavior outside proved Omega
  code.
- Each `machine` is a callable boundary surface.
- `requires` clauses are obligations the caller must prove before crossing the
  boundary.
- `ensures` clauses are guarantees accepted from the boundary implementation.
- Boundary-trait identity automatically contributes service reach; a written
  `reaches` clause adds other reachable services. `suspends` and `blocks`
  publish the operation's independent operational ceilings.
- Build policy decides which boundary providers are allowed for a target.
- Safe application packages cannot silently create new host boundaries. A
  provider must come from the toolchain, target configuration, or an explicitly
  whitelisted boundary package.

## Service Bindings

A trait name is an interface identity, never a runtime carrier. Use `dyn Trait`
for local dynamic dispatch. Authority to call a selected boundary service is an
explicit core carrier:

```omega
data Application {
    logging: Service<LoggingService> in Bound;
}
```

`Service<R>` names the stable slot for the exact closed boundary-requirement
application `R`; it does not contain a local conformance table or one provider
era's code address. `Bound` is routed authority established by component
installation/publication. It cannot arise from a record literal, zero
initialization, target bytes, or the mere existence of a provider. An
application root receives the established service explicitly:

```omega
machine Application::start(
    logging: Service<LoggingService> in Bound
) -> Application
{
    Application { logging }
}
```

In a statically fused build the compiler may erase the carrier and dispatch
directly. In an independently emitted build each call acquires exactly one
published era from the stable slot, retains that era until matching leave, and
then releases it. Rebinding changes the slot's current era; it does not rewrite
or reinject every `Service<R>` value.

`Service<R>` is affine by default. Code may move it or lend it by borrow, but
copying call authority requires an explicit owner-authorized duplication route
that returns another established carrier. A service-specific obligation that
must be discharged is represented by an ordinary linear session,
registration, or lease returned by a service operation; multiplicity is never
written on the boundary trait itself. Active calls and era-custodied returned
values may pin one provider era, while the rebindable service carrier does not.

Boundary requirements have an implicit shared service receiver when none is
written. `&mut self` requires exclusive access to the `Service<R>` carrier and
`self` consumes it. This receiver is distinct from the selected provider's
private state receiver.

Composite service authorities take the restrictive meet of their parents'
multiplicities. Projecting from a borrowed composite yields a borrowed service
carrier. Obtaining an owned child consumes and attenuates the composite, with
every omitted linear obligation returned or discharged; projection never
manufactures an owned, copyable call authority.

The current implementation's bare boundary-trait value spelling is a
transitional compatibility fence. It must migrate to `Service<R> in Bound`;
new design and examples must not infer a runtime carrier or its multiplicity
from a trait declaration.

### Local dynamic interfaces over bindings

A local `dyn` descriptor cannot cross a replaceable component boundary: its
table uses within-artifact calling semantics and copied descriptors cannot be
ledgered for unloading. An ordinary local proxy bridges the two mechanisms:

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

Callers own the proxy and derive `&dyn Logger` at the use site. The local
descriptor points to the proxy; the proxy crosses the boundary through
`LoggingService` with its selected `CallPlan`, `StatePlan`, entry contract,
and provider lease. This concentrates the ABI and replacement cost at one
auditable seam.

## Service Reach And Operational Clauses

The source `reaches` row is a `+`-separated ceiling of name-resolved boundary
services. Operational possibilities use their own clauses:

```omega
machine backup(
    src: [u8]::Utf8,
    dst: [u8]::Utf8
) -> BackupResult
  reaches Readable + Queryable;
  suspends;
{
}
```

`suspends;` says the invocation may park its activation. `blocks;` says it may
occupy its worker while waiting. `crashes Cause` publishes guarded no-return
routes. `terminates;` separately
guarantees eventual terminal progress under pinned premises. Service reach
accumulates by row union; suspension and blocking accumulate independently by
boolean may; crash routes compose by predicate substitution and disproof. If
`blocks` is omitted from a public contract, no checked callee or admitted
provider may block a worker. If `Writable` is absent from `reaches`, the machine
cannot reach that service even when it possesses Writable authority.

At a direct call, those two possibilities are acknowledged independently:

```omega
operation();
suspend may_park();
block may_block_worker();
suspend block may_do_either();
```

The prefixes mirror the statically known call envelope exactly. They describe
what may happen, not what must happen, and change neither propagation nor ABI.
Suspension creates a continuation boundary and is therefore restricted to a
complete statement, simple `let` right-hand side, transition subject, or
terminal expression. Blocking-only calls retain the ordinary stack and may
nest. Chapter 18 gives the full call-site rules.

Internal service and operational fields may be inferred. Exports, trait
requirements, and boundary operations publish them; omission means empty
service reach, never suspends, never blocks, or no permission for an omitted
crash cause on the corresponding axis.
Implementations and providers refine each ceiling independently. Imports use
pinned requirement contracts, so later provider selection cannot widen a
compiled consumer.

For a private body, omission of `reaches` requests inference, while an authored
memberless `reaches` clause publishes an explicit empty ceiling. The compiler
retains that authorship independently from the normalized set, so the two forms
cannot collapse merely because both begin with an empty row. Each authored
member is resolved once to its exact boundary trait. Normalization then adds
the services contributed by `invokes` and the transitive parent closure.
Package review points only to authored member occurrences—or to the `reaches`
keyword for an authored empty ceiling. It does not fabricate source coordinates
for inferred members, invocation-contributed services, or parent closure.

One deliberately narrow exception supports installation-bound provider
requirements whose exact service row is selected with the installed
realization:

```omega
pub boundary requirement InterruptAcknowledgement::complete(self)
reaches <= MachineControl + PortIo
requires
    self in InterruptAcknowledgement::Pending;
```

This explicit declaration introduces one abstract row keyed by the exact normalized requirement
path and bounded above by the ordinary `+`-separated service set. A fixed
`reaches MachineControl + PortIo` publishes that union to callers immediately;
`reaches <= MachineControl + PortIo` carries a symbolic exact row inside its
installation closure until selection resolves it to `PortIo`,
`MachineControl`, both, or the empty row. The row may not escape through an
ordinary callable package or component contract. Such a boundary must bind it
first or publish the fixed conservative bound. Manifests expose every
unresolved row and bound, and final admission rejects an unresolved row.

The form adds no Boolean reach algebra. `+` remains idempotent set union and
`<=` is the subset bound; negation, subtraction, lower bounds, and exclusive-or
do not exist. Provider-choice restrictions belong to provider admission, not
the reach row. Separate requirements receive separate abstract rows. An
installed-root receipt relates an interrupt entry to its completion provider;
equal service rows would neither establish provider identity nor prove token
lineage.

No masking, subtraction, scoped allowance, or algebraic handlers exist. A
checked in-memory Readable provider can remove a trust receipt and refine
operational behavior, but the abstract Readable reach remains visible. Omega
has no quantitative service members: heap/region bounds use capability contracts;
task and version capacity use their own declared budgets.

The complete laws, algebra, identity rule, tests, and deferred spaces are in
the service/operation contract brief.

## Service Identities And Inference

Each `reaches` member resolves to a boundary-trait identity. There is no global
standard-service vocabulary or numeric reach bitset. `Console`,
`FilesystemHost`, `Arena`, `MachineControl`, and application-defined boundary
traits all enter the same symbol-resolved service-row model. Boundary-trait
inheritance contributes parent closure.

Purity on this axis is an empty service row. Possible suspension, blocking, and
crashing are independent: their clauses publish separate may-ceilings, and
private bodies infer them. Empty service reach plus no operational possibility
still does not prove termination, absence of recoverable failure, absence of
authority use, or absence of owned-state mutation.

Declared reach rows are ceilings. A trait can say "any implementation of this
machine may reach at most these services." A concrete machine may declare the
same set or a smaller set, because some providers reach fewer services on a
given target. It may not declare a new service outside the trait requirement.

For a bounded installation row, the written set is the preselection upper
bound rather than the selected row. The selected provider's published row must
be a subset, and that exact row replaces the symbolic row in the final installed
closure. This preserves the ordinary pinned-import rule: only the explicitly
installation-bound closure may remain symbolic, and no later provider may
widen either its recorded bound or an ordinary compiled consumer.

### Declared service reach and installed terminal authority

Service reach and terminal authority are separate, joined review axes.
`reaches` is the portable, package-stable statement of which abstract boundary
services a callable may exercise. It remains meaningful before a target or
provider is selected and therefore remains the authority view used to review a
dependency as source. It is not a complete claim about which physical host
mechanisms one installed artifact will execute.

Installation derives that second view from the complete selected-provider
closure. Traversal continues through checked adapters and selected providers
until it reaches exact terminal mechanisms. Checked adapters are composites,
not leaves; cycles, missing leaves, duplicates, substitutions, and unclassified
physical operations reject.

Each leaf uses one role of a closed post-normalization sum: a structural
compiler-intrinsic execution identity; target ABI, syscall number, and checked
argument contract; normalized foreign locator and admitted implementation
contract; exact firmware/table identity and receiver contract; or exact checked
physical-operation catalog entry. The role discriminant and its complete
payload enter identity. The same integer or slot number on different targets
is not one mechanism. Provider context and service schema remain join inputs,
not fields that can change a physical leaf's classification.

The two views meet through accepted target policy, not through spelling:

```text
exact service identity + normalized schema
    -> permitted terminal-authority classes

exact post-normalization terminal-mechanism identity
    -> exercised terminal-authority classes

exercised terminal authority <= permitted terminal authority
```

The current first implementation rung applies this model to the closed
compiler-intrinsic family. Native realization classifies every demanded
compiler builtin under an explicitly supplied receiving policy and retains the
accepted policy version and commitment in the native artifact's strong
identity. Replaying against a different accepted policy fails. This is not yet
the service/schema containment join and grants no provider-execution admission.

Direct normalized foreign imports enter that same versioned policy through a
distinct `TerminalMechanismIdentity` role (introduced in version 2; version 3
adds the Linux write-byte ProcessOutput row). Each explicit row binds the exact
target and collision-resistant normalized locator identity to the strong
contract commitment of its canonical admitted `BoundaryEntryPlan`, never to a
provider report fingerprint. Native realization classifies every directly
demanded PE-by-name, PE-by-ordinal, versioned-ELF, or Mach-O import before
provider settlement. Missing or duplicate rows, locator or contract
substitution, wrong target, duplicate selected/external rows, and legacy
string-backed imports reject. The native artifact retains the complete policy
identity. Complete selected-provider-closure traversal and the service/schema
permission join remain separate, later work; this rung does not grant provider
execution or same-stack custody.

Review reports an excess as an explicit containment failure, and accepted
realization rejects it. For example, a filesystem provider whose selected
Linux binding reaches a process-execution syscall does not become safe because
it satisfies a filesystem requirement. The receiving D41 interpreter or
lowerer owns the versioned target policy and independently accepts or rejects
the producer's realization proposal. Its accepted policy commitment enters
realization evidence; inability to realize does not invalidate source or
target-neutral Terminal Psi.

Target policy may be partial over all possible operating-system coordinates,
but it is demand-complete for an accepted artifact: every demanded leaf has
exactly one row. Known authority-free mechanisms have explicit empty rows or
an exhaustive empty disposition for a closed structural family. Missing,
unknown, duplicate, or wildcard-default rows reject. Empty means only that the
mechanism exercises no class in this dangerous-authority vocabulary; it does
not prove purity, absence of other side effects, foreign-code trust, or
provider custody. Risk labels remain review metadata and package source cannot
mint them.

A row publishes the union of every authority reachable through its argument
values. It may narrow only when retained compiler-checked constants, ranges,
handle provenance, or another exact constraint proof excludes the broader
behavior, and that constraint identity enters the mechanism key. A service or
provider method named `open_read` proves no flag restriction by itself.

The closed terminal-class vocabulary distinguishes filesystem content read,
filesystem content write, filesystem metadata query, directory enumeration,
filesystem namespace mutation, filesystem metadata mutation, process output,
process termination, machine control, port I/O, interrupt control, interrupt
entry, and root-memory access. Exact service and mechanism identities remain
beside these grouping classes. Older broad `Filesystem` and `Process` risk
labels are transitional review summaries, not terminal-policy identities.

The temporary string-backed import bridge must normalize losslessly to the
same exact foreign-locator identity as ordinary imports before classification.
It is not a second durable terminal-root vocabulary. The normalized locator
sum must cover PE-by-name, PE-by-ordinal, versioned ELF, and Mach-O before
Darwin imports can enter installed-authority review. Source binding evaluation
produces that structural locator before the historical package-review
classifier keyed by blessed `(filename, trait-name)` pairs is replaced by the
exact service/schema permission table and binding-derived containment;
extending the transitional filename table for new service facets is forbidden.

Portable requirements do not enumerate operating systems. A Linux syscall, a
Windows import, and a firmware-table call may separately satisfy the same exact
portable boundary requirement. The `+` operator in `reaches` remains set union,
not provider choice or exclusive "one of" syntax.

Filesystem reach is faceted by operation authority rather than collapsed into
one `FilesystemHost` class. The portable minimum distinguishes content read,
content write, metadata query, directory enumeration, namespace mutation, and
metadata mutation. Exact requirement/method identity remains in evidence
alongside the facet; facets group authority without erasing which operation was
selected. An operation controlled by runtime flags must publish the
conservative union of every facet those flags can enable. Splitting the service
surface improves precision only when the selected lowering also pins or proves
the narrower argument contract.

These facets constrain which operations code may name, not which filesystem
objects it may touch. Today's raw integer descriptors are forgeable and carry
no read/write provenance, so evidence may truthfully say "may perform content
writes" but not "may write only files opened for writing." Object-level
confinement requires typed, unforgeable authority-bearing handles whose
attenuation and accepted operations are checked separately.

```omega
boundary trait Console {
    machine write_line(text: &[u8])
    reaches
        Console;
}

machine Console::write_line(text: &[u8])
reaches
    Console
{
    HostConsole::write_line(text);
}

// A host provider, not normal application code, binds HostConsole to Darwin
// libSystem, Linux syscalls, Windows APIs, firmware, or a test harness.
```

In that shape, `Console::write_line` is ordinary Omega standard-library code.
It is statically linkable and proof-checked like any other machine. The
boundary is the lower `HostConsole` provider edge where the implementation is a
syscall, imported symbol, firmware call, loader hook, or boundary test surface.

Service reach propagates through the call graph. The compiler computes direct
and transitive canonical rows for each machine, state, and call.

Reach declarations are policy surfaces, not required noise on every machine:

- Boundary requirements always publish reach. Their own boundary-trait identity
  is implicit; a written `reaches` row names additional services reachable
  through the requirement.
- Exported library APIs publish reach. Omission means empty reach, keeping the
  public contract stable and preventing an implementation from unexpectedly
  growing filesystem, network, process, dynamic-link, or other host behavior.
- Private/internal machines may omit `reaches`. The compiler infers and reports
  their reached services from their bodies and callees.
- Executable entry points may omit `reaches` in normal development builds. The
  final executable manifest still records the union of services reachable from
  the entry point so an OS, loader, store, or build policy can prompt, deny, or
  audit the requested behavior classes and authority flows.

When a private concrete machine declares a `reaches` block, that block is a
ceiling for the machine's reached services. Omitting the block means "infer and
report this machine's reach." On a published surface, omission means the strict
empty ceiling. Declaring the block always means "this machine must not reach
anything outside this set."

```text
start
  declared: <none>
  direct:   <none>
  reached:  Console

executable manifest:
  Console

Grep::search
  declared: Filesystem
  direct:   <none>
  reached:  Filesystem
```

A stricter release, OS, or audited build can require an explicit checked-in
reach and authority manifest for executable entry points. That requirement
belongs to build policy. It does not mean ordinary application authors must
manually thread every reached service through `main` while iterating locally.

Rows use compact interned identities and deterministic normalized sets, while
source, diagnostics, and manifests render canonical trait names. Provider
metadata records whether a `Console` implementation uses Darwin `libSystem`,
Linux syscalls, Windows APIs, firmware, or a test harness; that implementation
detail does not create a second service taxonomy.

## Synchronous Boundary Invocation

`reaches` is deliberately transitive and trait-granular. It is the stable
authorization and audit ceiling, not a synchronous call graph. Machines use a
separate `invokes` clause to publish the boundary bindings they may enter
before returning:

```omega
boundary trait EventSource {
    machine register_and_fire(handler: Handler) -> Registration
    invokes handler;
}
```

The clause is a may-ceiling: an execution need not call `handler`. If it does,
the call occurs synchronously within the current invocation. The handler
trait and the selected conformance's realized operational envelope contribute
automatically to the current invocation's reach.

Bodyful machines infer `invokes` from their checked bodies, including
forwarding through local helpers. Bodyless requirements declare it; omission
means no synchronous invocation. Parameter paths distinguish two values of the
same boundary trait. Internally selected bindings may be named by their trait
identity when no parameter path exists.

Typed lowering resolves every authored `invokes` target once, to either an
exact non-`self` parameter symbol and ordinal or an exact boundary-trait symbol.
Later effect inference and package review consume that retained identity; they
do not search for a same-spelled trait again. Compiler-issued review also keeps
the exact authored target-name span beside that identity, joins top-level
machines to the checked invocation plan, and fails closed on missing,
duplicated, stale, or malformed custody. The coordinate explains the row and
does not enter its semantic compatibility bytes.

Moving a binding into a linear registration has different timing:

```omega
boundary trait EventSource {
    machine register(handler: Handler) -> Registration;
}
```

The registration establishes an independently entered external root and adds
no synchronous edge to the registration call. The root may run later or
concurrently once established; that timing does not nest its stack beneath the
registration invocation. Establishment requires the root-admission policy to
permit the concrete handler envelope. The registration value retains that
selected conformance and envelope in compiler-tracked claim metadata, rather
than widening to the trait ceiling. An operation that also enters the handler
on its current call chain declares `invokes handler`.

The realized synchronous invocation graph across component boundaries must be
acyclic. Cycle checking uses `invokes`, never the transitive `reaches` closure.
Queues, mailboxes, scheduler handoffs, and other new-activation boundaries
break cycles structurally; adding another synchronous trait does not.

Console boundaries should use the same shape:

```omega
// The result of a byte-level read. `Eof` is ordinal 0: a zero-initialized
// ByteRead IS end-of-input—ZII, with no sentinel value.
data ByteRead {
    case Eof;
    case Byte(value: i32 [0..=255]);
}

boundary trait Console {
    machine write(text: &[u8])
    reaches
        Console;

    machine write_line(text: &[u8])
    reaches
        Console;

    machine read_line(out: &write [u8])
    reaches
        Console;

    machine read_byte() -> ByteRead
    reaches
        Console;

    machine write_byte(byte: i32)
    reaches
        Console;

    machine exit_process(code: i32)
    reaches
        Console;
}
```

The current `exit_process` spelling is a migration surface whose selected
native realization is physically nonreturning. D39 does not infer successful
external termination from the method name, its Unit result, a syscall number,
or backend convention. A complete language contract must attach one explicit
checked terminal-effect completion identity to the boundary operation and
retain it through Terminal and provider selection. Until that carrier lands,
the existing lowering is implementation evidence rather than reusable
`TerminalTraceV1` termination authority.

The write-only destination is an existing valid byte slice whose prior
contents the provider is not authorized to inspect. A checked provider is
verified transitively against that restriction. An opaque selected provider
admits compliance unless its target isolation enforces it physically. The
operation's outcome separately states the exact modified prefix; the untouched
suffix remains unchanged. This is neither vacant-storage initialization nor a
typed output parameter.

The byte ops are the universal filter surface (`stdin_checksum` and its
siblings): `read_byte` yields each raw byte as `ByteRead::Byte { value }`
and `ByteRead::Eof` at end-of-input -- the payload's declared `[0..=255]`
range is construction-enforced, so downstream arithmetic gets honest facts
for free, and native lowerings exploit the ZII rule directly (the result
slot is pre-zeroed; only an arrived byte writes the non-zero tag, so the
EOF path executes no write at all).

Each hosted target selects one complete Console provider. Friendly sequence
operations remain checked Omega adapters over the provider's byte operations;
compiler-owned leaves must name an exact registered requirement lowering. An
owned read destination derives its capacity from the actual place, and bounded
text writes must prove they fit that capacity.

Domain requirements stay normal proof language. A filesystem boundary should
not invent special "initialized" words when a domain is what it means:

```omega
domain [u8]::NonEmpty
    requires self.len > 0;

boundary trait Filesystem {
    machine open(path: &[u8]::NonEmpty)
    reaches
        Filesystem;
}
```

The same rule applies to text encodings and ABI string constraints. Instead of
growing separate surface types such as `CString`, `OsString`, or
`Utf16String`, a boundary should usually ask for the string domains it
actually needs:

```omega
boundary trait CConsole {
    machine write(text: &[u8]::Utf8 & NoNul)
    reaches
        CConsole;
}
```

That keeps encoding and interop requirements inside Omega's ordinary domain
system. The byte slice is the borrowed window passed across the boundary; no
separate `string` view type is required.

Text measures and text domains split by cost:

- `length` and `non_empty` are exposed first. They are cheap, O(1) facts read
  from the `{ptr,len}` descriptor.
- `no_nul` and `utf8` are domains established at a validating boundary
  constructor. The sequence-wide fact is asserted once at construction, then
  carried as a fact and never re-proved per use.

Establishing `no_nul` or `utf8` once at the validating constructor is the
decided answer to the cost of sequence-wide proofs: common text handling
downstream reads the carried fact instead of re-scanning the byte sequence.

## Capabilities And Authority Flow

Reach declarations are not authority by themselves. `Readable` or `Writable` says the
corresponding service surface may be reached, but it does not say whether the
code was handed a folder by the caller, prompted the user, stored a handle for
later, or merely derived a narrower file handle from a folder it already had.
The row is a ceiling; capability values are possession; provider receipts are
trust. None can substitute for another.

Omega should model authority as ordinary values plus facts. A filesystem handle
should usually be one stable type with permission domains, not a family of
separate permission-flavored types:

```omega
data Folder {
}

domain Folder::Readable
established by Desktop::choose_readable_folder;

domain Folder::Writable
established by Desktop::choose_folder;

domain Folder::ReadWrite
    requires self in Folder::Readable
          && self in Folder::Writable;
```

Boundary and standard-library APIs then state normal requirements and
guarantees:

```omega
boundary trait Desktop {
    machine choose_readable_folder(prompt: &[u8]::Utf8) -> Folder::Readable;
    machine choose_folder(prompt: &[u8]::Utf8) -> Folder::Writable;
}

boundary trait Writable {
    machine write_bytes(folder: Folder, path: &[u8]::Path, bytes: &[u8])
    requires
        folder in Folder::Writable;
}

boundary trait Readable {
    machine read_bytes(folder: Folder, path: &[u8]::Path, out: &mut Vec<u8>)
    requires
        folder in Folder::Readable;
}

boundary trait Filesystem: Readable + Writable {
}
```

This should not require new source keywords such as `uses capability` or
`acquires capability`. The compiler can infer authority flow from types,
domains, call contracts, returns, stores, drops, and boundary provenance.

Important report verbs:

- Accepts: authority enters through parameters or machine-owned fields.
- Uses: an operation requires authority facts such as `folder in
  Folder::Writable`.
- Returns: authority leaves through a return value or output parameter.
- Stores: authority is retained beyond the current call.
- Acquires: fresh authority is minted by a boundary, host prompt, ambient host
  surface, package permission grant, loader, or OS/runtime broker.
- Derives: a narrower or related authority is produced from an existing
  authority, such as `Folder::Writable -> File::Writable`.
- Releases: an authority is closed, dropped, revoked, or otherwise ended by the
  code.

`derives` is intentionally separate from `acquires`. Opening a file inside a
caller-provided folder is a sub-capability operation. It expands the set of
values flowing through the program, but it does not independently obtain new
host authority.

Example ordinary use:

```omega
machine Thumbnailer::write_cache(
    cache: Folder,
    image: Image
)
requires
    cache in Folder::Writable
reaches
    Writable
{
    Filesystem::write_bytes(cache, "thumb.bin", image.thumbnail_bytes());
}
```

Expected package report shape:

```text
authority flow:
  accepts: Folder where Folder::Writable
  uses: Folder::Writable
  derives: none
  stores: none
  acquires: none
  returns: none
  releases: none

service reach:
  Writable
```

Example acquisition:

```omega
machine Thumbnailer::choose_and_write_cache(image: Image)
reaches
    Desktop + Writable
{
    let cache: Folder = Desktop::choose_folder("Choose cache folder");
    Filesystem::write_bytes(cache, "thumb.bin", image.thumbnail_bytes());
}
```

Expected report shape:

```text
authority flow:
  accepts: none
  uses: Folder::Writable
  derives: none
  stores: none
  acquires: Folder::Writable via Desktop::choose_folder
  returns: none
  releases: none

service reach:
  Desktop, Writable
```

Package and build policy should be able to set ceilings over this inferred
flow. A package may be allowed to reach `Writable` only through
caller-provided folders, while being forbidden from acquiring a folder through
`Desktop::choose_folder` or opening an ambient absolute path.

Authority flow and boundary calls are related but separate reports:

- Authority flow answers what power-bearing values a package can accept, use,
  derive, store, return, release, or acquire.
- Service reach answers which abstract boundary traits the package directly or
  transitively reaches. Provider receipts separately answer which host,
  runtime, compiler, syscall, imported library, broker, or prompt realizations
  were selected.

A library can therefore be audited along three axes:

- Service/operational ceiling: which service surfaces may be reached and
  whether execution may suspend or block.
- Authority-flow ceiling: what authority values may move through or be minted
  by the package.
- Provider/trust ceiling: which direct and transitive realizations are allowed.

This distinction matters because two packages can both reach `Writable`
while having very different blast radii. One only writes into a folder supplied
by the caller. The other prompts the user, consults the environment, or calls a
raw host provider to acquire filesystem authority itself.

### Package builds

`build.omg` is an ordinary checked Omega program run with compiler-issued,
package-scoped Build facets. It has no ambient filesystem, network, process,
signing, secret, package-acceptance, or standard-library authority. `BuildSource`
may observe only the resolver-published, content-verified source snapshot,
`BuildOutput` may mutate
only the sponsored staging tree and publish explicit generated-source handoffs,
and `BuildLog` emits captured build observations. These facets are part of the
compiler build protocol and remain available in a freestanding toolchain with
no standard library. Before evaluation, their normalized effect demand gives
policy the static ceiling; after evaluation, receipts record realized
observations and outputs. The filesystem sponsor, not a package role or service
name, enforces the physical grants.

Only the canonical free build root initially receives `&mut Build`. It may lend
that activation to ordinary helpers, but delegation does not narrow audit scope:
their complete transitive reach, invocation, suspension, blocking, termination,
authority demand, Build-facet effects, and observations compose into the root.
A helper cannot turn an ordinary runtime `FilesystemHost` or `Console` boundary
into a build service; such a reach remains outside the compiler-owned build
protocol and rejects unless a future explicit host-service mechanism is
separately designed. A scoped name or receiver never grants build authority.

Dependency retrieval is a resolver operation performed before downloaded code
runs. The host-selected Git/SSH stack owns transport and credential authority;
the resolver owns the closed source protocol surface, archive reading,
expansion limits, path containment, content-verified object graph, and immutable
destination publication. A dependency's `build.omg` never inherits transport
or resolver authority. Imported boundary claims are inert until root policy
admits the compiler-derived complete claim set; any claim change appears in the
lock/review diff.

That acceptance records a root policy decision, not proof that an audit
occurred. Omega can derive bounded capability, authority-flow, provider,
representation-TCB, proof-status, and provenance rows and can recommend or
require review according to policy. It cannot establish that a human or LLM
understood the source or made a sound security judgment. Even a signature says
only who controlled a key, and a proof certificate establishes only its exact
mechanically checked proposition. Projects requiring stronger assurance enforce
their own reviewers, quorum, isolated builds, bootstrapped toolchain, and merge
controls around these deterministic facts.

For a callable with an actual checked source body, “realized reach” means the
compiler's exact inferred transitive body row, never the authored public
ceiling. Its separately retained concrete row is the preselection body base:
it excludes authority contributed only by unresolved installation bounds and
does not claim that a final provider was selected. Bodyless boundary, accepted, requirement, and external supply instead
carry an explicit no-checked-body disposition. A compiler-classified dangerous
service that is declared by a checked body but absent from that inferred row is
reported as audit-recommended contract slack, keyed by the exact callable and
service. Bodyless supply and package-authored lookalike services do not acquire
that classification. Package review rejects impossible internal combinations:
checked supply requires a body, accepted/requirement/external supply forbids
one, and boundary supply alone permits either form.

The ordinary rows are derived from the earliest coherent compiler-owned
representation in which each fact is semantically established. Exact
structural identity may come from private pre-Psi typed or resolved state, then
join checked acceptance, effects, proofs, and realization from the stage that
establishes them after successful compilation. Rows may use different private
representations; totality is required of the final projection, not of one
nominal intermediate stage. Unresolved or unprojectable required facts reject;
the compiler does not serialize raw internal IR or fill gaps with a “complete
enough” marker. Terminal Psi is required separately when a row claims
a checked property of final executable realization, lowering, ABI realization,
or fixed native resources, and when a hardened profile explicitly requests that
evidence. Opaque executable supply may remain an explicit trust/TCB row making
no Terminal claim. Terminal evidence is not a blanket prerequisite for checked
reach and authority admission. The package checker moves with the private
representations it consumes; their instability is internal compiler coupling,
not a package-format promise. It does not require a nominal report-only stage
unless independent semantics, shared invariants, transformations, or consumers
later establish one. Psi may repeat an invariant as a downstream backstop
without requiring package admission to reconstruct an already-settled earlier
fact from Psi.

Generated Omega source carries no build authority into the resulting program.
It is checked under the consuming artifact's ordinary runtime reach, crash,
work, conservation, and trust ceilings. Standard release-capable build
providers are hermetic or return replay receipts for every observation;
volatile providers are explicit development policy and cannot produce a
source-rebuildable release.

Target metadata such as library artifact, foreign symbol, syscall number,
calling convention, and realization binding belongs in toolchain target
packages or explicitly admitted provider packages. Pulling in `Filesystem`,
`Console`, or `ProcessExit` service reach is visible to the build; provider
receipts reveal which realization supplies it, and a restricted build can
reject either axis.

The compiler should understand boundary traits, provider packages, libraries,
symbols, calling conventions, boundary providers, and target image imports
generically. It should not special-case every Windows, Darwin, Linux, or SDK
API.

### Origin and custody

A claim records two independent provenance facts:

- **origin** answers where the claim first came from and remains audit history;
- **custodian** answers which owner currently gives the claim meaning and must
  remain available for reclamation.

Custody follows establishment, not representation. A transparent
`Transaction { slot: u64 }` may still be custodied by the provider era whose
session table interprets `slot`. Moves, returns, and stores preserve custody;
consumption discharges it.

Fresh claims established by a component default to that component era as
custodian. Custody can change only through a checked transfer to a named
receiver:

```omega
machine StableLedger::adopt(
    &mut self,
    transaction: Transaction in Live
) -> adopted: Transaction in Live
    ensures custodied_by(adopted, self);
```

`custodied_by` is a proof relation, not a freely assertable fact. A checked body
must prove the transfer and acknowledgment; a boundary implementation must
return an admitted receipt naming the receiver and subject. A written
postcondition creates an obligation and is never evidence by itself.

Multiplicity is independent. A copyable historical fact may need no discharge;
a linear session does. A copyable boundary binding names a current provider
slot and does not retain an old era. A session claim custodied by an old era
does retain it until transfer or discharge.

Long-lived old-era custody is sound but can delay deployment indefinitely.
Replacement reports therefore name retention edges and, where root metadata
permits, their holding paths such as `Cache.sessions[14]`. Deployment policy
may reject or require adoption of those edges without introducing a
`stable(custodian)` type predicate.

## Host Providers

Some targets do not need a named user-mode library for the lowest boundary.
Linux can expose a target syscall surface directly. That mapping is a
derived `ProviderPlan` for a boundary trait, not a different user-facing
callable concept. There is no `provides` declaration keyword and no authored
row-builder API.

The legacy `library "..." calling_convention ... { entry ... }` block and its
trailing `boundary host` / `boundary Name` levels are retired. So are trailing
boundary-level clauses on machines, capabilities, or requirements. Portable
identity belongs to the exact boundary trait requirement; realization and ABI
belong to the selected provider, evaluated `Binding`, and calling plan; build
admission approves that exact closure. Accepting a boundary-level word and then
discarding its host/name distinction during lowering is never a compatibility
strategy.

The target's core/std package declares boundary leaf machines satisfying the raw syscall
requirements `via Binding::Syscall { ... }` and ordinary checked adapter
machines satisfying Console. The compiler derives their normalized plan from
the explicit conformance closure, validates it, admits it with trust receipts,
and selects its provider type for the Console slot. `build.omg` normally
selects the target package's default provider set; a test harness or component
manager holding selection authority may substitute a different admitted
provider for an individual slot. Defaults are target-package declarations,
not compiler tables.

At the build root, an override is explicit and type-per-slot:

```omega
machine build(builder: &mut Build) {
    builder.select_provider<selected_target::Console, TestConsole>();
}
```

The build declaration can select only a complete candidate already present in
the loaded dependency closure and applicable to the selected target. It does
not append rows, admit a candidate, or widen the requirement's reach. The
boundary trait and provider type paths each resolve to one exact nominal
identity. There is no leaf-name fallback after ordinary name resolution.
Alternate spellings of the same exact pair cannot select the slot twice.

A generic provider requirement is still one selected slot. For example,
`ResidentContentTransfer<P, T>` does not create one independently selectable
slot for every application. Concrete artifacts retain their exact normalized
applications; separately compiled generic libraries export symbolic
applications. Final composition substitutes the reachable arguments, derives
the closed demanded set, and verifies that the selected provider covers every
application before installation binds its exact issuance occurrences. Only a
requirement whose distinct applications genuinely need different providers
declares an indexed slot family.

Every executable artifact closes that finite exact application set. This is
independent of whether a realization is generic: universal checking can prove
that one source template is semantically selectable for a whole telescope, but
it does not monomorphize an application or establish its representation,
layout, `Calling<C>` plan, register classes, stack placement, target admissions,
or emitted code. The production first rung is consequently exact-only. Generic
requirements in core do not themselves justify a generic-coverage row, and no
checked generic operator realization currently supplies one.

The exact set begins at checked uses, not at provider assertions. Each use
retains an ordered application against the selected requirement telescope.
Application arguments are structurally tagged by category; binder owner,
category, and ordinal are identity, while binder names and source spellings are
not. Production currently admits type arguments and const arguments. A const
argument is the canonical evaluated value in its declared carrier, so `2 + 2`
and `4` select the same `4 : u64` application. Lifetime, static-machine, and
proposition arguments remain fail-closed until their operator-specific
substitution and replay rules exist.

An application in still-generic code may refer to the enclosing artifact's
typed binders. That is a symbolic demand, not coverage. Final composition
substitutes reachable specialization arguments and publishes coverage only
after every argument is closed and validated against its category, carrier,
domain, bounds, and `where` requirements. Equal closed applications deduplicate
only after their selected-plan, realization, semantic, target, and admission
joins succeed. Checked source-use coordinates remain independent provenance
even when they share one semantic coverage row. D32 separately requires one
physical child for each boundary-operation occurrence that survives verified
optimization.

The first package-exported symbolic form is deliberately narrow: a public
generic callable may map each named boundary operator type binder directly to
one of its own type binders. The package row retains both package-qualified
declarations and the binder ordinals. It remains a blocking composition input,
not a selected-provider or realization claim. Nested symbolic types and other
static categories remain unsupported until their final substitution and
recheck are implemented end to end.

For a checked generic body, the compiler creates an ordinary authoritative
machine specialization per distinct closed application and rechecks the
substituted signature, contracts, effects, target restrictions, admissions,
selected provider plan, and semantic realization. Terminal retains the exact
operator occurrence, requirement, and tagged application as a demand. The
bound realization companion carries the strong selected-plan identity and a
role-tagged realization payload. The closed roles are specialized checked
body, nongeneric checked body, exact compiler intrinsic, and externally
admitted concrete authority. The role discriminant is part of canonical
identity; role-specific fields are not optional members of one common payload.

A boundary operator with a static telescope of length zero has one canonical
empty application. It performs no substitution but still rejoins its exact
selected plan and realization. An ordinary boundary-trait machine has no
telescope construct and never receives that application. Bodyless, opaque,
external, and separately supplied realizations
cannot borrow a checked template's authority; they require exact admitted
authority for every application. Zero-commitment bootstrap lowering is not a
coverage role. A builtin fallback that selects no boundary requirement emits
no row, while an authoritative artifact rejects a demanded boundary operation
that has not migrated from bootstrap lowering to a selected realization.

Terminal coverage establishes meaning, not emitted execution. An early
representation, layout, access, or calling plan may join that row only when its
carrier and validator belong at `representations` rank or below and the fact
is reconstructible before backend assignment or emission. Merely moving a
backend-shaped carrier downward does not satisfy that rule. Assigned registers
and stack homes, final call placement, relocation, and emitted bytes remain
physical facts owned by native realization.

Optimization does not rewrite the canonical Terminal artifact. Its validated
projection retains the canonical `TerminalPsiIdentity` and identifies the
boundary occurrences that survive as executable operations. Every such
occurrence has exactly one native physical child. Its role-tagged
`PhysicalChildParent` is either an `OperatorApplicationCoverageRef` to
reconstructible D29 coverage or a retained-and-replayed D41
`BoundaryTraitSettlement`. The D41 branch reuses `BoundaryExecutionBinding`;
the role is not duplicated. Equal D29 applications may share one semantic
parent but never one physical child. The child additionally binds its distinct
optimized-operation identity and retains target lowering, instruction
selection, assignment, relocation, and emitted-byte-span joins. Native replay
rejects a missing, duplicate, stale, substituted, padded, or role-swapped
child. It permits omission only when the verified optimization proof
establishes elimination; without optimization the projection is the identity
projection.

Package review may publish D29 semantic coverage without physical facts.
Native or external execution authority additionally requires the physical
child and complete set correspondence above. Housing both in one
`NativeArtifact` envelope does not merge their evidence classes or replay
rules.

A future universal row may be compiler-issued only for an exact checked Omega
body validated on the pristine symbolic graph. Its typed telescope retains
binder categories, declared domains and bounds, `where` requirements, and an
exact requirement-to-realization binder mapping. The requirement domain must
imply the realization domain; implementations may not collapse independent
binders or narrow the accepted set. Its plan claim covers symbolic provider
routing and dispatch only. Bodyless, external, opaque, compiler-intrinsic, and
separately supplied realizations cannot acquire universal coverage from an
authored claim and remain exact-only. A foreign universal contract would need
a distinct independently recheckable verifier.

The former non-authorizing indexed-application scaffold is retired by D35. Its
arity/string schema and provider assertion were never D29 evidence: equality
between an authored claim and compiler demand does not establish a checked
realization. Compiler-derived tagged applications are demands. Coverage exists
only after the compiler independently rejoins and rechecks the selected role-
specific realization. Until that join lands, generic provider-family review
fails closed. A boundary-operator telescope of length zero is one canonical
empty application, not proof of coverage; absence of a telescope on another
declaration form is not that value. Application evidence alone grants no
resident-content, transfer, or installation authority.

Native realization also retains the exact nonzero selected-closure identity
beside its source-free provider-plan projection. Component-candidate replay
requires both to match independently, preventing selected-closure or resolved-
reach drift from hiding behind unchanged plan rows without treating either
identity as authority.

This is the same proof shape as a library import:

- Omega proves caller-side type and state invariants.
- The imported boundary is accepted to satisfy its declared guarantees.
- An undiscoverable locator is authored as a compile-time `Binding` value on a
  `via` declaration; a compiler intrinsic is derived from exact realization
  identity instead. Either route is retained on the exact realization row.
- The build artifact records the exact selected realization, normalized
  binding, admission receipt, and provider-plan identity.

An accepted bodyless boundary guarantee is therefore a separate blocking trust
row in package review, not merely another callable API shape. Initial admission
and a newly introduced package require an exact root-policy decision for that
row. An unchanged accepted baseline remains visible without a recurring
blanket-approval prompt. A claim-free boundary declaration emits no such row.

Bodyless `boundary machine ... satisfies ...` declarations are the external-
provider supply form. `via` appears only when it carries an undiscoverable
payload such as a syscall number, DLL locator, or validated foreign-table
field. Compiler and instruction intrinsics are found from exact declaration,
signature, and target identity without an empty binding clause. Sequences,
argument reshaping, newline policy, caching, and other composition are normal
checked Omega machines. The satisfied requirement contributes the public
service-reach, suspension, blocking, and guarded-crash ceilings, while the
binding/provider contract supplies behavior that must refine each of them.
Trust is assigned at admission rather than selected by source spelling.

For review explanation, the compiler retains the exact authored `suspends` and
`blocks` keyword locations separately from the operational ceiling. Omitted or
inferred clauses receive no invented source location, and stale or missing
source custody rejects review. These remain may-ceilings: a public machine that
declares `suspends` is reviewed as permitting suspension even if its current
body happens not to suspend.

## Freestanding Targets And Hardware Facts

A hosted target's lowest boundary is an operating system. A FREESTANDING
target (ring 0, kernel, firmware payload) has no host below it: there is no
syscall surface, no stdin/stdout capability, no process exit. The lowest
boundary is the hardware itself.

The direction: freestanding is a target whose host-provider set is EMPTY and
whose boundary providers instead declare facts about hardware. The same
trust model applies unchanged -- a boundary is where proved Omega code accepts
declared, audited guarantees it cannot itself verify -- but the guarantees are
now hardware claims rather than OS claims:

- "after this admitted translation provider completes, the named mappings are
  active" (an MMU provider),
- "this MSR read returns the current value of register X" (a register
  provider),
- "stores to this physical range reach device Y in program order" (an MMIO
  access provider, see
  [Memory Layout And ABI](chapter_20_memory_layout_abi.md) on volatile),
- "this instruction sequence masks interrupts until the matching unmask" (an
  interrupt-control provider).

These are the most serious trust statements in any system built on Omega: a
kernel's trusted computing base is, in large part, exactly this provider set,
and it is enumerable in the build artifact like every other boundary. The
audited inline-assembly subset
([Inline Assembly](chapter_23_inline_assembly.md)) is the implementation
vehicle for many of these providers -- the asm instruction contracts ARE
hardware-fact declarations in small form.

A freestanding target needs two joined arrival contracts. The physical
requirement states who transfers control, the exact native parameters and
result, and the calling and machine-state policy. The stable semantic
requirement states which program facts and custody the installation introduces.
A target-owned entry schema fixes the physical requirement and bootstrap
adapter, selects the semantic requirement, and declares which typed parameters
the source continuation sees. `build.omg` binds that continuation only; no
source name is an entry by convention.

For program storage, the semantic requirement is
`ProgramStorageEntry::enter`. A UEFI schema composes it with a separate
target-owned physical requirement receiving `ImageHandle` and `SystemTable`
under `Calling<UefiX86_64>` and returning `EfiStatus`. The target-authored
bootstrap interprets those inputs through admitted, lifecycle-scoped providers;
neither physical input is an `Extent`. The handle identifies an image, while a
Loaded Image provider supplies admitted base/size correspondence. Initial
storage is separately owned and proved disjoint from the installed image root,
or allocated; the live provider-selected entry stack is not transferable
storage. A generated ABI shell invokes the bootstrap,
which obtains exact geometry and correspondence evidence, proves the stack and
storage partitions, crosses the semantic installation edge once, and calls the
source machine with only the declared semantic values. The installation receipt
joins both requirements, provider and input provenance, generated captures,
stack plan, and selected continuation.
The composed crash, reach, write, work, stack/state, provisioning,
introduction, result-map, and provenance contract enters the bound program
closure just like authored code.

Foreign trust is retained as exact service postconditions plus the physical
arrival premises that have no service call: valid system-table occurrence,
selected initial machine regime, and conformance to the selected entry-stack
profile. Geometry and separation are derived after those premises. A blanket
firmware admission is not substituted for the individual provider contracts.
The entry-stack minimum is a symbolic target-semantics observation until target
closure; actual firmware conformance remains an admitted physical fact.

Hosted schemas normally expose neither image nor initial-storage extents. A
freestanding schema may forward them because the selected program must perform
its own provisioning. A receiver-bound entry receives exactly one exclusively
lent instance; no ambient `static` name is introduced. Active stack and receiver
storage remain explicit conserved partitions in the target execution frontier;
source receives only disjoint residual storage, never a qualified extent with a
hidden inaccessible hole.

The physical result is target-authored. For UEFI, recoverable bootstrap
rejection maps to a declared `EfiStatus` and normal return from the current Unit
semantic continuation maps to success. A crash, trap, or abort does not return
through the result register and remains a non-returning route.

The returning `UefiApplication` profile lands first and keeps Boot Services
live. A successful OS-loader handoff is a separate lifecycle: its
memory-map/exit adapter is a bounded state graph, not an unmeasured retry loop.
Its decreasing attempt term and every non-copy boot-services capability,
allocation, snapshot, and key are threaded through state arrival contracts. A
stale-key outcome returns live custody for retry; success ends boot-scoped
providers while transferring already allocated storage under the same
occurrence lineage. Runtime services and newly claimable final-map regions
follow their own post-exit contracts.

`C` satisfies the ordinary core `CallingPolicy` relationship; its compile-time
machine evaluates the normalized signature to an accepted or structured-
rejected boundary plan. Accepted plans are compiler-validated and canonicalized.
The evaluated `CallPlan + StatePlan`, not the policy symbol or source body,
belongs to requirement identity;
`boundary(<Plan>)` is retired because it fused trust treatment with deployment
policy. The selected target profile supplies the freestanding fact and empty
host-provider baseline (see
`../design_briefs/build_and_package_model.md`). The machine-state
guarantees are normalized provider/entry-plan facts surfaced by the build
artifact and checked or accepted through the ordinary admission spine.

The selected provider binding does not choose a calling convention from its
mechanism name. `DllImport`, `Syscall`, `VtableField`, and similar realizations
must validate against the policy already pinned by the satisfied requirement.
Provider-specific register allocation and footprint certificates remain
implementation evidence behind that published plan identity.

`VtableField(name)` denotes a function-pointer field through one validated
foreign layout, principally for firmware protocol tables. Authored numeric
`VtableSlot(n)` is retired: the parser rejects the case before consuming its
payload because an ordinal is neither stable slot identity nor an adequate
substitute for the native schema. Downstream artifact enums and codecs retain
the ordinal variant only for compatibility decoding and reporting; source
cannot construct it. Named fields are the authored foreign-table surface.

Hardware entry points with no Omega caller are external artifact roots. Their
reach, trust receipts, state footprints, stack domains, nesting relations,
and version pins must enter whole-artifact analysis at installation; otherwise
an interrupt or callback could launder behavior by sitting outside the ordinary
call graph.

`EntryStack` says where the entered machine executes; it does not pretend that
all arrival and adapter storage occupies that domain. Installation validates a
separate target/provider realization containing every admissible arrival
context and a finite sequence of stack epochs for each. Each epoch fixes its
active domain, per-domain occupancy, and nesting allowance. Architectural
arrival follows a sealed target rule applied to the installed entry facts,
generated adapter use follows the emitted stub, and opaque adapter use requires
an admitted receipt. The body WCSU is charged only in the body epoch's execution
domain. Nested `Interrupted` entry is relative to the active parent epoch, and
unresolved contexts, stack domains, or evidence reject before publication.

The reusable extent, placed-view, checked-assembly, materialization, and root
ledger model is specified in
[`os_memory_and_hardware_foundation.md`](../design_briefs/os_memory_and_hardware_foundation.md).
Exact carrier APIs and validators remain open there; no separate interrupt or
MMIO grammar is implied.

### Admitted executable installation

Omega has no operation that converts arbitrary bytes into host code and no
general `ExecutableMemory` capability. Executable eligibility is a sealed
admission fact over a reusable immutable artifact. A package cannot establish
that fact for itself, and mutation invalidates it.

Installation borrows the admitted artifact and consumes authority over one
destination. Its normalized states are:

```text
CodePlacement (writable, non-executable)
    -> materialize declared sections and relocations
FrozenPlacement (readable, non-executable; no remaining writer)
    -> validate the exact final bytes and footprint
ValidatedPlacement
    -> contracted installation and instruction-fetch visibility
InstalledCode (readable, executable)
```

Each state is sealed: the only operation that can produce the next state
requires the previous one. The artifact remains reusable; the linear placement
authority prevents one destination from being spent twice. Validation evidence
is bound to artifact identity, placement, and final content, so it cannot be
transplanted to different bytes.

The installation provider alone performs the target-specific permission
transition, cache maintenance, ordering, and visibility work. Checked assembly
and installation providers emit the same admitted-artifact and
installation-authority obligations; neither is a raw bypass. A future fetcher requires visibility
before entry, while replacement of possibly running code separately requires
quiescence before retirement.

Installation prevents code injection. It does not prove that transfers within
installed code are legal. The two control-flow directions have different
answers. Backward-edge return integrity in checked Omega derives from memory
safety and compiler-owned, non-addressable live or parked continuation state;
WCSU is supporting provisioning evidence, not a separate CFI mechanism.
Forward-edge indirect targeting instead requires sealed entry references or
descriptors retaining requirement/satisfier identity. Local dynamic
descriptors, their object-safety rules, and their operational envelopes are
specified in [Traits And Conformance](chapter_14_traits.md); component
boundaries use bindings and local proxies rather than exporting those
descriptors.

An opaque provider must present an admitted `CallPlan + StatePlan` whose exits
preserve the boundary contract or remain behind adequate hardware isolation.
Supplying neither rejects admission. Independent final-byte transfer checking
and CET, PAC, or shadow-stack realizations are deferred PCC/TCB assurance, not
mandatory source semantics.

### Build policy and privileged reach

Package policy is an outer admission gate over compiler-derived reach, not the
only protection around privileged operations. A normal hosted/application
profile should reject roots whose transitive reach includes platform services
such as interrupt-table control, address-translation control, raw device control,
or admitted-artifact installation. Kernel and firmware profiles may grant a
small audited provider set instead.

The service identities are normalized package-qualified requirements, not
friendly type names and not a compiler-hard-coded list of "dangerous"
keywords. Registry/build policy classifies those identities. Direct checked
assembly contributes the same reach as the abstract operation it realizes, and
installed inbound entries are additional service-reach roots, so neither wrappers nor
hardware callbacks can launder reach out of the report.

Policy approval still does not manufacture authority. Admission must supply the
actual scoped capability, and the operation additionally requires its sealed
qualified input. For example, an OS IDT installer needs both CPU-scoped
publication authority and a content-bound table value defined by that OS; the
former cannot create table bytes and the latter cannot execute `lidt`. These
are consumer-defined values satisfying a general checked-instruction contract,
not compiler-owned IDT typestates. The complete defense is:

```text
compiler-derived reach
    -> registry/build-policy decision
    -> explicit provider capability grant
    -> sealed operation-specific input
    -> checked operation and receipt
```

There is no general `ExecutableMemory` grant to classify. Executable
installation accepts only an already-admitted immutable artifact and an exact
authorized destination.

## Views, domains, and foreign shapes

Imported signatures use ordinary types, domain qualifications, and contracts;
there is no parallel invariant-parameter list. For example, `&[u8]::NonEmpty`
or `&[u8]::Utf8 & NoNul` carries the same owner-defined facts used everywhere
else in Omega.

`Array` owns fixed inline storage, `Vec` owns dynamic contiguous storage, and
both borrow as `Slice`. Text is bytes plus an encoding domain: `[u8; N]::Utf8`,
`Vec<u8>::Utf8`, and `&[u8]::Utf8` are owned, growable, and borrowed forms of
the same semantic content. Public declarations expose browsable contracts such
as `Slice::range` and proof views such as `Slice::Length`; private pointer,
length, capacity, and provenance carriers remain compiler-managed.

Those private carriers do not define a foreign ABI. A calling policy may
classify a value only when its public normalized semantic/layout contract fixes
the ABI facts. Otherwise the native leaf declares the counterparty's actual
shape—separate pointer and length, null-terminated pointer, or a declared
record—and a checked adapter performs the conversion. Foreign retention must
consume a backing keepalive or authority into a linear protocol claim; a bare
borrow cannot survive return.

`addr` and `Ptr<T>` are inert representation carriers, never memory authority.
Core exposes no raw `Ptr::read` or `Ptr::write`; access requires an
authority-bearing view. Allocation likewise comes through an explicit arena or
provider, never an ambient `Vec` constructor. The single rule is: when the
semantic type fixes the ABI, a policy may classify it; when it leaves choices,
the leaf must state them.

## Boundary evidence and authority values

`boundary` records why a guarantee that cannot be proved from Omega code is
accepted. Callers still prove input refinements and invariants; the selected
provider is accountable for declared result guarantees. Core primitives and
foreign surfaces use the same registered-provider discipline, so acceptance is
auditable rather than a user-authored proof escape.

Pointer-level providers live under `omega::language::core::ptr`; safe source
normally reaches them through owners and views.

Runtime authority values are ordinary data with compiler-tracked domain facts.
Their fields carry runtime geometry, saved state, or provider keys. Domain
membership carries validation, provenance, rights, and authority without
adding a runtime tag.

An admitted provider may originate qualification when it satisfies an
owner-authorized boundary requirement whose signature identifies the exact
subject and fact. Provider selection and admission identify the accepted
evidence source in a receipt. Checked implementations still prove their
guarantees, and checked resource transformations preserve or divide existing
claims through normalized outcome mappings.

A content-bearing exact qualification publishes one owner-unique conformance to
the core `Content<A>` projection requirement. The conformance selects a closed
compiler-owned partial composition algebra and must normalize from the
content-projection fragment: subject field reads, proof-level scalar
embeddings, proof-defined closed arithmetic, and constructors of that algebra.
An ordinary boundary postcondition states geometry in the same algebra, while
selected provider evidence separately admits external supply, fresh issuance,
and custody. Establishment proves projected content lies within that backing;
access proves its touched footprint lies within content; and every n-to-m
transformation proves entry plus introduced content equals separated output
plus content that left checked custody. Per-output containment and scalar
measures do not establish this theorem. Ordinary claims without a projection
remain fully accounted for by whole-claim frontier transfer and cleanup.

Root origin is recorded per establishment occurrence. One statically enumerable
installed root position authorized by the qualification's domain route may
introduce a program-local account; selected admitted issuance creates a
provider-backed account. The requirement contract fixes exact finite capacity
per program-local occurrence, and installation fixes its finite cardinality,
artifact-instance scope, and lifecycle epoch. An ordinary checked call may
expose or transform an existing account but never originate one. The algebra
denominator is not an authority label: proof code may freely construct
`IntervalSet<PhysicalMemory>`, while external access still requires an exact
qualified root whose lineage carries backing or correspondence evidence for the
selected provider. Terminal validation rejects external operations justified
only by matching content arithmetic.

The target-neutral ledger retains both origin records through introduction,
split, borrow, mapping, and rejoin, and rejects recomposition across origin
kinds or independently introduced local accounts. The portable verifier derives
each program-local introduction schema; installation verification joins it to
the exact occurrence set and derives the aggregate for one artifact instance
and epoch. Cathedral composes those verified totals across live components and
coexisting replacement eras. A manifest-supplied aggregate is never authority,
and a new epoch's root is a new budget rather than recovered lifetime capacity.

The first Terminal producer-schema slice retains the exact boundary
requirement and authored parameter position, qualified carrier and normalized
domain identity, owner-unique projection and algebra, and normalized
per-occurrence capacity. Its canonical identity is independent of
module-local dense IDs. This row is a portable description, not a claim or an
establishment event: no program-local authority exists until installation
joins it to one exact installed occurrence, finite cardinality, artifact
instance, and lifecycle epoch.

Installation first verifies the complete target-required root-slot closure.
That opaque closure rejects omission, duplication, extra rows, and target-profile
substitution, but remains a description rather than a minting value. The exact
installed-code occurrence issues one non-clonable registry authority for one
installation scope; its ledger cannot be recreated after the claim is issued,
and it rejects roots from another installed occurrence even when compact report
identities resemble one another.

The root ledger then replays the closure against the installed members and
burns issuance of one program-local cohort verifier. The verifier has no public
constructor, admits no root outside that closure, and closes every eligible
prebinding atomically under one exact lifecycle ledger and epoch. Until that
cohort exists, a prebinding, count, or individually packaged lease is not an
origin. The cohort's aggregate preserves exact per-occurrence expressions and
derives cardinality from membership; it does not multiply interval or
subject-dependent capacity by convention.

The aggregate has a separate reporting projection. A private-construction
snapshot keeps the cohort identity, installed required-slot closure, and every
schema row but carries no claim or lifecycle lease. Live-era composition is
complete only when the exact component lifecycle roster contributes one
snapshot per epoch. The resulting report remains epoch-attributed and
unreduced so system policy can calculate a deployment-specific coexistence
peak without Omega inventing arithmetic between unlike content algebras.

Contracts call the exact owner-unique projection machine. Proof-only
`old(place)` selects a structural place at its callable-entry revision, while
compiler-owned `separate(...)` performs the closed algebra's partial n-ary
composition. Neither has runtime representation. Identity-preserving claim
reshuffles infer; partition-changing primitives author their theorem and
checked wrappers compose it. Terminal Psi carries each exact preserved-claim
mapping, reconstructs its projection equality, and retains active sum payloads
as distinct case-plus-field paths. It never infers separated composition across
independent claims.

Content equations resolve every projection to the exact owner-unique
`Content<A>::project` machine and normalize once per callable outcome and
algebra. `old` cannot select `result` or an arbitrary expression; projection
subjects are shared borrows of qualified structural places. Mixed algebras,
duplicate equations, and executable uses reject. `TASKS.md` owns expansion
beyond the source positions accepted by the compiler.

For example, an admitted platform provider may return an
`Extent::Granted & Physical`; its ordinary linear runtime carrier is
`Extent { base: addr, length: u64 }`.
Reconstructing those fields produces an unqualified Extent. `Granted` projects
the qualified subject into the compiler-owned interval-set algebra after its
`no_wrap(base, length)` predicate proves the unbounded proof-`Int` end fits the
target address space. Derived nonnegativity then licenses exact conversion of
both endpoints into the proof-`Nat` content algebra. The projection produces a
singleton canonical
`IntervalSet<PhysicalMemory>`; split consumes the qualified parent and proves
its set equals the separated composition of all children while preserving
compatible common root lineage. Merge proves the same theorem in reverse.
Permission attenuation cannot be reversed by joining permissions; authority
that must return is represented as a claim or loan.

`no_wrap` is transparently
`embed(base) + embed(length) <= addr::Bound`; it is not an executable boundary
call. `addr::Bound` is a canonical target-semantic constant whose symbolic
application and target closure remain in proof/artifact identity. The formula
proves geometry only. Because `Granted` declares exact `established by` routes,
copyable proof evidence cannot mint its Type-side authority.

Routed qualifications name their exact authorized trait requirements in the
domain declaration. Obligation-free domains may be qualified directly with
`as`; predicates must be proved, and routed provenance cannot be fabricated.
See
[`authority_values_and_boundary_evidence.md`](../design_briefs/authority_values_and_boundary_evidence.md).

A boundary call checks each structural `requires` row against the exact domain
qualifications already carried by the corresponding argument. The check is set
membership over domain identities; it does not mint a proposition or an
`ObligationId`. The qualification may have been established by predicate proof,
an authorized routed operation, propagation, or another route permitted by its
domain, but the boundary call neither repeats nor substitutes for that
establishment. Ordinary in-module call propositions remain proof-bearing and
are unaffected.

Downstream transformations may carry or conservatively forget qualifications,
but cannot derive them. A control-flow join may retain only qualifications
present on every incoming occurrence, and CSE/GVN must distinguish unequal
qualification rosters unless it deliberately forms their common intersection
and revalidates every use. Unioning rosters would mint authority on a path that
never established it. Forgetting a qualification is fail-closed, but an exact
Terminal-to-Terminal publication must reject that semantic change rather than
silently publish it as equivalent.

## Boundary Realization Catalog

Omega has one provider model. A boundary trait/operator or explicit top-level
`boundary requirement` publishes a selectable slot; an ordinary checked
machine or an irreducible bodyless `boundary machine` satisfies it; the
toolchain derives a candidate `ProviderPlan`; and the selected target profile,
`build.omg`, or installation binding chooses one admitted candidate for the
owned slot. A requirement never embeds that choice.

Top-level carrier-owned operations use the same declaration/reference path:

```omega
pub boundary requirement InterruptAcknowledgement::complete(self)
reaches <= MachineControl + PortIo
requires self in InterruptAcknowledgement::Pending;

machine LapicCompletion::complete(
    acknowledgement: InterruptAcknowledgement in Pending
)
satisfies InterruptAcknowledgement::complete
reaches MachineControl
{
    // checked completion
}
```

`build.omg` may select the exact requirement and provider type. Selection does
not create the `satisfies` edge, and equal reach rows convey neither provider
identity nor token lineage.

Compiler intrinsics use the same shape:

```omega
boundary machine CoreSliceProvider::index<T>(items: &[T], index: u64) -> T
    satisfies Slice::index;
```

`CompilerIntrinsic` has no authored string identifier. The exact realization
machine already has a resolved package-qualified symbol and normalized
signature. Together with the selected target, that identity keys a sealed
toolchain catalog entry containing the lowering and its checked operational
contract. Therefore no payload-free `via Binding::CompilerIntrinsic` clause is
authored. An absent entry, wrong signature, inapplicable target, unauthorized
origin package, or non-refining implementation rejects.

`via` is earned only when it carries an undiscoverable binding payload. Other
irreducible bindings use ordinary typed compile-time values produced by
target-package machines. `DllImport` carries one object-format locator sum case
whose case keeps a PE library/export pair, a PE library/ordinal pair, an ELF
object/symbol/version triple, or a Mach-O install-name/symbol pair inseparable.
The satisfied requirement's
`Calling<C, Policy>` relationship separately evaluates the ABI `CallPlan`;
`Binding` does not carry or reselect it.
Syscall, firmware, and vtable bindings carry equally typed physical operands.
Raw foreign bytes are therefore honest target data but never Omega symbols,
dispatch keys, requirement identities, ambient lookup strings, or provider
selections.

The complete evaluated binding, its producer closure, and target application
are fingerprinted. Changing a foreign locator changes every dependent final
artifact, forces relinking, and requires fresh admission. Audit reports retain
the actual locator rather than a nominal endpoint whose mapping could redirect
elsewhere; `build.omg` may select a target/provider but cannot rewrite the
evaluated binding.

The retired top-level `provider Name : Category;` declaration and
operator-local `provider Name` clause are bootstrap syntax. Their fixed
category vocabulary duplicated information already available from the
requirement, binding kind, and selected realization, while pinning
implementation supply inside the requirement owner's source. They do not
belong to the destination language. Audit reports instead enumerate selected
provider-plan rows with exact requirement, realization symbol, binding kind,
target applicability, normalized contract, admission receipt, and artifact
identity.

## Blocking Boundaries

Imported entries that can block must say what can unblock them, or they must be
reported as boundary opaque waits.

Blocking and parking are distinct. An imported/provider contract carries
`blocks` when it may occupy the calling worker and `suspends` when it may park a
task. Those ceilings are checked against the pinned requirement at admission; the
eventual provider cannot widen a consumer compiled against a no-block/no-park
slot. Positive wake/fairness premises are sealed, grant-backed opaque progress
profiles on the pinned operation/provider
contract. They participate in admission and trust reports but do not become
ordinary proof facts or follow merely from an operational clause.

Examples:

- A pipe read may block until a matching write, close, timeout, or external
  event.
- A process wait may block until the target process exits.
- A socket receive may block on external network input.
- A driver call may block on hardware interrupt, timeout, cancellation, or a
  boundary opaque device contract.

The proof/invariant checker can reason about modeled waits. It can audit
boundary opaque waits. A proved-concurrency build may reject opaque blocking
boundaries.

## Host vs Standard Library

An optional standard-library package may provide `Console.read_line`,
formatting, strings, slices, data structures, and higher-level process or
filesystem helpers. It is not a language component or privileged namespace: a
deployment may replace it, split it into narrower packages, or omit it. These
machines are ordinary Omega code unless they explicitly model the bottom host
edge.

Host packages are ordinary packages selected as the audited bottom edge. They
contain imported libraries, syscall surfaces, startup bindings, and boundary
providers; the exact selected declarations and provider plans confer their
meaning, not a `host` or `std` package role.

Typical layering:

```text
application code
  -> optional library Omega machines
    -> boundary trait requirement
      -> selected provider and calling plan
        -> syscall / imported symbol / firmware jump / loader hook
```

Static vs dynamic linkage is not the same question as boundary vs normal code.
A statically linked library wrapper is still normal Omega code if the
compiler can check its body. A dynamically imported, syscall-backed, firmware,
or externally supplied implementation is a boundary because its guarantees are
boundary rather than proved from Omega source.

Most users should not author raw Windows, Darwin, Linux, firmware, or console
SDK contracts for ordinary applications. They import portable standard
surfaces; the selected target contributes its default provider plan:

```omega
use omega::language::std::console;
use omega::language::std::filesystem;
```

There is no compiler-magic `omega::host` or `std` package. The repository's
current optional library places target providers under
`omega::language::std::targets`, but this is package layout rather than language
semantics. Target providers may instead live in dedicated ordinary packages;
target composition selects their exact declarations and schemas through the
accepted graph or an explicit slot-owner override in `build.omg`.

Advanced users can author libraries for custom OSes, firmware, game consoles,
or unusual hardware. Doing so explicitly expands the boundary base.

## Foreign callbacks through platform adapters

A foreign callback signature is an ordinary boundary requirement whose parent
`Calling<C>` policy fixes the target ABI and entry-state contract. A named
static `boundary machine` explicitly satisfies that requirement:

```omega
boundary trait WindowProcedure:
    Calling<MicrosoftX64>
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

A registration operation names that exact nominal requirement on its static
machine binder:

```omega
boundary trait WindowRegistrar:
    Calling<MicrosoftX64, MicrosoftX64Policy>
{
    machine register<machine Procedure>(
        specification: &WindowClassSpecification
    ) -> Registration
    where machine Procedure satisfies WindowProcedure::call;
}

let registration =
    WindowRegistrar::register<ApplicationWindow::dispatch>(&specification);
```

The registrar requirement owns its evaluated outbound plan. The target package
supplies only the ordinary external locator binding; callback placement adds no
declaration keyword:

```omega
windows_x86_64 machine User32Bindings::register_window_procedure()
    -> Binding<10, 16, 0>
{
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "user32.dll",
            export: "RegisterClassExW",
        },
    }
}

boundary machine User32::register_window_procedure<machine Procedure>(
    specification: &WindowClassSpecification
) -> Registration
where machine Procedure satisfies WindowProcedure::call
satisfies WindowRegistrar::register
via User32Bindings::register_window_procedure();
```

The requirement supplies the binder's complete signature and contract; they
are not repeated structurally. The selected machine must carry an explicit
satisfaction row for the exact requirement. Signature coincidence and visible
uniqueness never establish the relationship. Because the path appears without
a call signature, `WindowProcedure::call` must resolve to one overload or the
declaration rejects under the general signature-free requirement-path rule.

The compiler validates the published callback envelope, the selected machine's
actual refining envelope, and their evaluated `CallPlan + StatePlan`, then
generates the native inbound thunk. The native code address exists only in the
binding lowering. The returned linear registration owns unregistration and any
code/component lease. Its occurrence provenance retains the selected machine,
but a public caller may reason from the narrower actual envelope only when the
API explicitly forwards that guarantee.

For a nested callback, the native layout independently declares and places the
private demand:

```omega
WndClassWindowProcedureSlot:
    WndClassLayout satisfies
        PrivateCallbackSlot<WindowProcedure::call>;

// Inside WndClassLayout::plan; conceptual closed-vocabulary constructor.
Plan::place_private<WndClassWindowProcedureSlot>(
    plan,
    window_procedure_offset
)
```

The plan explicitly selects the named conformance. No conformance population
is searched, and the declaration alone changes no layout. Its subject fixes
the exact layout and its static argument fixes the exact signature-free
callback requirement. Layout evaluation resolves both declarations into
opaque normalized identities. The layout policy may author or compute the
target-dependent offset, but the calling plan neither repeats that offset nor
uses it as slot identity.

The registrar's evaluated outbound `CallPlan` separately maps the nominal
`Procedure` binder slot to one declared private native place. A direct callback
is an interleaved native-only parameter on the registrar requirement:

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

The source call omits `procedure`. It has no Omega runtime type or address
value, but its declaration contributes a nominal private-callback entry at the
authored position in the native ABI telescope. The compiler supplies its exact
target function-pointer shape and `NativePlace::Parameter` demand. The calling
policy may place that entry; it cannot create, reorder, or retarget it. This
single declaration is sufficient because the registrar requirement owns the
parameter list. A nested callback instead names a field through an
independently owned validated layout and therefore retains the explicit named-
conformance citation.

Both destination cases use one nominal native-parameter identity space:
ordinary entries originate in semantic formals, while native-only entries
originate in exact callback binder/requirement pairs. Declaration order fixes
ABI position but is not parameter identity. The plan never infers binder order
as an ABI ordinal, never appends an undeclared trailing argument, and never
stores a byte offset. A nested field is a private layout demand absent from the
source specification; complete plan validation requires one compatible supply
and rejects missing, duplicate,
overlapping, or unresolved rows. The normalized registrar-plan fingerprint is
independent of the selected callback machine. Per-use identity retains that
machine and its entry plan, and lowering joins the two only to emit a private
relocation.

The reusable physical `CallPlan` fingerprint is not the complete replay key.
The boundary-plan application identity additionally covers the exact
requirement, ordered native telescope, every nominal parameter-to-placement
row, and callback materialization. Consequently reordering two equally shaped
parameters rejects even when their raw register assignments appear unchanged.
The migration from ordinal-derived parameter IDs is format-versioned and
reissues affected plans and receipts; old rows are never reinterpreted as new
nominal identities.

Native argument backing is not a callback-row property. Direct placement uses
the declared register/stack destination; a copying registrar may use ordinary
call-scoped staging; and any post-return pointer retention follows the general
foreign-storage custody, provenance, snapshot, and capacity rules. Build
selection admits the realization and resources, but ordinary Omega control flow
calls the registrar and selects the callback machine. A successful call creates
the external root represented by `Registration`; rejection creates none.

A durable registration operation returns a linear package value. Its terminal
operation unregisters the callback and releases any code or component lease
owned by the registration. Per-instance state remains an ordinary Omega value;
the foreign protocol carries an explicit context token or a checked
generational handle into package-owned state.

The current compiler carries a direct callback through private native-function
emission, relocation, final-image replay, and canonical installation format 51.
That installation row preserves identity, source Psi, ABI, and final text
interval but grants no runtime registration authority. A runtime bridge can
consume its exact installed-entry attribution with an admitted external root,
provider success receipt, and one exact capacity occurrence. The bridge retains
the attribution and root/code pin until provider unregister and root quiescence
both succeed; every failed join or teardown preserves the complete retry
custody. The source `Registration` lowering and exact component-era lease join
remain engineering work. Runtime capacity still bounds live registrations,
not emitted thunk count, and is distinct from a consumable lifetime budget.

Platform packages normally expose a safer handler API above a re-entrant native
callback. Bodyless package surfaces declare direct synchronous entry through
`invokes`; bodyful handlers infer it. The checker rejects cycles in the
realized direct invocation graph. Synchronous platform queries use bounded
handlers; ordinary notifications may be queued until native dispatch returns.
Applications therefore consume a normal event/handler surface instead of
participating directly in the platform's recursive callback graph.

Blocking and affinity compose through the same contracts. A Windows
`GetMessage` binding is legal directly on a dedicated pinned UI executor whose
contract permits blocking. Codec-style calls that should not occupy a no-block
scheduler worker may instead use an ordinary blocking-executor package.
That package is built from activations, queues, moved custody, suspension, and
provider selection.

The selected entry plan states whether callback execution continues on the
provider stack, preflights that stack against the Omega WCSU and target reserve,
or enters a target-supported owned stack. Preflight proves that the predicted
Omega segment fits. A hard-limited owned stack also detects an underestimated
WCSU at its own boundary.

Checked and opaque providers satisfy the same boundary requirements. Checked
facts are derived from bodies; opaque facts are admitted by bindings. Each fact
retains its own trust class and exact provenance, and composite guarantees
report their weakest input. An opaque third-party binary loaded in-process
remains part of the trusted computing base even when a checked adapter wraps it.
TCB expansion follows selected providers rather than source reach. The boundary
manifest names each known provider/executable identity, static-selection or
Omega-mediated-runtime origin, evidence class, execution scope, and admitted
containment guarantees. An isolated process exposes an endpoint in the caller's
manifest and has a separate executable manifest for its own scope.
The artifact carrier enforces this shape: the caller records an admitted
endpoint and its containment evidence, while the isolated provider's code and
completeness live in a separately identified child manifest. An incomplete
child does not make the caller-address-space inventory incomplete; deployment
profiles evaluate the two scopes independently.

The manifest also reports whether the known-entry list is complete for that
scope. An uncontained opaque in-process provider makes it incomplete and is
named as the cause, because it may load or generate executable code without an
Omega admission. The runtime ledger therefore reports what Omega admitted,
never a falsely exhaustive map of an opaque process. Build profiles may permit
and mark that result or reject it before installation; platform baselines are
ordinary policy allowlists.
Omega-mediated runtime loading uses an append-only ledger for the exact
execution scope. Admission requires pinned executable and implementation
identities plus a unique mediation receipt—never merely a path—and the unioned
manifest marks those entries as runtime-origin. A runtime executable without a
separate closure receipt is reported as known while making the scope
attributed-incomplete; the ledger does not claim code loaded outside its
mediation boundary.
Deployment supplies admissions and profile rules. Validation follows exact
provider selection and binds acceptance to the output manifest and
installation. This is build policy, not a new language construct; `TASKS.md`
owns the remaining named `Build` API.

## Build Artifacts

The machine-readable manifest keeps separate rows for authority flow, service
reach, trust receipts, imported libraries, direct and transitive boundary
calls, selected provider-plan realizations, domain-evidence origins, resource
transformations, target imports, and executable TCB scope. Each row retains
stable requirement, realization-symbol, binding, target, artifact, and receipt
identities rather than a rendered source name alone. TCB entries remain
separate from scope completeness and its independent isolation, termination,
fault-containment, and resource guarantees.

The selected-realization list is the audit artifact. Every entry joins one
exact requirement to one admitted candidate and its normalized binding; an
unresolved, ambiguous, unknown, or unadmitted selection rejects before the
report is emitted.

Human package diffs collapse low-severity checked tokens and elevate transitive
changes in authority, admitted providers, boundary-evidence permission, or
revocation/generation machinery. Admission compares the final artifact's
transitive reachable-authority set, not only direct dependencies.

A build with proofs or contracts disabled should be stamped loudly rather than
silently behaving like a normal safe build.

The goal is not to pretend these edges disappear. The goal is to make every
boundary explicit, scoped, and auditable.
