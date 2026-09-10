# 0005: Build dependency scopes and isolated inputs

Status: proposed, not ratified or implemented. This augments ordinary Omega build
evaluation; it does not introduce a second build language. This revision selects
proposed semantics for the stage boundary, snapshot protocol, and output lifecycle;
API spellings remain proposed vocabulary. Existing scoped filesystem enforcement
is not evidence that these dependency scopes or default input isolation exist.

## Proposed decisions at a glance

| Question | Selected first-slice answer |
| --- | --- |
| One builder or two? | One `Build`; scope and authority are enforced at individual operations. |
| Build-only code? | Direct `build_depend`/`build_depend_as` declarations; separate host/product nameability and checked instances. |
| Reference target code from a host build? | Existing designated product operands plus qualified, non-callable product descriptions; no general host import of target code. |
| Filesystem default? | Exact read-only input tree, append-and-seal staging, deterministic logical metadata, no implicit symlink traversal or ambient services. |
| Extra host files? | Independently supplied captured inputs; live-host protocols are not added by this slice. |
| Build failure? | Checked local errors, linear required outputs, sticky explicit failure, and no final result set after interruption or failed checks. |
| Artifact-only application? | One direct root declaration, at least one completed artifact, and no executable roots or provider selections. |
| Generated entry or inspection of this build's final executable? | Separate staged compilation; no cycle through this invocation's generated source. |

Detailed rules below define these proposed decisions. The comparison section
explains why the compiler owns the boundaries but not application-specific work.

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
3. Explicit product-description selection without executing target code.
4. Generic required output publication, including artifact-only builds, without
   compiler knowledge of a package's artifact format or policy algorithms.

