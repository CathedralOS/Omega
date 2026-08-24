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
- Realizations use the same checked-body or `satisfies ... via <Binding>` supply
  forms as every other boundary requirement. The target profile or an
  authorized `build.omg` slot override selects one admitted candidate.

Users can inspect `Slice::index` and its proof contract without depending on the
private descriptor, pointer, or code-generation mechanism used after proof.

### Representation TCB and claim admission

A claim-free `boundary data` declaration is representation evidence, not by
itself a proposition, capability establishment, provider guarantee, or service
reach grant. It can still enlarge the code/ABI trusted computing base because
an external realization determines some or all of its representation.

Package evidence therefore always reports it as an exact representation-TCB
row. The row is keyed by the package-qualified declaration, target,
representation/ABI commitment, selected mechanism or explicit unbound status,
and source/toolchain/compiler evidence. Package-controlled names never classify
its risk.

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
- `export` names what symbols belong to an artifact/API surface.
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

## Boundary-Trait Values And Bindings

An ordinary trait name is not a runtime type; use `dyn Trait` for local dynamic
dispatch. A boundary trait name in value position instead denotes the selected
binding for that service:

```omega
data Application {
    logging: LoggingService;
}
```

The binding names a provider slot, not a provider-era address. In a statically
composed build the compiler can erase the indirection. In a replaceable build,
each call resolves the slot to one era and retains that era until the call
leaves. The source API is the same in both builds.

Boundary bindings use ordinary multiplicity:

```omega
boundary trait LoggingService [copy] {
    machine write(text: &[u8]);
}

boundary trait DeviceControl {
    machine reset(&mut self);
}

boundary trait Connection [linear] {
    machine release(self);
}
```

`[copy]` means the binding authority is fungible. The default is affine:
move it or share it by borrow, but do not duplicate it. `[linear]` additionally
requires explicit discharge. Copying a rebindable binding creates no retention
edge to an old provider era; active calls and era-custodied session values do.

A service that accounts for or may refuse duplication does not use `[copy]`.
It exposes an ordinary operation instead:

```omega
boundary trait LicensedService {
    machine duplicate() -> LicensedService;
}
```

Boundary requirements have an implicit shared binding receiver when none is
written. `&mut self` requires exclusive binding access, while `self` consumes
the binding. This receiver is distinct from the selected provider's internal
state mutation.

Composite bindings take the restrictive meet of their parents' multiplicities.
Projecting a parent from `&Composite` yields a borrowed parent binding. An
owned parent is obtained only by consuming and attenuating the composite, with
every omitted linear obligation returned or discharged. A borrowed projection
never silently manufactures an owned, copyable sub-binding.

### Local dynamic interfaces over bindings

A local `dyn` descriptor cannot cross a replaceable component boundary: its
table uses within-artifact calling semantics and copied descriptors cannot be
ledgered for unloading. An ordinary local proxy bridges the two mechanisms:

```omega
data LoggingProxy {
    service: LoggingService;
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

One deliberately narrow exception supports installation-bound provider
requirements whose exact service row is selected with the installed
realization:

```omega
boundary machine InterruptAcknowledgement::complete(self)
reaches <= MachineControl + PortIo
requires
    self in InterruptAcknowledgement::Pending
