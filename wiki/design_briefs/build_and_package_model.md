# Design Brief: Build And Package Model

Current as of 2026-08-24. `build.omg` is ordinary Omega code interpreted in an
explicit build-host context. It produces inspectable build data and may stage
assets or obtain external inputs through supplied services; it is not a second
configuration language.

`build.omg` is build orchestration, not the compiler's hermetic
[semantic evaluator](build_time_evaluation.md). The former may reach admitted
host services and records their observations. The latter evaluates constants,
proofs, plans, and generators under target semantics with no host reach.

## Design posture

The durable requirements are that build authority is explicit, reach is
checked, observations are retained, and package-controlled code cannot acquire
resolver or admission authority. The exact library decomposition remains
discovery-driven. Prefer ordinary Omega data and machines and remove a
provisional distinction when concrete compiler work shows that a smaller
existing mechanism proves the same thing.

This applies to basic language facilities as well as service vocabulary. Build
logic uses ordinary Omega arithmetic; if all required integer operations can
be discharged as `Exact`, no package-specific arithmetic policy should be
invented. A build-host operation needs an explicit checked authority surface,
but not necessarily a new public `boundary trait`: a pre-existing boundary,
an ordinary provider machine, or a narrow toolchain-owned operation may be the
better representation. Introduce a boundary only when there is a genuine
irreducible external contract with substitutable realizations or trust-bearing
evidence. Examples below describe intended Omega-shaped APIs, not a commitment
to preserve abstractions that implementation evidence makes unnecessary.

## Package declaration and build entry

Every package owns one stable human name declared in its own `build.omg`:

```omega
const PACKAGE: Package = Package {
    name: "arithmetic-kernels"
};
```

`Package` is toolchain-provided ordinary data. The compiler hermetically
evaluates this well-known constant before resolving the package's dependencies
or executing its build machine. Missing, duplicate, effectful,
dependency-dependent, generated, or non-canonical declarations reject.
Directory and repository names are advisory only.
The canonical name begins with an ASCII lowercase letter and otherwise contains
only lowercase ASCII letters, digits, and single hyphen separators, ensuring
that its default kebab-to-snake alias is a valid Omega identifier.

The declared `PackageName` is not security identity. `PackageKey` joins it to
canonical source lineage and is the security identity intended to qualify
package symbols across updates. Managed imports and authored symbols now retain
it. Post-resolution compiler symbols require one existing derivation origin and
inherit its exact authored package/toolchain provenance; truly source-free
symbols remain unresolved. Checked provider-adapter rows now retain a canonical
machine-overload identity and its exact package owner, and every compiler
consumer resolves both without falling back to a short spelling. Provider
selection and compiler-intrinsic toolchain identities are not yet fully sealed.
`PackageInstance` additionally binds exact source content, produced artifact
identity, each closure subject's obligation-semantics identity, locally
re-derived verification results, and disclosed open assumptions. Exact
certificate identity and proof route remain derivation provenance so different
valid certificates may establish the same semantic result. Compiler/toolchain
identity is separate review and cache metadata; it never certifies a result or
seals the instance. Spoof rejection for
same-named packages from different source lineages remains an admission
requirement until those joins are sealed.

Each package may define:

```omega
machine build(builder: &mut Build)
    reaches FilesystemHost, Console
    invokes FilesystemHost;
    invokes Console;
{
    ...
}
```

The tool invokes the exact one-parameter machine with a build-activation handle
and scoped standard providers. `Build` both exposes activation-scoped services
and accumulates the durable build result; ephemeral capabilities never enter
that normalized result. Console logging is a declared service call, not output
silently intercepted by the interpreter.

Unused build-host services are absent from the machine's ordinary
`reaches`/`invokes` contract and are not supplied through its activation.
Whether a particular one-purpose build service warrants a public boundary
trait remains an implementation discovery; no service or authority is ambient
either way.

## Code, not config grammar

`build.omg` uses normal data, calls, control flow, domains, and contracts. It
does not introduce `depends {}`, `target {}`, or another block dialect.

```omega
machine build(builder: &mut Build)
reaches FilesystemHost;
invokes FilesystemHost;
{
    builder.depend(Source::Path {
        location: "../../contracts/uefi"
    });
    builder.target = cathedral::targets::uefi_x86_64;
    builder.roots.bind(
        cathedral::targets::uefi_x86_64::ProgramEntry,
        Application::start
    );

    let input: &[u8] in Path = builder.source.resolve("assets/font.bin");
    let input_bytes: [u8; 4096];
    let input_descriptor: i32 = builder.filesystem.open(input, 0);
    let input_count: i64 = builder.filesystem.read(
        input_descriptor,
        &mut input_bytes,
        4096
    );
    let input_close: i32 = builder.filesystem.close(input_descriptor);

    let generated: &[u8] in Path = builder.output.resolve("font.generated.omg");
    let output_descriptor: i32 = builder.filesystem.create(generated, 438);
    let output_count: i64 = builder.filesystem.write(
        output_descriptor,
        "data GeneratedFont {}\n"
    );
    let output_close: i32 = builder.filesystem.close(output_descriptor);
    builder.output.include_source(generated);
}
```

`BuildSource::resolve`, `BuildOutput::resolve`, the ordinary `FilesystemHost`
surface, and the exact `BuildOutput::include_source` handoff are implemented.
The handoff accepts only an interpreter-retained Output-rooted path and becomes
usable only after matching sponsored staged-tree custody. In package-aware
checked compilation, Omega executes the frozen build prepass once, appends the
exact retained UTF-8 bytes under a compiler-owned `.omega/generated/...`
logical source path, and runs one final ordinary frontend/check pass without
rerunning dependency discovery or `build`. The final source-consumption
commitment includes those bytes and verifies them against retained staged-tree
custody rather than rereading an output path. Resolving a path, writing output,
and publishing generated source remain three separate operations.

The dependency's own `PACKAGE` supplies its name. Its default local import
alias is derived mechanically from kebab-case to snake_case; only a real local
collision uses the exceptional `builder.depend_as(alias, source)` operation.
The alias is name-resolution syntax, never package identity.

Library values such as `Source::Path`, `KiB`, and `Subsystem` carry the
vocabulary. Adding a target option normally extends `Build`/library data rather
than the parser. The compiler-provided build prelude now defines the canonical
`Source::Path`, `Source::Git`, `Build::depend`, and `Build::depend_as` surface as
ordinary Omega declarations. Package orchestration projects those canonical
direct literals hermetically from an immutable root `build.omg`, including an
exceptional validated explicit alias.
Package-aware compiler entrypoints now consume a closed requester-local graph
of alias-to-opaque-`PackageKey` bindings and canonical source roots. That mode
does not scan or combine package-authored dependency rows during import
discovery; source paths route loading but do not become nominal identity.
Package orchestration now translates only a validated resolver-custody closure
into that handoff, re-roots each package over exactly its transitive dependency
subgraph, and compiles the complete closure in deterministic dependency-first
order. Legacy standalone compilation retains only a narrow explicit
`depend_as(..., Source::Path { ... })` compatibility scanner until its canaries
migrate.

## Package-scoped filesystem roots

The build executor supplies two facets through the `Build` activation:

- one immutable root for the exact package source occurrence; and
- one fresh writable staging root for that build occurrence.

When the checked build ceiling admits the exact toolchain filesystem service,
the same activation also supplies it as `builder.filesystem`; no second
receiver object or ambient local provider is involved. The service and both
root facets disappear with the activation.

These are authority-bearing root capabilities, not path strings and not fields
of the durable build result. A source relative name becomes usable only after a
checked route binds its ordinary `&[u8] in Path` bytes to one exact root
occurrence. The resulting rooted path retains that root identity plus canonical
relative bytes. A routed qualification may certify the completed resolution,
but an erased domain over bare bytes cannot supply the missing root identity.

Resolution rejects absolute input, traversal beyond the root, ambiguous root
membership, and symlink escape before host access. `canonicalize` and other
operations returning an authorized path either return a value bound to the same
root or reject. `read_link` returns the stored payload only as inert bytes;
following that payload requires a new checked resolution. Thus an outside link
target may be inspected but never traversed through the package grant.

Writing into staging does not add anything to compilation. Only an explicit
handoff after successful build evaluation and evidence custody may publish a
staged file or tree. Failure discards the staging occurrence. Source roots,
staging roots, open handles, and rooted-path authority cannot escape the build
activation or enter runtime package data.

Canonical evidence renders rooted paths with stable spellings such as
`/source/assets/font.bin` and `/output/font.bin`. Those spellings are
serialization only: package code cannot use their byte prefixes to mint or
select authority. Evidence identity retains the closed root role/occurrence and
canonical relative bytes, never a compiler-host absolute path or process
working directory.

## Normalized `Build` core

The durable schema contains only pipeline-consumed selections and outputs:

```omega
data Build {
    target: TargetProfile;
    dependencies: DependencyBindings;
    roots: RootBindings;
    providers: ProviderBindings;
    outputs: BuildOutputs;
}
```

This is the durable projection, not a claim that the source-visible activation
handle serializes every ephemeral facet it exposes. In particular, its source
and staging roots and admitted `filesystem` service are absent from this
schema.

Hosted versus freestanding, subsystem/image format, default providers, calling
policies, fault supply, and resource supply belong to the selected target
profile. They are not repeated as independently mutable booleans or enums in
each build. In-source `target ... {}` blocks are transitional syntax to remove;
selection and slot bindings belong in `Build`.

The platform's launch calling plan is checked against the generated arrival
bridge, while the explicitly bound source machine is checked against the
target's entry shape. `build.omg` names the target-owned slot and exact source
implementation; it does not repeat the target's register/stack arrival contract
or discover an export by spelling. Binding is selection, not invocation:
`build.omg` supplies neither the machine's receiver nor its entry arguments.

## Target-declared slots

A target profile declares a closed typed slot set. Each slot owns:

```text
SlotDeclaration {
    identity,
    schema,
    direction: EnvironmentToProgram | ProgramToProvider,
    binding_shape: ExactRequirement(requirement)
                 | CompleteConformance(trait)
                 | EntryMachine(entry_shape),
    lifecycle: BuildBound | RuntimeInstalled,
    cardinality,
    required_indices,
    optional_indices,
    reserved_indices,
    installation_authority,
}

EntryShape {
    physical_arrival_requirement,
    semantic_arrival_requirement,
    bootstrap_adapter,
    physical_result_map,
    visible_parameters,
    semantic_result,
    receiver: None | ProvisionedZii,
}
```

### Installed program-local roots

