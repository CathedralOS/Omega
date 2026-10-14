# Build declarations and package identity

`build.omg` is ordinary Omega orchestration in an explicit build-host context.
It is distinct from the hermetic semantic evaluator for constants, proofs,
plans, and generators. Host effects need admitted build authority; there is no
second configuration grammar or ambient runtime service.

The default build environment is the benign confined snapshot/staging protocol.
Checked declarations and transitive build contracts expose restricted host
requests for [project acceptance](../packages/acceptance.md#restricted-build-acceptance)
in `omega.lock`; they cannot approve those requests themselves. Build selections
also cannot choose an external ecosystem's receiving policy. Producing a binary
requires no such policy declaration.

Project description is one interpreted Omega activation, not a privileged
syntax subset or a pair of programs. The compiler checks `build.omg` through the
Psi driver against toolchain-owned `omega::core`, package-local Build code, and
interfaces known before dependency resolution. It then runs the Terminal
artifact exactly once with one fresh mutable `Build`. Local helpers, control
flow, explicit inputs, effects, and resource bounds use ordinary checking and
admission. No Rust parser recognizes Build call names to decide project
semantics.

`Build::depend*` and `Build::build_depend*` record exact dependency requests and
may start package resolution, review, compilation, and admission without
suspending the activation. Product dependencies grant no callable authority to
Build code. Code from a build dependency enters through
`suspend Build::await_provider<R>(builder, alias)`, where the pre-known exact
requirement application `R` defines its callable surface and effect, reach, and
resource bounds. The operation returns immediately when that Build-scoped static
provider slot is installed, or preserves the same Terminal continuation while
package coordination recursively activates dependency Builds and Psi admits the
selected implementation. The interpreter installs the slot and resumes without
replaying any Build effect. Subsequent code calls `R::operation(arguments)`;
awaiting readiness does not create or return a routing value.

When `build` returns, package coordination joins every outstanding dependency
request, rejects failed, cyclic, unresolved, or unadmitted edges, and freezes
the normalized Build. Product selections may carry unresolved product
references while the activation is running, but those references are inert and
must resolve before the result freezes. Product bodies never execute during
Build; product compilation starts only after the join.

## Project role

A selected `build.omg` has exactly one free
`machine build(builder: &mut Build)` entry. Its kind is explicit:

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

`package`, `application`, `member`, `depend`, `depend_as`, `build_depend`, and
`build_depend_as` are compiler-supplied boundary operations on the mutable Build
output. Product and build dependency sets remain separate; aliases are unique
within each scope, with no cross-scope fallback. The interpreted result, not
source spelling, determines the requested graph. Default configuration does not
discover undeclared dependency edges from imports. Package tooling edits
ordinary Build source when persisting dependency changes to a loose project.

`depend`/`depend_as` authorize product imports; `build_depend`/`build_depend_as`
request a host-profile implementation eligible for exact provider-slot
installation in the build entry and its local helpers; only successful
`Build::await_provider<R>` makes that Build-scoped slot callable. A newly
requested package's static paths are unavailable when that entry is first
checked; `R` and the readiness site must be meaningful before the package
resolves. The host library uses its own ordinary
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
`depend_as` and `build_depend_as` provide explicit renaming. Directory/repository
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
Build-dependency helpers call an exact static requirement only after
`Build::await_provider<R>` has installed its admitted implementation for that
activation. Authority follows ordinary operation arguments and the checked call
graph, not provider-slot selection. Transitive reach,
invocation, blocking/suspension, termination, observations, and authority
demands compose into the root's published ceiling.

`Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)` is an
evaluated root-authorized product selection under the
[assumed-binding contract](assumed_binding_contracts.md), not a statically
projected project/dependency declaration. Its exact product references do not
execute either requirement or grant any assumption to the build-host context.

`BuildSource`, `BuildOutput`, and `BuildLog` are compiler-owned, activation-scoped
facets. They do not enter normalized Build output. The sponsor enforces source
read/output write roots, containment, limits, and custody; the confined view
they resolve against is specified by
[inputs and default filesystem](scoped_execution.md#inputs-and-default-filesystem),
so this vocabulary is not itself the containment boundary. Logging is explicit
and captured. Selecting a runtime filesystem or `Console` provider does
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
