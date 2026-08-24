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
`PackageInstance` additionally binds exact source content, evidence-schema
identity, compiler/toolchain provenance, and compiler-derived package evidence.
These identities support comparison and reproducibility; they do not certify
the compiler or prove a review occurred. Spoof rejection for
same-named packages from different source lineages remains an admission
requirement until those joins are sealed.

Each package may define:

```omega
machine build(
    builder: &mut Build,
    filesystem: &mut Filesystem
)
    reaches Filesystem + Console
{
    ...
}
```

The tool invokes the machine with a zero-initialized `Build` and scoped standard
providers. Filesystem access is rooted to the package/build directories in the
current implementation direction. Console logging is a declared service call,
not output silently intercepted by the interpreter.

The exact surface may omit unused providers through ordinary requirement/
provider machinery. Whether a particular one-purpose build service warrants a
public boundary trait remains an implementation discovery; no service or
authority is ambient either way.

## Code, not config grammar

`build.omg` uses normal data, calls, control flow, domains, and contracts. It
does not introduce `depends {}`, `target {}`, or another block dialect.

```omega
machine build(builder: &mut Build, filesystem: &mut Filesystem) {
    builder.depend(Source::Path {
        location: "../../contracts/uefi"
    });
    builder.target = cathedral::targets::uefi_x86_64;
    builder.roots.bind(
        cathedral::targets::uefi_x86_64::ProgramEntry,
        Application::start
    );
    filesystem.copy(
        "assets/font.bin",
        builder.output("font.bin")
    );
}
```

The dependency's own `PACKAGE` supplies its name. Its default local import
alias is derived mechanically from kebab-case to snake_case; only a real local
collision uses the exceptional `builder.depend_as(alias, source)` operation.
The alias is name-resolution syntax, never package identity.

Library values such as `Source::Path`, `KiB`, and `Subsystem` carry the
vocabulary. Adding a target option normally extends `Build`/library data rather
than the parser. These dependency calls are the target core-library surface;
the transitional compiler currently recognizes only
`builder.depend("alias", path("directory"))` and must migrate.
Package orchestration can already project canonical direct `Source::Path` and
`Source::Git` literals hermetically from an immutable root `build.omg`.
Package-aware compiler entrypoints now consume a closed requester-local graph
of alias-to-opaque-`PackageKey` bindings and canonical source roots. That mode
does not scan or combine package-authored dependency rows during import
discovery; source paths route loading but do not become nominal identity.
Orchestration still needs to translate the resolved package closure into this
handoff, and legacy standalone compilation retains the transitional scanner
until its canaries migrate.

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
    builder.providers.bind(
        windows_x86_64::Console,
        TestConsole::Complete
    );
}
```

`Application::start` is the exact machine selected by the entry-machine slot.
It may be free or carry one `&mut self` receiver according to the slot's entry
shape. `TestConsole::Complete` is a named complete conformance because `Console`
requests a complete trait surface. Binding shape is declared by the slot, not
inferred from the trait's current requirement count. Exact slot consumers can
cite only the selected requirement's normalized contract; they possess no
conformance identity from which trait laws could be cited.

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

Selection and binding identities must be nominal. The current compiler validates
normalized overload strings against typed declarations and binds provider plans
to exact package owners for the realizing machine, provider type, selected
service schema, and requirement owner. Provider bindings and selection keys
remain string-carried. Admission requires package-qualified typed identities
and sealed target/catalog metadata rather than trusting those spellings.
Changing sealed foreign or target metadata must change artifact identity and
trigger fresh admission rather than silently retargeting a nominal ID.

The exact `Build` library method names remain ordinary API design. Conceptually
the operations are target-profile selection plus type-per-slot override; users
do not repeat every default and cannot append or mutate derived plan rows.

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

Policy can consequently distinguish an ordinary development build, a release
that requires record replay, and a supply-chain release that requires
transitive source rebuildability. These are graph checks, not transitive
mutation of one package's local class.

## Package admission projection

The ordinary package baseline is derived inside the compiler from checked
semantic state. A total internal `PackageAdmissionProjection` converts that
state into canonical package-visible rows and rejects unresolved requirements,
unbound identities, compiler-private handles, and any fact it cannot represent
exactly. The lock persists only the versioned canonical evidence, with source,
target, evidence-schema, and compiler/toolchain provenance.

This projection is not another public IR stage and does not warrant a nominal
Chi stage merely for format stability. It has no execution semantics or
transformation pipeline of its own. A future shared stage is warranted only if
independent consumers or transformations establish an actual semantic boundary.

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
- evidence-schema identity and compiler/toolchain provenance;
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

The first implementation may accept only closures coinciding with one package.
That is an implementation restriction, not the semantic definition of
component. A concrete-machine call crossing a selected replaceable closure
rejects; a replaceable crossing names an ordinary requirement. The same
requirement may be statically selected and inlined in another build. No
hot-swap call syntax or `slot` keyword is implied.

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
declared service-reach ceiling and the realized transitive reach. An
underdeclared implementation rejects. An overdeclared ceiling remains visible
as contract slack; dangerous slack is audit-relevant, and a later transition
from unused to used authority changes realized evidence even when the public
ceiling is unchanged. Capability-flow, provider, trust, proof, installation,
operational, and executable-TCB rows must retain exact package-qualified
provenance. Provider-plan and provider-trust rows now retain package identity
for the realizing machine, provider type, selected service schema, and
requirement owner, but binding/selection and remaining artifact joins are
unfinished. Risk classes must come from compiler-owned metadata on admitted
nominal identities, never from package-controlled names.

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

LLM review is advisory output, not authority to mutate the lock. Review tools
consume canonical diffs rendered by Omega, with bounded and escaped
package-origin identifiers treated as quoted inert data. Package prose,
comments, README text, and commit messages do not enter capability triage. A
following source-code audit may still read attacker-controlled code; that risk
is handled by the reviewer workflow, not by granting package prose authority
over admission.

No package artifact proves that this workflow was performed seriously. Local
compiler output prevents dependency-authored manifests from impersonating
derived evidence, but the selected compiler is itself a trust root. Compiler,
toolchain, verifier, schema, source, and target identities are provenance for
replay and comparison, not proof of producer honesty. Likewise, signatures and
recorded review fields establish custody over a decision, not its quality; PCC
establishes only the exact proposition checked by its kernel. The accepted
project commit and the organization controlling it authorize the update.
Organizations that need stronger assurance impose their own branch, quorum,
isolated-build, bootstrap, reproducibility, and independent-review policy around
Omega's deterministic conflicts and recommendations.

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
foundation. The current Rust package crate is exploratory scaffolding and is
not an accepted admission implementation: it keys locks by package name,
accepts caller-constructed manifest JSON, requires caller-supplied aliases and
package names, stores fingerprints without a complete accepted baseline, and
uses free-form review receipts. The transitional compiler also syntactically
scans local dependency calls and may skip malformed dependency builds. These
seams must be replaced before install/update mutation.
`TASKS_PACKAGE_MANAGER.md` owns that migration.

## Still open

- workspace inheritance/ceiling details;
- the minimum concrete representation of each build-host service after package
  fixtures exercise it, including whether it needs a boundary trait at all;
- which standard provider families are actually needed beyond
  Filesystem/Console;
- initial root policy profiles for volatile-capable, record-replayable, and
  source-rebuildable builds; and
- UX for displaying the first failed provenance edge.