Program-local content introduction reuses this slot and entry mechanism; it is
not a new declaration category. A content-bearing domain names the exact trait
requirement authorized to establish its qualification. One of that
requirement's qualified parameter positions may become a fresh program-local
root only when it is an installed environment-to-program occurrence whose
cardinality is statically enumerable. At an ordinary call the same position is
a precondition and the caller supplies an existing lineage.

The requirement contract publishes an exact finite content expression per
occurrence, or an owner-constrained const family whose selected instance reduces
to one exact expression. The build selects an exact permitted requirement
instance; it does not author an unconstrained capacity. The portable verifier
reconstructs the route, subject position, qualification, content algebra, and
per-occurrence capacity. Installation verification joins that schema to the
slot's exact occurrence set, lifecycle, and epoch and derives the aggregate for
one installed artifact instance. A producer-authored aggregate field is ignored
or rejected rather than trusted.

Cardinality remains deployment policy. Several installed component instances
therefore multiply a per-instance demand exactly as they multiply stack or
storage demand. A component needing one shared cap receives child claims split
from one aggregate parent root; conservation prevents another child without
additional supply. The parent cap is itself scoped to one installed assembly
instance and epoch. Cathedral or another orchestrator derives peak system demand
by composing verified artifact-instance totals across all concurrently live
components and replacement eras. A cumulative cross-epoch ceiling requires
persistent authority carried across epochs.

The installation record retains the selected slot, requirement and satisfier,
semantic parameter position, normalized capacity, exact occurrence identities,
lineages, cardinality, and epoch. This record identifies concrete events but
does not add a second authorization layer: the domain owner's closed route set
remains the sole source-level authority. A later package may implement a public
authorized requirement, but implementation, ordinary invocation, and matching
data shape cannot mint a root.

The installed root ledger retains the verified required-slot closure by exact
member evidence and issues the program-local cohort verifier once. That verifier
has no public constructor. It accepts prebindings only from retained required
roots and seals all eligible members for one lifecycle ledger and epoch in one
transaction; failure returns every lease. The closed cohort derives an ordered
aggregate schema and cardinality while retaining each per-occurrence capacity
expression independently. A compact fingerprint or mutable prebinding count is
never an establishment value.

Before any such event can be introduced, installation derives an opaque exact
closure of the target profile's required build-bound root slots. Missing,
duplicate, extra, and cross-profile selections reject. The closure is
descriptive evidence, not authority. One non-clonable registry authority from
the exact installed-code occurrence then creates the sole root ledger for that
installation scope; dropping the ledger does not make the authority issuable
again. Slot and owner identities are derived from the target declaration by one
shared rule rather than restated as compiler-local numeric coordinates.

Direction is the root/provider distinction. An environment-to-program slot is
an external root; a program-to-provider slot is an outbound service. Lifecycle,
cardinality, and indexing are orthogonal to direction. Program entry, reset
vectors, interrupt vectors, callbacks, and ordinary providers therefore use
one binding model without becoming one undifferentiated slot kind. An entry
machine binding additionally lets a target adapt its physical arrival contract
to the smaller source signature it deliberately exposes.

An installable artifact explicitly binds every required build-bound slot:

```omega
machine build(builder: &mut Build) {
    builder.target = windows_x86_64;
    builder.roots.bind(
        windows_x86_64::ProgramEntry,
        Application::start
    );
    builder.select_provider<windows_x86_64::Console, TestConsole>();
}
```

`Application::start` is the exact machine selected by the entry-machine slot.
It may be free or carry one `&mut self` receiver according to the slot's entry
shape. `TestConsole` is the nominal provider type selected for the complete
`Console` surface; its ordinary conformances and `via` leaves determine the
derived plan. Binding shape is declared by the slot, not inferred from the
trait's current requirement count. Exact slot consumers can cite only the
selected requirement's normalized contract; they possess no conformance
identity from which trait laws could be cited.

The physical arrival requirement, semantic arrival requirement, and selected
source entry are different layers, not competing entry identities. For example,
a hosted profile records:

```text
slot:                          windows_x86_64::ProgramEntry
schema:                        HostedApplication
physical arrival requirement: WindowsProcessEntry::enter
physical calling policy:       WindowsX86_64
semantic arrival requirement:  ProgramStorageEntry::enter
bootstrap adapter:             WindowsProgramBootstrap
physical result map:           WindowsProcessExitMap
visible parameters:            ()
receiver:                      None | ProvisionedZii
binding shape:                 EntryMachine
```

The compiler generates the physical ABI shell and joins it to the exact
target-authored bootstrap adapter; it does not synthesize a meaning for
platform handles. The combined bridge derives a complete crash, reach, write,
work, stack/state, introduction, provisioning, and provenance contract and
composes it with the bound application closure. At launch, the environment
supplies physical values under the physical requirement. The adapter validates
them, installs scoped providers, establishes the semantic arrival, and supplies
only the source arguments declared by the schema. Its authored result map turns
pre-handoff rejection and normal semantic return into exact physical results.
A free source entry receives no implicit state. For an attached entry with one
`&mut self`, the bridge derives storage beneath an admitted entry root,
constructs exactly one ZII-valid receiver, and lends it for the activation. The
receiver is never globally nameable. A root used for receiver or active-stack
storage cannot also be forwarded whole to the source entry; the bridge retains
that partition in its execution frontier and forwards only an exact disjoint
residual. Generated entry code is never outside portable demand checking.

Receiver provisioning is occurrence-local. The receiver's nominal `data`
declaration remains pure; the generated bridge records storage, qualification,
lineage, and backing for the one provisioned occurrence rather than attaching
root authority or a storage class to the type.

A freestanding schema may instead publish image and initial-storage roots in its
visible parameter list. Those are ordinary arguments to the selected source
entry only because that target intentionally makes provisioning the program's
job. A hosted program sees neither extent by default.

The selected target determines the required-slot closure. Binding a slot owned
by another profile rejects regardless of mutation order. Duplicate bindings
report the first binding site; a missing required slot names the exact slot.
Package/library builds bind no roots. Runtime-installed slots may remain open,
but installation must validate the same binding shape, portable demands,
target supply, authority, and lifecycle before publishing reachability.

There is no `main`, `Main::run`, uniquely visible export, special entry field,
or ambient `static`. Project templates may write ordinary slot bindings, but
the language and build evaluator perform no entry discovery.

Stack demand is derived from WCSU and compared with the target's supplied
`StackPlan`; ordinary `build.omg` files do not choose a stack size. An explicit
target-supply override is deployment policy. When fixed platform supply is too
small, diagnostics identify the responsible call path and require a program
change rather than suggesting a nonexistent configuration knob.

## Provider selection

Selecting a target package also selects that package's ordinary default
provider types. A build, test harness, or component manager that owns a service
slot may override that slot with another admitted provider type. This is scoped
dependency injection, not row construction: conformances and `via` bindings
declare the provider, the toolchain derives its normalized `ProviderPlan`, and
configuration selects the already-declared candidate.

Requirement declarations never select their own provider. In particular, a
`boundary operator` carries no provider clause: checked satisfiers and
`satisfies ... via <Binding>` leaves declare candidates, and this build/target
slot mechanism chooses among them. The retired top-level
`provider Name : Category;` declaration and operator-local `provider Name`
clause are bootstrap syntax from a parallel primitive registry and must not be
preserved as a second selection path.

Selection identities are nominal; binding identities are normalized evaluated
values. `select_provider` retains two structural type paths through parsing,
resolves them to an exact boundary-trait symbol and provider-data symbol, and
carries their compiler-derived package owners into selection. Plans match only
exact `(package, canonical path)` slot and provider identities; authored
spellings remain diagnostic data and there is no leaf-name fallback. Checked
adapters likewise bind normalized overloads to exact package owners for the
realizing machine, provider type, selected service schema, and requirement
owner. The complete target-scoped binding producer closure and result enter
final admission. Changing a typed foreign locator, evaluated plan, or sealed
compiler-catalog entry changes artifact identity and triggers fresh admission;
`build.omg` cannot rewrite any of them while selecting a provider.

`Build::select_provider<Service, Provider>()` is ordinary typed API vocabulary.
It performs a type-per-slot override; users do not repeat every default and
cannot append or mutate derived plan rows.

The same selection owns componentization. A selection is `fused` by default;
the build may instead request `independent` emission through the typed `Build`
API. This mode is semantic rather than a packaging hint: an independent edge
must close its component graph, retain symbolic requirement imports and
exports, publish entry/leave and resource demands, and satisfy installation and
replacement obligations. Provider source cannot select this mode for itself.

The exact closed requirement application is the stable slot identity. A family
of independently selectable slots uses ordinary closed static arguments with
nominal or declared-domain identity; it never uses an authored ordinal, string,
vtable index, address, or artifact generation. One package may contribute any
number of independently selected roots.

The compiler derives the closure from the selection. Chosen satisfied
requirement identities become exports; requirement calls leaving the closure
become imports. A concrete-identity edge stays inside, pulls its target into
the closure when legal, or rejects. Duplicable immutable dependencies may be
shared or copied. Mutable state and linear custody have one owner and therefore
must be fused above every dependent closure or mediated by a selected service.

An indexed provider requirement is one schema rather than one ambient slot per
type application. In particular, `ResidentContentTransfer<P, T>` is selected
through one ordinary provider binding. The provider may implement the generic
requirement or publish the exact supported application family. A concrete
artifact records every normalized application it uses; a separately compiled
generic library exports symbolic applications over its own parameters. Final
composition substitutes reachable arguments, reconstructs the closed demanded
application set, and rejects unless the selected provider covers every member.
Installation records the exact concrete applications and issuance occurrences
it admits.

The application closure is verifier-derived, never a producer-authored total,
and does not create one slot per monomorph. An indexed slot family is introduced
only when distinct applications genuinely require independent provider
selection. Otherwise type indices refine one selected provider's obligation;
they do not multiply deployment choices or require ambient conformance search.

Provider selection also determines executable TCB provenance. Static selection
of an opaque in-process realization contributes a known executable entry even
though source reach is unchanged; selecting an isolated realization contributes
an endpoint instead. A package that fixes such a selection exports that
dependency transitively to every consuming artifact. A checked wrapper may
narrow its API but cannot erase the selected implementation's evidence,
execution scope, or containment guarantees.

The artifact manifest separates known entries from completeness. Known entries
record static versus Omega-mediated runtime origin. An uncontained opaque
in-process entry makes the caller-address-space manifest incomplete and names
the provider responsible, because that code may introduce further executable
bytes without an Omega admission. Build profiles evaluate exact identities,
platform-baseline policy, implementation evidence, scope completeness, and
required memory/termination/fault/resource containment. They may permit and
mark an artifact or reject it before installation.