ensures true;
```

This introduces one abstract row keyed by the exact normalized requirement
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

`build.omg` is an ordinary checked Omega program run with package-scoped build
providers. It has no ambient filesystem, network, process, signing, secret, or
package-acceptance authority. Before evaluation, its normalized contract gives
policy the static service and authority ceiling; after evaluation, receipts
record the realized observations and outputs.

Dependency retrieval is a resolver operation performed before downloaded code
runs. The resolver owns narrowly scoped network, archive-reading, expansion-
limit, path-containment, and destination authority. A dependency's `build.omg`
never inherits those providers. Imported boundary claims are inert until the
root build accepts the dependency's fingerprinted complete claim set; any claim
change invalidates that acceptance and appears in the lock/trust diff.

That acceptance records a root policy decision, not proof that an audit
occurred. Omega can derive bounded capability, authority-flow, provider,
representation-TCB, proof-status, and provenance rows and can recommend or
require review according to policy. It cannot establish that a human or LLM
understood the source or made a sound security judgment. Even a signature says
only who controlled a key, and a proof certificate establishes only its exact
mechanically checked proposition. Projects requiring stronger assurance enforce
their own reviewers, quorum, isolated builds, bootstrapped toolchain, and merge
controls around these deterministic facts.

The ordinary rows are derived from the earliest coherent checked state for each
row by a total internal package-admission projection. Rows may use different
compiler-private representations; totality is required of the final projection,
not of one nominal intermediate stage. Unresolved or unprojectable required
facts reject; the compiler does not serialize raw internal IR or fill gaps with a
“complete enough” marker. Terminal Psi is required separately when a row claims
a checked property of final executable realization, lowering, ABI realization,
or fixed native resources, and when a hardened profile explicitly requests that
evidence. Opaque executable supply may remain an explicit trust/TCB row making
no Terminal claim. Terminal evidence is not a blanket prerequisite for checked
reach and authority admission. The earlier checked representations used for
ordinary rows remain Psi-owned semantic state; this choice does not establish a
pre-Psi path or another semantic owner.

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

The target's core/std package declares leaf machines satisfying the raw syscall
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

This is the same proof shape as a library import:

- Omega proves caller-side type and state invariants.
- The imported boundary is accepted to satisfy its declared guarantees.
- The irreducible mapping is authored as a compile-time `Binding` value on a
  `via` declaration and retained on the exact realization row.
- The build artifact records the exact selected realization, normalized
  binding, admission receipt, and provider-plan identity.

`via` bindings are the external-provider supply form of otherwise ordinary
machines. Raw syscall numbers, imported DLL functions, firmware jumps,
compiler intrinsics, and instruction leaves are binding details; sequences,
argument reshaping, newline policy, caching, and other composition are normal
checked Omega machines. The satisfied requirement contributes the public
service-reach, suspension, blocking, and guarded-crash ceilings, while the
binding/provider contract supplies behavior that must refine each of them.
Trust is assigned at admission rather than selected by source spelling.

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
neither physical input is an `Extent`. A generated ABI shell invokes the
bootstrap, which obtains exact geometry and correspondence evidence, crosses
the semantic installation edge once, and calls the source machine with only the
declared semantic values. The installation receipt joins both requirements,
provider and input provenance, generated captures, and selected continuation.
The composed crash, reach, write, work, stack/state, provisioning,
introduction, result-map, and provenance contract enters the bound program
closure just like authored code.

Hosted schemas normally expose neither image nor initial-storage extents. A
freestanding schema may forward them because the selected program must perform
its own provisioning. A receiver-bound entry receives exactly one exclusively
lent instance; no ambient `static` name is introduced. Active stack and receiver
storage remain explicit conserved partitions in the target execution frontier;
source receives only disjoint residual storage, never a qualified extent with a
hidden inaccessible hole.

The physical result is target-authored. For UEFI, bootstrap rejection maps to a
declared `EfiStatus`, normal return from the current Unit semantic continuation
maps to success, and a declared crash remains a crash rather than becoming an
implicit status. The UEFI memory-map/exit adapter is a bounded state graph, not
an unmeasured retry loop: its decreasing attempt term and every non-copy
boot-services capability, allocation, snapshot, and key are threaded through
state arrival contracts. A stale-key outcome returns live custody for retry;
success ends boot-scoped providers while transferring already allocated storage
under the same occurrence lineage. Runtime services and newly claimable final-
map regions follow their own post-exit contracts.

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
mechanism name. `DllImport`, `Syscall`, `VtableSlot`, and similar realizations
must validate against the policy already pinned by the satisfied requirement.
Provider-specific register allocation and footprint certificates remain
implementation evidence behind that published plan identity.

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
`entry(place)` selects an entry-version structural place, while compiler-owned
`separate(...)` performs the closed algebra's partial n-ary composition. Neither
has runtime representation. Identity-preserving claim reshuffles infer;
partition-changing primitives author their theorem and checked wrappers compose
it. Terminal Psi carries each exact preserved-claim mapping, reconstructs its
projection equality, and retains active sum payloads as distinct case-plus-field
paths. It never infers separated composition across independent claims.

Content equations resolve every projection to the exact owner-unique
`Content<A>::project` machine and normalize once per callable outcome and
algebra. `entry` cannot select `result` or an arbitrary expression; projection
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

Routed qualifications name their exact authorized trait requirements in the
domain declaration. Obligation-free domains may be qualified directly with
`as`; predicates must be proved, and routed provenance cannot be fabricated.
See
[`authority_values_and_boundary_evidence.md`](../design_briefs/authority_values_and_boundary_evidence.md).

## Boundary Realization Catalog

Omega has one provider model. A boundary declaration publishes a requirement;
an ordinary checked machine or an irreducible `via` machine satisfies it; the
toolchain derives a candidate `ProviderPlan`; and the selected target profile,
`build.omg`, or installation binding chooses one admitted candidate for the
owned slot. A requirement never embeds that choice.

Compiler intrinsics use the same shape:

```omega
machine CoreSliceProvider::index<T>(items: &[T], index: u64) -> T
    satisfies Slice::index
    via Binding::CompilerIntrinsic;
