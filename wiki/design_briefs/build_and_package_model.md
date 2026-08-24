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
than the parser. The compiler-provided build prelude now defines the canonical
`Source::Path`, `Source::Git`, `Build::depend`, and `Build::depend_as` surface as
ordinary Omega declarations. Package orchestration projects those canonical
direct literals hermetically from an immutable root `build.omg`, including an
exceptional validated explicit alias.
Package-aware compiler entrypoints now consume a closed requester-local graph
of alias-to-opaque-`PackageKey` bindings and canonical source roots. That mode
does not scan or combine package-authored dependency rows during import
discovery; source paths route loading but do not become nominal identity.
Orchestration still needs to translate the resolved package closure into this
handoff. Legacy standalone compilation retains only a narrow explicit
`depend_as(..., Source::Path { ... })` compatibility scanner until its canaries
migrate.

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
Future rooted evidence must reject or virtualize absolute path bytes returned
unconditionally by `canonicalize`/`final_path_name_by_handle` or conditionally
by `read_link`.
Observation schema v3 carries operation-attempt schema v4: an ordered
successful-run call-start trace of exact provider, operation tag, scalar return,
and post-operation error state. Grant-gate denials retain every exact operand
ordinal, read/write access, and unresolvable/outside-root reason; host errors
carry no fabricated refusal. Granted evaluator failures retain partial usage and
typed returned/evaluator-halt outcomes; worker failures mark evidence
unavailable. Omega emits fixed non-admission counts and no review row. Concrete
operands, rooted paths, mutable outputs, logical handles, and content remain
absent; raw
runtime descriptor returns therefore remain non-replayable.
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
replay verdict. A `Receipted` row still requires a canonical operation
transcript, retained input bytes, staged-output commitment, and replay checker.

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
not the model from semantic instructions embedded in reviewed code. Joining
recovered custody to triage and invoking a model remain future work. Model
output is policy advice, never package evidence or proof of review.

## Package admission projection

The ordinary package baseline is derived inside the compiler from checked
semantic state. A total internal `PackageAdmissionProjection` converts that
state into canonical package-visible rows and rejects unresolved requirements,
unbound identities, compiler-private handles, and any fact it cannot represent
exactly. The lock persists only the versioned canonical evidence, with source,
target, evidence-schema, and compiler/toolchain provenance.

The compiler-issued envelope also carries a separate canonical commitment to
the reconciled package/alias input graph and every exact package or toolchain
source path and byte sequence consumed by the frontend. Absolute cache paths
and load order are excluded. Source-only changes therefore change consumption
identity without contaminating the normalized capability/API comparison bytes.
The resolver retains exact immutable source resolutions and rechecks whole
snapshots plus compiler-retained bytes around compilation; an OS isolation
boundary is still required against a deliberately hostile same-user racer.

The envelope separately identifies the producer executable file bytes observed
before and after closure review, rejecting if those observations differ and
retaining one verified commitment on every emitted row. That commitment stays
outside capability/API comparison bytes. It is useful provenance for exact
comparison and replay policy, but is not compiler certification, compiler-source
identity, a reproducible-build receipt, or proof of the executable image already
loaded by the operating system. Those stronger source/toolchain joins remain
part of sealing `PackageInstance`.

The implementation should read each row from the earliest coherent checked
compiler state that contains it. Different rows may come from different
internal representations; the final projection must be total, but no single
intermediate representation must contain every row. This may couple the checker
to compiler-private representations: the checker is part of the compiler and
moves with them. That coupling does not make an internal representation a
package format or public compatibility surface. The representations remain
Psi-owned semantic state; using one "earlier" than Terminal Psi does not create
a separate semantic owner or bypass the Psi pipeline.

This projection is not another public IR stage and does not warrant a nominal
Chi stage merely for format stability. It has no execution semantics or
transformation pipeline of its own. A future shared stage is warranted only if
independent consumers or transformations establish an actual semantic boundary.

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
nominal in a public type is qualified by exact package ownership, an explicit
toolchain marker, or an unresolved marker, while generic binders are
alpha-normalized without an invented owner. Public data projects its supply,
generic shape, properties, fields/variants/payloads, relevance, and stable
numbered and retired identities. Those numbered ordinary-data identities are
the wire contract; the retired standalone `wire data` form is not projected as
a duplicate API. Any public quotient, default-domain proof fact, or static
machine/proposition parameter rejects until the projection has an exact row.
Public domain rows likewise retain exact declaring-package identity,
alpha-normalized type/const binders, carrier type, and index arguments.
Synthesized semantic paths retain an authored provenance span without replacing
their canonical spelling with the source substring. Transparent aliases
recursively flatten to sorted, deduplicated package-qualified atoms. Authored
toolchain nominals bind a canonical toolchain-relative source path plus exact
source-byte commitment in review evidence; this does not replace the
whole-toolchain commitment required for sealed admission. Compiler
carry aliases expand to explicit toolchain-unbound atoms until exact toolchain
commitment lands. Predicate-body presence and currently representable
structural expression/membership facts retain the domain carrier and exact
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
and trait symbols. Public trait requirements retain unnamed `requires` and
`ensures` through the same closed structural fact/expression vocabulary as
public callables, joined to their exact checked state-signature owner. Their
abstract published crash ceilings come from exactly one checked capsule keyed
by the trait and requirement symbols and retain canonical causes and guards;
they do not fabricate realized body sites or calls. Generic selected-conformance
telescopes, public-trait invariants, named evidence contracts, boundary clauses,
and unsupported expression forms reject until complete canonical rows exist.
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

The same callable row retains the exact checked-body, boundary, or accepted
supply tier and canonical entry signature. A bodyless boundary guarantee is an
explicit trust-bearing accepted claim; a claim-free boundary symbol is not.
The signature includes lifetime arity, alpha-normalized type/const binders,
ordered parameter names and modes,
package-qualified lifetime-sensitive parameter types, and result type. This is
contract evidence, not merely ABI layout. Binder renames are stable, while a
changed generic bound, parameter/result type, mode, or borrow relationship
changes evidence. Until exact canonical rows exist, reviewed callable
conformance bounds, static machine/proposition parameters, and non-public,
external, operator, or lifetime-parameterized trait realizations fail closed
rather than being omitted; binder-free generic requirements, explicit evidence
binders, and non-generic selected conformances use the same canonical row as
public traits. Checked
realizations of public, ordinary, lifetime-free traits retain exact package-qualified trait and requirement
identities, alpha-normalized arguments, and any explicit conformance alias.
Public callable `requires`, `ensures`, and boundary clauses retain exact
structural rows for the closed boolean/integer expression subset over parameter
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
place join. Computed members, proposition-argument members without that join,
and unsupported call and aggregate expression forms fail closed. Contract
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
uses free-form review receipts. Legacy standalone compilation also retains a
syntactic local-Path compatibility scanner that may skip malformed rows;
package-aware compilation never consults it, and no admission path may treat it
as authoritative dependency projection. This seam must be removed before
install/update mutation.
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