The current compiler integration takes those deployment-owned facts through a
programmatic `ExecutableTcbBuildPolicy`, not through new language syntax. After
provider selection it validates opaque admissions against the exact selected
rows, evaluates the optional profile, and carries the sealed acceptance to the
filesystem installation gate. Named profile selection in `build.omg` remains
ordinary `Build` API design; no method spelling is frozen here.

Terminal Psi carries fingerprinted crash causes, guarded routes, and the
statically known abandonment-frontier lower bound at each site. Build and
installation may reject artifacts whose routes violate a deployment profile,
but those facts do not establish safe continuation after a crash.

A target fault plan may support restart only for an independently verified
closed-custody component, or for explicitly crash-safe shared resources and
external reset/transaction protocols. The installation record identifies the
selected isolation and restart evidence. Without that evidence an uncontained
crash terminates the artifact's execution domain. Co-location, handler
mechanics, and physical isolation remain installation facts rather than
portable meanings of `crashes`.

A filesystem path or unresolved loader name is not executable identity.
Ordinary package policy rejects an opaque provider whose content, signer, or
profile-owned platform identity cannot be pinned. Explicit admission of a known
opaque binary still expands the TCB; identity prevents substitution and enables
revocation, not behavioral verification.

## Build-time authority and execution split

`build.omg` may perform authorized package-local staging and may use filesystem,
network, process, signing, or other build services when their requirements are
explicitly selected and admitted. No such service is ambient. Package
resolution still consumes exact source identities; granting a general network
operation does not turn an unpinned response into a dependency identity.

```text
tool-in-hand
  -> interprets build.omg with selected build-host service slots
  -> receives augmented Build data, staged assets, and observation receipts
  -> resolves/fetches pinned dependencies
  -> compiles, links, emits, and records artifacts
```

Build-entry admissibility uses the complete normalized machine contract for the
selected build-host providers. Unlike semantic evaluation, its service reach
need not be empty. Provider trust, authority roots, retained storage, resource
bounds, failure, and ordinary `terminates` guarantees must fit the build
executor's policy. A blocking service must publish the progress/failure premise
under which the build entry can satisfy `terminates`.

Dependency source retrieval precedes dependency-code execution. The root build
binds an alias to an exact content identity, local path, or repository revision;
the resolver fetches and unpacks that source under resolver-owned authority.
Downloaded `build.omg` code never receives the resolver's network or archive
authority. The resolver is consequently a security boundary in its own right:
retrieval identity, revision resolution, archive path containment, expansion
limits, and destination writes are checked and receipted rather than treated as
package plumbing.

Each dependency build then runs with its own explicitly supplied, package-
scoped providers. It does not inherit the root build's filesystem, network,
process, signing, secret, or acceptance authority. A general host filesystem
provider is not a compatibility escape; source and output roots remain explicit
and anything broader is a separately admitted provider visible in policy.

Generated Omega source crosses no authority from the build into the program.
It is compiled and checked as ordinary source under the consuming artifact's
runtime reach, crash, work, conservation, and trust ceilings. Build-time access
to a network or schema file therefore cannot authorize generated runtime code
to reach either one.

## Build observations and reproducibility

Reproducibility is a property of selected build operations and their evidence,
not of a service name. Reading a content-captured file and enumerating a live
directory may both use `Filesystem` while providing different replay
guarantees. Fetching an expected content hash and requesting an unpinned
`latest` document may both use `Network` with the same distinction.

Every build-host operation publishes one normalized observation ceiling:

```text
BuildObservationClass =
    Hermetic
  | Receipted
  | Volatile

Hermetic < Receipted < Volatile
```

- **Hermetic:** the operation observes no external build-host state.
- **Receipted:** every possible observation returns sufficient value/content
  evidence for replay.
- **Volatile:** some possible observation lacks complete replay evidence.

This is an operational-contract axis on the requirement/provider plan, not a
keyword in `build.omg`. Selected implementations must refine the requirement's
published ceiling. Standard release-capable providers are `Hermetic` or
`Receipted`: clock, randomness, environment, directory enumeration, and similar
observations must return replay evidence. `Volatile` remains an explicit
development-policy class, never an ambient convenience provider and never
eligible for a source-rebuildable release.

The build entry records the same three columns used by other resource and
operational plans:

```text
ceiling    join of statically reachable operation classes
realized   join of operations actually reached
evidence   observation records and replay receipts
```

The ceiling is computed for the concrete build invocation after selected target
and configuration values are known. A branch proven unreachable contributes
nothing; an unresolved branch contributes conservatively. Release policy may
therefore reject `ceiling > Receipted` before executing a long build. The
realized class still reports what happened and may be narrower than the
ceiling.

Calling a volatile operation is observable even when its returned value is
discarded. Effectful observations are never removed merely because their value
looks dead.

### Record replay and source rebuildability

Two graph verdicts remain separate:

```text
ReplayableFromRecord(build) =
    realized <= Receipted
    and every recorded input is available

RebuildableFromSource(build) =
    ReplayableFromRecord(build)
    and every input receipt traces to a declared reproducible root
    and every dependency artifact is RebuildableFromSource
    and toolchain/target-provider inputs are pinned
```

A current build that consumes a hashed dependency artifact may be replayable
from its record even when that dependency was produced using a volatile clock
or unreceipted network response. The whole graph is then not rebuildable from
source. Recording a volatile value makes it auditable; it does not manufacture
source provenance.

The unified artifact publishes:

- the static observation ceiling;
- the realized observation class;
- receipt and recorded-input identities;
- `ReplayableFromRecord`;
- `RebuildableFromSource`; and
- the first failing provenance edge for either verdict.

The current compiler implements only the first conservative execution rung.
It derives the selected build machine's static filesystem-observation ceiling
from exact reachable canonical toolchain service identity and retains whether
the evaluator actually invoked that host family. Because the scoped real
filesystem provider does not yet emit a replay transcript, both reachable and
realized filesystem use classify as `Volatile`; pure and console-only builds
remain `Hermetic`. Declared but unreachable filesystem slack does not fabricate
an observation. Console-only execution receives no real filesystem provider.
Statement- and value-position calls can enter the real filesystem provider only
through an exact requirement symbol owned by canonical toolchain
`filesystem_host.omg`; package-authored lookalikes remain ordinary unsupported
calls even when an evaluator grant exists. The canonical signature maps to one
of 50 closed, explicitly tagged operation identities; both providers match the
same enum exhaustively, while aliases and platform alternatives stay distinct.
Authorized results from `canonicalize` and `final_path_name_by_handle` remain
bound to their exact root or reject. `read_link` returns only inert payload
bytes; using that payload as a path requires checked resolution through a root.
Observation-summary schema v19 carries operation-attempt schema v18: an ordered
successful-run call-start trace of exact provider, operation tag, normalized result,
post-operation error state, and every direct scoped path authorization.
Authorized paths retain exact operand/access, closed Source/Output root, and
canonical slash-separated relative UTF-8 bytes without physical root spellings.
Grant denials remain distinct; host errors retain prior authorization without
fabricating a refusal. Duplicate/conflicting/unresolved roots, unrepresentable
paths, and the 16 MiB aggregate retained-path ceiling reject before host access;
ceiling exhaustion non-catchably halts the evaluator. Granted evaluator
failures retain partial usage and typed outcomes; worker
failures mark evidence unavailable. Omega emits fixed non-admission counts and
no review row. Descriptor, native-handle, and find-handle inputs retain exact
Resolved/Null/Unknown disposition immediately after successful typing. A later
preparation failure keeps the completed prefix, while a fully prepared call
must reproduce the exact logical-handle plan; successful opens mint monotonic logical
lifetimes, duplicates and borrowed views bind their source, and successful
closes retain all invalidated lifetimes. Raw provider-token reuse cannot reuse
logical identity, failed closes retire nothing, and successful use of an
otherwise Unknown token traps. A token live in a different logical domain
rejects before provider access. Virtual duplicates share the source cursor. Real
descriptors retain their rooted write grant through duplicate and borrowed
views; content, extent, metadata, ownership, and host-lock mutations deny before
sponsor or host access when the origin was admitted only for source reads.
`open_at`/`unlink_at` accept only one portable relative component. Real path
outputs either reconstruct one lossless root-relative result under the same
grant or reject. Successful descriptor/find/native-handle
results retain only their logical identity in evidence; provider token integers
do not survive. Non-handle
results and failed handle-result sentinels remain exact scalar values, and
package commitments type-tag both result lanes. Fully prepared calls whose
evidence reservation succeeds retain ordinal-ordered non-handle
I32/U32/I64/U64 scalars plus exact immutable write/FILETIME payloads;
validated at-family components retain their exact portable bytes. Rooted and
path-alias spellings never enter the payload lane. A separate rooted-resolution
lane retains exact operand ordinal, closed Source/Output identity, and canonical
relative bytes before physical provider-path lowering. It survives later
preparation failure and is exactly cross-checked against the fully prepared
call's compiler-private semantic sidecar. It is not grant authorization;
authorization separately retains access and the canonical rooted location
selected after symlink and nested-root resolution. Mutable byte and i64 carriers
retain a distinct complete resolution-time snapshot as their operands are
evaluated, so a later preparation halt keeps the prefix. Mutable byte carriers
separately retain complete provider pre/post capacity, including unchanged
tails, and mutable i64 carriers retain exact provider pre/post values. Provider
pre-state follows all authored argument evaluation because a later argument may
alias an earlier carrier; the snapshots need not match. Post-state follows
provider return or halt, including unchanged input-only ABI carriers. A
separate 256 MiB aggregate operand-evidence sponsor covers immutable, path-like,
rooted-resolution, exact returned-path bytes, one mutable resolution copy, and
both provider copies. Directory-entry names,
symlink targets, find patterns, and other non-rooted path-like operands occupy
their own ordinal-tagged lane rather than impersonating rooted authorization or
payload. Each successfully typed non-handle scalar, immutable payload, and
path-like operand is retained as preparation advances, so a later preparation
halt keeps the completed ordinal prefix; a fully prepared call must reproduce those rows
exactly before provider access. Prior or nested staging effects remain cleanup-
contained. Package commitments frame these rows without rendering payload
bytes as text. Provider successful-write branches retain exact meaningful
`read_link`, canonical, and final-path bytes without terminators or stale tails,
with exact output ordinal, closed kind, and Complete/LimitReached disposition.
Provider-known target length distinguishes exact-fit from truncated `read_link`;
failure and insufficient-capacity returns add no row. Package-rooted builds
reject canonical and final absolute outputs, while `read_link` remains inert.
Successful `read`/`read_at` calls designate the exact zero-offset region of the
already-retained mutable post-carrier as sequential or positioned file content.
Its length must equal the nonnegative result; EOF retains an empty row and
failure retains none. The row copies no bytes and adds no sponsor charge.
Package commitments bind its kind and coordinates plus the referenced mutable
post-state. `read_dir` similarly designates exact `DirectoryRecords`, while
`find_first` and entry-producing `find_next` designate complete 320-byte
`FindEntry` records. Directory EOF and no-entry find returns retain empty rows;
failed enumeration retains none. Successful path, descriptor, and no-follow
metadata operations additionally retain one canonical target-neutral row with
all 14 `StatRecord` fields. The compiler obtains the selected target's checked
`StatLayout<StatRecord>` from private typed/layout state, validates its exact
fields, widths, bounds, and non-overlap, and passes only that closed descriptor
to the Psi evaluator. The evaluator zeroes and fills the complete authored ABI
carrier (whose API minimum is 144 bytes) through that layout and checks it
against the semantic row; package
commitment binds both. Filesystem-reaching builds load and check the standard
layout policy before execution. This private seam does not create a public IR
contract or nominal Chi. Complete replay remains absent, so the record remains
non-receipted.
The first bounded replay rung handles exactly one successful Source-rooted,
flags-zero `open` -> `read` -> `close` chain. It reruns the build without any
filesystem provider, supplies recorded results and read bytes, reconstructs
logical descriptors, and requires exact event order, inputs, outputs,
exhaustion, and final result. The summary binds this successful partial replay.
Compiler replay-record v1 canonically retains every lane of the verified chain
and strictly recovers only the current semantic schemas and exact source-read
shape. Review-baseline capsule v2 keeps those opaque bytes across restart,
binds their commitment to the parent build observation, and accounts them under
one aggregate capsule ceiling. The checksum and association are custody checks,
not authenticity or admission. This does not change the observation class:
replay from reopened custody, broad operation replay, output-tree reproduction,
and a complete replay verdict remain absent.
Byte-valued inputs are evaluated once by the shared preparer and reject above
the evaluator's current 16 MiB sponsor ceiling before provider cloning/
allocation. Raw transfer counts use one checked conversion and
reject negative, wrapped, or above-ceiling values before allocation. This is
not a language limit. A shared closed preparer checks exact arity, consumes all
authored operands once in left-to-right order, rejects wrong kinds, and retains
validated mutable cells/capacities, including fixed ABI inputs such as Win32
`OVERLAPPED`, before either provider or grant access. It
includes provider-unused ABI operands, and the canonical source test pins all
50 operand schemas and result widths. Canonicalize enforces its declared
1024-byte `PATH_MAX` carrier at that gate. Process memory and CPU remain
unbounded.
Scoped hard links require write authority on both names, so source custody
cannot be bypassed by aliasing a read-only source inode into writable staging.
Namespace-mutating calls authorize the canonical parent plus the leaf they
actually mutate instead of following an existing leaf symlink. Open/truncate
and other target-following calls retain full canonical target authorization.
Package review now owns one session-wide object/namespace account across the
closure and shares it with every package build. Package output roots count as
entries; namespace entries are otherwise counted by name, regular-file bytes
once per object, symlink payloads as bytes, and unlinked objects until their
last open descriptor closes. Mutations reserve the candidate account state
before the host operation and commit only after success. Ceiling refusal has a
distinct resource-exhaustion outcome. The initial compiler ceiling is 4,096
entries, 256 MiB total logical bytes, and 256 MiB per object extent. A
per-package or path-summed quota is not a valid substitute.
This summary is compiler-issued execution evidence kept outside canonical
capability/API comparison bytes. It is not a receipt and does not claim either
replay verdict. Sponsored package review does retain a versioned commitment to
the complete fresh Output tree after successful evaluator/provider teardown
and before deleting the disposable session. The canonical tree binds sorted
Output-relative portable UTF-8 paths, empty directories, canonical file modes,
file lengths and content digests, and exact validated self-contained relative
symlink spellings. It excludes host roots, timestamps, ownership, ACLs, ambient
permissions, inode identity, and hard-link topology. Capture requires a
quiescent sponsor and cross-checks namespace kinds, extents, and object groups;
unknown kinds, external symlinks, custody disagreement, and bounded-resource
excess reject. An empty successful tree is committed explicitly. The package
observation commitment binds its digest and topology-independent unique-content
count. The compiler-owned review row retains the complete canonical tree behind
private fields and can materialize it into an existing empty concrete directory,
then independently re-inspect exact paths, kinds, modes, targets, and bytes
before returning the same commitment. Hard-link topology is neither retained
nor leaked through the count. This is output-tree custody and replay only. A
`Receipted` row still requires canonical operation replay, retained observed
inputs, generated-output handoff, and a complete record replay checker. This
custody rung does not exclude a hostile same-user process racing the review
session.

