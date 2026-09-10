# 0008: Build dependency scopes and isolated inputs

Status: proposed, not ratified or implemented. This augments ordinary Omega build
evaluation; it does not introduce a second build language. API spellings below
are candidates. Existing scoped filesystem enforcement is not evidence that the
proposed dependency scopes or default input isolation already exist.

## Problem and scope

A build should be able to import a third-party code generator or topology checker
without granting application source access to that package, requiring it to run
on the application's target, or giving it ambient access to the developer's
machine. The helper should read explicit inputs, compute using ordinary Omega,
and return data or write private staged artifacts.

The current package model has one unconditional dependency set. A library used
only by the build is still an ordinary package dependency. This does not imply
that its code is emitted in the runtime binary, but runtime reachability is not
a substitute for a checked dependency boundary. Existing build filesystem access
is scoped real access; it is not an isolated virtual input snapshot by default.

Retain one `machine build(builder: &mut Build)` entry. Add:

1. Explicit build-only dependency declarations and separate selection contexts.
2. Read-only captured inputs and private staged outputs as the default filesystem
   contract, with explicit delegation for any additional host authority.
3. Generic required output publication, including artifact-only builds, without
   compiler knowledge of a package's artifact format or policy algorithms.

The customers are code generation and
[0007's topology package](0007_checked_boundary_topology.md). Neither needs
topology syntax, a plugin registry, native execution of arbitrary downloaded
code, or compiler-specific graph policies. Reflection is optional for authoring
helpers, not a dependency of this proposal.

## Existing owners

Extend these contracts rather than introduce parallel acquisition, evaluation,
or evidence systems:

- [Build declarations](../spec/build/declarations.md): root discovery, package
  identity, and direct dependency projection.
- [Build execution](../spec/build/execution.md): admission before effects,
  generated-source strata, dependency build outputs, and observation custody.
- [Modules](../spec/language/modules.md) and
  [package boundaries](../spec/packages/boundaries.md): authored name selection,
  public visibility, and carrying foreign types.
- [Configuration](../spec/build/configuration.md): exact-target evaluation and
  result identity.
- [Package acceptance](../spec/packages/acceptance.md): independently accepted
  policy, immutable dependencies, and separation from resolver authority.
- [Component publication](../spec/build/component_publication.md): complete
  component interfaces and installation requirements. This proposal supplies
  no replacement topology or deployment semantics.

## Dependency declarations and discovery

Keep `depend` / `depend_as` for product dependencies. Add `build_depend` /
`build_depend_as` for build-only dependencies, with the same source-selection
vocabulary. Both forms must be direct statements on the canonical root builder,
just like existing dependency declarations. Aliases are unique within each scope;
the same alias may identify different packages in the two contexts without
cross-context lookup or fallback.

Illustrative build source:

```omega
use topology::policies;
use build_support::configuration;

machine build(builder: &mut Build) {
    builder.application("payments");
    builder.build_depend_as("topology", Source::Path {
        location: "../topology"
    });
    builder.depend_as("protocol", Source::Path {
        location: "../payment-protocol"
    });
    configure_products(builder);
}
```

Here `configure_products` is an ordinary helper from the local
`build_support::configuration` module. Code needing only a schema, input bytes,
or an output sink receives those values instead of the whole builder. Passing
the whole builder is deliberate authority delegation, not the library default.

Discovery parses the root's imports without resolving or executing them, extracts
both dependency sets, resolves their immutable closures, then checks and admits
the build entry and its helpers. A top-of-file import may therefore refer to a
build dependency declared later in the entry. Package/dependency discovery cannot
call imported helpers, evaluate a dependency-dependent path, or read generated
source. Source location/selector arguments retain the existing direct-projection
restrictions; no extra discovery-time evaluator is introduced.

The build can be split across ordinary imported source files. Only the selected
root declares its role and dependency sets; imported helpers do not become build
roots or acquire declaration authority. Importing another package's build entry
remains forbidden. Share ordinary public library code, not another activation's
root or builder state.

## Two checked contexts

| Occurrence | Selectable dependency code | Execution context |
| --- | --- | --- |
| Build entry and transitive helpers | Its package-local build sources, core, and its declared build dependencies | Admitted build evaluator on the host |
| Product source and its generated source | Its package-local product sources, core, and its ordinary dependencies | Selected product target |

Core's exact toolchain vocabulary is directly available. Std is an ordinary
package: a build-only std use requires a build dependency, a product use an
ordinary dependency, and both uses require both edges. Import spelling does not
grant an exception or host authority. A build dependency's ordinary dependencies
are its library implementation dependencies in the host/build graph, not edges
exported into the consuming product graph. Its own build dependencies belong to
its separate build activation. Detect cycles across these role-specific build
and artifact prerequisites; splitting scopes must not hide a scheduling cycle.

Source bytes may be acquired once, but checked instances, aliases, target
selection, provider choices, generated bundles, and evaluation results cannot
be shared merely because the bytes match. Cache/admission keys bind package
instance, purpose (build or product), host/evaluator and selected target facts,
dependencies, inputs, and policy. Never reuse host-generated configuration or
provider plans as target evidence. A helper may receive the intended product
target as explicit data without being executed as target-native code.

A local file imported in both contexts is checked separately against each
context's dependencies and authority. This is not filename-based trust: naming
a file `build_support.omg` grants nothing. A shared file importing a build-only
alias rejects in the product context. Compiler-issued Build vocabulary must be
nameable by authorized build helpers without leaking build authority into product
code or confusing an application-owned type with the same name.

Build-only packages remain part of build provenance and reproducibility. They
do not automatically become runtime code, product-source imports, or runtime
providers. Exporting generated source that refers to one still requires an
ordinary product dependency. If generated bytes depend on a build library,
their origin remains recorded even when the resulting program has no runtime
dependency on that library.

This revises the existing one-set nameability rule; it is not a claim that current
dead-code elimination already enforces separate contexts. Migration must classify
existing edges as product, build, or both, with diagnostics for missing edges.
Do not silently infer broader permission from an old lock entry. A versioned
lock/review migration retains acquisitions while rechecking role-specific edges.

## Inputs and default filesystem

Every build activation, including a dependency's own build, starts with:

- A read-only view of its own exact package source snapshot and explicitly
  supplied immutable inputs.
- A private, initially empty staged output tree with bounded storage.
- Explicit bounded diagnostics and evaluation resources.
- No ambient home directory, consumer source tree, credentials, network,
  subprocesses, or unrestricted host filesystem.

"Virtual filesystem" specifies observable isolation, not mandatory RAM storage.
The implementation may use an in-memory tree or confined disk backing, provided
paths, metadata, reads, writes, and failure behavior have the same contract.
Unsupported access returns a checked error; a no-op filesystem must not report
successful writes or fabricate empty files. Path traversal, escaping symlinks,
host-handle fabrication, and concurrent host-path substitution must not escape
the input/output views. Disk backing needs a race-safe implementation or an
explicitly adequate isolation premise, not canonicalization alone.

Package input membership comes from the exact captured source inventory; no
implicit reads of parent directories or mutable host files are permitted. A
secret included in that inventory is readable input, not protected by its name.
Extra files are captured by the caller/sponsor before being exposed as immutable
inputs. Record their exact consumed bytes and admitted metadata. Replaying a
build cannot reread a changed host pathname and call it the same input.

Code generators should normally consume input bytes/views and a narrow output
writer. A library needing filesystem-shaped access can accept explicitly passed
snapshot/staging capabilities through an ordinary adapter. Importing std's
filesystem API must not silently switch to a real-host runtime provider; the
current compiler-owned facet admission remains the starting enforcement boundary.
The exact reusable facet interface is an implementation prerequisite, not an
assumption that every existing std filesystem call runs in builds today.

Additional real-host access is opt-in under independently accepted consumer and
executor policy. Requests by downloaded code cannot grant themselves authority.
Specify operation, root/object scope, lifetime, bounds, and observation/replay
requirements; do not use an undifferentiated "trust this package" flag. A host
grant does not propagate automatically to dependency builds or sibling helpers.
Prefer capturing another input over lending live filesystem authority. Network
and subprocess facilities, if later justified, need their own admitted protocols;
a filesystem grant or std import provides neither. Resolver credentials and
package retrieval remain outside build code's authority.

Calling a library within the root build does not create a new sandbox. It can
use capabilities passed by its caller. Passing the root builder may permit all
of that builder's operations; use narrower values when that is not intended.
Reading permitted secrets and writing them to permitted logs/artifacts is still
possible. This proposal provides confinement and explicit delegation, not
information-flow security or protection after an intentionally broad grant.

## Staged products and failure

Retain the existing generated-source rule: included bytes join a later product
checking stratum and cannot redefine the already admitted build or introduce
new build helpers for that invocation. Dependency-generated source enters only
through the corresponding role/target-bound handoff, never a mutable output path.

Add a generic required-output obligation under the owning Build authority.
Illustrative names are `builder.output.require(name)` and completion with a
staged artifact. The obligation is activation-local, cannot be forged or silently
dropped, and names one output identity; duplicate completion rejects. Helpers may
receive one output obligation without authority over all build products. A build
with an uncompleted required output fails even if an ordinary error result was
ignored. Optional scratch files establish no successful product.

Ordinary package code computes and serializes its artifacts. Completing an output
binds exact bytes and declared dependencies; it does not cause the compiler to
understand or approve a package's semantic claim. A library's checked-result type
can gate its normal serializer, but an independent consumer must still check the
artifact's meaning and assumptions. A malicious writer can produce arbitrary
bytes, not a valid proof by marking an output complete.

On unsuccessful evaluation or incomplete obligations, publish no new successful
product set. Retain prior successful outputs separately; diagnostics are not
success artifacts. Internal scratch writes may occur before failure. Explicitly
granted live-host effects need their own failure contract and are not magically
rolled back by discarding staged products.

An application build may publish companion artifacts. A proposed explicit
`builder.artifact_only()` output intent publishes only declared artifacts and
requires no native ProgramEntry; it is not a fourth package/workspace role.
Omitting runtime roots must not implicitly select that intent or hide a broken
executable build. Existing executable applications retain their root obligations.
The exact surface and static-discovery treatment of output intent remain design
choices; they must not depend on executing a library to discover package identity.

Artifact formats, codecs, graph policies, policy evidence, and installers belong
to packages. Build records the admitted execution, exact inputs, dependencies,
and outputs through its generic provenance machinery. Publishing bytes is not
proof that a graph models a component or that a deployment is confined. No new
compiler callback that recognizes a topology verifier or arbitrary native plugin
is introduced. A separately selected installer receives its own explicit runtime
authority and independently validates package-defined artifacts before activation.

## Target inspection and reflection

Build execution may inspect explicitly supplied, independently verified component
descriptions without executing those components. Use the existing component and
Terminal evidence owners to expose closed imports/exports, authority completeness,
and exact semantic identity as owned descriptions. Reading raw artifact metadata
does not make it verified. Missing completeness or unaccepted assumptions remain
failure to establish the required claim, not an empty interface.

Type reflection may help construct schemas for explicitly authorized types, but
does not reveal arbitrary binaries, private code, installed endpoint identities,
or complete executable authority. A build-only alias does not authorize reflection
over all product dependencies. Start with explicit component descriptors rather
than a new privileged "reflect the whole application" API. Any missing generic
verified-description consumer must be scoped in its existing owner with positive
and mutation tests; this document does not claim that interface implemented.

## Acceptance and open choices

| Case | Required result |
| --- | --- |
| Top-of-file import of a directly declared build dependency | Discovery succeeds before import resolution; helpers execute only after admission. |
| Dependency declaration hidden in a helper or conditional | Reject during discovery without executing it. |
| Local multi-file build helper | Checked in the build context with exact Build vocabulary and normal visibility. |
| Application or generated source imports a build-only dependency | Reject, even if build evaluation already loaded its source. |
| One source package used for host tools and a different product target | Shared acquisition permitted; checked contexts and outputs remain distinct. |
| Changed build-tool revision or input | Build provenance and affected results change; no stale output reuse. |
| Dependency generator reads its template and writes generated code | Succeeds using snapshot/staging capabilities without live-host access. |
| Reads home/consumer secrets, follows an escaping link, fabricates a handle | Explicit refusal before unauthorized host access. |
| Helper is deliberately given a wider input | May read that input; no false confidentiality claim. |
| Runtime filesystem/network provider selected for a build helper | No bypass of build-facet admission or automatic host grant. |
| Partial output, ignored error, unfinished required output | No new successful product publication. |
| Artifact-only build versus executable missing an entry | Only the explicit artifact-only intent may omit executable roots. |
| Validly published file contains a forged topology verdict | Package verifier/installer rejects; publication is not semantic approval. |

Run actual acquisition, multi-file build execution, and generated-source checks,
not just dependency-parser tests. Exercise snapshot/staging isolation and failure
paths on Windows and macOS; source inspection is not a host execution pass.
Compare two packages using the same input and output names to detect cross-build
leakage, and test same-package dual-purpose cache entries with different targets.

Before ratification settle the declaration names, alias migration, reusable
input/output facet API, required-output failure surface, and artifact-only intent.
Specify which captured metadata is observable and how input/grant changes affect
replay keys. Host-native plugin loading, dynamic dependency discovery, ambient
network access, and a general deployment product are out of scope.

One Build argument with scoped operations is the baseline: two arguments do not
by themselves enforce dependency or authority separation. A separate build-tool
package per project remains an alternative if it reduces context complexity, at
the cost of another entry/configuration and explicit product-input handoff.
Pure byte-input/byte-output generators are a narrower viable first customer;
they need not wait for every virtual filesystem operation. No-op I/O and relying
on linker stripping to enforce build-only access are rejected alternatives.

Acceptance updates the owning specs and implementation tasks. Neither this
proposal nor 0007 is an execution-board prerequisite before that decision.