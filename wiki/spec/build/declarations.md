# Build declarations and package identity

`build.omg` is ordinary Omega orchestration in an explicit build-host context.
It is distinct from the hermetic semantic evaluator for constants, proofs,
plans, and generators. Host effects need admitted build authority; there is no
second configuration grammar or ambient runtime service.

The default build environment supplies confined filesystem backing through
ordinary service contracts. Snapshot/staging is a supported library profile.
Checked declarations and transitive build contracts expose restricted host
requests. The invocation supplies the grants for this Build; a parent can
delegate them to a child. The [version 4 workflow](../packages/acceptance.md#restricted-build-acceptance)
also checks its retained lock acceptance during migration. Build selections
also cannot choose an external ecosystem's receiving policy. Producing a binary
requires no such policy declaration.

A Build runs as one checked Omega program. The invoker supplies the root's
library bindings and the parent configures each child's bindings before that
child starts. Before entry the compiler resolves the local and external imports
it needs to check that Build. See [checked Build stages](scoped_execution.md#checked-build-stages).

The running Build may conditionally register product libraries with
`Build::dependency`. The registration fixes the description's source, alias
and delegated context before the coordinator can schedule work. Its immutable
`DependencyRegistration` identifies that request for
[validation](validation_subjects.md#buildvalidate). Registering a library or
suspending for work cannot make new code callable by this Build. At resumption
the interpreter preserves the same mutable Build and live storage.

When `build` returns, package coordination joins every outstanding dependency
request, rejects failed, cyclic, unresolved, or unadmitted dependency edges, and
freezes the normalized Build intent. Product selections may retain inert exact
designated references; product discovery and checking must resolve and validate
them before a completed product or executable plan is admitted. Product bodies
never execute during Build; product compilation starts only after the join.

## Native binding intent

The owning package records a direct native binding with
`Build::bind_native::<Requirement>(builder, target, locator)` under
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
author API for manufacturing trusted plan rows.
`Build::bind(builder, slot, entry)` selects the product entry.

## Project role

A selected build file has one free entry. The invoker owns the plan and
passes it by mutable borrow.

```omega
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };
}
```

The example uses the package already supplied to this invocation. A helper edits the
same plan. When the driver runs the selected successor, it supplies that
activation with a fresh plan of its own.

The invoker installs the activation's [sandbox bindings](scoped_execution.md#build-sandbox-bindings)
before entry. Filesystem and console use ordinary provider selection. Selection
identifies the implementation. The caller supplies its admitted backing, such
as a confined disk directory or a bounded output sink. The first filesystem
implementation may use disk throughout. A Git package that performs a live
fetch declares its network and filesystem effects and runs within its
invocation's grants. A later consumer of the fetched package has its own grants.
[Dependency-use ceilings](../packages/staged_inputs.md#dependency-reach)

The invoker initializes `plan.product` to `BuildProduct::Unselected`. The Build
or an ordinary helper assigns the product before returning.

| Product | Meaning |
| --- | --- |
| `BuildProduct::Package { name: "kit" }` | Publish the named package. |
| `BuildProduct::Application { name: "editor" }` | Build the named application. |
| `BuildProduct::Workspace { members: paths }` | Declare the workspace's member paths. |
| `BuildProduct::InterpretedBuild { entry: "configure.omg" }` | Run another Build in the selected package source. |

The driver reads the value in the returned plan. A helper may assign the field
again before return. The final value must describe one complete product.
Name and entry views must live through that return. A helper can accept
`plan: @mutable BuildPlan<'s>` and `name: @'s [u8]` to assign a name with
the plan's storage lifetime. Borrowing an array local to the Build cannot
establish that lifetime. [Stored views](../language/lifetimes.md)

A Build can keep bytes it computes locally by explicitly copying them into its
invocation's immutable storage. Import `use {omega}/build/storage.omg;` and
call `|storage|::copy(plan, @bytes[..])`. Its result has the plan's storage
lifetime and can initialize any shared byte view with that lifetime, including
a Product name or successor entry. The source buffer can be reused immediately.
The Build's evaluation storage allowance covers the copy.

An authored Build that returns an unselected product, an invalid name or
path, an empty workspace, or a repeated member rejects. The `with_member(plan, catalog, path)` operation in
`{omega}/build/workspace.omg` returns a new `WorkspaceMembers` value with
that pathname appended. The invocation owns the copied pathname bytes.
`WorkspaceMembers {}` starts an empty catalog. Only the
catalog stored in the returned Workspace contributes member paths.
A query that requires a named Product uses the name
and role in the plan at that call; the returned plan must preserve the identity
that issued its description. A successor receives a fresh plan. After the current entry
returns successfully and its dependency join completes, the driver freezes
the selected source view, resolves the named file within it, and checks its
free Build entry against the registered package
inputs for the interpreter target. Missing or invalid entries reject. The
driver admits the successor before running it and follows its plan to the
project's final result. Any failed stage fails that chain. A dependency still
has to produce the package its consumer requested.
[Package handoff and example](../packages/staged_inputs.md#from-source-to-the-next-stage)

`Owner::build` is not a project root and cannot acquire a synthesized receiver.
An ordinary same-spelled machine remains ordinary source.

With no selected build file, the compiler supplies ordinary default Build
source and runs it through the same one-entry checking, interpretation, join,
and freeze route. The default entry leaves the initial product field unselected
and retains the invoker's supplied role. No file need be written. A standalone loose root defaults to
application role; an already resolved package retains its supplied role and
owner. Checking source does not require an executable entry. Executable output
still needs ordinary target-slot selections and provider checks. When the
named root has no build entry, core supplies the product entry binding under
[build-first entry selection](command_line.md#build-first-entry-selection).
The target checks that binding under [entry admission](entry_roots.md).

Product selection and dependency registration operate on this activation's Build plan. The invoker's bindings determine which libraries
Build code can import. The executed plan determines which libraries Product
code can import. Aliases are unique within each scope. A package used in both
scopes needs a binding in each, including system. Package tooling may persist a
product dependency by editing ordinary Build source. Neither an import nor a
legacy lock can supply a missing edge.
[Scoped execution](scoped_execution.md#two-checked-contexts) defines
purpose-specific scheduling, acquisition reuse and provenance.

Applications default to executable output. The Build activation may
request `artifact_only` once for an application, never a package/workspace. It
requires at least one completed required artifact and rejects executable
root/provider selections. It is not a new role or target profile.
[Staged products](scoped_execution.md#staged-products-and-failure) defines
completion and whole-result publication.

### Successor package and directories

Named decision: `successor-package-and-directories`.

An interpreted Build product has one package root and may supply additional
runtime directory mappings to the immediate successor. When `package_root`
is omitted, the successor uses the current captured package. The parent may
instead supply a directory view it holds, such as a directory containing a
generator’s output. The plan retains an owned view or a borrow valid through
handoff. After the preparing Build returns and its dependencies join, the
driver freezes that view once. It resolves `entry` and local imports within
this snapshot. External imports use the ordinary registered package bindings.

The successor sees that same frozen package read-only and receives fresh,
private, bounded writable scratch. Omitting `directories` and supplying an
empty list both select this default. An additional mapping supplies a guest
path, a directory view held by the parent, and read-only or read-write access
within the parent's authority. A mapped directory is available to runtime
filesystem calls. It does not supply a second package root. The grant ends
at this successor unless explicitly delegated onward.

A parent selects its generated package with an issued path. An empty relative
path selects the held directory itself.

```omega
let generated: BuildPath = BuildOutput::resolve(@plan.output, "");
plan.package_root = BuildPackageRoot::Directory { path: generated };
plan.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };
```

A Build can also return additional runtime mounts while keeping its original package.
For example, a preparing Build that holds the `sdk/headers` source subtree and its
private output can lend both to its successor.

```omega
let headers: BuildPath = BuildSource::resolve(@plan.source, "sdk/headers");
let out: BuildPath = BuildOutput::resolve(@plan.output, "");
plan.directories = Build::map_readonly(plan.directories, headers, "/sdk");
plan.directories = Build::map_readwrite(plan.directories, out, "/results");
plan.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };
```

Each call extends the supplied list. The returned plan selects the list the
successor receives. A directory backed by a captured snapshot can be delegated read-only.
A grant of private output retains its sponsor and is subject to the same limits
on entries, bytes and file extent. Duplicate guest roots are rejected.

A Build can forward an absolute pathname in the preparing Build's
existing runtime mappings. For example, a Build receiving an SDK at
`/sdk` can forward only its `headers` directory.

```omega
plan.directories = Build::forward_readonly(plan.directories, "/sdk/headers", "/include");
```

The selected subtree retains its more-specific mounts, including any that hide
files in a broader grant. A read-only handoff makes every descendant read-only.
A writable handoff requires a writable root and preserves each descendant's
access mode. Looking up an ungranted path refuses the handoff.

With the default `BuildPackageRoot::Current`, the entry belongs to the
preparing Build's captured package. If the parent also maps an SDK at `/sdk`
read-only, the successor can read SDK data while it runs. With an empty
mapping list it can still import and read its generated `parser.omg`, while
the parent's SDK and output directory stay private.

The same rules apply to confined disk directories and views with other
backing. They use ordinary filesystem provision and admission. A registered
dependency's explicit override follows the separate
[child filesystem contract](scoped_execution.md#supplying-a-filesystem-to-a-child-build).

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

## Library responsibilities and names

Named decision: `operating-system-library-name`, settled 2026-11-12. Rename
the current `std` library to `system` and `alloc` to `allocation`. The names
describe responsibilities that guide library organization:

| Name | Responsibility |
| --- | --- |
| `core` | Foundational language-facing types, contracts and mechanisms. Compiler privilege remains confined to exact primitives, not the entire package. |
| `build` | The stock Build sandbox’s typed host interface, used by providers running in that environment. [Build sandbox bindings](scoped_execution.md#build-sandbox-bindings) |
| `system` | Interfaces and adapters to the execution environment: operating systems, firmware, platform entry and related services. |
| `allocation` | Strategies for obtaining, partitioning, reusing and returning storage under their contracts. |
| `collections` | Containers and operations on their elements, using the storage arrangements their contracts permit. |

Firmware includes UEFI and its Boot Services handoff; `system` therefore
does not mean only operating-system calls. OS independence is not an admission
criterion for `core`: parsers, collections and compression can remain ordinary
packages. Allocation need not use a heap or ambient allocator; a strategy can
work over explicitly supplied storage without creating backing authority.

These are library responsibilities, not compiler categories or a requirement
to version every responsibility separately. Modules can express the logical
separation; independent use, dependencies or replacement can justify later
package boundaries. The existing allocation library's container customer does
not make all containers allocation strategies or fix their permanent home.
No container rename, generalization of element support or collections extraction
is required by this decision. Storage growth remains explicit where the
container contract requires it; a name does not authorize hidden allocation,
release or loss of resident-value cleanup obligations.

Names should help readers predict the abstraction; conventional vocabulary is
useful when it does so, and spelling out an abbreviation alone proves nothing
about the contract. `std` can describe a standard distribution offering, but
`system` describes the responsibility chosen for this package. Neither name,
nor matching paths, grants compiler custody, platform authority or privileged
provider selection. Those remain governed by exact primitives, admitted
contracts and resolved dependencies.

The packages declare their names in their own Build entries. Ownership and
compiler coupling follow the rules below.

## Ordinary library packages

Named decision: `ordinary-library-packages`, settled 2026-11-12. `system` and
`allocation`, wherever no compiler privilege is required, use ordinary package
ownership, dependencies, validation and immutable version selection. They may
be distributed beside the compiler, fetched from Git or supplied by another
accepted source without acquiring a new semantic package kind. The
[standard distribution](../packages/toolchain.md#selecting-a-bundled-package)
ships them for explicit selection by package name.

Keep four identities and obligations distinct: the distribution digest verifies
the release contents; the package owner identifies declarations; exact sealed
bindings identify compiler-supplied behavior; and dependency resolution chooses
the package instance used by a program. A whole-bundle digest alone does not
merge nominal owners or prove that packages share primitive authority. Making
`system` optional must preserve integrity verification for the components the
distribution declares, not disable it to tolerate missing files.

A request requiring only its selected core and target support must not require
an installed `system` merely to make library discovery succeed. Default runtime
and entry support follows the target's declared dependencies and contracts
through normal selection. Do not silently import pieces of bundled `system` with
toolchain-contract custody or mix them into an independently selected compatible
`system` revision. Distribution defaults may select ordinary package revisions;
they do not override the resolved dependency graph by filename or short name.

Genuinely compiler-supplied primitives and unavoidable entry/ABI bridges retain
explicit, tightly identified support under the existing sealed catalog and
admission rules. Separate those minimal declarations from ordinary wrappers;
do not make an entire surrounding library compiler-special or move all of
`system` into core. A toolchain-owned core declaration is not automatically a
compiler intrinsic merely because of its package owner. A package named `system`,
matching paths or copied signatures/source bytes cannot confer compiler custody.
Console formatting, collections, filesystem adapters and other ordinary library
behavior retain normal checked implementations and package identities.

The [validation delivery plan](../../drafts/designs/validation_subjects.md#ordinary-library-migration)
tracks removal of mandatory/pinned `system` coupling independently of the
registration-validation API and narrow core-check-root repair. This settlement
does not itself deliver the library rename above or the
[runtime providers](task_runtime.md#runtime-providers-and-call-authority).

## Default builds and anonymous owner identity

A missing build file is not missing provenance. Preserve any exact owner already
supplied by the resolved source graph or an imported artifact. Core retains its
toolchain owner; ordinary system retains its resolved package owner. Neither shares
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

## Releases with independent products

An ordinary release program coordinates independent project builds. For example,
to package a kernel and a bootloader, it requests each product and passes the
two successfully published artifacts to an image packer. The packer can be an
ordinary machine. A separate image project can also consume those artifacts
as explicit inputs. Each producer's own Build describes its project.

The release program can launch `omega build` as a subprocess if its execution
environment grants process access, or call the same project-build operation
through a library. Both routes apply ordinary package resolution and review,
Build admission, target selection and artifact publication to each request.
For example, an interpreted release tool with process access can call a native
compiler executable. A native release tool can instead link the project-build
library. The release tool supplies each consumer with the producer's committed
artifact, so a failed producer cannot supply a successful result.

This decision (`independent-product-orchestration`, 2026-11-22) puts coordination
in the caller. Build has no operation for launching independent product builds
and joining their outputs. Package dependency registration still prepares a
dependency's own Build for its registered purpose.

### Outgoing operations from a checked entry

The compiler library exposes a read-only, in-memory query on a checked product.
Given an entry belonging to that product, it returns a typed result identifying
the boundary operations and authority that execution from that entry may reach.
The answer applies to the exact checked product, target and provider selections.
A caller can query several products before requesting native emission from the
same checked inputs. The query requires neither another Build execution nor an
on-disk report.

Follow the [checked calls](../terminal-psi/product.md) through helpers, callbacks
that can run as a consequence of the entry, cleanup and selected providers. A
separate maintenance entry contributes an operation to the public handler's
answer only if the handler can reach it. For replaceable providers, account for
every implementation allowed by the replacement contract. For opaque code, use
its admitted contract and identify the assumptions on which the bound depends.

The result distinguishes a complete upper bound from incomplete coverage. A
complete bound includes every possible outgoing operation, even when some occur
only on particular paths. With incomplete coverage, return the known operations
and identify where the analysis loses the bound. Only a complete empty bound
establishes absence. Apply the same [coverage and authority rules](component_publication.md#verified-component-descriptions)
to external calls as to the component inventory.

The release application maps operations to service instances using its endpoint
bindings and checks the resulting routes against its own policy. Knowing that an
entry can send through a socket or raw network device leaves its possible peers
unrestricted unless the installed transport or device enforces a narrower bound.
A complete inventory of send operations can therefore still leave the destination
unknown.

## Workspaces

A workspace is a path-declared catalog of locatable members, not a combined
dependency graph or a node with its own package key. Each application/package
remains an independently selected closure root; building one does not include
unrelated members. A check that names no member takes every declared member as
a [validation subject](validation_subjects.md); a product may add registered
dependencies to its own subjects with `Build::validate`. Shared pins and
ceilings may be passed into member Build
values, and members may only narrow them. Source code never searches parent
directories for imports; only tooling discovers the enclosing build/workspace.

Remote selection uses the member's declared name, not its path. Paths remain
navigation and the base for relative requests; relocation does not rename the
package. Reject undeclared/escaping members, absent/duplicate names, and recursive
search for undeclared build files. Source kinds are extensible; Git is not the
semantic definition of a source. There is no independent package version field:
the exact resolved source supplies instance identity.

Under [staged package inputs](../packages/staged_inputs.md), each package
authors its direct requests. Workspace membership supplies no pins or approvals.
In the version 4 workflow the workspace owns its lock and dependency locks do not pin the consumer's graph.
Membership does not merge acceptance across roots. A multi-root workspace lock
must retain each root's source closure, target sections and acceptance separately;
neither catalog membership nor another root's acceptance can fill missing state.