Policy can consequently distinguish an ordinary development build, a release
that requires record replay, and a supply-chain release that requires
transitive source rebuildability. These are graph checks, not transitive
mutation of one package's local class.

Source/provenance triage is independently review-only. The package orchestrator
now compares compiler-issued closure rows directly: capability/API byte drift
and source-lineage replacement are blocking, while unavailable old source,
retained dangerous authority, changed build observation, and introduced or
changed representation-TCB evidence recommend audit. Initial admission uses an
empty baseline and applies the same dangerous-authority and representation-TCB
recommendations. The bounded advisory projection contains canonical package-key
commitments and closed compiler vocabulary only; it rejects rather than
silently truncating. Source code enters a separate hostile-data packet derived
only from exact-key resolver custody. That packet binds complete immutable
resolutions and renders deterministic raw-tree changes, including executable,
directory, symlink, entry-kind, and line-ending distinctions, under independent
capture, metadata, line, algorithm-work, trace-memory, and output ceilings.
Binary or non-UTF-8 changes are commitment-visible but model-incomplete and
therefore require standalone audit. Byte escaping protects the packet grammar,
not the model from semantic instructions embedded in reviewed code. The
review-input join now requires the complete candidate custody and compiler rows
to agree bijectively on exact key and immutable resolution. A shared validator
also rejects duplicate reviews, package/projection identity mismatch, mixed
deployment targets, and mixed compiler-executable commitments before either
capability comparison or source rendering. Recovered baseline custody is
validated against its compiler row, and unavailable old source is derived from
absence. Its aggregate ceiling preserves separate compiler and hostile-source
frames. Invoking a model remains future work. Model output is
policy advice, never package evidence or proof of review.

## Package admission projection

The compiler derives a review proposal from checked semantic state. A total
internal `PackageAdmissionProjection` converts that state into canonical
package-visible rows and rejects unresolved requirements, unbound identities,
compiler-private handles, and any fact it cannot represent exactly. Those rows
remain `CompilerIssuedPackageReview`: useful deterministic input to review, but
never authority and never directly promotable into a `PackageInstance`.

Sealing is trust by checking. A consumer starts from the exact requested source
subject and exact produced artifact, reconstructs their canonical obligation
set under the selected semantics, and checks the exact retained certificates.
The produced subject is the canonical package artifact for ordinary package
claims; final-realization claims additionally bind the exact native artifact and
Terminal evidence. It is never merely a compiler-authored verdict packet.
Concretely, the ordinary artifact is the complete versioned package-admission
semantic row set under one exact package key, target, dependency closure, and
obligation schema. Compiler review may emit candidate bytes in that same
vocabulary, but only independent reconstruction from the exact source subject
and byte-for-byte comparison gives them evidentiary force. Source bytes,
certificates, proof routes, compiler observations, and decisions remain separate
subjects or provenance. The current incomplete review-v52 rows are not promoted
by terminology.
The resulting package-evidence record is a cache of this re-derivable fact, not
an assertion a verifier may ask consumers to believe. Exact certificate bytes,
proof routes, and kernel dependencies remain derivation provenance; semantic
identity records the subjects, obligation applications, discharge results, and
open assumptions.

Local obligation reconstruction may consume the earliest coherent
compiler-owned representation in which each obligation is semantically
complete, including private pre-Psi or pre-Terminal state. This checker seam may
move with compiler internals. Only the versioned canonical obligation ledger,
exact replay subjects, certificates, results, and open obligations persist; raw
IR and compiler-private handles do not. A nominal Chi stage is not created just
to stabilize that seam. It becomes warranted only if implementation discovers a
real reusable semantic boundary; an existing coherent stage such as Exact is
preferred whenever it preserves the same meaning with less machinery.

The first bounded replay component exists at Terminal Psi. The verifier exposes
one complete ordered obligation set for executable operations, call and nominal
cleanup requirements, and contract guarantees; each row retains exact owner,
class, proposition, assumptions, and reconstructed axioms. Verification consumes
and retains the same set. A canonical ledger binds it to exact Terminal-Psi and
source-backed verifier trust-graph identities while leaving certificate route
as separate provenance. Decoding is not acceptance: the consumer reconstructs
the set and requires exact equality. Terminal artifact identity retains the
ledger fingerprint separately from semantic and proof identities, and the
replay lowering path performs that comparison before proof checking. This
component does not establish ordinary package capability/API evidence or
authorize a `PackageInstance`.

The closure is heterogeneous and transitive. Every package or other subject
retains its own obligation-semantics and evidence-schema identity. Checked
dependency obligations compose upward, while open obligations remain visible
at every parent and are re-evaluated under each consumer's policy. A producer's
admission decision never settles a downstream consumer's decision. A versioned
schema migration may support incremental reuse only through a checked delta
that identifies unchanged, added, strengthened, reinterpreted, retired, or
encoding-only obligation classes. Unknown or meaning-changing deltas force
reconstruction; newly exposed gaps are open obligations, not automatic
admissions.

The compiler-issued envelope also carries a separate canonical commitment to
the reconciled package/alias input graph and every exact package or toolchain
source path and byte sequence consumed by the frontend. Absolute cache paths
and load order are excluded. Source-only changes therefore change consumption
identity without contaminating the normalized capability/API comparison bytes.
The resolver retains exact immutable source resolutions and rechecks whole
snapshots plus compiler-retained bytes around compilation; an OS isolation
boundary is still required against a deliberately hostile same-user racer.
Physical local snapshot custody is keyed by both canonical source lineage and
content identity. Byte-identical packages from different lineages therefore
keep distinct compiler roots even though their content commitments agree; one
physical source root is never ambiguously assigned to two package identities.

