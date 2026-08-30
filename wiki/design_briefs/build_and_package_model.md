# Design Brief: Build And Package Model

Current as of 2026-08-28. `build.omg` is ordinary Omega code interpreted in an
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

Every package owns one stable human name declared in its own `build.omg`,
through the same build surface that already carries `depend_as`,
`select_provider`, and `roots.bind`:

```omega
machine build(builder: &mut Build) {
    builder.package("arithmetic-kernels");
}
```

Every `build.omg` states its kind explicitly. A workspace root lists members; an
application declares itself an application:

```omega
machine build(builder: &mut Build) {
    builder.member("source/library/std");
    builder.member("source/omega");
}
```

```omega
machine build(builder: &mut Build) {
    builder.application("omega-compiler");
}
```

**No role is ever inferred from an absent declaration.** A missing kind is an
error in every reader, not an application in one and a broken package in
another. The earlier `const PACKAGE: Package` literal — matched statically by a
bespoke parser over roughly fifteen shape errors — is retired (settled
2026-08-25). Project role is projected hermetically before build execution from
direct root calls whose receiver is the canonical `builder: &mut Build`
parameter: exactly one package or application declaration, or one or more
workspace members. Mixed kinds, helpers, control flow, expression use, authored
toolchain vocabulary, and dependency-dependent declarations reject. Package
dependency projection consumes that same parsed role projection; a role-less
file cannot produce dependencies or receive an automatic dependency edit. This
keeps the surface ordinary Omega while making identity independent of build
authority or resolved dependencies. Missing, duplicate, or non-canonical
declarations reject. Directory and repository names are advisory only.
Compiler project loading applies that shared projection to the exact retained
bytes of the selected free `build.omg` before injecting build vocabulary or
executing it. A companion-free focused compilation remains valid; a selected
free build file cannot omit its role. A selected `build.omg` has exactly one
free `machine build(builder: &mut Build)` entry. `Owner::build` cannot become a
project root: a scoped name establishes neither project identity nor authority,
and there is no receiver for the compiler to synthesize. The same spelling in
ordinary source remains an ordinary machine.

Evaluated composition may be factored through ordinary helpers that borrow the
root's `&mut Build`; authority follows that value and the checked call graph,
never a machine name or receiver type. Every helper's transitive reach,
invocation, suspension, blocking, termination, authority demand, and build
observations compose into the root, whose published ceiling must cover them.
This is the ordinary callable-contract rule applied to the build activation,
not a second manifest surface.

Manifest declarations remain stricter. `package`, `application`, `member`,
`depend`, and `depend_as` are direct statically projected statements owned by
the free root and may not be hidden inside a helper. Provider selection, root
binding, filesystem staging, and other evaluated work may be delegated after
graph closure.
The canonical name begins with an ASCII lowercase letter and otherwise contains
only lowercase ASCII letters, digits, and single hyphen separators, ensuring
that its default kebab-to-snake alias is a valid Omega identifier.

The declared `PackageName` is neither globally unique nor security identity.
`PackageKey` joins it to canonical source lineage and is the security identity intended to qualify
package symbols across updates. For Git, that lineage is the canonical repository
namespace and excludes the requested revision, resolved commit, tree, and source
content; exact resolution belongs to `PackageInstance`. Managed imports and
authored symbols now retain it. Post-resolution compiler symbols require one existing derivation origin and
inherit its exact authored package/toolchain provenance; truly source-free
symbols remain unresolved. Checked provider-adapter rows now retain a canonical
machine-overload identity and its exact package owner, and every compiler
consumer resolves both without falling back to a short spelling. Provider
selection and compiler-intrinsic toolchain identities are not yet fully sealed.

Packages and applications use that same `PackageKey`: both declare the same
validated name and occupy one source lineage. Their explicit
`BuildDeclarationKind` remains separate from the key. A selected closure root
may be a package or application and may own dependencies; every dependency edge
must resolve to a package. The resolver already enforces this once through its
root/non-root admission path across local, workspace, and Git sources. It does
not infer application role from `ProgramEntry` or from the absence of one.

After admission, the root retains `{ PackageKey, BuildDeclarationKind }` through
closure evidence, locks, review, compiler handoff, diagnostics, and audit output.
The role is not hashed into `PackageKey`, and non-root nodes need not redundantly
carry `Package` after edge admission has established it. Changing package to
application breaks dependency compatibility; changing application to package
breaks an activation that expects an executable root. Review reports the
affected contract rather than treating every role change as symmetric or
manufacturing a new nominal identity.

The compiler handoff carries the same declaration-domain
`BuildDeclarationKind`; it does not introduce a parallel role enum.
`PackageCompilationInputs`, its source-path-free dependency closure, and the
ordinary obligation ledger retain and canonically recover the selected root
role. A package-only construction is explicit, workspace role rejects, and
independent compilation of a non-root node uses the package role already
established by dependency admission. Source-consumption v3 and production-
manifest v2 identities bind the role rather than allowing package and
application roots with otherwise equal graphs and bytes to collapse. Candidate-
closure commitment v4 binds it independently. Baseline comparison reports the
directional broken contract: package to application loses dependency
compatibility, while application to package loses activation compatibility.
Baseline-backed deterministic triage v2 binds the fixed directional reason to
the exact root decision and blocks the update.

`PackageInstance` additionally binds exact source content, produced artifact
identity, each closure subject's obligation-semantics identity, locally
re-derived verification results, and disclosed open assumptions. Exact
certificate identity and proof route remain derivation provenance so different
valid certificates may establish the same semantic result. Compiler/toolchain
identity is separate review and cache metadata; it never certifies a result or
seals the instance. Spoof rejection for
same-named packages from different source lineages remains an admission
requirement until those joins are sealed.

Each package may use the compiler-owned build facets:

```omega
machine build(builder: &mut Build) {
    let input = builder.source.read("inputs/table.txt");
    builder.output.write_source("table.generated.omg", generate(input));
    builder.log.write_line("generated table");
}
```

The tool reserves the first parameter as the build-activation handle,
`builder: &mut Build`. `BuildSource`, `BuildOutput`, and `BuildLog` are
compiler-owned, activation-scoped capabilities; they are not supplied by std or
by a runtime provider. Their exact operations contribute build-effect and
observation rows to the root contract, while ephemeral capabilities never enter
the normalized build result. The sponsor separately enforces authenticated
source reads, staged-output writes, path containment, limits, and custody.

An ordinary boundary service is not a build-host service. Importing a library
trait named `FilesystemHost` or `Console`, selecting a provider for it, or
placing it in a helper does not cause the build activation to route it. Build
logging is an explicit `BuildLog` operation and captured observation, not output
silently intercepted by the interpreter. A future additional build-host effect
requires a new compiler protocol operation and policy row; no service or
authority is ambient.

## Code, not config grammar

`build.omg` uses normal data, calls, control flow, domains, and contracts. It
does not introduce `depends {}`, `target {}`, or another block dialect.

Graph discovery is nevertheless static projection, not execution. The package
manager parses `build.omg` and projects the closed graph-forming surface
(`package`, `member`, and dependency requests) before any dependency can run or
supply build services. These declarations must be direct and statically
projectable; hiding them behind arbitrary machine control flow rejects. Ordinary
build behavior such as provider selection and root binding remains evaluated
Omega code after graph closure.

Target-conditioned dependencies reuse ordinary Omega control flow rather than a
second dependency API. The author branches on immutable, source-visible
`builder.target` and writes the same `depend` or `depend_as` call in an exact
profile arm:

```omega
machine build(builder: &mut Build) {
    builder.depend(Source::Path { location: "../portable" });
    transition builder.target {
        TargetProfile::WindowsX86_64 -> windows(builder)
        TargetProfile::LinuxX86_64 -> linux(builder)
        TargetProfile::MacosArm64 -> macos(builder)
        _ -> portable(builder)
    }

    state windows(builder: &mut Build) {
        builder.depend_as("native_api", Source::Path { location: "../win32" });
    }
}
```

The package manager still does not execute this machine. It statically walks
the finite state graph and emits
`ProjectedDependencies { common, by_profile }`. Unconditional transitions may
factor the graph; exact `transition builder.target` arms constrain a column;
nested exact constraints intersect; shared states contribute to every exact
profile that reaches them; separate arms naming the same profile merge; cycles
close by graph fixpoint. Each authored dependency occurrence projects once. A
dependency is `common` only when an authorized path reaches it without crossing
a target partition.

A dependency occurrence rejects if it is unreachable, if any path to it crosses
a transition on another runtime subject, or if any path reaches it through the
`_` target arm. Mixed authorized and tainted paths reject too: a safe path does
not cleanse a dynamic one. Wildcard arms remain valid for ordinary build
behavior but may not introduce dependency edges. This restriction purchases a
stable invariant: adding a profile to the target catalog never changes any
existing dependency column or silently grants the new profile an edge. Removing
or renaming a referenced profile is correspondingly a catalog compatibility
event.

Rejection retains both the dependency source span and the transition/arm path
that tainted it. A wildcard diagnostic identifies the `_` arm and directs the
author to hoist a genuinely common edge above the target partition or replace
the wildcard with exact profile arms. Runtime-subject and mixed-path failures
likewise name the transition that made static graph authority impossible.

The active request set for profile `P` is `common + by_profile[P]`. Alias
uniqueness is checked over that set: mutually exclusive exact-profile columns
may reuse an alias, while a common alias conflicts with the same alias in every
column. No `DependencyCondition`, `depend_when`, condition string, or evaluated
manifest path is introduced.

The landed projector owns each authored request once and stores common/profile
membership as occurrence indices. It closes unconditional and exact target
paths by fixpoint, retains stable identities for exactly the referenced
profiles, and rejects wildcard, runtime-subject, mixed, unreachable, and
profile-less-resolution cases. Active-set alias checking, profile-aware closure
resolution, review identity, and lock sections remain downstream work.

```omega
machine build(builder: &mut Build) {
    builder.depend(Source::Path {
        location: "../../contracts/uefi"
    });
    builder.roots.bind(
        cathedral::targets::uefi_x86_64::ProgramEntry,
        Application::start
    );

    let input = builder.source.read("assets/font.bin");
    builder.output.write_source(
        "font.generated.omg",
        "data GeneratedFont {}\n"
    );
}
```

The example uses the settled narrow surface; exact buffer-oriented spellings may
vary as the implementation migrates. The legacy implementation currently joins
`BuildSource::resolve` and `BuildOutput::resolve` to std's `Path` domain, routes
I/O through `Build.filesystem: FilesystemHost`, and publishes through
`BuildOutput::include_source`. That std seam is implementation debt: the final
Build protocol owns its relative-path carrier, rooted read/write operations,
generated-source publication, and log observations. The handoff remains usable
only after matching sponsored staged-tree custody. In package-aware
checked compilation, Omega executes the frozen build prepass once, appends the
exact retained UTF-8 bytes under a compiler-owned `.omega/generated/...`
logical source path, and runs one final ordinary frontend/check pass without
rerunning dependency discovery or `build`. The final source-consumption
commitment includes those bytes and verifies them against retained staged-tree
custody rather than rereading an output path. Resolving a path, writing output,
and publishing generated source remain three separate operations.

Dependency compilation consumes the same output through an opaque, compiler-
issued bundle rather than executing the dependency build again. Review
orchestration compiles dependency-first and requires one bundle for every
package in the consumer's transitive closure, including an explicit empty
bundle for a build with no handoff. Each bundle binds the producer package,
selected target, producer dependency closure, producer source-consumption
commitment, canonical generated paths, and retained bytes/digests. The initial
consumer frontend loads those bytes under the producer package identity and
compiler-owned logical path; generated imports can therefore resolve without a
physical output tree. The consumer's own source-consumption commitment includes
the injected bytes. Missing, duplicate, foreign, root-self, target/closure, and
same-review custody mismatches reject. This carrier is neither canonical
admission evidence nor a package instance and has no decoder or public
constructor.