```

`CompilerIntrinsic` has no authored string identifier. The exact realization
machine already has a resolved package-qualified symbol and normalized
signature. Together with the selected target, that identity keys a sealed
toolchain catalog entry containing the lowering and its checked operational
contract. An absent entry, wrong signature, inapplicable target, unauthorized
origin package, or non-refining implementation rejects.

Other irreducible bindings use ordinary typed compile-time values produced by
target-package machines. `DllImport` carries one object-format locator sum case
whose case keeps a PE library/export pair, a PE library/ordinal pair, or an ELF
object/symbol/version triple inseparable. The satisfied requirement's
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

The standard library is the portable API most application code should use. It
can provide `Console.read_line`, formatting, strings, slices, data structures,
and higher-level process or filesystem helpers. These machines are ordinary
Omega code unless they are explicitly modeling the bottom host edge.

Host packages are the audited bottom edge. They contain imported libraries,
syscall surfaces, startup bindings, and boundary providers.

Typical layering:

```text
application code
  -> standard-library Omega machines
    -> boundary host trait/provider
      -> syscall / imported symbol / firmware jump / loader hook
```

Static vs dynamic linkage is not the same question as boundary vs normal code.
A statically linked standard-library wrapper is still normal Omega code if the
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

There is no compiler-magic `omega::host` package. Target providers live under
`omega::language::std::targets`, satisfy the same public requirements, and are
selected by target defaults or an explicit slot-owner override in `build.omg`.

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
windows_x64 machine User32Bindings::register_window_procedure()
    -> Binding<10, 16, 0>
{
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "user32.dll",
            export: "RegisterClassExW",
        },
    }
}

machine User32::register_window_procedure<machine Procedure>(
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

The registrar's evaluated outbound `CallPlan` separately maps the nominal
`Procedure` binder slot to one declared private native place. A direct callback
argument names a native parameter; a nested callback names a field through its
validated layout identity. The plan never uses binder order as an ABI ordinal,
never appends an undeclared argument, and never stores a byte offset. A nested
field is a private layout demand absent from the source specification; complete
plan validation requires one compatible supply and rejects missing, duplicate,
overlapping, or unresolved rows. The normalized registrar-plan fingerprint is
independent of the selected callback machine. Per-use identity retains that
machine and its entry plan, and lowering joins the two only to emit a private
relocation.

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