The envelope separately identifies the producer executable file bytes observed
before and after closure review, rejecting if those observations differ and
retaining one verified commitment on every emitted row. That commitment stays
outside capability/API comparison bytes. It is useful provenance for exact
comparison and replay policy, but is not compiler certification, compiler-source
identity, a reproducible-build receipt, or proof of the executable image already
loaded by the operating system. No stronger producer-pedigree join seals a
`PackageInstance`: direct source-and-artifact checking does. Reproducible builds,
toolchain closures, signatures, and execution measurements may remain separate
supply-chain or incident-response metadata. An admission may cite an exact
artifact solely to scope the semantic obligation being assumed; the artifact's
pedigree never proves that obligation.

Ratified 2026-08-24: the implementation should read each fact from the earliest
coherent compiler-owned representation in which its semantics are established.
Exact structural identity may come from private pre-Psi typed or resolved
state, while checked acceptance, effects, proofs, and realization come from the
stage that establishes them. The projector joins those facts only after
successful checking. Different rows may therefore come from different internal
representations; the final projection must be total, but no single intermediate
representation must contain every row. This may couple the checker to unstable
compiler-private representations: the checker is part of the compiler and
moves with them. That coupling does not make an internal representation a
package format or public compatibility surface. Unchecked syntax, diagnostics,
and convenient-but-unsettled shapes remain inadmissible as evidence.

This projection is not another public IR stage and does not warrant a nominal
Chi stage merely for collection or format stability. It has no execution
semantics or transformation pipeline of its own. A future shared stage is
warranted only if independent consumers, shared invariants, or transformations
establish an actual semantic boundary. Psi may repeat the same invariant as a
downstream backstop without becoming the mandatory reconstruction source for a
fact already complete in an earlier compiler-owned representation.
Conversely, discovery may place more rows in an existing coherent
representation such as `Exact` when that simplifies the compiler without
erasing meaning.

Canonical rows may carry compiler-issued explanatory source coordinates without
making those coordinates semantic identity. Paths are canonical UTF-8 and
relative to their package or toolchain source owner; spans are exact byte
offsets into compiler-consumed source. The coordinates remain outside canonical
capability bytes, but changed-row conflicts bind the exact old/new coordinates
shown to the reviewer. Dangerous-authority rows include the toolchain authority
declaration and package exposure declarations. Generated symbols follow their
authored derivation origin, while compiler-derived rows state a closed reason.
Ordinary projection retains the exact declaration symbol beside each semantic
row and sorts the pair; dangerous-authority projection retains the exact service
declaration and exact exposing callables while deriving the row. No later source
join reconstructs those coordinates from reduced nominal identity.
Provider candidate derivation captures a compiler-internal sidecar beside each
semantic plan: exact boundary-schema and optional nominal-provider symbols, and
the exact requirement plus realizing machine for every external or
checked-adapter row. Review v41 encodes those schema, provider, requirement, and
realization declarations as package-qualified nominal identities; readable
plan and overload strings remain operational/audit data. Projection verifies
each declaration against the selected plan's exact package owner, or against an
exact authored toolchain-source identity when the plan carries no package
owner. Package-less user source, unresolved/source-free ownership, and owner
drift reject. Explanatory source custody records each exact requirement
declaration separately from its realizing machine, preventing a provider row
from retaining only its implementation anchor. Selection
and canonical sorting keep that pair intact, adding exact authored build/target-
default sites or a closed reason for an implicit unique choice. The resulting
selected-provider row may mix authored coordinates and compiler-derived reasons
without reconstructing provenance from reduced names, schemas, or fingerprints.
Nested use sites remain incremental provenance carriers in existing Psi stages
or compiler-internal sidecars, not a reason to create nominal Chi.

Proposition and named-evidence projection is the concrete model for this
cross-representation rule. Typed proposition applications own structural
declaration symbols, binder arguments, and ordinary value-expression
arguments. Checked proof facts own acceptance, evidence-term and witness-
interface routing, and proof/admission disposition. The package projector joins
those facts into one canonical row without making either source representation
public. Checked display strings remain diagnostics and may never become
package identity. Where a witness-interface argument is currently retained
only as text, the responsible existing typed or checked representation must
gain a structural carrier before that form is admissible; parsing diagnostic
text back into semantics is forbidden.

Package-visible structural identity follows the same rule: every non-binder
nominal in a public type is qualified by exact package ownership, an exact
source-backed toolchain commitment, or an unresolved marker, while generic
binders are alpha-normalized without an invented owner. The compiler joins a
toolchain nominal through private `SourceId` state, but only the canonical
source commitment enters review bytes; a missing join rejects exact review
rather than degrading to the generic toolchain marker available to weaker
compiler-local identity. The compiler's 22 exact root builtin-type slots instead
encode closed compiler atoms, selected by root position and `BuiltinType` kind
rather than name. Package-authored lookalikes and source-free generated symbols
remain unresolved. Carry permissions use closed enum atoms in a non-nominal
tagged lane. Value domains, layout atoms, and other source-free compiler
semantics require their own closed structural carriers before they can enter
package evidence. Arithmetic domains and aggregate carry policy are already
closed enums; rendering their compiler-owned labels is not an authority hole.
Typed domain constraints now distinguish declared, carry, closed value-domain,
and `OmegaLayout` subjects. Layout retains a closed grammar and an exact
structural schema type with its declaration symbol. Symbol-backed declarations
remain declared regardless of diagnostic spelling. Review v41 encodes the
compiler variants structurally and rejects legacy/unclassified layouts,
malformed subjects, residual const calls, unsupported index forms, and missing,
duplicate, or incomplete checked index selections.
Review v41 also parses the compiler-reserved canonical-const transport atom
back into a closed type-and-encoding term and excludes its diagnostic display.
Decimal const leaves become numeric terms. Both forms are legal only under an
exact declared const parameter; fixed-array and open-expression binders must
reconcile uniquely to the exact alpha-normalized telescope. Residual const
declarations and unrelated source-spelled leaves reject. The transport atom is
never itself package identity.
Review v42 and canonical row v2 close the corresponding proposition-binder
split. A concrete type argument is a structural type identity, so compiler builtins use closed atoms
and authored nominals require exact package/toolchain-source ownership. A
machine argument remains an exact nominal declaration. Unresolved ownership is
diagnostic state inside ordinary compiler identity only: exact review type
projection returns no identity, and the canonical encoder rejects any unresolved
nominal that reaches it.
Public data projects its supply, generic shape, properties,
fields/variants/payloads, relevance, and stable numbered and retired identities.
Those numbered ordinary-data identities are
the wire contract; the retired standalone `wire data` form is not projected as
a duplicate API. Any public quotient, default-domain proof fact, or static
machine/proposition parameter rejects until the projection has an exact row.
Public domain rows likewise retain exact declaring-package identity,
alpha-normalized type/const binders, carrier type, and index arguments.
Synthesized semantic paths retain an authored provenance span without replacing
their canonical spelling with the source substring. Transparent aliases
recursively flatten to sorted, deduplicated package-qualified atoms. Authored
toolchain nominals bind a canonical toolchain-relative source path plus exact
source-byte commitment in review evidence; this records their semantic origin
but does not make producer pedigree authoritative. Compiler carry aliases
expand to closed `CarryPermission` atoms rather than invented nominal owners;
valid package declarations cannot enter that lane merely through a resembling
diagnostic path. Whole compiler/toolchain commitment remains separate.
Predicate-body presence and
currently representable structural expression/membership facts retain the
domain carrier and exact
package-qualified member/domain identities. A typed fact is admissible only
when it has exactly one checked definition row, one fact-keyed ownership
record, and exact checked dependency places for nested member paths. Callable
or proposition-shaped applications, semantic roles, and domain operators fail
closed until their authority and exact rows are settled.
Closed compiler-owned classifications and authorized establishment routes
retain exact route kind plus package-qualified trait and requirement identity;
alternative routes are canonically sorted and deduplicated.

Package-owned public traits retain exact identity, boundary status,
alpha-normalized lifetime/type/const binders, ordered package-qualified parent
applications, and ordered machine/operator requirement signatures. Requirement
rows retain lifetime arity, parameter names and modes, package-qualified
lifetime-sensitive signature types, and fixed operator spelling plus exact
declared service reach, installation-bound status,
synchronous invocations as exact non-`self` parameter ordinals or
package-qualified services, suspension, blocking, and termination. Progress
premises retain package-qualified public profile identity, receiver/non-`self`
parameter roots, and package-qualified field projections. Parent applications
retain exact alpha-normalized lifetime-binder arguments;
renaming a binder is stable while changing a borrow relationship changes
evidence. Generic conformance requirements retain an optional alpha-normalized
evidence-binder ordinal, exact subject ordinal, package-qualified public trait
identity, and structural type arguments. Binder-free `where T satisfies Trait`
does not fabricate evidence. Non-generic selected conformances retain exact
package-qualified conformance, carrier, and underlying public-trait identities
plus carrier/trait applications; the semantic declaration owns exact carrier
and trait symbols. Public trait requirements retain named and unnamed
`requires` and `ensures` through the same closed structural fact/expression and
evidence vocabulary as public callables, joined to their exact checked
state-signature owner. Named inputs retain ordered proposition and evidence-
interface identity while treating their source aliases as local. Named outputs
also retain their public selector identity. Their
abstract published crash ceilings come from exactly one checked capsule keyed
by the trait and requirement symbols and retain canonical causes and guards;
they do not fabricate realized body sites or calls. Generic selected-conformance
telescopes and unsupported expression forms reject until complete canonical
rows exist. Trailing `boundary host` / `boundary Name`
clauses and trait `invariant` clauses are retired rather than awaiting package
rows. Trait requirement witnesses remain ordinary explicit contracts rather
than package-only evidence syntax.
Requirements also retain whether their checked declaration supplies a default
realization; implementation bodies remain checked source subject to universal
update triage rather than entering the evidence format as compiler-private IR.

Terminal Psi evidence remains a separate evidence class for checked
final-realization claims: Omega-emitted executable code, asserted properties of
native or externally supplied code, lowering- or ABI-bound guarantees, fixed
native resource claims, and hardened profiles that explicitly require
final-code replay. Opaque executable supply may remain an explicit trust/TCB
row making no Terminal claim. Ordinary package reach, authority, provider,
proof-status, and build-contract admission does not require complete Terminal
coverage. A row without Terminal evidence makes no Terminal claim; a generic
partial/completeness bit must not blur the distinction.

## Dependencies and the lock artifact

Code imports package-local aliases. `build.omg` records source requests and
update selectors. It does not assert the fetched package's name, capability
manifest, or resolved immutable identity. Fully qualified paths do not bypass
the declared alias/reach set.