The post-relocation filesystem-producing two-package canary remains blocked only
on the Build-facet engineering migration in
`OPTIONAL-STDLIB-BUILD-PROTOCOL-AND-SEMANTIC-BINDINGS`. It requires no
ordinary-package staging-authority role. A physical-path, package-name, or
spelling exception would invalidate the authority model; the lower-level
generated-source custody and import path is independently tested without one.

The dependency's own `builder.package("canonical-name")` declaration supplies
its name. Its default local import alias is derived mechanically from
kebab-case to snake_case; only a real local collision uses the exceptional
`builder.depend_as(alias, source)` operation. The alias is name-resolution
syntax, never package identity. Different requesters may bind different aliases
to the same package key; no ancestor renames an alias inside a dependency.

A Git request separates acquisition from package selection. Acquisition names
one repository and revision. Selection normalizes to `Root` or
`Named(PackageName)` and is excluded from `SourceIdentity`, so several members at
one revision share one authenticated fetch and tree. `Named` selection consults
only the authenticated root's statically declared member set and requires one
member whose own package declaration matches. The resolved member path is
retained as navigation/replay custody and as the base for relative dependencies,
but it is not `PackageKey` identity.

The source resolver owns one syntax-neutral authenticated tree session. It
opens the exact root declaration, accepts bounded member paths from the
manager's Omega-aware planner, batch-opens those exact member declarations, and
publishes only the selected authenticated member subtree. Declaration bytes are
retained outside the compilation root for replay. Multiple selections in one
closure reuse one exact commit/root-tree pin even when the authored revision is
symbolic; they do not independently observe a moving branch.

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
order. Standalone compilation does not interpret dependency declarations: it
resolves ordinary root-relative and toolchain imports only. Requester-local
package aliases therefore require the validated package-aware entrypoint.

## Package-scoped filesystem roots

The build executor supplies two facets through the `Build` activation:

- one immutable root for the exact package source occurrence; and
- one fresh writable staging root for that build occurrence.

The final protocol performs sponsored reads and writes through these facets
themselves; it does not route std's runtime `FilesystemHost` into the build.
Both facets and every handle derived from them disappear with the activation.

These are authority-bearing root capabilities, not path strings and not fields
of the durable build result. A source-relative name becomes usable only after a
facet operation binds compiler-owned relative bytes to one exact root
occurrence. The resulting internal rooted path retains that root identity plus
canonical relative bytes. A routed qualification may certify completed
resolution, but an erased domain over bare bytes cannot supply the missing root
identity. Runtime std's `Path` domain is not part of this protocol.

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
    optimizations: Optimizations;
    outputs: BuildOutputs;
}
```

`target` is the one exact profile supplied immutably by the build invocation.
Build source may inspect it but cannot assign, substitute, or multiply it. A
CLI `Host` convenience is resolved to one concrete profile before semantic
build evaluation; `Host` is never retained as artifact identity. Building four
targets means four activations and four artifacts. A fat or universal artifact
requires its own explicit target profile rather than several selections in one
activation.

This is the durable projection, not a claim that the source-visible activation
handle serializes every ephemeral facet it exposes. In particular, its source
and staging roots and admitted `filesystem` service are absent from this
schema.

`optimizations` is an exact, empty-by-default set of individually named,
semantics-preserving transformation families. It is orthogonal to target,
debug information, diagnostics, assertions, and packaging: the build model has
no `debug`/`release` optimization categories and no `O1`/`O2`/`O3` intensity
levels. During the experimental phase only the root package's authoritative
`build.omg` may populate it; dependency metadata and embedding defaults cannot
enable optimization. The optimizer architecture defines the vocabulary,
identity, fail-closed behavior, and eventual manifest projection.

Hosted versus freestanding, subsystem/image format, default providers, calling
policies, fault supply, and resource supply belong to the selected target
profile. They are not repeated as independently mutable booleans or enums in
each build. In-source `target ... {}` blocks and `builder.target = ...` are
transitional syntax to remove. Target choice belongs to the invocation;
target-qualified slot and provider bindings remain ordinary authored build
data.

### Requested target and target admissibility

The invocation requests one exact `TargetProfile`; the activation presents
that immutable value as `Build.target`. Source does not separately assert a
supported-target set or accept the request ceremonially. Instead, the selected
target is admissible exactly when the artifact-role closure validates:

```text
omega build --target linux_arm64   # exact request
omega build                        # CLI resolves Host to one exact request
```

```text
requested exact target
    + target-qualified roots and provider realizations
    + target semantics, ABI, layout, resources, and reach ceiling
    -> one valid artifact closure, or rejection
```

An application without its selected profile's `ProgramEntry` rejects. A
library has no such requirement; a component must close its declared slots;
and a target-neutral artifact need not acquire a native entry at all. Thus
absence of an application root is not a universal declaration of unsupported
target status. The output role determines which closure must exist.

This rule establishes mechanical target admissibility under one pinned target
profile and semantic version. It does not assert that a human tested every
configuration. Any separately reported tested-target matrix is operational
metadata and never substitutes for closure validation. Conversely, because
new profiles are admitted by successful validation rather than an authored
allowlist, validation coverage is load-bearing: target-sensitive assumptions
must be explicit predicates or observations, not unchecked folklore such as
"addresses are probably 64 bit."

The `TargetProfile` schema and target-observation vocabulary are
toolchain-owned and closed. Concrete profile declarations come from validated
target packages in the selected toolchain closure; they are not a forever
hard-coded compiler enum. Ordinary packages may name an exact profile but may
not reinterpret one. The canonical profile spelling is exact—for example,
`windows_x86_64`, not a second `windows_x64` alias.

### Runtime reach ceiling

Provider selection and runtime authorization are distinct. Selecting no
filesystem provider does not mean "filesystem denied," because absence of a
declaration cannot create policy. The authoritative build declares one
explicit complete runtime-reach ceiling, and validation proves:

```text
inferred transitive runtime demand
    ⊆ authored complete reach ceiling
    ⊆ selected target supply
