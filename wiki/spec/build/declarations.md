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

`package`, `application`, `member`, `depend`, and `depend_as` are statically
projected. Alias uniqueness covers one unconditional dependency set. No target
control-flow interpretation, conditional dependency helper, or per-profile map
discovers additional edges. A role-less file cannot yield dependencies or receive
an automatic dependency edit.

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

## Exact-target evaluation

An invocation selects one target or a nonempty explicit canonical set. Share
target-independent source/parsing, project role/name, and unconditional dependency
rows. Each exact-target child filters target-scoped implementation declarations
and resolves its own root slots. Flat target-qualified root bindings are not a
support matrix.

Evaluated Build state is not shared evidence: target, slot admission, filesystem
observations, generated output, provider selection, and optimization selection
belong to the exact child. A checkpoint or cache name cannot make them independent
of target. No conditional dependency surface is reserved for a hypothetical use.