Dependency-source projection is hermetic even when later build staging is not.
Source rows cannot depend on build-host observations, generated files, imported
code, or dependency build outputs. Resolution fetches a source, extracts its
own package declaration and dependency projection, and closes the source graph
before downloaded build code receives any provider.

The unified lock artifact records the resolved closure:

- package names, source-qualified `PackageKey` values, and exact
  `PackageInstance` values;
- source selectors plus resolved commit/tree/content identities;
- per-subject obligation-semantics and evidence-schema identities;
- exact certificate provenance, re-derived discharge results, and transitive
  open obligations;
- compiler/toolchain provenance as separately labeled review metadata;
- the normalized accepted package capability/API baseline, not only its
  fingerprint;
- build observation ceilings, realized classes, and replay receipts;
- record-replay and source-rebuildability verdicts;
- boundary/provider trust receipts;
- accepted proof/grant identities; and
- component/build contract identities needed for reproducibility.

The compiler consumes the accepted lock and never silently re-resolves a
mutable selector. The lockfile is generated/checked state, not a second
hand-authored dependency language, and should normally be committed. Source
caches and expanded artifacts may be ignored. If the lock embeds only an
evidence fingerprint while the corresponding normalized baseline is absent, it
is not sufficient for update admission.

The first implementation performs no semantic-version solving. Requests for
one `PackageKey` must reconcile to one immutable instance or fail with every
conflicting dependency path.

## Package reach boundary

The package is the dependency-reach boundary:

- `pub` says what the package offers;
- `build.omg` says what packages/services it may reach;
- undeclared aliases are not nameable; and
- a subsystem requiring a meaningfully different reach set is a separate
  package rather than a hidden nested manifest.

Direct dependency rows authorize authored selection of declarations owned by
the named package; they are not required merely because that package's nominal
type flows through an already-declared dependency's API. Such a value may be
moved, borrowed, stored, returned, passed back through the declaring surface,
and checked for multiplicity without granting access to its owner's methods,
fields, cases, operators, conformances, or ordinary explicit consuming
machines. The reserved owner-attached `T::drop` hook is compiler-only and
authored selection rejects. An authored `omega::core::drop(value)` is instead
an ordinary consuming call; the concrete cleanup plan it triggers remains
carried semantics. Compiler-planned layout and automatic cleanup are carried
type semantics rather than authored declaration selection.

The transitive closure still retains the type owner's exact package instance.
Artifact dependency evidence retains the exact foreign declaration when the
checked program uses it: private flow affects rebuild/content identity, while a
public-signature occurrence also affects semantic API compatibility. A coarse
whole-package edge is a sound over-rejecting implementation until exact
declaration edges land. `pub` exposes only declarations owned by the current
package; there is no `export` item that relabels a dependency declaration.

Implementation uses a package-agnostic authored-selection ledger captured while
resolution still owns exact source spans and public/private syntactic position.
It is finalized only after successful checking, where late-bound receiver
calls, overloads, operators, and inferred conformances have exact selected
declarations. Static rows may be complete earlier; each row is joined from the
earliest coherent owner of its facts. Missing, ambiguous, or unjoinable rows
reject. The ledger is a compiler-internal sidecar and does not justify nominal
Chi.

Expression custody follows the declaration that publishes it. Public machine
contracts, public data/domain predicates, and public trait contracts use
public-interface exposure; executable state/body expressions and
`terminates by` ranking witnesses remain private. The public termination
promise is `terminates`, not the measure used to prove it. Membership facts
retain their selected domain path as a declaration row, while their
parameter/local value roots remain lexical places. Independently nameable
declarations own ordinary `pub`, including carrier-qualified declarations;
only genuine members with one exact semantic owner inherit visibility. A
public-interface selection of a private declaration rejects.

Generic conformance bounds follow the same authored-authority rule. A subject
parameter and optional evidence binder are lexical. The right-hand trait is an
exact trait selection; a qualified `Carrier::Evidence` bound selects both the
carrier declaration and the package-scoped conformance. Machine and trait
bounds inherit their enclosing declaration's exposure. This does not decide
whether every selected declaration family is independently publishable.

The direct-dependency gate consumes only finalized authored-selection rows.
Checked carried nominals, automatic cleanup, layout, and move/copy facts feed a
separate exact semantic-dependency set with private/public disposition. They
affect artifact and compatibility identity without widening source
nameability. Compilation must admit the selections applicable at a stage before
executing selected package or build-time code. Final execution consumes the
finalized ledger. Earlier effect-free execution consumes exact early targets or
fails closed unless the complete compiler-derived candidate set is confined to
admitted owners; an implementation whose current order cannot establish that
must reorder or split the work rather than weaken the gate.

The initial exact carrier is a checked-flow sidecar assembled only after
checking succeeds. It derives machine-head and exact checked call-result types,
joins ownership-place types, promotes public-interface exposure, and retains an
automatic cleanup machine only when its exact attached nominal declaration
matches. A same-spelled cleanup attached elsewhere cannot satisfy that edge.
For an owned erased value, the compiler-built descriptor carries the same exact
movement and cleanup plan with payload custody; this is lifecycle metadata, not
trait evidence or a source selection. Borrowed erased views never acquire
cleanup ownership for their referents.
This sidecar is package-neutral and compiler-private. The compiler's review
projector qualifies its consumer and dependency declarations by exact package,
emits versioned blocking rows for each dependency kind and exposure, and keeps
exact source anchors for both sides. It is comparison evidence, not an accepted
lock schema or a reason for nominal Chi; total admission coverage remains a
separate requirement.

Earlier effect-free compiler evaluation uses that split directly. Const-
generic calls, fixed-array const calls, const-domain facts, laid/placed layout
policies, wire policies, and calling policies retain exact invocation custody
and consult a package-neutral authority backed by the reconciled direct graph
before executing. The authority walks each concrete build-time call closure and
checks both caller-to-callee edges and the reachable bodies' authored selection
occurrences. A shared policy is reusable only after every authored application
site is admitted. Exact resolved symbols are authoritative; facts awaiting
later checked resolution fail closed unless the compiler can prove the whole
candidate set remains within toolchain, self, or direct dependencies. Operator
fallback uses the checked layer's conservative intrinsic judgment, never
spelling. Const value substitution retains only a declaration-provenance symbol
and occurrence, preserving package custody without carrying const semantics
into typed or runtime trees.

These checks belong at the earliest coherent private typed/probe representation
owned by the compiler. Coupling to that non-public representation is acceptable
because checker and representation evolve together. This does not introduce
nominal Chi; such a stage would require an actual semantic boundary rather than
the desire for a stable report shape.

Explicit nominal type selections are retained during symbol-resolved-to-typed
lowering. At that point the selected symbol is exact and the enclosing
declaration still determines whether the occurrence belongs to a public
interface or private implementation. Public data, domain, machine-head,
trait, and wire type positions are public; private declarations, internal state
signatures, local type annotations, casts, and public-machine owned storage are private. Generic bases
and named dynamic-trait conformances retain exact custody. Primitive types,
binders, locals, and source-free compiler nodes do not acquire fictional package
provenance. These authored rows enforce direct source authority; they do not
replace the separate carried-type dependency rows needed for artifact and API
compatibility identity.

Conformance custody follows the same earliest-coherent-owner rule. Resolution
records each exact source-backed static conformance argument at its own token.
Checked operator selection records the exact trait conformance selected for an
authored operator token. When an unbound generic conformance requirement is
satisfied by exactly one declaration, specialization validation retains that
declaration as an inferred conformance argument and fingerprints its package-
qualified identity; checked finalization attaches it to the authored call
occurrence. Explicit evidence arguments and inferred selections remain separate
subsets so an explicit argument is not fabricated again at the call token.
Package admission therefore rejects an inferred transitive-only conformance
even when the caller never names the conformance alias.

The same custody applies to statement calls whose result is Unit or explicitly
discarded. The resolver records the source call before rebuilding statement
trees into tables, preserving exact targets and explicit static conformance
arguments. If receiver typing or specialization must settle the target later,
the checked flow call finalizes that exact source obligation and contributes any
uniquely inferred conformance. Compiler-owned build markers and lowered
assembly operations use closed intrinsic ledger variants rather than synthetic
package declarations. A statement call to ordinary package code remains an
authored selection and requires direct authority.

Static call arguments do not form a separate dependency loophole. Every
source-backed declaration path is retained recursively at its own span.
Conformance paths keep their evidence-specific kind; type, static-machine, and
forwarded-binder paths use one static-argument kind because their exact symbol
and the selected callable telescope carry the category. Integer literals name
no declaration. Named const reduction preserves the exact const declaration in
the existing substitution-provenance row. An unresolved static path remains a
late obligation and package admission fails closed if no exact declaration is
available.

The same authoritative build surface owns concrete channel/store compatibility
demands. `builder.require_wire_compatibility<Edge, Lineage, Local, Peer, ...>();`
requests only the directional wire facts named after the first four type
arguments. The compiler evaluates those requests against published schema,
codec, unknown-member, canonicalization, and `FormatMigration` evidence; it
reports every fact and rejects unmet requested facts. This is edge/deployment
policy, not intrinsic version metadata on `Local` or `Peer`.

Packages normally compose statically and may optimize across package edges.
They are not ABI or replacement boundaries merely because they are packages.
A build may select a provider realization for independent deployment; the
component is that realization plus its compiler-validated owned closure.

An initial composition also authorizes the runtime replacement envelope for an
independent slot: permitted imports and authority, compatibility and
observation profile, target semantics, resource ceilings, accepted execution
modalities, admission policy, and continuity constraints. A candidate that
fits this frozen envelope may be accepted by the runtime verifier without
re-running the entire build program. A candidate that widens it requires a new
owner-controlled build/composition transaction. Provider code and downloaded
artifacts never authorize their own widening.

The first implementation may accept only closures coinciding with one package.
That is an implementation restriction, not the semantic definition of
component. A concrete-machine call crossing a selected replaceable closure
rejects; a replaceable crossing names an ordinary requirement. The same
requirement may be statically selected and inlined in another build. No
hot-swap call syntax or `slot` keyword is implied.

## Target-dependent public identity

A target-neutral package may export a symbolic constant or type application
that depends on the sealed target-semantic capsule or on one selected target
realization. Its public manifest retains the exact observation and realization
applications rather than only the eventual folded value. Independently closed
artifacts compose only when those applications are compatible.

Adding, removing, or changing such a dependency in a public signature is a
breaking semantic-API revision even when every currently supported target
happens to produce the same scalar. A dependency confined to a private body or
plan changes content/target artifact identity and forces rebuilding or relinking
without changing the public contract. Compatibility diagnostics retain the
producer and consumer closures plus an origin chain through aliases, constants,
generic applications, and selected plans.