```

Exclusion from that sealed complete set is an explicit denial. A violation
retains a provenance chain to the dependency, declaration, or selection that
introduced the demand so diagnostics can identify the edge that exceeded the
ceiling even when only prebuilt artifact evidence is available.

The platform's launch calling plan is checked against the generated arrival
bridge, while the explicitly bound source machine is checked against the
target's entry shape. `build.omg` names the target-owned slot and exact source
implementation; it does not repeat the target's register/stack arrival contract
or discover an export by spelling. Binding is selection, not invocation:
`build.omg` supplies neither the machine's receiver nor its entry arguments.

The resulting physical-contract plan retains a domain-separated SHA-256
commitment to the exact toolchain package identity, canonical package-relative
source path, and source bytes. The prior FNV source value remains only as an
explicitly non-authoritative report fingerprint. Exact toolchain origin, source
membership, contract identities, and the complete calling plan remain separate
checks; holding the compact report value equal cannot substitute different
package source bytes.

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
`Console` surface; its ordinary satisfiers and bodyless boundary leaves
determine the derived plan. A leaf uses `via` only for a payload the exact
declaration and target cannot derive. Binding shape is declared by the slot, not inferred from the
trait's current requirement count. Exact slot consumers can cite only the
selected requirement's normalized contract; they possess no conformance
identity from which trait laws could be cited.

Compiler orchestration retains that selected entry as one build-owned typed
settlement carrier. Target-slot resolution runs first, exact source-shape
validation second, and optional physical/semantic plus program-storage calling-
plan validation third. The source signature and both calling surfaces remain
joined through component-progress and provider projection, then split only at
their backend and storage consumers. A test-harness entry-name override is not
part of this carrier and cannot acquire slot, signature, calling-plan, or
storage authority.

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

The selected target determines the required-slot closure. One build source may
author rows for several exact profiles; only rows owned by the requested
profile enter that activation's durable projection. A row whose slot is not
owned by its authored profile rejects, as do duplicate active bindings. A
missing required slot names the exact slot. Package/library builds bind no
roots. Runtime-installed slots may remain open, but installation must validate
the same binding shape, portable demands, target supply, authority, and
lifecycle before publishing reachability.

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
bodyless boundary satisfiers (with `via <Binding>` only for an explicit payload)
declare candidates, and compiler policy
chooses one exact plan. Unique covering selection remains per exact operator
coordinate. An authored operator override instead selects the complete exact
package-qualified same-path family atomically: the provider must cover every
canonical overload coordinate or the whole selection rejects. The retired top-
level `provider Name : Category;` declaration and operator-local
`provider Name` clause are bootstrap syntax from a parallel primitive registry
and must not be preserved as a second selection path.

Selection identities are nominal; binding identities are normalized evaluated
values. `select_provider` retains one static selectable-declaration reference
and one provider-data type path. The first resolves to one exact boundary
trait, one exact package-qualified boundary-operator family, or one exact
top-level `boundary requirement`; the second resolves to one provider-data
symbol. Both carry compiler-derived package owners
into selection. Plans match only exact `(package, canonical path)` slot and
provider identities; authored
spellings remain diagnostic data and there is no leaf-name fallback. Checked
adapters likewise bind normalized overloads to exact package owners for the
realizing machine, provider type, selected service schema, and requirement
owner. The complete target-scoped binding producer closure and result enter
final admission. Changing a typed foreign locator, evaluated plan, or sealed
compiler-catalog entry changes artifact identity and triggers fresh admission;
`build.omg` cannot rewrite any of them while selecting a provider.

The compiler retains the selected target's provider-default producer roster in
one consuming carrier while target markers are erased and typed trees are
constructed. That owner rebinds the exact typed machines before provider
selection; frontend drivers do not transport a raw machine-name side channel.
Producer names are deterministic, authored selection rows retain their source
order and identity, and build overrides continue to outrank target defaults.

`Build::select_provider<Slot, Provider>()` is ordinary typed API vocabulary.
It performs a declaration-family-per-slot override; users do not repeat every
default and cannot append or mutate derived plan rows. Strings, normalized
signature spellings, ordinals, compiler fingerprints, and declaration order
never select an overload.

Opaque by-value representations use the parallel typed operation
`Build::select_representation<Opaque, Conformance>()`. `Conformance` must be one
exact named satisfaction of the compiler-owned
`OpaqueRepresentation<Opaque>` trait, and remains inert until selected. Build
policy cannot author sizes, alignments, ABI classes, field offsets, movement
rules, or numeric representation identifiers; the compiler derives the closed
descriptor from the conformance's concrete carrier. Compiler-owned target
families such as `Ptr<T>` resolve from pinned `TargetSemantics` without an
authored selection.

Resolution is demand-driven. Reference-only opaque pointees and proof-erased
values may remain `Unbound`; a runtime by-value occurrence may not. Each demand
must close before calling-policy evaluation, and all selected producers and
consumers must agree on one exact application. A missing, conflicting,
lookalike-trait, foreign-target, or shape-invalid selection rejects with the
full demand and selection provenance chain.

Selection remains nominal and argument-free. Target-specific values such as a
Windows standard-output handle or Linux file descriptor belong inside the
selected target-owned realization contract, not as value arguments to
`select_provider`. A target-neutral provider facade may resolve to checked
target-specific leaves. When two configurations are genuinely distinct
authored choices, they use distinct nominal/static realization identities;
runtime-variable descriptors remain ordinary capability values.

A top-level requirement candidate is still declared in source rather than
invented by build policy:

```omega
machine LapicCompletion::complete(
    acknowledgement: InterruptAcknowledgement in Pending
)
satisfies InterruptAcknowledgement::complete
reaches MachineControl
{
    // checked LAPIC completion
}
```

The build may then select
`select_provider<InterruptAcknowledgement::complete, LapicCompletion>()`.
Missing, private, ambiguous, foreign, or duplicate candidates reject. The
selected operation retains its identity independently from its reach row;
equal rows never select or correlate providers.

Boundary-operator family membership is a semantic set. Evidence deduplicates
exact overload coordinates and serializes them in canonical coordinate-identity
order; source reorderings do not change selection identity. Each coordinate's
static telescope is a separate axis: coverage may be one generic realization or
an exact application family covering every verifier-reconstructed concrete or
symbolic demand. Generic applications are not overload coordinates.

A provider may compose checked software and target-owned external leaves. An
exact call to a public realization machine delegates directly and does not
redispatch through the selected operator family; spelling the operator inside
its own provider would recurse. The delegated leaf's target restriction,
contract, reach, binding, and admissions compose transitively. External leaves
are bodyless `boundary machine ... satisfies ...` declarations. A compiler
intrinsic is discovered from exact declaration, signature, and target identity;
only bindings carrying an undiscoverable payload retain `via`.

Adding or removing an overload coordinate in a public family is a compatibility
break for every authored family override. An existing provider that does not
cover the new coordinate rejects with a diagnostic naming that member. Keep
operator paths as deliberate, reasonably stable policy units; a future
per-coordinate override requires a separately justified typed declaration-
reference facility rather than hidden signature syntax.

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

The implementation foundation now models that closure without granting
authority. Exact package-qualified schema identity and arity, concrete and
artifact-qualified symbolic demands, exact substitutions, and a generic or
exact-family provider coverage assertion are retained inside one selected
provider closure and close canonically against its exact plan. Reordering or
duplicate demand cannot change the result, while unresolved or unused
substitutions, schema/arity/plan drift, and missing exact-family members reject.
The result retains the selected provider closure, plan, concrete applications,
and complete coverage assertion, but it neither derives the demand or coverage
total from verified artifacts nor binds an issuance occurrence. Those
derivation, composition, and installation joins remain open. Native realization
retains the selected closure's compact compatibility report identity and a
domain-separated SHA-256 commitment beside the source-free provider-plan
projection. The commitment covers the exact plans, execution scope, indexed
coverage, opaque executable admissions, and installation-reach resolutions.
Component-candidate replay independently recomputes it from the complete
source-selected facts and requires both the commitment and report-coordinate
drift check to match. Consequently a compact-collision substitution, coverage
change, or resolved-reach change cannot pass merely because the projected plan
rows are unchanged; this replay is still non-authorizing.

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
Observation-summary schema v20 carries operation-attempt schema v18: an ordered
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
The first bounded replay rung handles one or more complete, non-interleaved
Source-rooted source-read chains. Each chain contains one flags-zero `open`, one
or more `read`/`read_at` calls on its distinct created descriptor, and its exact
retiring `close`. It reruns the build without any filesystem provider, supplies
recorded results and read bytes, reconstructs logical descriptor lifetimes, and
requires exact event order, inputs, outputs, exhaustion, and final result. Each
chain's sequential cursor starts at zero and advances by successful sequential-
read results; positioned reads retain an exact nonnegative offset without
advancing it. Ordered operation kinds, counts, offsets, results, carriers, and
observed regions determine cursor semantics without separately trusted fields.
Empty sequences, failed reads, descriptor reuse, cross-chain operations,
interleaving, and incomplete chains reject. Summary v22 binds this successful
partial replay. Compiler replay-record v4 canonically retains every lane of the
verified chains and strictly recovers only the current semantic schemas and
exact source-read shape. Review-baseline
capsule v2 keeps those opaque bytes across restart,
binds their commitment to the parent build observation, and accounts them under
one aggregate capsule ceiling. The checksum and association are custody checks,
not authenticity or admission. This does not change the observation class:
checked compilation can now strictly rehydrate reopened custody into the PSI
executor's exact typed source-read chains and evaluate the build machine with
no host filesystem provider. Retained source bytes serve that build call even
after host source drift, while changed authored paths, counts, positioned
offsets, operation or region kinds, or event structure reject. This uses the
existing compiler-private checked/evaluator seam and does
not justify nominal Chi. Direct unsponsored host execution remains `Volatile`;
broad operation replay, output mutation and output-tree reproduction, package-
command integration, and receipts for other grammars remain absent.

Summary v23 and replay-record v5 broaden the same bounded executor to an
ordered source-input stream. Successful Source-rooted `read_metadata` and
`read_symlink_metadata` events can surround the existing closed read chains.
Each retains its authored rooted path separately from the authorized target,
its exact follow/no-follow semantic kind, all 14 target-neutral metadata
fields, and the complete selected-target carrier. Canonical recovery validates
the operation-specific lane shape and both relative paths. Provider-free replay
reconstructs the selected checked `StatLayout` carrier and compares every
field, zero padding, and tail byte before requiring exact stream exhaustion and
build result equality. Failed metadata and descriptor-backed metadata remain
outside this rung. The record remains review-only, `Volatile`, and neither an
authenticity claim nor a receipt.

Summary v24 and replay-record v6 admit the first complete operation receipt for
one exact generated-source pattern. One or more Source-input events are
followed by exactly one Output-rooted direct-child ordinary-file
`create(438)`/full-write/close chain and one matching `include_source`. Replay
serves Source results from custody but actually executes the Output operations
against a fresh virtual namespace, verifies the complete event and evidence
stream, requires descriptor and namespace quiescence, and reconstructs the
exact one-file tree from the executed path and bytes. Initial issuance requires
that tree to equal independently sponsored physical staged-tree custody;
unsponsored execution cannot mint the record. Reopened custody repeats the no-
host execution and restores the generated source without consulting drifted
host Source or Output bytes. The filesystem reach ceiling remains `Volatile`,
while this exact realized grammar is `Receipted`; broader operations and output
trees remain `Volatile`. This is replay evidence, not proof of audit,
authenticity, or admission. A separate 16 MiB aggregate replay-retention
ceiling rejects before cloning, and validated attempt custody is shared across
evaluator handoff.

Summary v27 adds the zero-mutation completion of that grammar. A successful
Source-input-only build reconstructs the canonical empty Output tree after
complete provider-free event replay, exact result equality, empty generated-
source handoff, and replay-namespace quiescence. Direct initial issuance still
requires equality with independently sponsored physical empty Output custody;
an unsponsored host execution remains `Volatile`, and any unexplained sponsored
entry rejects. Reopening replay-record v8 executes in a fresh virtual namespace
and reconstructs the empty tree without consulting host Output. The record
remains non-authoritative; package admission separately requires its canonical
Source-metadata identity to equal current compiler-validated package custody.
Earlier summary schemas reject through the record's existing semantic-schema
binding; record framing is unchanged.

Summary v28 and replay-record v9 separate an ordinary build artifact from a
generated-source handoff. After admitted Source-input events, the same exact
direct-child ordinary-file `create(438)`/full-write/close chain may end with no
`include_source`. Provider-free replay executes that chain in a fresh virtual
Output namespace, requires the generated-source handoff to remain absent, and
reconstructs the exact one-file tree. Direct issuance still requires equality
with independently sponsored physical staged-tree custody, and an unexplained
entry rejects. Reopening ignores host Output drift and does not add the
artifact to the compiler source set. Record v9 carries an explicit absent-or-
present handoff disposition so recovery cannot infer source publication from
the mere existence of an output file. The record remains non-authoritative and
retains the same package Source-metadata join required by the generated-source
and empty-Output lanes.

Summary v29 and replay-record v10 generalize only the cardinality of the
ordinary-artifact lane. After the Source-input prefix, a nonempty sequence of
distinct direct-child files may each use the same exact
`create(438)`/full-write/close chain, with no generated-source handoff. Paths
and logical descriptors must be distinct, chains may not interleave, and every
operation replays in authored order in the fresh virtual namespace. The final
namespace must contain exactly those files and bytes with no live handles or
other namespace state. Canonical tree identity sorts the files independently
of authored chain order before comparing the complete tree with sponsored
custody. Existing replay-retention, staged-entry, path, and unique-content
ceilings remain mandatory. Nested paths, directories, other operations, and
handled failures remain outside this rung; the present-handoff generated-source
grammar deliberately remains one file.

Summary v30 and replay-record v11 close the corresponding explicit-publication
cardinality. Any ordered subset of the distinct repeated output files may be
handed to `include_source`; unselected files remain ordinary artifacts. Each
handoff row binds the exact Output-relative path and the number of completed
filesystem attempts at the call. Rows remain in authored call order, ordinals
must be nondecreasing, each path appears once, and no row may precede its
matching file's successful close. Multiple calls may share one ordinal and
handoff order need not equal output-chain order. Record v11 replaces the
absent-or-present bit with the complete bounded path-and-ordinal sequence.
Replay authorizes only the next exact row at its exact ordinal and finally
requires complete sequence equality alongside operation, namespace, result,
and tree equality. Existing `.omg`, reserved-name, regular non-executable file,
explicit-handoff, final-frontend, sponsored-custody, and resource gates remain
unchanged. This admits mixed generated sources and ordinary artifacts without
making an output filename implicit source authority.

Summary v31 and replay-record v12 generalize each direct-child file chain from
one write to one or more complete sequential writes. Every write must use the
fresh descriptor, return its complete immutable operand length, preserve zero
post-error state, and occur without interleaving, seek, positioned write,
descriptor duplication, reopen, or failure. Zero-length full writes remain
valid. The reconstructed file bytes are the checked ordered concatenation of
all operands, matching the existing fresh virtual descriptor cursor that
starts at zero and advances by each full result. Chain parsing and handoff
validation use the actual variable close ordinal rather than fixed three-row
arithmetic. Partial writes remain observed but non-receipted even if later
writes could produce the same final bytes. Existing aggregate retention,
staged-tree, sponsored-custody, handoff, and final-frontend gates remain
unchanged.

Summary v32 and replay-record v13 add complete positioned writes to those
fresh direct-child file chains. Sequential `write` and absolute-offset
`write_at` may be mixed in authored order. Each positioned row binds one exact
nonnegative offset, overwrites or zero-fills and extends as required, and leaves
the sequential cursor unchanged. A zero-length positioned write remains an
observed no-op and does not change extent. Reconstructed bytes must agree with
fresh virtual execution and independent sponsored staged-output custody.
Malformed or negative offsets, partial or failed writes, arithmetic or
retention overflow, interleaving, seek, descriptor duplication, and reopen
remain non-receipted. Existing handoff, custody, resource, and final-frontend
gates remain unchanged.

Summary v33 and replay-record v14 admit a fresh zero-byte Output file as an
exact `create(438)`/`close` pair. The repeated-file grammar consequently uses
zero or more complete sequential or positioned writes between create and
close. This receipts an actual empty ordinary file without synthesizing a
zero-byte write; an authored zero-byte write remains a separate retained
operation. Fresh virtual replay and independent sponsored staged-output custody
must still agree on the zero extent. Missing or failed close, interleaving, and
all otherwise unsupported operations remain non-receipted. Compiler-internal
types and accessors call this unit an Output file rather than a write chain.

Summary v34 and replay-record v15 admit successful `sync` and `sync_data`
operations at any authored position between one fresh Output file's create and
close. Each row binds the exact operation kind and same live descriptor and
must return zero with zero post-error state. Sync operations alter neither
bytes, extent, nor sequential cursor, but remain ordered replay events. Fresh
virtual replay and sponsored staged-output custody remain mandatory. Failed or
malformed syncs, wrong descriptor lineage, and sync after close remain
non-receipted.

Summary v35 and replay-record v16 admit successful nonnegative `set_len`
operations at any authored position between one fresh Output file's create and
close. The row binds the exact requested length and same live descriptor and
must return zero with zero post-error state. Replay truncates or zero-extends
the file without moving its sequential cursor. The replay resource gate binds
peak extent over the complete operation sequence rather than only final extent;
extend-then-truncate cannot hide allocation. Negative or host-unrepresentable
lengths, failures, malformed lanes, and wrong descriptor lineage remain
non-receipted.

Summary v36 and replay-record v17 admit successful canonical `seek` operations
between one fresh Output file's create and close. `SEEK_SET`, `SEEK_CUR`, and
`SEEK_END` retain exact signed offset, whence, same live descriptor,
nonnegative result, and zero post-error state. Acceptance recomputes the result
from the current cursor and extent with checked arithmetic; replay changes only
the sequential cursor. Unsupported whence values, negative or overflowing
results, mismatched retained results, failures, malformed lanes, and wrong
descriptor lineage remain non-receipted.

Summary v37 and replay-record v18 admit successful descriptor-scoped
`set_file_permissions` operations between one fresh Output file's create and
close. Each row binds the exact authored `u32` mode, same live descriptor, zero
result, and zero post-error state. Replay retains the exact final permission
operand and derives the staged tree's ordinary/executable class from its execute
bits; absence remains distinct from explicitly restoring the create mode. The
operation changes neither bytes, extent, nor cursor. Failed calls, malformed
lanes, wrong or closed descriptors, and path-based permission changes remain
non-receipted.

Summary v38 and replay-record v19 admit successful descriptor-scoped
`set_file_times` operations between one fresh Output file's create and close.
Each row binds the same live descriptor and the complete mutable timespec
carrier at resolution, provider entry, and provider return. The carrier must be
at least the existing 32-byte pair of `{ seconds: i64, nanoseconds: i64 }`
records, remain byte-equal across all three observations, and accompany a zero
result and zero post-error state. Replay applies the operation in authored order
inside the fresh virtual namespace and reproduces the exact carrier evidence.
Timestamps remain deliberately absent from canonical staged-tree identity and
materialization, so this receipts the authored operation without making ambient
host timestamp precision part of an artifact. Failed calls, short or changed
carriers, malformed lanes, and wrong or closed descriptors remain
non-receipted. Existing evidence, replay-retention, and session ceilings charge
every retained carrier copy; no separate timestamp quota is introduced.

Summary v39 and replay-record v20 admit a successful `duplicate(original)`
immediately followed by successful `close(duplicate)` between one fresh Output
file's create and final close. Both rows bind exact original/fresh descriptor
lineage, successful results, zero post-error state, and immediate retirement;
only the original descriptor remains eligible for the other admitted Output
operations. Duplicate identities are globally distinct and at most 1,024 are
retained per replay. Delayed retirement, use through a duplicate,
duplicate-of-duplicate graphs, failures, and descriptor reuse remain
non-receipted.

Summary v40 and replay-record v21 admit an adjacent successful
`lock_file(original, 6)` / `lock_file(original, 8)` pair between one fresh
Output file's create and final close. The exact scalars are
`LOCK_EX | LOCK_NB` followed by `LOCK_UN`; both calls bind the same resolved
original descriptor, scalar result zero, and zero post-error state. The
non-blocking acquire is the minimal provider-safe lane and provider-free replay
executes both operations in order against its fresh virtual namespace. At most
1,024 pairs are retained per replay. Shared or blocking modes, delayed release,
locks through duplicates, failures, contention histories, and Win32 ranged
locks remain non-receipted.

Summary v41 and replay-record v22 admit one successful
`create_dir(Output/direct-child, 493)` as the complete Output lane after the
Source-input prefix. It binds the exact rooted path, write authorization,
provider, mode, result, and post-error state, and reconstructs one empty
directory under fresh virtual namespace and sponsored staged-tree equality.
No file, child, generated-source handoff, alternate operation, or failed result
is inferred from the final tree.

Summary v42 and replay-record v23 generalize that lane to a nonempty ordered
sequence of exact empty Output directories. Every path is canonical and
distinct under one Output root; a nested path is admitted only after its exact
parent has already been created. At most 4,096 paths, 4,096 bytes per path, and
16 MiB of aggregate path spelling are retained. Provider-free replay executes
the complete authored sequence and requires exact operation, namespace,
teardown, result, and sponsored staged-tree equality. Files and directories
still do not mix; missing or late parents, duplicate paths, root changes,
generated-source handoffs, alternate operations, and failures remain
non-receipted.

Summary v43 and replay-record v24 unify the directory and regular-file lanes
as one ordered Output-tree grammar. Each entry is either one exact successful
`create_dir` or one complete file `create`/admitted-operation*/`close` chain;
file chains remain contiguous while directory and file entries may occur in
authored order. Every nested entry, including a regular file, is admitted only
after its exact parent directory has been created. Exact path collisions,
file-as-parent shapes, missing or late parents, root changes, and descriptor
reuse reject. Regular files retain the complete operation grammar already
admitted above, including explicit generated-source handoffs at their exact
post-close ordinals. Provider-free replay reproduces the authored attempts and
requires exact final mixed namespace, teardown, handoff sequence, and sponsored
staged-tree equality. One tree retains at most 4,096 entries, 4,096 bytes per
path, and 16 MiB of aggregate path spelling; existing operation, descriptor,
extent, and unique-content ceilings continue to apply. Symlinks, hard links,
implicit parent creation, interleaved file chains, alternate namespace
operations, and failed outcomes remain observed but non-receipted.

Summary v44 and replay-record v25 add one exact successful symbolic-link entry
to that ordered Output-tree grammar. The operation row retains tag 20, the
verbatim nonempty target spelling as operand-0 path-like evidence, the exact
compiler-rooted link path and matching write authorization at operand 1,
successful scalar result zero, and zero post-error state. The link path obeys
the same parent-before-child and namespace-collision rules as every other
entry. Provider-free replay recreates the exact virtual link mapping and
requires final namespace and sponsored staged-tree equality. Receipt issuance
additionally requires the existing staged-output policy: UTF-8, canonical,
self-contained relative target spelling that cannot escape the Output root.
Absolute, malformed, NUL-bearing, escaping, over-ceiling, missing-parent,
colliding, failed, or alternate symlink operations remain non-receipted. Link
path and target bytes share the existing 16 MiB aggregate spelling ceiling;
the individual target ceiling is 4,096 bytes.

Summary v45 and replay-record v26 add exact successful Output hard links to
that ordered grammar: portable tag 19 and Win32 tag 27. The existing and new
operands are canonical names under the same Output root, and each must carry
matching write authorization. The existing name must be an earlier
regular-file or hard-link entry; authored order, provider-specific operand
order, successful result spelling, zero post-error state, parent-before-child
ordering, and destination collision checks remain exact. Provider-free replay
recreates the link relation. Sponsored staged-output custody intentionally
normalizes every linked name to ordinary regular-file content and therefore
does not commit inode identity or hard-link topology. Missing, late,
directory, or symbolic-link sources, cross-root names, insufficient authority,
collisions, alternate operations, and failures remain non-receipted.

Summary v46 and replay-record v27 add exact successful Source-rooted
`read_link` events to the ordered Source-input grammar. Each event binds tag
21, the authored rooted symlink name and separately authorized no-follow
target, requested count, scalar result, post-error state, complete mutable
resolution/pre/post carrier, and the exact meaningful returned bytes. Complete
targets and capacity-limited prefixes are distinct closed outcomes; a limited
prefix does not imply or retain an unseen suffix. Provider-free replay restores
the exact carrier and event order before requiring build-result and Output-tree
equality. Returned bytes remain inert and cannot acquire path authority without
a new checked root resolution. Failed, Output-rooted, malformed, or internally
inconsistent read-link attempts remain non-receipted.

Summary v47 and replay-record v28 admit a nonempty exact Output tree beginning
at filesystem attempt zero. Constant-generating builds no longer need a
synthetic Source filesystem event; the empty Source-event prefix is replayed
vacuously. This does not relax package identity: canonical Source metadata and
compiler custody remain mandatory and are revalidated independently. Exact
generated-source ordinals begin at zero, and the existing ordered directory,
file, symbolic-link, and hard-link grammar still governs every Output attempt.
Empty streams, malformed prefixes, unexplained physical Output, and changed
canonical Source identity remain non-receipted.

Summary v48 and replay-record v29 add exact Source directory-enumeration
chains: a flags-zero Source open, one or more successful tag-23 `read_dir`
calls, and exact descriptor retirement. Each call binds its count, result,
post-error state, exact directory-record region, complete byte-carrier
resolution/pre/post states, and complete mutable cursor resolution/pre/post
states. Provider-free replay restores both carriers in authored order. Packed
records remain target-specific inert bytes; no entry name, unseen suffix, path
authority, or exhaustive-listing claim is inferred. Failed calls, leaked
descriptors, malformed tails, changed counts, and reordered chains remain
non-receipted.

Summary v49 and replay-record v30 add the first exact failed-operation receipt:
a nonempty failure-only sequence of authorized tag-9 removes against canonical
Output-rooted paths. Each attempt binds the rooted path and matching write
authorization and must return `-1` with post-error state `2`. Provider-free
replay executes the sequence against a fresh virtual Output namespace and
requires the same failures, an empty final namespace, and no generated-source
handoffs. The lane is bounded to 4,096 attempts and 16 MiB of aggregate path
spelling. Refused or unrooted paths, other error classes, successful removes,
and mixed mutation/failure lifecycles remain non-receipted.

The Windows `find_first`/`find_next`/`find_close` family remains outside this
receipt. Its existing plain-byte `directory/*` input embeds a physical Source
root, which is neither relocation-stable identity nor safe to ignore during
prepared-input matching. Its receipted form is ordered after the compiler-owned
root-aware Build path facet and must bind a Source root plus relative pattern
coordinate.

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
These summary fields are compiler-issued execution evidence kept outside
canonical capability/API comparison bytes. In isolation they are not a receipt
and do not claim either replay verdict; only the exact v24/v6 generated-source,
v27/v8 empty-Output, v28-v29/v9-v10 ordinary-artifact, v30/v11 ordered-handoff,
v31/v12 sequential-full-write, v32/v13 positioned-full-write, v33/v14
empty-file, v34/v15 successful-sync, v35/v16 successful-set-length, v36/v17
successful-seek, v37/v18 successful-descriptor-permission, v38/v19
successful-descriptor-time, v39/v20 immediate-descriptor-duplicate, v40/v21
successful-descriptor-lock, v41/v22 single-empty-directory, and v42/v23
empty-directory-tree, v43/v24 mixed-output-tree, v44/v25 symbolic-link-output,
v45/v26 hard-link-output, v46/v27 Source-read-link, v47/v28 Output-only-tree,
v48/v29 Source-directory-enumeration, and v49/v30 absent-Output-remove grammars
above may join them to verified operation replay and reproduced staged-output
equality.
Sponsored package review does retain a versioned commitment to
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
nor leaked through the count by this staged-tree representation; exact v45/v26
operation replay retains the authored link relation separately. In isolation
this is output-tree custody and replay only. The exact v24/v6 generated-source,
v27/v8 empty-Output, and
v28-v29/v9-v10 ordinary-artifact, v30/v11 ordered-handoff, v31/v12
sequential-full-write, v32/v13 positioned-full-write, v33/v14 empty-file, and
v34/v15 successful-sync, v35/v16 successful-set-length, v36/v17
successful-seek, v37/v18 successful-descriptor-permission, v38/v19
successful-descriptor-time, v39/v20 immediate-descriptor-duplicate, v40/v21
successful-descriptor-lock, v41/v22 single-empty-directory, and v42/v23
empty-directory-tree, v43/v24 mixed-output-tree, v44/v25 symbolic-link-output,
v45/v26 hard-link-output, v46/v27 Source-read-link, v47/v28 Output-only-tree,
v48/v29 Source-directory-enumeration, and v49/v30 absent-Output-remove grammars
above supply canonical operation replay and retained observed inputs.
Generated-source cases bind the complete present
handoff sequence; ordinary-artifact cases bind its absence. All broader shapes
still require those missing pieces. This custody rung does not exclude a
hostile same-user process racing the review session.

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
also rejects duplicate reviews, package/projection identity mismatch, and mixed
deployment targets before either capability comparison or source rendering.
Recovered baseline custody is validated against its compiler row, and
unavailable old source is derived from absence. Its aggregate ceiling preserves
separate compiler and hostile-source frames. The current-process executable
hash is optional incident metadata only: package core neither derives nor
stores it, and it is absent from review validation, baseline capsules,
capability conflicts, fingerprints, closure commitments, and triage.

The runner-neutral model protocol lives in the optional
`omega-package-advisory` tooling crate, outside `omega-package-manager`. It
keeps fixed system instructions separate from bounded manager-rendered
evidence, selects no model, and supplies no ambient network authority. The
runner streams response bytes into an owned sink enforcing the caller-supplied
output ceiling. Only the exact
canonical result envelope with one of two tokens—`recommend_audit` or
`no_additional_audit`—is accepted, with no prose. The result is
monotone policy advice: it may add an audit recommendation, but cannot suppress
compiler recommendations, alter blockers, prove an audit, resolve conflicts,
admit a package or evidence, set policy, or mutate state. Concrete
provider/configuration and CLI wiring remain. The outcome is bound to the exact
rendered input by a domain-separated commitment.

## Package admission projection

After successful checking, the compiler derives a review proposal by joining
each fact from its earliest semantically complete compiler-owned
representation. A total internal `PackageAdmissionProjection` converts that
join into canonical package-visible rows and rejects unresolved requirements,
unbound identities, compiler-private handles, and any fact it cannot represent
exactly. Those rows remain `CompilerIssuedPackageReview`: useful deterministic
input to review, but never authority and never directly promotable into a
`PackageInstance`.

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
subjects or provenance. The current incomplete review rows are not promoted by
terminology.
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

The current ordinary review vocabulary now exercises that rule directly. A
source-handle-free `OrdinaryPackageObligationLedger` retains one exact package,
target, compiler-consumed dependency closure, and strictly ordered canonical row
set while leaving source paths, resolutions, bytes, copied display-name strings,
and explanatory coordinates outside that graph coordinate. Each opaque package
identity still binds its declared name and source lineage. The closure comes
only from validated compiler inputs and retains every reachable package identity
and requester-local alias edge. Recovered row envelopes prove only canonical
framing and must be joined to that separately reconstructed closure. The local
compiler reconstructs the complete ledger from the earliest semantically
complete compiler-owned representations after successful checking and requires
exact equality, and fresh closure-review publication performs that
reconstruction before exposing its rows. Missing, reordered, stale,
mixed-package, mixed-target, renamed-alias, or changed-closure subjects reject;
relocation alone does not change the closure. This uses existing typed/checked
carriers and the internal package projector; it creates no Chi or other nominal
stage. The ledger also carries an explicit obligation-semantics schema identity
separate from its outer codec and review-row versions. One bounded canonical
whole-ledger frame contains that schema, package, target, complete path-free
package/alias closure, and exact canonical rows. Decode revalidates schema and
row vocabulary, graph closure/reachability/cycles/order, row framing, resource
ceilings, and canonical re-encoding. Kind-specific payload meaning remains
opaque to this framing decoder and is accepted only by exact local
reconstruction. Its domain-separated fingerprint names the complete framed
replay question but proves no result. Compiler-issued closure review retains
the validated ledger rather than discarding it, with one overflow-safe 64 MiB
aggregate retained-ledger ceiling across the review session.

This is not yet the ordinary accepted artifact described above. The current
review vocabulary remains incomplete, and the ledger has no lock-promotion
route. Exact produced-artifact subjects, certificate replay and results,
transitive open obligations, certificate/evidence-schema identity and checked
migration, dependency composition, and local admission decisions remain
separate required joins before a `PackageInstance` can exist. Canonical decode
or a matching ledger fingerprint cannot substitute for local reconstruction.

Source-selector custody now closes one prerequisite independently from that
ledger. A resolved closure retains the exact validated root request separately
from normalized lineage and immutable commit/tree/content resolution. A zero-
copy request-set view joins it, and every requester-owned dependency row by
authored ordinal, to the exact selected package key and resolution. Thus two
different requests that converge on one package remain two selection
occurrences; the resolver never invents a primary request. Aliases remain edge
naming, and transport observations remain provenance rather than package
identity. Repository-root Git traversal exercises the same path, including a
retained `HEAD` selector. This custody is not canonical lock encoding, compiler
evidence, admission, or a package instance, and it is deliberately absent from
the ordinary obligation ledger. Git dependency rows now normalize omitted
selection to `Root` and canonically retain explicit `Named(PackageName)`
selection; named traversal remains fail-closed until authenticated member
binding lands.

`CanonicalSourceClosureSubject` now gives that exact source-selection question
a bounded canonical form. It retains the root request and every requester-
owned dependency occurrence by authored ordinal, together with its resolved
alias and exact selected package key, immutable resolution, and content
identity. Strict recovery reconstructs and validates one canonical closed graph;
its fingerprint is inert until a consumer independently resolves, snapshots,
reconstructs, and compares the complete subject. Cache/snapshot paths, source
bytes, transport execution observations, compiler source-consumption and build
observations, artifacts, certificates, admissions, and open obligations remain
separate subjects. No accepted-lock or `PackageInstance` constructor consumes
this record.

`CanonicalPackageReconstructionQuestion` now associates that complete source
subject with one complete ordinary obligation-ledger frame for every package in
strict full-`PackageKey` source order. Each ledger must name the corresponding
package identity, one common target, and exactly the transitive package and
requester-local alias closure independently derived for that package. Missing,
foreign, swapped, identity-colliding, mixed-target, and graph-drifted
associations reject. Fresh matching reconstructs the aggregate from current
resolver custody and a newly compiler-issued review set. Canonical recovery and
the domain-separated aggregate fingerprint grant no authority and expose no
route to an accepted lock or `PackageInstance`; artifacts, certificates,
results, open obligations, build observations, and admissions remain separate.

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
Final local-source issuance additionally remains under the snapshot entry lock
while the resolver rejoins the exact request, canonical live root, retained
publication, compiler-bounded limits, custody identity, and final exact-tree
rehash into one opaque non-admitting observation. The public snapshot cannot be
assembled or mutated by callers. This closes successful-result association; it
does not claim strict isolation from a same-user process that can race later
compiler reads.

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

Ratified 2026-08-26: the implementation should read each fact from the earliest
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
warranted only if implementation discovers a genuine reusable semantic
invariant boundary. Additional consumers or transformations may reveal such a
boundary; stability, layer purity, or local simplification alone do not. Psi
may repeat the same invariant as a downstream backstop without becoming the
mandatory reconstruction source for a
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
Public-trait composition is the first such carrier: canonical sorting keeps
each typed parent identifier's exact authored span with its trait row under a
closed `trait_parent` role. Syntax, resolved, and typed contracts retain the
exact authored clause-keyword span independently from semantic facts. Direct
machine, public trait/top-level-requirement, and public-operator contracts carry it under
`contract_clause`; projected declaration families recursively collect the same
anchor from structural static-machine parameter contracts. This uniformly covers expression, membership, proposition,
named-evidence, and outcome-group forms. Accepted-claim rows share the callable
source sidecar and therefore point to the trusted `ensures` clause. These
coordinates remain outside semantic row bytes,
while checked body calls add exact `body_call` anchors by joining checked-flow
coordinates to typed statement, expression, and named-transition sites during
checked lowering, before provider settlement may rewrite typed call identity.
Statement and transition sites retain explicit authored call-selection
occurrences; expression sites reuse their existing attached occurrences. The
join verifies checked target, receiver, receiver shape, and operational
acknowledgement at capture. A legitimate late-bound target does not invalidate
source custody: the span proves where a call was authored, not that target
finalization already occurred. Missing, duplicate, unknown, or contradictory
provenance rejects, while compiler-generated calls produce no invented source
location. Authored `invokes` targets are retained as one typed record binding
their exact parameter-symbol/ordinal or boundary-trait symbol to the target-name
span. Invocation inference consumes that target rather than reselecting by
spelling. Callable, public trait/top-level-requirement, and recursively structural
machine-parameter review rows carry the span under `synchronous_invocation`,
with top-level rows joined exactly to checked invocation facts. Those facts
retain the exact symbolic published and inferred targets before provider
settlement; package review does not re-infer effects from the transformed typed
tree.
Authored `reaches` clauses retain every keyword and target occurrence through
syntax, resolution, typed lowering, copying, and specialization. Resolution
binds targets once to exact boundary-trait symbols. Projection rederives the
parent-closed semantic row from those targets plus invocation-contributed
services and joins it exactly to typed and checked facts. A private memberless
authored clause is a published empty ceiling, not omitted private inference.
Review carries authored member spans—or the keyword span for an empty row—under
`service_reach`, without inventing locations for inferred, invocation-only, or
parent-closure entries. Recovery envelope v6 and conflict fingerprint
v9/rendering V8 bind that reach-source schema. Authored `suspends` and `blocks`
keyword occurrences now survive syntax, resolution, typed lowering, trait-
default synthesis, copying, and specialization. Callable, public trait/top-
level-requirement, and recursively structural machine-parameter rows carry distinct
`suspension` and `blocking` roles. Projection requires retained keywords,
authored booleans, and checked published/internal interfaces to agree exactly;
omission and inference receive no invented location. A public or otherwise
contract-supplied machine's checked operational fact remains its published may-
ceiling, not an observation that the current body exercised that permission.
Review v75/row v33, recovery v13, conflict fingerprint v16, and renderer V15 bind
the current source schema. External executable leaves retain the exact authored
`via` keyword beside the normalized binding identity on the same conformance.
Projection requires binding/span parity and carries that occurrence under
`external_binding` for public and private trait or operator supply. Semantic
row bytes remain unchanged; missing, source-free, or contradictory custody
rejects. Public const declarations additionally retain the exact parsed
initializer-expression span through symbol resolution and typed lowering,
before substitution erases the value tree. `PublicConst` rows carry it as
`const_initializer` beside the declaration-name anchor. Relocation changes the
explanatory coordinates but not the semantic row bytes. Transparent public
propositions retain their complete formula extent at the parser boundary under
`proposition_formula`; primitive and witness propositions receive no invented
formula location. Every authored proof fact retains its full semantic-token
extent under `proof_fact` through syntax, resolution, typed lowering, generic
synthesis, and checked specialization. Public domain/data facts require that
custody, as does every fact beneath an authored public contract clause;
source-free compiler synthesis receives no invented coordinate. Absent
late-stage spans must be retained by their earlier owner, not reconstructed
from source text.
Public trait rows additionally retain every exact machine-requirement
declaration under `trait_requirement`; public data rows retain fields, sum
cases, and payload fields under `data_member`. These roles consume the existing
typed declaration symbols. Direct declarations use their authored spans;
generated declarations expose only their real derivation origin.
Reviewed package callables, public operators, and public trait/top-level requirements
likewise retain every value-parameter declaration under `callable_parameter`.
The same compiler-owned walk covers value parameters nested in structural
static-machine contracts. These coordinates bind what review displays without
changing semantic row identity.

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
a duplicate API. Public data has a closed ordinary/quotient form. A quotient
row binds its exact carrier-family type identity and package-qualified public
relation declaration after package review independently reruns the complete
formation judgment. The relation's public-proposition row separately binds its
telescope, body, and evidence classification. The selected equivalence proof
implementation is private admission custody and does not enter quotient API
identity. Data declarations do not admit proposition parameters; static-machine
parameters and the representable default-domain proof fragment have exact
canonical rows, while unsupported proof forms fail closed.
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
record, and exact checked dependency places for nested member paths. Public
domain semantic contributions are retained from the exact typed role record as
closed compiler-owned tags. Every retained role must name the declaration's own
typed semantic identity; canonical evidence stores the package-qualified domain
and role rather than the private semantic-domain ID or a name inference. Public
domain operators remain separate exact `PublicOperator` rows. Unsupported
callable forms continue to fail closed until their authority and exact rows are
settled.
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
does not fabricate evidence. Selected conformances retain exact
package-qualified declaration, complete alpha-normalized application,
instantiated subject, and underlying public-trait application; the semantic
declaration owns exact conformance, subject, and trait symbols. Public trait requirements retain named and unnamed
`requires` and `ensures` through the same closed structural fact/expression and
evidence vocabulary as public callables, joined to their exact checked
state-signature owner. Named inputs retain ordered proposition and evidence-
interface identity while treating their source aliases as local. Named outputs
also retain their public selector identity. Their
abstract published crash ceilings come from exactly one checked capsule keyed
by the trait and requirement symbols and retain canonical causes and guards;
they do not fabricate realized body sites or calls. Proposition/evidence
arguments in selected conformance applications and unsupported expression forms
reject rather than producing partial canonical rows. Trailing `boundary host` / `boundary Name`
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

Every package-owned bodyless external realization, including a private
implementation leaf, therefore projects as a separate blocking
executable-supply trust row, not as callable API, reach, boundary
representation, accepted proof, or Terminal evidence. The row binds the exact
package-qualified callable and tagged requirement application—trait conformance
or operator overload coordinate—to one closed mechanism: import library and
symbol, syscall number, compiler intrinsic, vtable slot, vtable field, or table-
function field. Projection cross-checks the machine supply mode, satisfies
binding, and external-binding table and rejects
missing, duplicate, mismatched, or unsupported state. It makes no claim that
the supplied executable was audited or that its implementation satisfies the
callable contract.

The compiler reads each component from the earliest coherent private
representation in which it is semantically settled. Structural external-
binding identity may come from pre-Terminal state and join the checked callable
and requirement identity only after successful compilation. This checker may move
with compiler internals; only the versioned canonical row is durable. Psi may
repeat the consistency invariant as a downstream backstop, but no package
format or public IR depends on it. Nominal Chi is unwarranted unless later work
discovers an independently useful semantic boundary; an existing coherent
stage such as Exact should be reused when it carries the same facts more
simply.

Package review v70/canonical row v28 implements this lane. Each external leaf
must have exactly one conformance application and a bodyless supply mode whose
binding, mechanism, conformance reference, and structural table identity agree.
Malformed import/syscall/vtable/table payloads and table fields without one
exact attached data declaration reject. The callable plus complete conformance
application is the row key and the structural binding is its value, so a
binding-only update changes one `OpaqueBlocking` supply row while leaving
callable API bytes stable. Private leaves receive the trust row without being
promoted into public callable rows. Canonical recovery, source accounting, and
conflict rendering carry the row; none of them asserts an audit or Terminal
verification.

Package review v72/canonical row v30 generalizes that same row's key from a
trait-only conformance to a tagged exact requirement: either the complete trait
conformance application or one existing package-qualified operator overload
coordinate. The first operator lane accepts bodyless external supply for a
public, named, nongeneric boundary operator. Public realization machines also
retain the coordinate in their callable row; private leaves remain absent from
public callable API while retaining their opaque supply row. Selected-provider
evidence remains separate and is cross-checked against the exact operator,
realization symbol, package, normalized machine identity, and structural
binding. Thus disclosure never implies selection. Compiler-known intrinsics
are the first executable mechanism; ordinary or private operators, aliases,
generic/lifetime applications, and fixed-token boundary operators reject.

Package review v71/canonical row v29 binds each supported checked ordinary
operator realization into its public callable value, whether the declaration
has a fixed token or only its named call surface. Checked lowering retains the exact
machine/operator symbols, conformance/admission form, normalized overload
shape plus exact lifetime-bearing type nodes, both complete canonical contract sets, and exact typed semantic
snapshots of their contract graphs in full. The compiler requires exact
equality with a fresh derivation, reruns signature-directed selection and the
equality/`&&` `requires`/`ensures` contract judgment, then records the selected
public, nongeneric, lifetime-free operator's existing package-qualified
overload coordinate. Post-check redirection and coordinated typed-contract
mutation both reject. Changing only a valid selected declaration changes only
the callable row. Private, generic/lifetime-parameterized, aliased, and bodyless
checked realizations reject. Operator-bound external supply uses the distinct
v72 trust-bearing association rather than borrowing a trait-conformance shape.
A fixed-token checked realization uses the same exact declaration
coordinate as its named call surface. Its public-operator row already owns the
closed compiler spelling, so the realization edge neither repeats that spelling
nor introduces another identity form. Checked-body boundary realizations use
the same edge for contract satisfaction. Active choice remains exclusively in
the existing selected-provider set, whose exact requirement and realizing-
machine declarations join back to the operator and callable rows. Projection
repeats that exact symbol, slot, checked-adapter binding, package, and machine
join. A positive named-boundary canary covers the unique-candidate route.
Fixed-token boundary operators remain fail-closed until checked-adapter token
dispatch exists. Authored selection across a same-path overloaded family
must use the atomic family rule above. Operators with outcome-specific
contracts reject until their refinement rules exist.

Package review v90/canonical row v48 admits checked operator crash refinement.
For each provider crash cause, the compiler substitutes operator parameters
with realization parameters and requires every provider route to be an exact
member of the operator route set. An unconditional operator route admits any
provider route for that cause; an unconditional provider route requires an
unconditional operator route. Omitted provider causes narrow the contract.
Undeclared causes and stronger routes reject. Ordinary checked-crash
validation still proves that the realization body stays within its own
published routes, and projection reruns the operator-refinement judgment
before retaining the existing complete operator, callable, and checked-crash
rows. This is deliberately structural containment, not an unimplemented
logical-implication prover.

Package review v91/canonical row v49 admits exact nominal-member selection from
a computed contract receiver when that receiver is already representable by
the closed structural expression vocabulary. Projection recursively retains
the receiver and requires one finalized public-interface member-token
selection joined to the typed member declaration; it derives case identity
from that selected declaration. Missing, duplicate, redirected, or typed-
symbol-mismatched custody rejects. This reuses the existing member row and does
not broaden admission to arbitrary computed or aggregate expressions.

The association is a retained compiler-private
checked baseline, not a persisted package row, hash, or defense against a
trusted component rewriting typed state and checked facts; it is not a reason
for nominal Chi.

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

- package names, source-qualified `PackageKey` values, the selected root's
  explicit package/application role, and exact `PackageInstance` values;
- source acquisition requests, explicit `Root`/`Named` package selections,
  resolved member-path custody, and resolved commit/tree/content identities;
- requester-local alias edges and the package's complete statically projected
  `{ common, by_profile }` dependency-request map;
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

Projection is target-independent: it enumerates every exact profile column from
one fetched package without fetching any dependency. That makes the projected
map complete for that package only, not for the transitive graph. A dependency's
own map is unknown until its source is resolved. Package review and diagnostics
must distinguish those facts rather than labeling an unexplored transitive
column merely "unreviewed."

One workspace lock carries independently populated closure/review sections per
target profile. Ordinary resolution selects one explicit column. In locked mode,
an absent column fails without network access; an explicit operation may resolve
all projected columns sequentially. Common immutable instances may be shared
across sections, but a retained inactive section grants no authority to the
current build. Git edges cannot be "resolved but not fetched": authenticating a
commit, tree, workspace declaration, and selected member already exercises
resolver authority.

Profile keys are checked during projection against the trusted toolchain target
catalog. The retained projection identity includes the condition-schema version
and only the exact profile identities the package referenced, not a whole-catalog
fingerprint; unrelated catalog growth therefore leaves the package stable.
Package-authored lookalike profile values reject. The target catalog owns the
canonical Omega value, semantic profile identity, canonical CLI spelling, and
input-only aliases. Locks retain the semantic identity, never a string, ordinal,
or temporary Rust enum case.

The first implementation performs no semantic-version solving. Requests for
one `PackageKey` that resolve to one immutable source instance deduplicate even
when their authored selectors differ. Different immutable resolutions fail with
every conflicting dependency path; there is no undefined "compatible request"
relation. Multiple simultaneous instances per key are unsupported. Supporting
them would require nominal types, conformances, provider selections, and evidence
rows to use package-instance identity, not merely another alias.

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

Trait composition is likewise authored authority. Header parents and body
`requires` clauses normalize to one semantic edge, while every source-backed
edge retains the exact resolved trait as a type-reference selection under the
enclosing trait's public/private exposure. The direct-dependency gate consumes
that row; the separate `trait_parent` source coordinate explains where the edge
was authored but grants no authority by itself.

An attached declaration head such as `machine Data::operation` independently
selects the exact carrier declaration. The compiler retains that coordinate as
a type-reference row under the machine's interface exposure, including
exported boundary supply even without `pub`. Qualification neither relabels
the carrier nor admits an owner available only transitively.

Quotient declarations retain every authored formation coordinate: carrier,
right-hand relation path, repeated static-`where` relation subject, sealed
`Equivalence` trait and arguments, and named proof conformance. Relation and
trait rows inherit the data declaration's exposure. Only the selected proof
conformance is private formation custody and absent from quotient API identity;
it remains subject to ordinary visibility and direct-dependency admission.

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
Review v89 and canonical row v47 retain each authored selected-provider grant
on the exact selected provider row as its selector kind plus the
collision-resistant `ProviderPlanDigest`. The compiler carries the selecting
build-machine symbol and exact `build.omg` source span from typed build
evaluation through provider settlement; projection rejects a missing, foreign,
or orphan grant and records that span as `ProviderGrant` explanatory custody.
The selector string is not persisted as authority: plan-name grants rejoin the
retained exact plan, slot grants rejoin its exact schema declaration, and both
bind the same complete selected plan and strong digest. Canonical-row recovery
v14 adds the source role.
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
private nominal contracts reject. Proposition parameters and
proposition/evidence arguments in selected conformance applications remain
fail-closed. Selected type, const, and machine arguments use the same
categorized static-argument vocabulary.
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
an ordinary package-qualified callable.
Review v77 and canonical row v35 admit compiler-installed builtin-function calls
in public contract expressions. The projector rejoins the exact checked call
selection to the same fixed root slot and symbol kind, and encodes the stable
closed builtin ordinal rather than its spelling. Same-spelled late-root, nested,
package-authored, and generated symbols remain non-builtin; static arguments or
target-symbol custody disagreement reject.
Review v78 and canonical row v36 extend that closed identity to selected
boundary-operator provider execution. Compiler settlement retains the exact
builtin function beside, but distinct from, the authored realization machine;
projection rederives it from the checked overload and fixed builtin root slot.
Missing, mismatched, or non-intrinsic spoofed state rejects. Primitive-expression
intrinsics remain fail-closed until they receive their own closed atoms.
Review v79 and canonical row v37 add the first such primitive-expression atom:
named-float negation retains the exact checked `f32` or `f64` format. The atom
is selected by compiler dispatch from the exact checked boundary overload and
external realization join, never parsed from the authored realization-machine
name; that machine remains a separate package-qualified nominal. Projection
rederives and cross-checks the atom, while absent, cross-format, non-intrinsic,
and otherwise spoofed state rejects.
Review v80 and canonical row v38 close named-float conversion as one atom
containing the exact checked numeric source type, numeric target type, and
arithmetic domain. This distinguishes float-width conversion from every
float-to-integer width and signedness and distinguishes `Exact`, `Saturating`,
and `Trapping` integer results. The compiler derives all three coordinates from
the exact checked overload; changing or omitting any coordinate rejects during
review reconciliation rather than degrading to an authored name.
Review v82 and canonical row v40 retain the v81 primitive float binary atoms
and add exact atomic boundary-operator family-selection evidence. Each family
row binds the exact package-owned family path, nominal provider, selected
target, selection authority, complete-declaration coverage, and canonical
coordinate-to-plan mapping. Projection rejoins every coordinate against the
selected plan and its retained declaration provenance; absent, duplicate,
cross-family, or provider-drifting mappings reject. Generic/exact-application
coverage remains fail-closed until it receives a distinct compiler-owned
carrier.
Review v83 and canonical row v41 admit width-landed float literals in public
contract expressions, including transparent propositions whose comparison
operand is a typed named parameter and callable contracts whose comparison root
is `result`. Result landing comes from the exact return type of the owning
state, operator, or trait requirement. The row contains the checked `f32` or
`f64` format and its exact IEEE bits; decimal source spelling never enters
identity. Equivalent spellings therefore compare equally, while format or bit
changes alter the package contract. A float literal that reaches review without
one exact checked width landing remains fail-closed.
Review v84 and canonical row v42 admit explicit denotational reference
formation in public contract expressions. The row retains shared, mutable, or
write-only access plus the recursively projected target; runtime loan identity,
lifetime spelling, and diagnostic text remain absent. Proposition applications
recheck an explicit reference argument against the exact declared parameter
type before projection, so access or referee disagreement introduced after
checking rejects. Omega's implicit shared lending remains a plain argument
whose receiving parameter already carries shared-reference identity; review
does not invent syntax the typed expression does not contain. Operator-law
conformance and package rederivation compare reference access as well as the
borrowed target, preventing shared/mutable drift from satisfying the same law.
At the same v84/row-v42 schema, named operator calls in public contracts reuse
the structural call row. Projection invokes typed Psi's exact named-operator
resolver, rejoins its symbol with the authored call-selection occurrence, and
encodes the package-qualified operator target. A static namespace such as
`Token` in `Token::ordered(left, right)` is path qualification, not a value
receiver. Target drift and explicit reference arguments inconsistent with the
selected callable telescope reject. No new canonical discriminant is needed.
Review v85 and canonical row v43 admit exact atomic-load expressions in public
contracts. The row binds the recursively projected loaded value and one closed
load-valid ordering: `NoOrdering`, `Receive`, or `GlobalOrder`. Projection
requires an invalid result handle because loads have no secondary result
carrier. Store, read-modify-write, swap, compare-exchange, publish-bearing load
ordering, missing value, and post-check result-carrier drift reject rather than
being generalized into a package claim.
Review v75 and canonical row v33 likewise admit the compiler-owned collection-
length projection in public contract expressions. Checked proof-static member
resolution derives the receiver type from its retained declaration symbol,
prefers an actual package field, and selects `CollectionLength` only for `len`
on a fixed array or slice. Projection requires that exact public-interface
selection occurrence and encodes the structural receiver without inventing a
package owner. A package field named `len` remains nominal. Other compiler
intrinsics remain fail-closed.
Authored `!` and `~` likewise retain the exact operator token through checked
selection custody, including when nested in a public contract expression.
Review requires that public-interface occurrence to finalize as the closed
builtin-operator meaning before projecting the existing structural unary
operator. That custody-only change did not alter the then-current v76/row v34
bytes; it closes a source-custody join rather than adding a semantic
discriminant.
Review v61 and canonical row v19 admit exact raw byte-sequence literals in
public contract expressions. The projector uses typed Psi's decoded octets
directly and assigns them no text encoding. Escape-equivalent source spellings
therefore have identical canonical identity, while changing any octet changes
the reviewed contract. Unsupported aggregate and advanced call forms remain
fail-closed.
Review v62 and canonical row v20 admit inherited requirement surfaces for
lifetime-generic public conformances when the selected trait has no lifetime
telescope of its own. Requirement rows apply the complete inherited type
substitution before deriving alpha-normalized lifetime topology. Renaming
binders or changing private realization bodies is stable; selecting another
lifetime ordinal changes canonical identity. Review v86 and canonical row v44
extend that identity to lifetime-parameterized target traits. The conformance
header supplies every target-trait lifetime explicitly;
each resolves to an alpha-normalized declaration-order ordinal in the
conformance telescope and is retained beside the target type arguments through
checked closure, inherited requirement substitution, public review, and
canonical encoding. Binder renames are stable and another ordinal is a different
public conformance. Package review consumes the already-resolved mapping and
never repeats application-site inference.
Review v87 and canonical row v45 extend each selected boundary-operator family
coordinate with static-telescope application coverage. A non-generic
coordinate carries an explicit non-generic atom. A generic coordinate rejoins
one exact indexed-application row already attached to the same selected plan,
schema, package owner, and arity; package review independently checks canonical
ordering and encodes every normalized structural argument beside the compact
application report coordinate. Missing, stale, duplicate, cross-coordinate,
reordered, and arity-drifting evidence rejects. Generic coverage has no review
variant until the compiler retains proof that the selected realization is
genuinely generic, and this structural row remains non-authorizing.
Review v63 and canonical row v21 admit selected generic-conformance
applications in public generic bounds. The row retains the exact
package-qualified conformance declaration, alpha-normalized lifetime
arguments, categorized type/const/machine arguments, instantiated subject, and
the exact public trait with its instantiated type arguments. Checked closure
first validates the complete declaration telescope; review independently
rejoins its semantic declarations and never uses display strings as identity.
Binder renames are stable, while changing any selected application argument
changes canonical evidence. Proposition/evidence arguments and non-public
selections remain fail-closed.
Review v64 and canonical row v22 admit the proof-only representation
observation `zero_value<T>()` in public contracts. Canonical identity retains
the exact package-qualified, alpha-normalized observed type rather than layout
bytes, source spelling, or a checker verdict. Proposition-local type binders
receive exact symbols before typed lowering; binder renames are stable, while a
different observed type changes the row. Quotient targets remain rejected by
the settled representation-observer fence before package review.
Expression-owned type positions retain their exact authored public/private
disposition through symbol resolution. Cast targets, cast domain indices, and
`zero_value<T>()` therefore lower nominal selections under their real contract
position rather than a private default; proposition casts now resolve those
types through the same exact symbol path as machine casts. Checked visibility
and direct-dependency admission reject private or transitive-only targets.
Review v65 and canonical row v23 admit outcome-specific `ensures` without
collapsing them into unconditional postconditions. Each row carries the exact
package-qualified result-data and result-case identities, its public selector
when named, its checked evidence-lane position, and the ordinary canonical fact.
Projection rejoins exactly one producer-side checked guarded-guarantee carrier;
missing, duplicate, or mismatched custody rejects. Authored group/row ordering
is irrelevant, while moving a fact between cases or renaming a public selector
changes canonical identity.
Review v66 and canonical row v24 admit public-operator crash ceilings. Checked
lowering issues one exact operator-symbol-keyed crash-contract row for every
root and domain-homed operator, including crash-free declarations, and package
review requires the retained table to equal compiler rederivation. Cause
buckets retain unconditional truth or canonical structural guard expressions;
the latter use package-qualified call, member, and selected-overload identity
rather than textual runtime-predicate fallbacks. Clause/guard ordering and
duplicates are stable; a changed cause or guard changes the operator row.
Review v67 and canonical row v25 admit ordered array literals in public
contracts. Elements recursively use the existing structural contract
vocabulary and depth/byte limits. Nested arrays are therefore exact and
ordered; changing or reordering an element changes identity, while an
unsupported child rejects the complete row rather than becoming opaque.
Review v68 and canonical row v26 admit nominal record and sum-case constructor
expressions. Canonical identity retains the exact package-qualified data,
optional selected case, every exact field, and recursively projected values.
Field order follows exact semantic field identity rather than source order.
Changed cases/fields/values change the row; unresolved or mismatched symbols
reject, while private constructor selection is rejected by the existing public-
interface gate before projection.
Review v69 and canonical row v27 admit indexed and ranged contract expressions.
`[` retains an authored operator span and must join exactly one checked public-
interface `Index` or `Range` selection. Canonical identity carries builtin or
declared operator meaning, collection, scalar index or optional endpoints, and
inclusive-end semantics. Changing any retained field changes the row; missing,
ambiguous, or mismatched checked custody rejects. The same recursive projector
allows indexed expressions inside arrays.
Without changing the v69/row-v27 bytes, checked proof custody now includes one
exact `OperatorDeclaration` owner row for every non-crash public-operator
`requires`/`ensures` fact. Projection rejoins that declaration symbol, kind,
and fact exactly; missing, duplicate, or mismatched rows reject. Operator
contracts remain Omega's existing unnamed surface—this adds neither binding
syntax nor evidence lanes.
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
`public_const` conflicts; private const-v0 declarations remain unprojected. The
parsed initializer occurrence survives value substitution as source custody
and is rendered under the closed `const_initializer` role; its spelling remains
outside semantic row identity.
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
coordinate or explicit builtin meaning rather than only a token. Unresolved
proof-static selections reject closed.
Complete name-first conformances now retain their declaration-owned `pub`
through syntax, source profiling, resolved/typed/checked trees, and stage
snapshots. Exact selection gates reject private cross-package use and public-
interface citation, public headers cannot hide a private carrier or trait, and
private member machines remain implementation. Lexical conformance-binder
requirements inherit the enclosing declaration rather than becoming package
declarations. Explicit row references retain their authored source occurrence
through exact target normalization and obey ordinary package visibility. Every package-owned
`pub Name: Subject satisfies Trait<...>` declaration contributes the exact
package-qualified conformance identity, normalized static telescope, optional
subject, exact trait application, and complete normalized checked requirement
interface. The referenced public-trait row owns requirement contracts and laws;
the conformance must discharge them before projection. Private member machines,
proof bodies, source text, and physical code identity are not public
compatibility material. An
exact machine requirement-satisfier edge and its optional `as Name` grouping
label do not produce this row. Private cross-package conformance selection and
public-interface citation reject; same-package private selection remains legal.

Review v55 and canonical row v15 admit package-owned public conformances with
alpha-normalized lifetime/static telescopes, nominal or telescope-parameter
subjects, exact trait application, and overload-qualified inherited machine
requirements. Closed and attached-machine forms normalize to the same row.
The projector matches every retained closed realization back to the public
closure but fingerprints none of the realization machine, state, body, or
inline/reference/default choice. Validation checks attached and closed
realization signatures plus substituted trait laws before projection. Target
traits with unretained lifetime arguments, inherited lifetime substitution,
and proof-static trait parameters reject rather than producing a partial row.
The conformance declaration is package-owned; its public subject and trait may
come from a direct dependency.
Exact requirement-local `satisfies` edges remain authored selections even
though they do not mint a whole conformance. Trait edges retain the exact trait
and result-dispatch-selected requirement; operator edges retain the exact
signature-selected overload. The realizing machine's interface exposure
governs both rows. Identity settles before checked, boundary, accepted, or
external supply policy, so rejecting one association cannot erase or substitute
the declaration the source selected.
Domain `established by Trait::requirement` paths retain the same exact trait
and requirement coordinates at signature-free normalization, after uniqueness
and subject authorization are proved. Each source occurrence inherits the
domain's exposure even when the normalized semantic route set deduplicates an
equivalent alternative.

The occurrence key is the exact authored token plus its declaration-owned
exposure and selection kind. Compiler-derived expression copies share that one
key; an exact resolved target dominates an unresolved provisional copy, while
two conflicting resolved targets reject. Checked-only compiler vocabulary is
retained as a closed intrinsic identity. Private termination rankings keep
exact typed expression roots beside their normalized rendered witness, so
neither package admission nor termination checking reconstructs ownership from
display text.
Nominal callable machine-parameter contracts preserve the complete authored
`Trait::requirement` path after signature-free resolution. Typed lowering emits
the exact trait and requirement selections under the enclosing declaration's
exposure, including recursively nested contracts; transitive-only and private
public-interface selections reject before package review.
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
Other non-public or lifetime-parameterized trait realizations likewise remain
fail-closed; binder-free generic requirements, explicit evidence binders, and
selected conformances with representable complete applications use the same
canonical row as public traits. Checked realizations of public, ordinary,
lifetime-free traits retain exact package-qualified trait and requirement
identities, alpha-normalized arguments, and any explicit conformance alias.
The separate v71 operator-realization lane admits only the checked public,
nongeneric form described above; its unsupported neighbors do not inherit
trait-conformance or external-supply semantics.
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
place join and exactly one finalized public-interface member-token selection
to the same field. Missing, duplicate, or mismatched custody rejects. Simple
total, pure calls retain their optional receiver, exact
checked package-qualified entry target, and ordinary arguments after a unique
public-interface declaration-selection join. Their helper bodies remain pinned
by the separate whole-source commitment rather than being confused with
signature identity. Symbolic const declarations or expressions,
proposition/evidence static arguments, quotient calls, true nested
machine/conformance applications, other compiler-intrinsic calls, computed
members whose receivers are not in the closed expression vocabulary,
proposition-argument members without their checked join, and unsupported
aggregate expression forms fail closed.
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
lane. A demanded runtime by-value row binds the package-qualified declaration
to its exact selected or compiler-derived representation application: the
authorized source, target-semantics identity, closed shape graph or sealed ABI
leaf, physical movement/finalization, representation version, and evidence
origin. `Unbound` is accepted only when no runtime by-value crossing demands the
declaration. Introduction or material change strongly recommends a code/ABI
audit, while unchanged rows remain visible without recurring blanket approval.
Opacity alone is not a blocking trust claim. Deployment policy may still
classify an exact compiler-owned mechanism as dangerous and blocking.

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

The current review-only implementation establishes the exact in-memory join:
one closed accept/reject disposition is required for every blocking conflict,
the complete set binds the candidate-closure commitment, and that commitment
covers the source graph plus every candidate package's target, compiler,
source-consumption, build-observation, and whole-review evidence. Each conflict
also binds its baseline and candidate package observations. Construction rejects
missing, duplicate, stale/foreign, wrong-candidate, or non-blocking decisions.
Accept means only that root policy accepts that exact candidate row;
the current object reports only whether all blocking rows were accepted and
does not decide whether the wider transaction may proceed. Neither disposition
claims that an audit occurred. The complete result now has a bounded fixed-
vocabulary canonical text record. Recovery maps each encoded fingerprint to
the exact current compiler-derived conflict and owning package, reruns complete
resolution, checks its reconstructed commitment, and requires byte-identical
canonical re-encoding. At that layer, policy-origin/file custody, governance
metadata, accepted-lock reference, and revalidation while sealing accepted lock
state remained downstream work.

The first file-custody layer is now concrete. Trusted command orchestration
supplies an already-open root-owned policy-directory capability and one bounded
lowercase portable canonical filename; nested paths are unrepresentable and
dependency traversal performs no policy-file discovery. Every operation is a
direct-child operation relative to that handle. Symlink and non-regular leaves,
case-alias spellings, and existing destinations reject. A read retains its file
handle while the current compiler-derived conflict set reconstructs the complete
resolution, then rereads and compares the bytes and rechecks that the live name
still denotes that file. A new file is written, synchronized, reread, and
identity-checked under a private same-directory stage before one atomic
no-overwrite hard-link publication. Required directory synchronization follows.
A later failure reports `published but unconfirmed`, because the complete
canonical file may remain recoverable. The library cannot independently prove
that the caller supplied a root-owned command directory. This is filesystem
custody, not
audit proof, governance, accepted-lock reference, or transaction authorization;
the command UX may choose its eventual directory and filename later.
The reread and name-identity checks detect ordinary concurrent change. They do
not claim one linearizable observation against a hostile process already holding
the root author's filesystem credentials and deliberately alternating valid
states; final install/update transaction locking and immediate revalidation own
that boundary.

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
evidence. The capsule can be persisted only as a direct child of an explicitly
supplied project-owned directory capability under a bounded lowercase portable
name. Recovery does not follow symlinks, reads under the capsule ceiling,
performs canonical decode, then rereads the retained handle and rechecks its
live pathname identity. New files use synchronized private staging and atomic
no-overwrite publication; Unix mode is `0600`. This is review-state custody,
not accepted-lock storage. The capsule checksum and association are corruption/
consistency detection, not authenticity or evidence of serious review, and the
type has no route to accepted lock state or `PackageInstance`. Promotion
requires independent source-and-artifact obligation reconstruction, certificate
checking, transitive open-obligation disclosure, and local admission decisions;
completing producer provenance cannot promote the capsule.

LLM review is advisory output, not authority to mutate the lock. Optional
review tools consume canonical diffs rendered by package core; package
acceptance and deterministic audit recommendations are identical when those
tools are absent. Bounded and escaped package-origin identifiers are treated as
quoted inert data. Package prose, comments, README text, and commit messages do
not enter capability triage. A following source-code audit may still read
attacker-controlled code; that risk is handled by the reviewer workflow, not
by granting package prose authority over admission.

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

Members are declared by **path**, so relocating a subtree is a one-line manifest
edit rather than a repository-wide rewrite. A remote dependency names a package,
not a directory: the resolver fetches the source, reads its root manifest,
consults the member list, and selects by declared name. One repository may
therefore publish several packages, and moving one inside its repository breaks
no consumer.

The selected path remains operational custody: it locates retained bytes and is
the base for the member's relative dependency requests. It never becomes stable
package identity. Missing and duplicate declared names, undeclared or escaping
member paths, and recursive `build.omg` search reject.

`SourceIdentity { kind, locator, resolved }` keeps `kind` open. Git is one
supported source kind, not the blessed model. Package selection is a separate
projection and does not fragment one fetched tree into several source identities.
There is no package version field: `PackageKey` is the declared name plus
canonical repository lineage, while `PackageInstance` carries the exact resolved
revision/tree/content. A moving locator such as a branch resolves once and is
pinned thereafter.

One `omega.lock` lives at the workspace root, and a dependency's lock is never
read by its consumers. The lock belongs to whoever builds an artifact; a library
does not pin its consumers' graph. What composes upward from a dependency is its
manifest and its disclosed admissions — separate artifacts with separate rules.
The single lock may contain several independently resolved target-profile
closures; their presence, absence, review state, and exact semantic profile
identity are explicit rather than inferred from the host running the command.

A workspace is a catalog of locatable members, not one combined graph and not a
graph node with its own `PackageKey`. If it contains several applications, each
is an independently selectable closure root; selecting one does not include the
others or unrelated package members. A command may explicitly build or check
several roots, but workspace membership alone never makes a member part of an
artifact.

Compilation itself does not discover or mutate that lock. A compile request
supplies one complete in-memory admission set; the compiler independently
reconstructs the exact required obligations and returns the consumed,
unresolved, and unused rows with its product. The command coordinator reads the
workspace policy for an ordinary fail-closed check. Only the explicit
`--accept-admissions` operation replaces the admitted set with the exact
compiler-reconstructed set. A missing lock therefore never turns ordinary
compilation into implicit approval, and trust-report files remain diagnostics
rather than policy authority. Filesystem-free obligation/report construction
lives in `omega-trust-model`; `omega-trust-ledger` is limited to coordinator-
facing `omega.lock` custody.

`omega::language::core` is bundled with the compiler by decision rather than by
omission. It is the language: the checker cannot typecheck without it, its
version is the language version, and two versions of it can never coexist in one
graph, so welding it enforces something real instead of hoping a resolver agrees.
`omega::language::std` has no corresponding privilege. It is one possible
ordinary fetchable convenience package with its own version line; it may be
replaced, split into narrower packages, omitted, or retired without changing
the compiler's semantic contract. The compiler grants no authority from the
name `std`, its source lineage, repository, path, filenames, or same-spelled
declarations. Freestanding builds require no std, and the compiler-owned Build
protocol must remain usable when no standard-library package exists.

Where composition genuinely needs to recognize a declaration supplied by an
ordinary package—currently target entry/profile integration and consumer risk
classification—it binds the exact nominal declaration and normalized schema
inside the accepted resolved closure. The binding does not bless the package,
does not grant a provider or capability, and cannot be reconstructed from an
alias or source location. Candidate designations guide confined review only;
accepted bindings come from consumer policy.

The first vertical implementation canary resolves the repository's real std
directory as an ordinary local package, derives its default
`omega_language_std` alias from its own declaration, compiles a consumer from
resolver snapshots, and retains imported std declarations under std's exact
user-package identity. Omitting the dependency edge rejects the import rather
than consulting bundled std. This establishes the package and compiler-handoff
model independently of remote workspace-member selection and final application
role-evidence plumbing. It does not complete the production migration: legacy
`omega::language::std` import routing, build-filesystem seeding, macOS GUI
injection, and core/std toolchain-source classification remain explicit seams
to remove. Only `omega::language::core` keeps its magic toolchain mount.

## Current engineering delta

The scoped filesystem executor and real/virtual filesystem modes are the live
foundation. The Rust package crate now has reviewed production building blocks
for immutable source custody, typed identity/closure, compiler handoff/review,
row conflicts, candidate-bound root-policy decisions, restart-stable review
baselines, and triage, but it is not yet an accepted admission implementation.
The name-keyed lock, caller-constructed manifest JSON, mandatory caller-supplied
name/alias, fingerprint-only baseline, and free-form receipt prototypes are
deleted rather than retained as a parallel test model.
The legacy standalone local-Path compatibility scanner is deleted. Standalone
compilation cannot mint package roots or aliases from `build.omg`; only the
validated compiler handoff can route dependency imports. The migration also
preserves package-aware placed-access semantics: discovery rejects ambiguous
policy/schema spellings, retains both exact source identities, and checks both
declarations' package visibility and direct-dependency authority before
synthesis. Because `Placed<P, S>` is erased before ordinary type-selection
capture, both nominal inputs must be public even for local use; this prevents a
public signature from laundering a private declaration through the source-free
compiler shell. Its inert opaque field carriers follow shell visibility,
callable operation visibility follows exact `AccessExposure`, binding-private
access remains policy-package-confined, and statement-position operations
retain their exact generated target.
The package-facing source/staging root capabilities, checked relative resolver,
explicit generated-source handoff, and frozen package-review final pass are
implemented. The native-image entry still rejects generated-source builds until
`PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION` runs it inside the same sponsored
package transaction; it must not invent ambient staging custody. That route
follows recheckable package evidence and accepted-lock state so a rebuild can be
compared before installation; it is not a standalone compiler escape hatch.
`TASKS_PACKAGE_MANAGER.md` owns that integration.

## Still open

- workspace inheritance/ceiling details;
- the minimum buffer-oriented spelling of the compiler-owned Build-facet
  operations after package fixtures exercise them;
- which optional library provider families are actually useful beyond the
  current Filesystem/Console packages;
- initial root policy profiles for volatile-capable, record-replayable, and
  source-rebuildable builds; and
- UX for displaying the first failed provenance edge.