The customers are code generation and
[0004's topology package](0004_checked_boundary_topology.md). Neither needs
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

In this proposal, host means the admitted build execution profile, not a value
inferred from the compiler process's OS. `Build.target` continues to describe the
product child as in the existing configuration contract. Build helper code and
its own ordinary dependencies are checked for the execution profile. If a build
helper needs its own build invocation, that invocation produces the helper for
this execution profile; it does not receive the consuming application's target
unless passed explicitly as input. Thus a macOS code generator for a Windows
application must not select Windows filesystem implementations for itself.

Model scheduling nodes as `(package instance, purpose, execution profile,
requested product target)`, omitting only coordinates that do not apply. A
product node waits for its build output and product dependencies; a build node
waits for its host helper libraries and captured input artifacts. A helper library
uses its ordinary dependency edges in that host context, not its own build-only
edges as public library imports. Reject cycles in this complete prerequisite
graph before running an affected activation. No implicit recursive compiler call,
new dependency discovery, or reading the current product's unfinished output can
resolve a cycle.

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

For the first migration, keep `depend`/`depend_as` product-only and require authors
to add the build edge explicitly where needed. Do not infer an edge from loaded
imports or silently turn each existing dependency into both. Acquisition records
can be reused after exact identity validation; a lock lacking dependency-purpose
information is not accepted as a scoped-build verdict. Report the requesting
occurrence, missing scope, and suggested explicit declaration. Same-alias entries
in different scopes remain independent even during migration.

## Selecting product declarations without executing them

Build code must bind product entrypoints and providers, even though product-only
code is not a host library. This requires direct compiler support at a small,
explicit boundary. Ordinary imports, calls, and generic applications in build
helpers always resolve in the build context. They never fall back to product
names when lookup fails.

Retain the existing toolchain operations with designated product operands:

```omega
builder.roots.bind(windows_x86_64::ProgramEntry, Application::start);
builder.select_provider<windows_x86_64::Console, TestConsole>();
```

Only those declaration-reference operands are resolved in the product context,
using the occurrence's package authority, exact target, and complete expected
slot/requirement signature. The `bind`/`select_provider` operations themselves run
as admitted Build operations. No product receiver, function body, initializer,
or provider is executed. A user-defined same-named method or generic helper gets
no special resolution. This extends the existing "selection is not invocation"
rule, not general syntax quotation or an exception to ordinary call checking.

For reusable build helpers, expose owned typed descriptions through the existing
Build facet: `ProductEntryRef`, `ProductProviderRef`, and `ProductTypeSchema` are
candidate names for distinct roles, not a single untyped compiler handle. A
fallible query such as `builder.product.entry(path, slot)` resolves a logical
product declaration path under the query author's visibility and product
dependency scope. It binds one exact declaration/application and target or
reports missing, inaccessible, ambiguous, or incompatible selection. Strings are
locators, not authority or overload identity; signature checking selects or
rejects rather than taking the first matching name. Reading a description does
not run the selected declaration. Helpers should receive already selected
descriptions, not the owner's entire selection capability.

Descriptions retain exact semantic identities and allowed operations, not raw
compiler indices or native pointers. They cannot be invoked, converted into host
types, used to fabricate a service grant, or supplied as ordinary static machine
arguments. Product schemas may be inspected as data, but a host reflection query
over a type does not become target-layout evaluation by accident. Geometry is
available only from an already validated target layout; if layout depends on the
build's unfinished selection, the query rejects as a phase dependency. Product
identity and host type identity are not interchangeable even when names match.

The query authority is lexical, as with semantic reflection. Passing Build to
a foreign helper grants operational capabilities but not visibility into the
caller's private declarations. An owner can select its own private entry and
pass the resulting restricted reference for binding, just as it can pass a
checked callable; the helper cannot enumerate sibling private declarations.
Product-reference provenance binds author, purpose, admitted source checkpoint,
target, and role. Final root/provider admission rechecks compatibility against
the completed product without rediscovering a different declaration by name.

Use ordinary description data with compiler-established qualifications and
declared role-specific operations, following the existing authority model.
The spellings above are not three new language type categories. No qualifier
can be established merely by deserializing fields or copying a semantic key.
An owned description may be retained as descriptive data, but using it for a
selection requires the matching live Build occurrence and target. The durable
result stores exact selected identities, not an escaping build capability.

Static product operands use explicit product-qualified paths or declarations
in the root's product module; ordinary host `use` exposure is not reused to
resolve them. A helper package can select only product owners within its own
authorized dependency scope and present in the supplied product frontier. It
cannot use the consumer's aliases or private namespace merely by borrowing
Build. Unbound helpers instead receive the owner's selected reference. This
keeps cross-stage resolution explicit even when an alias denotes different
packages in the build and product graphs.

Only the frozen authored product frontier and already admitted dependency
artifacts may be described during the build. Names introduced by this invocation's
generated source cannot be queried or bound as new build inputs. A generated
entry therefore needs a separately staged compilation in this first slice;
an authored wrapper cannot call it to evade the same visibility rule.
Checking a signature needed by a query cannot depend on the query's own build
result. Reject that cycle with the participating phases, not an empty schema.
Final authority closure and executable admission still occur after generation;
an early declaration description never claims the final component is complete.

This boundary is intentionally narrower than compiling all product code as a
second host library, introducing `product::` as a general language namespace,
or giving libraries access to a mutable compiler world. The compiler must own
cross-stage identity and scope enforcement; libraries own algorithms over the
returned data. Reflection and plugin frameworks cannot supply that identity
without the compiler establishing it somewhere.

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
successful writes or fabricate empty files. Path traversal, implicit symlink following,
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
assumption that every existing std filesystem call runs in builds today. The
following minimal protocol fixes its behavior without requiring POSIX emulation.

### Snapshot and staging protocol

Use exact logical UTF-8 relative paths with `/` separators, case-sensitive byte
comparison, and no Unicode normalization. Empty interior components, `.`, `..`,
absolute paths, backslashes, NUL, and drive-qualified spellings reject. The root
is selected by a capability, not embedded in a string. Names that the host cannot
capture losslessly reject capture; never silently rename or case-fold them. A
disk-backed implementation must preserve logical distinctions independently of
host path rules, or report inability to provide the requested snapshot before
execution. Host checkout location and directory iteration order are unobservable.

| Operation | Snapshot behavior | Private staging behavior |
| --- | --- | --- |
| `lookup` | Return exact entry kind or `NotFound`, without following a final symlink. | Same over the current private tree. |
| `list` | Immediate children sorted by unsigned UTF-8 name bytes, with kinds; no implicit recursive walk. | A stable listing of the tree at that operation. |
| `open` / read at byte offset | Regular files only; exact bytes, EOF at length, `InvalidRange` beyond length or on overflow. | Sealed files can be read through immutable views. |
| `metadata` | Kind and byte length for regular files; link-target byte length for links. No timestamp, inode, uid, ACL, host mode, or device identity. | Same logical metadata; no host clock or allocator address. |
| `read_link` | Exact captured target bytes, inert data. | Link creation is unsupported in the first staging protocol. |
| `create` / write / seal | `Denied`; no source mutation. | Exclusive fresh file, one writer, bounded append, then immutable bytes. Existing paths and open-reader/writer conflicts reject. |
| `mkdir` | `Denied`. | Explicit directory creation; an existing directory is an idempotent success, a file collision is an error. |

File reads take an unsigned offset and requested length. They return exactly
`min(requested, file_length - offset)` bytes; offset equal to length returns EOF,
offset beyond length returns `InvalidRange`, and zero requested length returns
empty bytes only for an otherwise valid offset. No host short-read or timestamp
observation changes this result. Directory metadata has only its kind; child
count comes from a complete bounded listing, not a fabricated byte length.
Staging paths are visible as files during writing but cannot be opened for reading
until sealing; such an attempt returns `InvalidState`, not `NotFound`.

No operation traverses a symlink implicitly, including an intermediate path
component. A package helper may resolve a relative link target as data and request
another operation under the same capability; an outside target still cannot
escape. Hard links in captured source have independent immutable file views;
there is no observable host inode alias. Sockets, devices, FIFOs, and other live
special files cannot be captured as readable source files. No cwd, environment,
locale, wall clock, random host seed, file locks, mmap, process launch, or network
operation is in this default protocol. An adapter must report unsupported
operations rather than supply fake timestamps or successful no-op I/O.

Core errors are explicit `InvalidPath`, `NotFound`, `WrongKind`, `AlreadyExists`,
`Denied`, `SymlinkTraversal`, `InvalidRange`, `InvalidState`, and `Unsupported` outcomes with the
operation and logical path. These are proposed semantic cases, not assigned wire
numbers. Resource exhaustion interrupts the activation unsuccessfully; code
cannot catch it and manufacture a different successful artifact based on host
storage pressure. Unexpected backing-store failure is an executor failure, not
`NotFound`, EOF, or a cacheable successful observation. Reads of a denied path
need not reveal whether a host object exists outside the capability.

Limits cover input/output bytes, file count, path bytes, traversal work, diagnostics,
temporary storage, and computation; immutable read repetition does not replenish
the sponsor's work budget. Checked append overflow refuses before mutation. Sealing
requires consuming the sole writer and retaining its exact bytes. Sealed files
cannot be reopened for writing, renamed, removed, or replaced in that occurrence.
There are no hard-link or overwrite operations in the initial staging protocol.
A helper may abandon scratch by releasing its custody, but a completed output
retains the immutable blob independently until result disposition. Parallel helper
work requires disjoint writer custody and deterministic publication; no general
concurrent filesystem is required for this first protocol.

Source capture happens before authored build execution. The resolver's immutable
package inventory supplies dependency inputs. A local root's capture uses an
explicit invocation inventory of files/subtrees; it does not implicitly expose
the entire working directory. A user-selected subtree includes its contents,
including any secrets there: ignore conventions are not a security boundary.
Already-required source files must be included. Other assets are listed by exact
path or explicit subtree, not by running a dependency's scanner with host authority.
Directory membership, missing declared inputs, and link bytes are committed as
well as file content. A required capture entry that is absent fails capture;
an explicitly optional entry records absence in the inventory. Capture must
return a coherent immutable tree or fail; files changing while being copied
cannot produce a falsely attributed snapshot. Tools may construct a capture
request under user authority, but downloaded build code cannot widen it.

Additional inputs arrive as a caller/sponsor-supplied map of named immutable
files or trees, captured before that activation. Names are scoped input slots,
not arbitrary host paths. The build may narrow a tree to a file/subtree capability;
it cannot widen one or fabricate another root. Dependency builds get only the
inputs explicitly assigned to their occurrence, not all inputs of the consumer.
This first slice has no build-time request that pauses to solicit arbitrary new
host access. A missing input is an explicit failure and a changed invocation.

Per-dependency input assignments are part of the pre-execution invocation and
its prerequisite graph, keyed by exact package/purpose/target occurrence. The
consumer's running build cannot grant new inputs retroactively to a dependency
whose build has already completed. To use newly computed inputs, invoke an
ordinary generator helper in the current build with those values, or arrange
a separate later build using an explicitly completed artifact. There is no
implicit graph mutation or recursive build API in this proposal.

For first implementation, cache keys conservatively include the entire admitted
input inventory and metadata profile, both dependency-purpose closures, executor
semantics, selected target, and accepted grant policy. This accounts for negative
lookups and directory listings without trusting user-authored watch lists.
Content-identical read-only snapshots can share storage; grants and activation
handles cannot. An output cache hit still requires exact provenance and current
admission/resource eligibility; it is not a way to accept code that the current
policy would refuse. Finer observed-input caching is deferred. No persistent
scratch cache is implicitly visible to a build.

Additional real-host access is opt-in under independently accepted consumer and
executor policy. Requests by downloaded code cannot grant themselves authority.
Specify operation, root/object scope, lifetime, bounds, and observation/replay
requirements; do not use an undifferentiated "trust this package" flag. A host
grant does not propagate automatically to dependency builds or sibling helpers.
Prefer capturing another input over lending live filesystem authority. Network
and subprocess facilities, if later justified, need their own admitted protocols;
a filesystem grant or std import provides neither. Resolver credentials and
package retrieval remain outside build code's authority.

The first portable protocol makes additional host files available by capturing
read-only inputs, not by lending arbitrary live paths. Existing explicitly admitted
live filesystem routes may remain separately identified transitional modes; they
must not report snapshot-isolated execution. Their host observations and external
effects require the existing replay/admission evidence, and are excluded from the
new snapshot-output cache unless a complete replay contract is supplied. Enabling
a mode is an independently accepted executor decision, never an option a package
can use to approve itself. A generally extensible live-host grant API is deferred.

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

Add a compiler-issued linear `RequiredOutput` under the owning Build authority.
`builder.output.require(name)` reserves one logical output name and returns its
activation-bound obligation, or a checked error. Duplicate names and file/directory
prefix collisions reject; no last writer wins. Names use the same logical path
rules as staging. Registration may depend on admitted build computation, unlike
package/dependency discovery. A caller needing a fixed output roster supplies
that roster as an invocation requirement: never infer that a conditional branch
which omitted `require` nevertheless checked the intended artifact.

Use ordinary ownership for delegation, with one narrow executor-owned completion
record that cannot be forged, reset, or satisfied by dropping the source value:

| Transition | Result and custody |
| --- | --- |
| `require(name)` | Record `Pending` and return its unique obligation. |
| `complete(obligation, sealed_file)` | Consume the obligation, bind the exact immutable file from this staging occurrence, record `Completed`, return an output receipt. |
| Completion error | Return the obligation and file custody; state remains `Pending`, so an explicit retry is possible. |
| `fail(obligation, diagnostic)` | Consume the obligation, record `Failed`; the activation cannot publish a successful product set. |
| Undischarged obligation | Ordinary linear checking rejects source-level discard. If interruption prevents disposition or finalization finds it pending, the activation fails; no implicit cancellation or new general linear-drop rule. |
| Build return | Succeed only if all registered and invocation-required outputs are completed and no sticky build failure exists. |
| Trap, cancellation, exhaustion, or executor failure | Abort the occurrence and its new product set; do not synthesize completion receipts. |

`complete` cannot accept an open writer, a mutable host path, a file from another
activation, or a previous run's receipt. A retained immutable blob may back a new
staged file, but requires a new completion under the current obligation. Completion
cannot trigger loading of artifact-defined code. Required outputs are files in
this first slice; packages can serialize manifests/archives for variable-sized
trees rather than demand another directory-product lifecycle.

Names belong to the output set, separately from scratch paths. Completion binds
the sealed bytes to the reserved output name; it does not rename or move a host
file. A completion receipt is provisional occurrence evidence until the final
result-set commit, not permission for an external consumer to observe early output.
No host/export executable bit is inferred from content, filename, or such a receipt;
executable admission remains a separate product/installation operation.

Use checked errors for recoverable helper/I/O failures. A root can retry or supply
an alternate valid result before completion. Once it chooses `fail`, that failure
is sticky. An ordinary log message is not failure and an ignored error cannot
complete an outstanding output. This uses compiler support because the evaluator
owns interruption, activation identity, and the publication boundary; a library
Boolean or destructor alone cannot enforce it. There is no arbitrary compiler
validation-plugin registry or new general effect mechanism.

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

Publish a complete immutable output-set manifest only after build execution,
generated-source checking, and all requested product checks succeed. Final
publication is one committed result identity, not a promise of an atomic rename
across arbitrary host paths. A failed export into a user-selected directory cannot
make a partial copy an admitted result; materialization reports its own failure.
Consumers read the committed manifest and bound blobs, not whatever files happen
to exist in a previous output folder. Per-target children retain independent
outcomes; failure of one child is not hidden by another child's successful set.

An application build may publish companion artifacts. Select
`builder.artifact_only()` as an optional direct unconditional declaration in the
root build entry, extracted with project role before execution. It is valid once
for an application, invalid for a workspace/package declaration, and cannot be
introduced by a helper, alias, conditional, or generated code. No fourth project
role is added. Executable is the application default. Artifact-only mode rejects
executable root/provider selections rather than silently ignoring them and must
complete at least one required artifact. Ordinary packages can publish companion
files without this application-mode declaration.

Artifact-only builds retain an explicit target/evaluation context from their
invocation; the mode does not imply cross-target equivalence or a new target
profile. A generator may inspect `Build.target`, so changing it changes the key.
This first slice does not automatically merge outputs from different target
children. Executable builds still require the target's ProgramEntry and other
mandatory roots even when they also publish artifacts.

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

The exact sequencing is:

```text
direct root/dependency/output-mode discovery
  -> dependency-purpose closure and immutable input capture
  -> host build checking + frozen authored product selection frontier
  -> build admission and execution using only those inputs/descriptions
  -> generated-source incorporation and final target/component checks
  -> required-output completion check and committed result set
```

Previously completed independent components may feed this sequence as explicit
immutable input artifacts. Inspecting this invocation's final component during
its own build is a cycle. A topology project that inspects freshly built services
therefore uses a separate composition build after those services, not a hidden
post-compilation callback in their build entry. A build cannot rewrite its own
dependency or input frontier by serializing a new manifest as output.

## Acceptance

| Case | Required result |
| --- | --- |
| Top-of-file import of a directly declared build dependency | Discovery succeeds before import resolution; helpers execute only after admission. |
| Dependency declaration hidden in a helper or conditional | Reject during discovery without executing it. |
| Local multi-file build helper | Checked in the build context with exact Build vocabulary and normal visibility. |
| Product-only entry/provider selected from a build with no host import of that implementation | Exact designated product operand resolves; no target code executes on the host. |
| Same alias names different host and product packages | Ordinary calls select only the host package; product operands select only the product package. No fallback. |
| User-defined lookalike of a product-selection operation | Ordinary build resolution; no special product authority. |
| Helper queries a caller-private product member or uses a description as a host callable | Reject; an explicitly delegated restricted entry reference may only be used for its permitted binding. |
| Build queries its own generated entry or final component | Reject the phase cycle; use a separately staged build. |
| Application or generated source imports a build-only dependency | Reject, even if build evaluation already loaded its source. |
| One source package used for host tools and a different product target | Shared acquisition permitted; checked contexts and outputs remain distinct. |
| Changed build-tool revision or input | Build provenance and affected results change; no stale output reuse. |
| Dependency generator reads its template and writes generated code | Succeeds using snapshot/staging capabilities without live-host access. |
| Same snapshot with different host file order, mtime, uid, or checkout location | Same logical observations; unsupported metadata is not fabricated. |
| Missing input, negative lookup, or changed directory membership | Accounted in the input commitment; no stale watch-list-based reuse. |
| Reads home/consumer secrets, follows an escaping link, fabricates a handle | Explicit refusal before unauthorized host access. |
| Helper is deliberately given a wider input | May read that input; no false confidentiality claim. |
| Runtime filesystem/network provider selected for a build helper | No bypass of build-facet admission or automatic host grant. |
| Partial output, ignored error, unfinished required output | No new successful product publication. |
| Failed completion followed by explicit successful retry | Custody is returned on the failed attempt; one valid completion may succeed. |
| Duplicate completion, consumed obligation reuse, or previous activation's receipt | Reject; cannot substitute equal bytes for activation custody. |
| One output complete and another failed; or later generated source is invalid | No new committed successful product set. |
| Conditional branch never registers an invocation-required output | Final result rejects the missing requested member. |
| Artifact-only build versus executable missing an entry | Only the explicit artifact-only intent may omit executable roots. |
| Validly published file contains a forged topology verdict | Package verifier/installer rejects; publication is not semantic approval. |
| Sealed or completed output has a surviving attempted writer | Reject the mutation; the completion receipt keeps the same immutable bytes. |

Run actual acquisition, multi-file build execution, and generated-source checks,
not just dependency-parser tests. Exercise snapshot/staging isolation and failure
paths on Windows and macOS; source inspection is not a host execution pass.
Compare two packages using the same input and output names to detect cross-build
leakage, and test same-package dual-purpose cache entries with different targets.

The proposed first slice uses the declaration names above, explicit purpose
migration, product-reference roles, snapshot/staging operations, completion state
table, and statically discovered artifact-only mode. Ratification must accept or
revise those choices rather than delegate their observable failure behavior to
implementation. Numeric wire tables and exact source signatures for the chosen
operations belong in the existing build/protocol owners before implementation
can claim compatibility; the semantic outcomes and authority rules are fixed by
this proposed contract.

Demonstrate a generator reading a template through a narrowed snapshot and
completing a required file, a multi-file helper taking a restricted product entry
reference, and the topology composition build receiving prebuilt components.
Measure retained input/output state and compare the added compiler mechanisms
with a separate build-tool-package route. These prototypes validate usability and
enforcement, not an implementation status inferred from the proposal's length.
They may expose a new design choice; document it explicitly before promotion.

Live-host grant extension, persistent writable caches, symbolic generated-root
binding, concurrent filesystem operations, dynamic dependency discovery, and
native plugin execution are deferred. This leaves code generation and topology
composition usable without requiring a virtual operating system or action DSL.

## Comparison and mechanism choice

| Existing idea | Useful lesson | Omega-specific decision |
| --- | --- | --- |
| [Cargo build dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#build-dependencies) | Build and product dependency sets are separate; build dependencies follow the host. | Retain the purpose split, but enforce it through exact checked contexts and admitted execution. |
| [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) | Host configuration differs from target configuration; scripts generate files before product compilation. `OUT_DIR` may persist and change detection can use declared watched paths. | Keep `Build.target` distinct from execution profile, use fresh private staging and conservative input commitments, and do not treat an output-directory convention as a sandbox. |
| [Bazel rules/actions](https://bazel.build/extending/rules) and [execution platforms](https://bazel.build/extending/platforms) | Tools and targets can use distinct configurations; explicit inputs/outputs and dependency DAGs make scheduling checkable. | Use typed dependency-purpose edges and explicit output custody without adding a second rule language or arbitrary action/plugin graph to the first slice. |
| [Nix derivations](https://nix.dev/manual/nix/stable/store/derivation/) and [sandboxing](https://nix.dev/manual/nix/stable/command-ref/conf-file.html#conf-sandbox) | Builders consume explicit input closures and produce named outputs; isolation prevents accidental host dependencies. Sandbox settings and platform behavior matter. | Specify logical snapshot observations and fail closed if the executor cannot provide them. No implicit sandbox fallback or fixed-output network exception is authorized here. |

These comparisons describe the cited mechanisms, not equivalent security claims
or performance results. Cargo's dependency split alone supplies no confinement;
Nix store identity alone supplies no Omega proof; a Bazel provider is not an Omega
authority receipt. Reuse the architectural separation, not another system's
trusted assumptions.

Direct compiler support is justified for dependency scope, product-reference
identity, root capability issuance, observation enforcement, interrupted-build
failure, and result publication. The compiler/executor already owns those
boundaries, and an ordinary library cannot enforce them against itself. Use
ordinary qualified values, loans, checked results, and bounded storage around
that boundary. File transforms, graph policies, codecs, and artifact semantic
verification remain package code. Generic mechanisms should remove duplicated
ownership, not hide a small compiler operation behind an unneeded plugin system.

One Build argument with scoped operations is the baseline: two arguments do not
by themselves enforce dependency or authority separation. A separate build-tool
package per project remains an alternative if it reduces context complexity, at
the cost of another entry/configuration and explicit product-input handoff.
Pure byte-input/byte-output generators are a narrower viable first customer;
they need not wait for every virtual filesystem operation. No-op I/O and relying
on linker stripping to enforce build-only access are rejected alternatives.

Acceptance updates the owning specs and implementation tasks. Neither this
proposal nor 0004 is an execution-board prerequisite before that decision.