Target selection chooses exact declarations and realizations; it never splices
fields or cases into an existing nominal type. Different native field sets use
different ABI-specific nominal schemas behind one stable portable requirement.
Different sizes, offsets, padding, and alignment of one stable schema remain
ordinary target-dependent layout-plan facts.

## Authority evidence and admission

Runtime authority uses ordinary data layout plus domain evidence. The eventual
compiler-issued package-admission artifact must derive evidence from the
fetched, checked candidate; callers and packages cannot author or patch it. It
must record each owner-authorized boundary establishment, checked resource
transformation, provider/backing requirement, admitted claim, and reachable
authority. Public data shape and domain trust policy enter contract/component
compatibility identity; private implementation bodies and proof evidence affect
content identity while remaining outside public contract identity.

For every public callable and the build machine, evidence retains both the
declared service-reach ceiling and, when an actual checked body exists, the
exact inferred transitive reach plus its preselection concrete-transitive base.
The concrete base excludes authority contributed only by unresolved
installation-selected upper bounds and is not final-provider evidence. Bodyless boundary, accepted, requirement, and
external supply instead retain an explicit no-checked-body disposition; their
published ceilings are not relabeled as realized facts. An
impossible combination rejects: checked supply requires a retained body, while
accepted, requirement, and external supply forbid one; boundary supply permits
either an adapter body or a bodyless declaration. An
underdeclared implementation rejects. An overdeclared ceiling remains visible
as contract slack. Compiler-classified dangerous slack emits a separate
audit-recommended row keyed by exact callable and service identity, with both
source coordinates; bodyless supply and package-authored lookalikes emit none.
A later transition
from unused to used authority changes realized evidence even when the public
ceiling is unchanged. Capability-flow, provider, trust, proof, installation,
operational, and executable-TCB rows must retain exact package-qualified
provenance. Provider-plan and provider-trust rows now retain package identity
for the realizing machine, provider type, selected service schema, and
requirement owner, but binding/selection and remaining artifact joins are
unfinished. Risk classes must come from compiler-owned metadata on admitted
nominal identities, never from package-controlled names.

The same callable row retains the exact checked-body, boundary, or accepted
supply tier and canonical entry signature. A bodyless boundary guarantee is an
explicit trust-bearing accepted claim; a claim-free boundary symbol is not.
The compiler also emits one distinct blocking accepted-claim row carrying that
callable's complete published envelope and exact declaration provenance.
Initial or newly introduced trust requires exact root-policy resolution;
unchanged accepted evidence does not become a recurring blanket prompt.

The older standalone trust-lock lane cannot supply package claim admission.
Domain names and unmatched strings are rejected rather than converted into FNV
receipts or bare accepted-fact rows, and domains are absent from trust reports.
Exact selected-provider grants remain valid. Exact accepted-machine grants are
retained only as temporary standalone compatibility; package-aware compilation
rejects them because admitting one selector is not admitting the complete exact
accepted-claim inventory.
The signature includes lifetime arity, alpha-normalized type/const/static-
machine binders,
ordered parameter names and modes,
package-qualified lifetime-sensitive parameter types, and result type. This is
contract evidence, not merely ABI layout. Binder renames are stable, while a
changed generic bound, parameter/result type, mode, or borrow relationship
changes evidence. Review v43 and canonical row v3 retain a structural static-
machine contract's recursively alpha-normalized telescope, complete value
signature, proof/crash contracts, reach, invocation, suspension, blocking, and
termination envelope. A nominal contract retains its exact public trait and
requirement identities. Checked proof/crash rows are keyed to each structural
binder, including nested binders; missing custody, excessive nesting, and
private nominal contracts reject. Proposition parameters and static-machine or
proposition arguments in selected conformance applications remain fail-closed.
Review v44 and canonical row v4 extend contract-call rows with static-machine
arguments. Each retains either the exact caller machine-binder ordinal or the
exact concrete machine entry identity.
Review v45 and canonical row v5 rejoin each contract call to exactly one
selected callee static telescope. Supported arguments retain their category as
a direct concrete type identity, parser-canonical integer const literal, caller
machine-binder ordinal, or exact concrete machine entry identity. Nested static
applications, forwarded or symbolic type/const binders, proposition/evidence
static arguments, quotient calls, compiler intrinsics, and malformed or
ambiguous joins remain fail-closed.
Review v46 and canonical row v6 add bounded recursive generic data-type static
arguments in contract calls. Each application base rejoins exactly one checked
data declaration, whose telescope is recursively classified; changing a nested
type changes canonical evidence. This rung admits zero-lifetime generic data
applications only. Lifetime-bearing applications, generic machine/conformance
applications, unresolved forwarded type/const binders, proposition/evidence
static arguments, quotient calls, and compiler intrinsics remain fail-closed.
Review v47 and canonical row v7 admit lifetime-bearing recursive generic data
static arguments in contract calls after an exact data-declaration lifetime-
arity join. Lifetime arguments retain alpha-normalized caller lifetime-binder
ordinals: renames are stable, while selecting a different lifetime changes
canonical evidence. Generic machine/conformance applications, unresolved
forwarded type/const binders, proposition/evidence static arguments, quotient
calls, and compiler intrinsics remain fail-closed.
Review v48 and canonical row v8 admit contract-call forwarding of caller type
and const binders. Each argument is validated against the exact caller and
selected-callee telescope categories and encoded by its alpha-normalized caller
static-telescope ordinal: binder renames are stable, while selecting a different
binder changes canonical evidence. The frontend now resolves const-parameter
carrier types on machines and traits. Symbolic const declarations or
expressions, proposition/evidence static arguments, true nested
machine/conformance applications, quotient calls, and compiler intrinsics
remain fail-closed.
Review v49 and canonical row v9 admit public-trait proposition-family
parameters with their mandatory declaration-site value signature. Each retains
the ordered, package-qualified and alpha-normalized value-parameter types.
Trait, proposition, and value-parameter binder renames are stable, while
changing a signature type changes canonical evidence. Non-default
`const`/`mut`/`self` value-parameter modes remain fail-closed because current
proposition-family compatibility checking does not certify those modes.
Proposition-valued or evidence contract-call static arguments remain
fail-closed, as do symbolic const declarations or expressions, true nested
machine/conformance applications, quotients, and compiler intrinsics.
Review v50 and canonical row v10 admit unnamed public contract facts whose
proposition endpoint is a containing proposition-family parameter. The fact
retains the exact static-telescope ordinal and ordered, checked contract
expressions supplied to that family. Static-binder renames are stable, while
selecting another proposition-family slot or changing its value arguments
changes canonical evidence. Compiler validation rejects named generic
proposition evidence because the unresolved family has no exact witness
interface; proposition-valued contract-call static arguments remain a separate
incomplete form. Generic proposition law conformance now compares the exact
normalized proposition declaration and structural application; rendered labels
are diagnostic only, and a same-spelled foreign endpoint cannot discharge the
selected law. This compiler result still does not become standalone package
proof until it is carried by the total recheckable package evidence artifact.
Review v51 and canonical row v11 admit the four compiler-owned byte-sequence
predicate calls in public contract facts. The checked authored-selection row
now retains the exact closed predicate instead of one undifferentiated
intrinsic tag; projection cross-checks that identity against an unresolved,
receiver-free call before encoding it. Changing the predicate changes
canonical evidence, while a package declaration with the same spelling remains
an ordinary package-qualified callable. Other compiler intrinsics remain
fail-closed.
Review v52 and canonical row v12 add blocking standalone public-proposition
shape. Every package-owned `pub proposition` is retained whether used or not;
primitive publication records only vocabulary, while witness and transparent
forms retain their structural interface or normalized expansion. This source
API row does not mint a primitive fact, and a transparent alias remains absent
from normalized proposition identity while still participating in source
compatibility.
Review v53 and canonical row v13 add blocking standalone public-const shape.
Every package-owned public const contributes its exact package-qualified
declaration identity, exact typed declared-type identity, and canonical
structural declaration value even when unused. Neither source initializer text
nor runtime storage identity substitutes for that semantic subject. A public
const whose type exposes private data, or whose value lacks supported canonical
identity, rejects closed. Declared-type or value changes become source-backed
`public_const` conflicts; private const-v0 declarations remain unprojected.
Ordinary `pub operator` visibility is now retained independently of carrier or
domain qualification. Exact authored source provenance supplies package
ownership, while proof-static late operator selections finalize only when
exact typed operands choose one declaration; checked lowering then repeats the
ordinary visibility gate. Private cross-package selection rejects and
same-owner implementation use remains legal. Review v54 / canonical row v14
add one blocking standalone public-operator shape. The row key uses exact
package-qualified declaration identity plus the compiler's canonical operand
and result-dispatch identities; the value retains boundary status, fixed
spelling, the complete signature, and declaration contracts without depending
on use-site facts. Public contract binaries retain an exact declared overload
coordinate or explicit builtin meaning rather than only a token. Unsupported
operator crash contracts or unresolved proof-static selections reject closed.
In particular, true nested machine static applications such as
`consumer<family<Selected>>()` now reject during compiler validation, before
checked lowering. Treating the argument as the uninstantiated `family`
declaration checked the wrong callable shape; monomorphization also has no
closed recursive application identity in conflict equality, specialization
keys/fingerprints, or retained specialization evidence. Supporting this form
requires recursive specialization plus exact declaration-telescope, lifetime,
and static-argument identity throughout those paths. This is distinct from
already coherent bare generic-machine selection and call-target use such as
`Schema<Selected>(...)`.
Other non-public, external, operator, or lifetime-parameterized trait
realizations likewise remain fail-closed; binder-free generic requirements,
explicit evidence binders, and non-generic selected conformances use the same
canonical row as public traits. Checked
realizations of public, ordinary, lifetime-free traits retain exact package-qualified trait and requirement
identities, alpha-normalized arguments, and any explicit conformance alias.
Public callable `requires` and `ensures` retain exact structural rows for the
closed boolean/integer expression subset over parameter
ordinals, `result`, generic binders, and package-qualified nominals. Domain-
membership rows retain the exact value expression and package-qualified public
domain; a private package domain cannot leak through a public callable.
Projection reads the earlier typed semantic tree only after checked compilation
succeeds. Proposition rows retain an exact package-qualified primitive
endpoint, alpha-normalized declaration binders and parameter types, structural
binder/value arguments, and fact-only or witness classification. Transparent
aliases expand before identity. Witness rows retain exact root arguments and
the complete package-qualified direct/inherited requirement surface. Named
contracts join checked evidence-term identity and positional lane; local
`requires` alias spelling is omitted while public `ensures` selector spelling
remains. Diagnostic strings do not enter the row. A proof-static
`evidence.member` binder argument retains its source named-`requires` lane,
exact package-qualified declaring trait, structural requirement-argument
template, and exact requirement while omitting the local evidence alias. The
lane binds that template to the source proposition application's concrete
arguments. Matching checked evidence-term, interface, and projection facts are
required. Direct parameter-rooted member paths retain their receiver ordinal
and exact package-qualified case/field chain after a unique checked semantic-
place join. Simple total, pure calls retain their optional receiver, exact
checked package-qualified entry target, and ordinary arguments after a unique
public-interface declaration-selection join. Their helper bodies remain pinned
by the separate whole-source commitment rather than being confused with
signature identity. Symbolic const declarations or expressions,
proposition/evidence static arguments, quotient calls, true nested
machine/conformance applications, other compiler-intrinsic
calls, computed members, proposition-argument members without their checked
join, and aggregate expression forms fail closed.
Contract
casts retain their structural operand, alpha-normalized target, arithmetic
policy, package-qualified semantic domain and arguments, and value/recast form.
Diagnostic spellings are absent, and private package domains reject when a
public cast would expose them. The coarse 64-bit machine-contract fingerprint
is no longer
package-review identity, so private state-machine shape cannot alter the public
contract baseline. Complete rows for the remaining unsupported forms and exact
proof/admission dispositions still gate sealing.

