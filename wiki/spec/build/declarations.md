# Build declarations and package identity

`build.omg` is ordinary Omega orchestration in an explicit build-host context.
It is distinct from the hermetic semantic evaluator for constants, proofs,
plans, and generators. Host effects need admitted build authority; there is no
second configuration grammar or ambient runtime service.

The default build environment supplies confined filesystem backing through
ordinary service contracts. Snapshot/staging is a supported library profile.
Checked declarations and transitive build contracts expose restricted host
requests for [project acceptance](../packages/acceptance.md#restricted-build-acceptance)
in `omega.lock`; they cannot approve those requests themselves. Build selections
also cannot choose an external ecosystem's receiving policy. Producing a binary
requires no such policy declaration.

Named decision: `staged-build-capability-attenuation`, settled 2026-10-28.
Project description is one logical interpreted Omega activation, not a
privileged syntax subset or replayed discovery program. Psi checks and admits
the initial prefix against core, local Build declarations and available exact
dependencies, then resumes subsequent checked continuations as dependencies
are explicitly prepared. The Terminal interpreter preserves the same mutable
Build and live storage without rerunning earlier statements. The compiler owns
stage checking, dependency/source identity and publication; ordinary libraries
own filesystem and attenuation algorithms. No source-text call scanner decides
the meaning of arbitrary same-spelled user code.

`Build::dependency` and `Build::build_dependency` atomically register configured
product and interpreted-build dependencies respectively. A description's source,
alias, purpose and delegated context are fixed before it may be scheduled.
Ordinary description helpers may set an alias before registration; an alias
cannot be renamed, reused ambiguously or retargeted afterward. Product
dependencies supply no callable Build authority. `use dependency;` is inert
logical name selection and can remain deferred until a dependent checked stage;
it never acquires source or executes an imported package.

`suspend Build::prepare_dependency(builder, registered)` prepares an exact
registration under its supplied execution context. The dependent continuation
resolves names, types and contracts and is admitted before executing them.
Initial Build checking need not already know a newly admitted package's shared
non-core interfaces. Exact provider-slot installation still checks the selected
requirement and implementation under
[Build provider readiness](provider_selection.md#build-provider-readiness).
Neither preparation nor an ordinary import chooses a default implementation,
grants host access or fabricates declaration identity.

When `build` returns, package coordination joins every outstanding dependency
request, rejects failed, cyclic, unresolved, or unadmitted dependency edges, and
freezes the normalized Build intent. Product selections may retain inert exact
designated references; product discovery and checking must resolve and validate
them before a completed product or executable plan is admitted. Product bodies
never execute during Build; product compilation starts only after the join.

## Native binding intent

The owning package records a direct native binding with
`Build::bind<Requirement>(builder, target, locator)` under
[foreign bindings](foreign_bindings.md#build-owned-native-binding). The exact
requirement is an inert designated product-selection reference until product
discovery and checking resolve its complete owned declaration/application.
The target is an ordinary exact toolchain profile reference. The locator is
ordinary closed data; fixed-byte literal lengths are inferred without explicit
`ForeignBinding<...>` counts or mandatory helper machines.

The operation borrows this activation's original Build, records target-keyed
intent, and never invokes the product operation or installs its backing into
the running Build. It grants no ordinary import of unresolved product code and
does not replace explicit Build-host readiness. Package bindings compose beneath
the chosen portable provider, so roots do not repeat dependency DLL symbols or
name private raw requirements. Existing ownership, defaults/override precedence,
and complete-coverage checks remain; ambiguous same-precedence rows reject.

The compiler derives the native candidate and executable plan from that intent.
Exact contract, lifetime, memory-access, calling-policy, native applicability,
foreign admission, provenance, and runtime-authority obligations remain.
There is no source `via` clause or per-symbol forwarding realization, and no
author API for manufacturing trusted plan rows. This accepted operation is
implementation work under `BUILD-OWNED-NATIVE-BINDINGS`; it does not change the
existing entry-binding overload of `Build::bind`.

## Project role

A selected `build.omg` has exactly one free build entry with the explicit mutable
Build parameter and any declared execution-backing parameters selected and
admitted by its invocation. A simple entry remains
`machine build(builder: &mut Build)`; a filesystem-using entry may additionally
take `filesystem: &mut FilesystemBacking` under an ordinary selected
`Files satisfies Filesystem<FilesystemBacking>` binder. These are explicit
source arguments, not a synthesized receiver or ambient provider. Its kind is explicit:

| Build result | Role |
| --- | --- |
| `Build::package(builder, "name")` | Package |
| `Build::application(builder, "name")` | Application |
| One or more `Build::member(builder, "path")` calls | Workspace |

The one interpreted activation must produce exactly one package/application
declaration or the workspace member set. Mixed, missing, duplicate, and
noncanonical results reject.
`Owner::build` is not a project root and cannot acquire a synthesized receiver.
An ordinary same-spelled machine remains ordinary source.

With no selected `build.omg`, the compiler supplies ordinary default Build
source and runs it through the same one-entry checking, interpretation, join,
and freeze route. No file need be written. A standalone loose root defaults to
application role; an already resolved package retains its supplied role and
owner. Checking source does not require an executable entry. Executable output
still needs ordinary target-slot selections and provider checks; the default
does not discover a machine named `main` or bypass
[entry admission](entry_roots.md).

`package`, `application`, `member`, `dependency` and `build_dependency` are
compiler-owned operations on the activation's Build value. Product and build
dependency sets remain separate; aliases are unique
within each scope, with no cross-scope fallback. The interpreted result, not
source spelling, determines the requested graph. Default configuration does not
discover undeclared dependency edges from imports. Package tooling edits
ordinary Build source when persisting dependency changes to a loose project.

Product registrations authorize product imports; Build registrations authorize
checked interpreted dependency preparation. Unprepared external uses cannot
execute, and a later registration cannot justify earlier code. The host library
uses its own ordinary
dependencies in that host context. Using a package in both contexts requires
both edges, including std. Legacy locks cannot imply a missing edge.
[Scoped execution](scoped_execution.md#two-checked-contexts) defines
purpose-specific scheduling, acquisition reuse, provenance, and migration.

Applications default to executable output. The Build activation may
request `artifact_only` once for an application, never a package/workspace. It
requires at least one completed required artifact and rejects executable
root/provider selections. It is not a new role or target profile.
[Staged products](scoped_execution.md#staged-products-and-failure) defines
completion and whole-result publication.

## Names, lineage, and resolution

Names begin with an ASCII lowercase letter, followed by lowercase letters,
digits, and single underscore-separated segments. Leading, trailing and repeated
underscores reject. Default import aliases preserve the declared name exactly;
the dependency description provides explicit renaming before atomic registration.
Directory/repository
names are advisory; the fetched package declares its own name.

| Identity | Binds |
| --- | --- |
| Package name | Human-facing name; not globally unique or security identity. |
| `PackageKey` | Declared name and canonical source lineage for a named project; tagged content identity for an anonymous root as defined below. |
| `PackageInstance` | Key at an exact immutable source resolution, including workspace member selection. |
| Root role | Separate package/application contract, retained alongside the key. |

For Git, lineage is the canonical repository namespace, excluding requested
revision, resolved commit, tree, and content. Those belong to resolution.
Same-named packages from different lineages remain distinct even with equal
content or policy. Authored and derived compiler symbols preserve exact package
or toolchain provenance; a short spelling cannot replace the selected owner.

Packages and applications share key derivation. A selected closure root may be
either; every dependency edge must resolve to a package. Workspace role is not
a compilation root role. Do not infer application kind from ProgramEntry.
After dependency admission, non-root package role need not be redundantly stored.

Root role survives closure evidence, lock, review, compiler handoff, diagnostics,
and audit. It does not enter the nominal key but does enter root-consuming
commitments. Package-to-application loses dependency compatibility;
application-to-package loses executable-activation compatibility. Report the
directional broken contract, not a new package identity or a symmetric change.

## Default builds and anonymous owner identity

A missing build file is not missing provenance. Preserve any exact owner already
supplied by the resolved source graph or an imported artifact. Core retains its
toolchain owner; ordinary std retains its resolved package owner. Neither shares
an absent-owner key with the root, and repeated imports never mint fresh owners.

A genuinely anonymous root uses an invocation-private activation identity while
its Build runs. That identity scopes mutable state, continuations, diagnostics,
and dependency requests but never becomes a declaration owner or portable
artifact identity. After Build returns and every dependency request joins, the
root receives a content-derived owner before product source compilation. Its key
is a domain-separated strong digest of a versioned canonical record containing:

- The complete captured source snapshot, with root-relative paths and exact
  bytes under [source capture](../packages/sources.md#local-source-capture).
- Its resolved dependency edges, including purpose, local aliases, exact owner
  identities and immutable resolutions; include the applicable core identity.
- The language/default-build semantics identity and captured semantic inputs
  that affect source meaning. Output paths, debug information and backend
  optimization selections are not such inputs.

The anonymous variant is distinct from named and toolchain owners. Use canonical
ordering and unambiguous field framing, not discovery order or private object
serialization. Host locations used to capture or load those inputs, timestamps,
random IDs, cache state, unrelated consumers and previous compilations are not
added to the record. Dependency identities remain exact; changing a dependency's
lineage is a changed input, not merely relocating its retained snapshot. Root role
and host/target build-instance selection remain separate from declaration owner.
Generated outputs retain their generating owner and exact derivation identity;
do not hash a root's own future compiled output to establish its input owner.

Equal anonymous records have equal owner identity, including after relocation
or cache deletion. Changing the snapshot, dependency resolution or semantic
inputs changes that identity. This is not inferred project continuity across
edits: named source lineage supplies that when required. Distinct named lineages
remain distinct even with equal content. An artifact digest identifies an exact
product, not the evolving project that produced it.

Declarations retain their owner plus owner-local declaration identity through
checking, specialization and publication. Artifact consumers preserve these
identities rather than rebasing them onto the importing root. Importable products
must carry the necessary identities and provenance; a cache may locate them but
is never their authority. Existing products without that information cannot
acquire portable nominal provenance from a guessed path or registry entry.
Identity alone authenticates no producer and grants neither package acceptance
nor execution authority.

## Evaluated build work

Local and package-owned helpers may borrow the root Build value for dependency
requests, provider selection, root binding, staging, and other evaluated work.
Build-dependency helpers call ordinary prepared declarations or an exact static
requirement after its admitted implementation is installed for that activation.
Authority follows ordinary operation arguments and the checked call
graph, not provider-slot selection. Transitive reach,
invocation, blocking/suspension, termination, observations, and authority
demands compose into the root's published ceiling.

`Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)` is an
evaluated root-authorized product selection under the
[assumed-binding contract](assumed_binding_contracts.md), not a statically
projected project/dependency declaration. Its exact product references do not
execute either requirement or grant any assumption to the build-host context.

Filesystem operations use ordinary explicitly supplied backing and selected
implementations. The old BuildSource/BuildOutput filesystem facets are retired
under `STAGED-BUILD-CAPABILITIES`, not the public dependency-helper interface.
The sponsor enforces authorized roots, containment, limits and custody under
[inputs and default filesystem](scoped_execution.md#inputs-and-default-filesystem),
not a constructor name or a fabricated handle. Compiler publication and
diagnostic records remain activation-scoped and do not become filesystem
authority. Logging is explicit and captured. Selecting a runtime provider does
not route it into build execution. Additional host effects require explicit
protocol operations and policy; package code gets no resolver credentials.

[Build execution](execution.md) specifies admission, generated-source visibility,
and dependency handoff.

## Exact-target evaluation

[Configuration and target selection](configuration.md) defines exact-target
admissibility, activation-local state, and identity-preserving staged fan-out.

## Workspaces

A workspace is a path-declared catalog of locatable members, not a combined
dependency graph or a node with its own package key. Each application/package
remains an independently selected closure root; building one does not include
unrelated members. Shared pins and ceilings may be passed into member Build
values, and members may only narrow them. Source code never searches parent
directories for imports; only tooling discovers the enclosing build/workspace.

Remote selection uses the member's declared name, not its path. Paths remain
navigation and the base for relative requests; relocation does not rename the
package. Reject undeclared/escaping members, absent/duplicate names, and recursive
search for undeclared build files. Source kinds are extensible; Git is not the
semantic definition of a source. There is no independent package version field:
the exact resolved source supplies instance identity.

The workspace owns its lock; dependency locks do not pin the consumer's graph.
Membership does not merge acceptance across roots. The current
[lock codec](../../../omega-rust/omega/packages/package-manager/src/lock/README.md)
retains one selected source closure with target sections, not a complete
multi-root workspace lock. Multi-root storage remains implementation work;
neither catalog membership nor another root's acceptance can fill missing state.
