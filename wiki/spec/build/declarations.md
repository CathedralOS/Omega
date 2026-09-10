# Build declarations and package identity

`build.omg` is ordinary Omega orchestration in an explicit build-host context.
It is distinct from the hermetic semantic evaluator for constants, proofs,
plans, and generators. Host effects need admitted build authority; there is no
second configuration grammar or ambient runtime service.

## Project role

A selected `build.omg` has exactly one free
`machine build(builder: &mut Build)` entry. Its kind is explicit:

| Direct root declaration | Role |
| --- | --- |
| `builder.package("name")` | Package |
| `builder.application("name")` | Application |
| One or more `builder.member("path")` calls | Workspace |

Exactly one package/application declaration or the workspace member set is
required. Mixed, missing, duplicate, and noncanonical declarations reject.
`Owner::build` is not a project root and cannot acquire a synthesized receiver.
An ordinary same-spelled machine remains ordinary source. Focused compilation
without a selected companion build file does not invent a project role.

Role and dependency discovery project the exact retained source bytes before
build execution or dependency-provided services. Declarations must be direct
statements on the canonical build parameter. Helpers, control flow, expression
use, generated declarations, authored substitutes for toolchain vocabulary, and
dependency-dependent declarations cannot supply project identity.

`package`, `application`, `member`, `depend`, `depend_as`, `build_depend`,
`build_depend_as`, and `artifact_only` are statically projected. Product and
build dependency sets are separate and unconditional; aliases are unique within
each scope, with no cross-scope fallback. No target
control-flow interpretation, conditional dependency helper, or per-profile map
discovers additional edges. A role-less file cannot yield dependencies or receive
an automatic dependency edit.

`depend`/`depend_as` authorize product imports; `build_depend`/`build_depend_as`
authorize imports in the build entry and its local helpers. An imported host
library uses its own ordinary dependencies in that host context. Using a package
in both contexts requires both edges, including std. Legacy locks cannot imply
a missing edge. [Scoped execution](scoped_execution.md#two-checked-contexts)
defines purpose-specific scheduling, acquisition reuse, provenance, and migration.

Applications default to executable output. The optional direct unconditional
`builder.artifact_only()` declaration is valid once in an application root, never
in a package/workspace or helper. It requires at least one completed required
artifact and rejects executable root/provider selections. It is not a new role
or target profile. [Staged products](scoped_execution.md#staged-products-and-failure)
defines completion and whole-result publication.

## Names, lineage, and resolution

Names begin with an ASCII lowercase letter, followed by lowercase letters,
digits, and single hyphen-separated segments. Default import aliases replace
hyphens with underscores. Directory/repository names are advisory; the fetched
package declares its own name.

| Identity | Binds |
| --- | --- |
| Package name | Human-facing name; not globally unique or security identity. |
| `PackageKey` | Declared name and canonical source lineage. |
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

## Evaluated build work

After graph closure, helpers may borrow the root Build value for provider
selection, root binding, staging, and other evaluated work. Authority follows
that value and the checked call graph, not a helper name or receiver spelling.
Transitive reach, invocation, blocking/suspension, termination, observations, and
authority demands compose into the root's published ceiling.

`BuildSource`, `BuildOutput`, and `BuildLog` are compiler-owned, activation-scoped
facets. They do not enter normalized Build output. The sponsor enforces source
read/output write roots, containment, limits, and custody. Logging is explicit
and captured. Selecting a runtime `FilesystemHost` or `Console` provider does
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
[lock codec](../../../omega-rust/omega/packages/manager/src/lock/README.md)
retains one selected source closure with target sections, not a complete
multi-root workspace lock. Multi-root storage remains implementation work;
neither catalog membership nor another root's acceptance can fill missing state.