Claim-free opaque `boundary data` is retained in a separate representation-TCB
lane. Its row binds the package-qualified declaration to the exact target,
representation/ABI commitment, external mechanism or explicit unbound status,
and source/toolchain/compiler evidence. Introduction or material change
strongly recommends a code/ABI audit, while unchanged rows remain visible
without recurring blanket approval. Opacity alone is not a blocking trust
claim. Deployment policy may still classify an exact compiler-owned mechanism
as dangerous and blocking.

Accepted propositions, boundary/provider guarantees, authority establishment,
executable mechanisms, and derived dangerous reach remain independent
admission rows. Public ABI incompatibility may block on the API axis without
being mislabeled as proof trust. Missing `reaches` does not suppress the
representation row, and package-controlled type names never determine risk.

Open/deferred proof obligations reject package admission. This is an admission
requirement, not a claim that ordinary compilation already exposes such a
status: the current compiler has no explicit deferred-proof carrier, and one
contract-entailment tier may stand down on facts outside its engine language.
Package-aware checked compilation now records exact machine/contract/fact
coordinates and a closed reason for each checked-implementation stand-down
found on the pristine typed graph; the review projection rejects every row.
Accepted and opaque supply stays trust-bearing rather than becoming a proof
stand-down.
This closes the in-memory review hole, but sealed/terminal propagation and any
exact later-discharge ledger remain before admission evidence exists.
Kernel-checked proofs are rechecked.
Accepted axioms and opaque boundary claims must remain explicit trust-bearing
rows; authored postconditions remain obligations. Boundary providers must
satisfy exact package-qualified requirement identities, so a same-spelled trait
from another source lineage grants nothing. The current provider carrier pairs
the normalized requirement identity with its exact owner package, but binding
and selection carriers remain unfinished.

Package policy admits the transitive reachable-authority set of the final
resolved artifact. It does not approve dependencies one edge at a time. A new
root-memory, DMA/IOMMU, executable-installation, interrupt-publication, or
equivalent reach blocks unless deployment policy explicitly grants it,
regardless of which transitive package introduced the change. Network,
filesystem, process, dynamic-loader, signing, secret, and other intrinsically
dangerous authority remains audit-relevant even when a candidate update does
not expand the package's declared authority set. Updating
`filesystem + network` to another `filesystem + network` package version may
be lock-admissible only if the normalized capability manifest still matches,
but the update flow should still surface a recommended audit finding because
the changed implementation can now misuse already-admitted power.

Package capability admission uses conflict-resolution artifacts rather than
approval prompts, and it is part of install/update rather than a disjoint
workflow. `omega install` treats the prior admission baseline as empty for the
new dependency closure; `omega update` compares against the normalized accepted
baseline retained by the existing lock. Either command writes a
compiler-generated capability conflict when
the candidate introduces blocking or suspicious authority, stops before
mutating `build.omg` or `omega.lock`, and resumes only after an exact resolution
artifact accepts or rejects every blocking delta. Initial install therefore
requires root-policy resolution and recommends audit when a new dependency
brings filesystem, network, process, dynamic-loader, signing, secret,
executable-installation, root-memory,
DMA/IOMMU, interrupt-publication, or equivalent suspect authority. The conflict
fingerprints the old and new source identities, old and new package manifests
or empty baseline, delta identities, dependency path, and canonical rendered
evidence. The resolution binds the exact conflict fingerprint and the decision
for every blocking row. Root policy may additionally require reviewer
identities, signatures, quorum, tickets, or reasons. Missing, stale,
mismatched, duplicated, dependency-supplied, or overbroad resolutions reject
before lock mutation. `omega.lock` records the admitted result and references
the resolution; it remains generated/checked state, not an authored policy
file.

Every source update also receives provenance and source-diff triage because an
implementation can misuse already-admitted power without changing capability
evidence. Retained dangerous authority always produces an audit recommendation.
Claim-free representation-TCB findings appear in the same command and may
produce `admitted-with-audit-recommended` without manufacturing a conflict or
resolution artifact when no independent policy blocks them.
The prior source tree improves review quality but is not the admission baseline:
if it is unavailable, lock-based capability comparison still works and source
review escalates to a standalone candidate audit. If the accepted lock baseline
is unavailable, the complete closure undergoes fresh admission.

The normalized comparison baseline is restart-stable independently of source
custody. The current implementation captures a bounded review-only binary
capsule containing the complete exact-key graph, immutable source selections,
comparison commitments, canonical comparison rows with explanatory source
sidecars, and any compiler-verified bounded source-read replay record. A fresh
process can decode that capsule and obtain identical
row conflicts, fingerprints, triage, and source-review behavior; absent old
source still selects standalone candidate review. Row framing and sidecars are
decoded by the compiler, while package orchestration does not interpret row
payloads. Replay recovery similarly checks exact semantic versions and all
operation-specific lanes without giving package code a decoder, while a parent
association and aggregate byte budget prevent accidental mix-and-match and
unbounded pre-rejection retention. Recovered rows and replay records use
distinct review-only types and cannot impersonate newly compiler-issued
evidence. The capsule checksum and association are corruption/consistency
detection, not authenticity or evidence of serious review, and the type has no route to
accepted lock state or `PackageInstance`. Promotion
requires independent source-and-artifact obligation reconstruction, certificate
checking, transitive open-obligation disclosure, and local admission decisions;
completing producer provenance cannot promote the capsule.

LLM review is advisory output, not authority to mutate the lock. Review tools
consume canonical diffs rendered by Omega, with bounded and escaped
package-origin identifiers treated as quoted inert data. Package prose,
comments, README text, and commit messages do not enter capability triage. A
following source-code audit may still read attacker-controlled code; that risk
is handled by the reviewer workflow, not by granting package prose authority
over admission.

No package artifact proves that this workflow was performed seriously. Local
compiler output prevents dependency-authored manifests from impersonating
review rows, but the selected compiler remains an untrusted producer for
package soundness. Consumers trust the small checking base, canonical semantics,
and their explicitly accepted admissions, and independently reconstruct the
question from exact source and artifact subjects. Compiler, toolchain, and
execution observations remain provenance for replay, cache partitioning, and
incident response, not proof of producer honesty. Likewise, signatures and
recorded review fields establish custody over a decision, not its quality; PCC
establishes only the exact proposition checked by its kernel. The accepted
project commit and the organization controlling it authorize the update.
Organizations that need stronger assurance impose their own branch, quorum,
isolated-build, bootstrap, reproducibility, and independent-review policy around
Omega's deterministic conflicts and recommendations.

Tools render these layers separately. A mechanical verdict reports locally
re-derived obligations and certificate results. Admissions report the exact
semantic assumptions accepted by local policy. Producer, reproduction,
signature, and audit metadata appears in a distinct review section and never
inside a `verified` verdict; presentation must not launder pedigree into
checking.

Boundary statements imported from a dependency are inert requests. The root
accepts one package claim set rather than repeating an approval for every
statement. The accepted identity fingerprints the package plus its complete
normalized claim set; adding, removing, or changing any claim invalidates the
acceptance and presents the exact diff. A package cannot accept its own imported
claims, and a claim the checker can refute remains an error despite acceptance.

The complete manifest remains machine-readable. Human diffs are
severity-ranked: checked local tokens collapse to a short summary, while new
admitted providers, boundary-evidence permissions, provider-owned backing,
generation/revocation machinery, or system authority are elevated with their
dependency path. Package policy decides who may enter with power; checked
contracts still constrain behavior after admission.

## Workspace composition

A workspace build composes member `Build` values with ordinary Omega code.
Shared pins and ceilings may be passed into members and members may only narrow
them. Source code never searches parent directories for ambient imports; only
the build tool discovers the nearest enclosing workspace/build entry.

## Current engineering delta

The scoped filesystem executor and real/virtual filesystem modes are the live
foundation. The Rust package crate now has reviewed production building blocks
for immutable source custody, typed identity/closure, compiler handoff/review,
row conflicts, restart-stable review baselines, and triage, but it is not yet an
accepted admission implementation. Name-keyed locks, caller-constructed
manifest JSON, mandatory caller-supplied names/aliases, fingerprint-only
baselines, and free-form receipts survive only in quarantined crate tests.
Legacy standalone compilation also retains a
syntactic local-Path compatibility scanner that may skip malformed rows;
package-aware compilation never consults it, and no admission path may treat it
as authoritative dependency projection. This seam must be removed before
install/update mutation.
The package-facing source/staging root capabilities, checked relative resolver,
explicit generated-source handoff, and frozen package-review final pass are
implemented. The native-image entry still rejects generated-source builds
until it runs inside the same sponsored package transaction; it must not invent
ambient staging custody. That route follows recheckable package evidence and
accepted-lock state so a rebuild can be compared before installation; it is
not a standalone compiler escape hatch. `TASKS_PACKAGE_MANAGER.md` owns that
integration and the compatibility-scanner migration.

## Still open

- workspace inheritance/ceiling details;
- the minimum concrete representation of each build-host service after package
  fixtures exercise it, including whether it needs a boundary trait at all;
- which standard provider families are actually needed beyond
  Filesystem/Console;
- initial root policy profiles for volatile-capable, record-replayable, and
  source-rebuildable builds; and
- UX for displaying the first failed provenance edge.
