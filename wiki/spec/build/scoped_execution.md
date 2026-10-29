# Build staging, dependency scopes, and capabilities

Named decision: `staged-build-capability-attenuation`, settled 2026-10-28.

This contract defines ordinary Omega build evaluation's checked stages,
dependency scopes, delegated execution capabilities and product handoff. It does not
introduce a second build language. These rules are normative; implementation
coverage is tracked in [the execution board](../../../TASKS.md).
Existing scoped filesystem enforcement is not evidence that the full contract
is implemented. API role names and schematic examples below do not establish
shipped signatures or wire compatibility.

## Contract summary

| Question | Required behavior |
| --- | --- |
| One builder or two? | One `Build`; scope and authority are enforced at individual operations. |
| Build-only code? | Atomic `Build::build_dependency` registration and explicit preparation, then checked dependent continuations; ordinary logical `use` and separate host/product scopes. |
| Reference target code from a host build? | Existing designated product operands plus qualified, non-callable product descriptions; no general host import of target code. |
| Filesystem? | Ordinary filesystem contracts with explicitly supplied backing; parents may delegate read-only, subtree, snapshot or copy-on-write views. |
| Extra host access? | Actual invocation grants and independently accepted authority, never a build-only input registry or an import side effect. |
| Build failure? | Checked local errors, linear required outputs, sticky explicit failure, and no final result set after interruption or failed checks. |
| Artifact-only application? | One interpreted Build request, at least one completed artifact, and no executable root/provider selections. |
| Generated entry or inspection of this build's final executable? | Separate staged compilation; no cycle through this invocation's generated source. |

Detailed rules below define these requirements. The comparison section
explains why the compiler owns the boundaries but not application-specific work.

## Problem and scope

A build should be able to import a third-party code generator or topology checker
without granting application source access to that package, requiring it to run
on the application's target, or giving it ambient access to the developer's
machine. The helper should read explicit inputs, compute using ordinary Omega,
and return data or write private staged artifacts.

Runtime reachability is not a substitute for a checked dependency boundary.
A build-only library must not become product-nameable merely because acquisition
loaded its source. A scoped real-host filesystem is not an isolated virtual input
snapshot; transitional implementations must distinguish those modes.

Use one ordinary Build activation with:

1. Explicit interpreted build-only dependency requests and separate selection contexts.
2. Explicit filesystem/provider arguments under ordinary generic or dynamic
  selection. Being a root grants no authority; the invocation supplies backing.
3. Explicit product-description selection without executing target code.
4. Generic required output publication, including artifact-only builds, without
   compiler knowledge of a package's artifact format or policy algorithms.

The customers are code generation and
[the topology package](../packages/topology.md). Neither needs
topology syntax, a plugin registry, native execution of arbitrary downloaded
code, or compiler-specific graph policies. Reflection is optional for authoring
helpers, not a dependency of this contract.

## Existing owners

Extend these contracts rather than introduce parallel acquisition, evaluation,
or evidence systems:

- [Build declarations](declarations.md): root description, package identity,
  interpreted dependency requests, and static provider-slot readiness.
- [Build execution](execution.md): admission before effects,
  generated-source strata, dependency build outputs, and observation custody.
- [Modules](../language/modules.md) and
  [package boundaries](../packages/boundaries.md): authored name selection,
  public visibility, and carrying foreign types.
- [Configuration](configuration.md): exact-target evaluation and
  result identity.
- [Package acceptance](../packages/acceptance.md): independently accepted
  policy, immutable dependencies, and separation from resolver authority.
- [Component publication](component_publication.md): complete
  component interfaces and installation requirements. This contract supplies
  no replacement topology or deployment semantics.

## Dependency declarations and binding

`Build::dependency` registers a product dependency; `Build::build_dependency`
registers a dependency for interpreted Build code. Both consume a configured
dependency description and return its exact registration. The description may
be prepared by ordinary library helpers, including `dependency::with_alias`;
registration atomically fixes source selection, purpose, alias and delegated
execution context before coordination may schedule work. There is no mutable
half-registered edge, post-registration alias setter or spelling-based retarget.
Descriptions and registration receipts grant no host or package authority by
construction alone. The readable dependency names replace `depend`, `depend_as`,
`build_depend` and `build_depend_as`; separate alias-registration variants are
unnecessary when the unregistered description is configurable.

Aliases are unique in their requesting package and purpose scope. A duplicate
or conflicting registration rejects rather than choosing an order-dependent
winner. The same spelling may name different packages in Build and Product
scopes without cross-context fallback. A conditional registration becomes
available only along paths on which it executed and was prepared.

`use topology;` selects a logical package/module name; it never acquires source,
executes a dependency, mounts a directory or grants visibility beyond the exact
authorized dependency. Build imports may remain unresolved until the checked
continuation that needs them. Unused imports normally produce a warning, with
no acquisition or execution; types, contracts and proof uses count as uses.
An explicitly executed dependency registration still has its ordinary scheduling
effects even if no imported declaration is ultimately referenced.

`Build::prepare_dependency` is the readiness boundary for an exact registered
dependency. It admits and prepares that dependency for the requested execution
profile under its delegated context. It may suspend; it does not rerun earlier
Build statements. It is not a request for arbitrary native plugin execution.
After preparation, the compiler resolves and checks the dependent continuation
before it executes. Exact trait/provider installation still obeys
[provider readiness](provider_selection.md#build-provider-readiness); a newly
admitted dependency can supply ordinary shared non-core interfaces, without
copying declarations or structurally identifying same-spelled traits.

The following illustrates the selected surface, not a current executable fixture.
`dependency::from_source` and configuration helpers are ordinary library code;
source selection is resolved through the invocation's authorized acquisition
route or supplied filesystem backing, never the compiler's ambient filesystem:

```omega
use topology;

machine build(builder: &mut Build)
suspends
{
  Build::application(builder, "payments");
  let mut request = dependency::from_source(Source::Path {
    location: "../topology"
  });
  dependency::with_alias(&mut request, "topology");
  let registered = Build::build_dependency(builder, request);

  suspend Build::prepare_dependency(builder, registered);
  topology::configure_products(builder);
}
```

Ordinary generator calls should instead receive the particular bytes, filesystem
or output writer they need. Giving a helper the root builder is deliberate
delegation of its operations, not a consequence of importing that helper.
Filesystem backing is explicit data, with an ordinary selected implementation;
no parameter named State or implicit receiver supplies hidden authority.

## Checked Build stages

Parse syntax and establish local lexical structure before execution. Execute
only a completely checked and admitted prefix. A dependency's unresolved public
types, contracts or implementation identities stop checking of the dependent
continuation at an explicit preparation boundary; they do not justify executing
an unresolved call. The continuation's operands, types and saved storage must
already have valid checked representations when they cross that boundary.

Preparation extends the authorized environment of the subsequent continuation,
not the meaning of earlier code. Capture exact declarations, substitutions,
source scope, aliases, facts, loans and resource custody across the suspension.
No later discovery may reinterpret an earlier value, prove an earlier unsafe
operation retroactively, replace an alias used by checked code, or execute a
previously unchecked initializer. Cycles that require the missing dependency to
decide how to acquire or prepare itself reject with their phase dependency.

All syntax and already-resolvable obligations remain checked independently of
whether execution selects a branch. A deferred branch can remain unresolved if
not selected, but no claim of complete checking or readiness is made for it.
On selection, resolve and check its complete executable continuation before any
effect in that continuation. Unknown imports and invalid remote obligations
cannot be silently treated as valid or optimized away. Once a dependency is
admitted, its supplied ordinary declarations retain their normal source checking
requirements, including unselected target implementations.

At a join, dependency availability must hold on every incoming path where it is
used; otherwise an explicit checked preparation is required before that use.
Branch decisions, staged source and context identities remain part of replay
and result applicability. Later-stage failure cannot publish a successful
Build result, and real effects performed by earlier admitted stages have no
automatic rollback. This is checked staged execution, not a fully checked whole
Build body before its first effect or a transactional host process.

Only ordinary Build continuations are staged this way. Generated product bytes
join later product checking; they cannot redefine the currently executing Build
or introduce new helpers into an already checked prefix. The mandatory final
join covers all requests actually executed and the admitted executable closure,
without asserting that unexecuted unresolved continuations were checked.

The build can be split across ordinary imported source files. Only the selected
root declares its role and dependency sets; imported helpers do not become build
roots or acquire declaration authority. Importing another package's build entry
remains forbidden. Share ordinary public library code, not another activation's
root or builder state.

## Two checked contexts

| Occurrence | Selectable dependency code | Execution context |
| --- | --- | --- |
| Build entry and transitive helpers | Its package-local build sources, core, and exact static provider slots installed through `Build::await_provider<R, Backing>` | Admitted build evaluator on the host |
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

Here host means the admitted build execution profile, not a value
inferred from the compiler process's OS. The product's targets are the
compilation's realization set; the Build observes none of them
([multi-target compilation](configuration.md#multi-target-compilation)). Build helper code and
its own ordinary dependencies are checked for the execution profile. If a build
helper needs its own build invocation, that invocation produces the helper for
this execution profile; it does not receive the consuming application's target
unless passed explicitly as input. Thus a macOS code generator for a Windows
application must not select Windows filesystem implementations for itself.

Model scheduling nodes as `(package instance, purpose, execution profile,
requested product target, delegated execution context)`, omitting only
coordinates that do not apply. The context includes exact selected provider
implementations, granted backing/views, admitted source and filesystem
observations, configuration and grant policy. Mutable object addresses and
alias spellings are not durable context identities. Distinct delegated contexts
may execute the same package separately; they do not have to agree on a global
input map or share the union of their grants. A
product node waits for its build output and product dependencies; a build node
waits for its host helper libraries and captured input artifacts. A helper library
uses its ordinary dependency edges in that host context, not its own build-only
edges as public library imports. Reject cycles in the known prerequisite graph
before running an affected node. Because requests are ordinary execution
effects, package coordination may observe a new edge after an activation starts;
it must reject any newly closed cycle before installing a binding and again at
the mandatory final join. Build code cannot resolve a cycle through a static
alias lookup, an implicit compiler call, or the current product's unfinished
output.

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

This replaces the legacy one-set nameability rule; it is not a claim that current
dead-code elimination already enforces separate contexts. Migration must classify
existing edges as product, build, or both, with diagnostics for missing edges.
Do not silently infer broader permission from an old lock entry. A versioned
lock/review migration retains acquisitions while rechecking role-specific edges.

During migration, old `depend` spellings remain product-only and require authors
to add the build edge explicitly where needed. They receive deprecation
diagnostics and are removed after the source migration, not retained as another
accepted dialect. Do not infer an edge from loaded
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

Toolchain operations have designated product operands:

```omega
Build::bind(builder, windows_x86_64::ProgramEntry, Application::start);
Build::bind<WindowsAudioPlaybackIo::c_play_sound>(builder, windows_x86_64, native_locator);
Build::select_provider<windows_x86_64::Console, TestConsole>(builder);
Build::assume_binding_contract<WritableMemory::claim, RangeSupplier::supply>(builder);
_ = builder.tests.group<Tests::memory>();
```

Only those declaration-reference operands are resolved in the product context,
using the occurrence's package authority, exact target, and complete expected
slot/requirement signature. The `bind`, `select_provider` and
`assume_binding_contract` operations themselves run
as admitted Build operations. No product receiver, function body, initializer,
or provider is executed. A user-defined same-named method or generic helper gets
no special resolution. This extends the existing "selection is not invocation"
rule, not general syntax quotation or an exception to ordinary call checking.

The native-binding overload's requirement argument is an exact designated
product reference; its target and locator are ordinary checked Build data.
It records package-local intent for resolution against the completed checked
product, not an ordinary import or a call into product source. Literal locator
byte lengths are inferred. Product binding never installs a provider in the
running Build activation. This accepted form replaces source `via` and
per-symbol forwarding declarations; its implementation and migration remain
`BUILD-OWNED-NATIVE-BINDINGS` work under
[foreign bindings](foreign_bindings.md#build-owned-native-binding).

The assumed-binding operation registers a root-authorized runtime relationship
only. Neither its implementation nor its admitted facts become build-executable;
the [runtime-only prohibition](assumed_binding_contracts.md#runtime-only) applies
independently of product-reference resolution.

[Test-group registration](testing.md#registration-and-discovery) selects one exact
product requirement; its group-scoped `service` operands select ordinary
product service/provider declarations under the same rules. Build execution
does not invoke the selected tests. The later runner discovers checked local
satisfiers, admits their separate execution authority, and invokes them.
This adds no general private-symbol enumeration API to build helpers.

For reusable build helpers, expose owned typed descriptions through the existing
Build facet: `ProductEntryRef`, `ProductProviderRef`, and `ProductTypeSchema` are
API vocabulary for distinct roles, not a single untyped compiler handle. A
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

Build code uses ordinary filesystem traits with explicit backing arguments and
ordinary selected named implementations or `dyn` dispatch. A type parameter
such as FilesystemBacking names the backing data, not a magic State parameter
or implicit service. The dependency calls the filesystem contract without
needing to know whether its backing is a host directory, a restricted view or a virtual
tree. A parent choosing and inspecting a virtual backing may name its library
implementation explicitly. Dynamic dispatch is optional, not a condition of
sandbox transparency.

Every activation receives only its actual invocation/parent grants, explicit
bounded diagnostics and evaluation resources. The safe default can be a
read-only view of its exact package snapshot plus private bounded writable
storage, constructed through the same filesystem interfaces. A root may receive
broad host backing only when the invocation actually grants it. Root position,
a package import, a lock acceptance row or a filesystem trait name supplies
none. Descendants do not inherit home directories, consumer source trees,
credentials, resolver access, network or unrestricted host filesystem access.

The public library surface covers file reads/writes and streaming operations,
directory listing/metadata and namespace operations; restricted subtrees,
read-only views and snapshots; private or copy-on-write overlays and explicit
mounts; and change inspection plus checked application/export of changes.
The selected filesystem contract fixes supported behavior. An unavailable
operation reports Unsupported rather than falling back to another provider.
These are ordinary library responsibilities, not compiler-injected input and
output object categories or another filesystem instruction language.

The parent attenuates and supplies backing before the child's execution is
admitted. A restricted provider may retain the same declared theoretical reach
as a real one; that declaration does not create physical backing or host
authority. Admission covers the entire executable closure, so a child cannot
escape by importing another filesystem package, selecting an unprovided raw
host implementation, fabricating a handle or obtaining the parent's builder.
The same principle applies to future network or other service contracts; this
decision neither introduces those APIs nor grants them by analogy.

"Virtual filesystem" specifies observable isolation, not mandatory RAM storage.
The implementation may use an in-memory tree or confined disk backing, provided
paths, metadata, reads, writes, and failure behavior have the same contract.
Unsupported access returns a checked error; a no-op filesystem must not report
successful writes or fabricate empty files. Path traversal, implicit symlink following,
host-handle fabrication, and concurrent host-path substitution must not escape
the delegated views. Disk backing needs a race-safe implementation or an
explicitly adequate isolation premise, not canonicalization alone.

Package input membership comes from the exact captured source inventory; no
implicit reads of parent directories or mutable host files are permitted. A
secret included in that inventory is readable input, not protected by its name.
Extra files are captured by the caller/sponsor before being exposed as immutable
inputs. Bind their exact bytes and admitted metadata for this activation;
reopening a changed host pathname cannot supply the same immutable input.

Code generators normally consume explicit bytes/views or a filesystem and a
narrow writer through ordinary function arguments. They do not configure their
package's Build activation through a consumer-specific hidden input registry.
Compiler support retains dependency admission, source ownership, checked
stages, resource accounting and publication; filesystem algorithms and
attenuation/composition belong to ordinary libraries. Current facet-based
enforcement is migration debt, not proof that every existing std filesystem
operation already executes under these contracts.

Copy-on-write overlays expose ordinary read-your-own-writes filesystem behavior
without changing their base. A changeset names additions, modifications and
deletions relative to its exact base/view. Applying it is a separate operation
under the destination's authority, with path containment, permissions and
explicit base/conflict checks; it is not an implicit host commit. Publishing
generated source/artifacts and exporting files are separate actions. Failure
can discard an unpublished overlay, not undo already performed host effects.

The following protocol is the deterministic snapshot/staging library profile,
not a requirement that every filesystem impersonate that profile or expose
Build-specific handles. Other selected backings report their actual supported
contract and observations.

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
operation and logical path. These are required semantic cases, not assigned wire
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
the entire working directory. Source inventory retains the
[project-control exclusions](../packages/locks.md#source-and-control-state);
acceptance state is loaded independently, not obtained by a build's source read.
A user-selected subtree includes its admitted contents,
including any secrets there: ignore conventions are not a security boundary.
Already-required source files must be included. Other assets are listed by exact
path or explicit subtree, not by running a dependency's scanner with host authority.
Directory membership, missing declared inputs, and link bytes are committed as
well as file content. A required capture entry that is absent fails capture;
an explicitly optional entry records absence in the inventory. Capture must
return a coherent immutable tree or fail; files changing while being copied
cannot produce a falsely attributed snapshot. Tools may construct a capture
request under user authority, but downloaded build code cannot widen it.

### Delegated files and configuration

Extra files or configuration are passed as ordinary values or explicit
filesystem capabilities from an authorized caller. They do not require a second
public BuildInputs/BuildSource registry. Capture and exact inventory retention
remain internal owners of reproducible filesystem observations, not authoring
APIs that turn strings into host access. A missing file or denied operation has
the selected filesystem's checked result; no fallback grants a broader root.

## Execution contexts and reuse

Two consumers can prepare the same source package under different backing,
provider selections or configuration. They receive distinct execution contexts;
neither must adopt the other's input map, and coordination never unions their
authority. Source acquisition and compiled code may be shared only where their
actual checked identities permit it. Build results, generated code and effects
are not shared merely because package bytes or aliases match. Joining produced
variants still requires compatible exact product identities; separate execution
does not authorize merging contradictory generated declarations.

Before reusing an execution result, prove applicability to the exact code,
purpose, execution profile, configuration, selected providers, actual backing
grants, source inventory, filesystem observations and accepted policy. The
conservative implementation commits the complete delegated immutable view and
logical metadata profile, including negative lookups and directory membership.
Content-identical immutable snapshots may share bytes without sharing grants,
writer custody or live activation identities. A library-supplied changeset or
hash alone is not an authority or replay receipt. Product reuse is not evidence
that the Build ran again and cannot silently omit required observable effects.
Materializing cached files or applying retained changes still requires the
destination's actual authority and the selected conflict/failure contract.

Mutable/live backing is not an immutable cache input. Capture an adequate
immutable observation closure under its actual authority, or decline persistent
result reuse when applicability cannot be established. Resource or host failures
cannot be relabeled as stable NotFound/EOF observations. No persistent scratch
cache becomes implicitly readable. Different contexts are ordinary supported
occurrences, not a conflict that must always be detected before the package's
first-ever activation. Each occurrence and each newly reached continuation is
admitted before its own effects; earlier authorized execution has no rollback.

Additional real-host access is opt-in under independently accepted consumer and
executor policy. Requests by downloaded code cannot grant themselves authority.
Install/update must expose direct and transitive restricted build requests before
executing them. The consuming project's `omega.lock` retains accepted request
meaning under [restricted-build acceptance](../packages/acceptance.md#restricted-build-acceptance);
the invocation supplies actual grants separately. No dependency's own lock,
runtime acceptance, or ordinary resolution refresh substitutes for that decision.
Specify operation, root/object scope, lifetime, and bounds;
do not use an undifferentiated "trust this package" flag. A host
grant does not propagate automatically to dependency builds or sibling helpers.
Prefer capturing another input over lending live filesystem authority. Network
and subprocess facilities, if later justified, need their own admitted protocols;
a filesystem grant or std import provides neither. Resolver credentials and
package retrieval remain outside build code's authority.

The default portable profile prefers captured read-only views. Separately
admitted live backing uses the same ordinary service abstraction while retaining
its real observations, external effects and actual scoped authority. It must not
claim snapshot isolation or enter snapshot-result reuse by a mode flag. Library
provider selection cannot approve its own host grant. Network and process APIs
remain separately specified capabilities, not consequences of filesystem access.

Calling a library within the root build does not create a new sandbox. It can
use capabilities passed by its caller. Passing the root builder may permit all
of that builder's operations; use narrower values when that is not intended.
Reading permitted secrets and writing them to permitted logs/artifacts is still
possible. This contract provides confinement and explicit delegation, not
information-flow security or protection after an intentionally broad grant.

## Staged products and failure

Retain the existing generated-source rule: included bytes join a later product
checking stratum and cannot redefine the already admitted build or introduce
new build helpers for that invocation. Dependency-generated source enters only
through the corresponding role/target-bound handoff, never a mutable output path.

Add a compiler-issued linear `RequiredOutput` under the owning Build authority.
`Build::require_artifact(builder, name)` reserves one logical output name and returns its
activation-bound obligation, or a checked error. Duplicate names and file/directory
prefix collisions reject; no last writer wins. Names use the same logical path
rules as staging. Registration may depend on admitted build computation, as may
atomic dependency requests. A caller needing a fixed output roster supplies
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
activation, or a previous run's receipt. Ordinary filesystem operations supply
immutable captured bytes; publication does not require the retired BuildOutput
writer category. A retained immutable blob may back a new staged file, but
requires a new completion under the current obligation. Completion
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
`Build::artifact_only(builder)` once during Build execution. It is valid
for an application and invalid for a workspace/package. No fourth project role
is added. Executable is the application default. Artifact-only mode rejects
executable root/provider selections rather than silently ignoring them and must
complete at least one required artifact. Ordinary packages can publish companion
files without this application-mode declaration.

Artifact-only builds retain an explicit target/evaluation context from their
invocation; the mode does not imply cross-target equivalence or a new target
profile. A generator cannot inspect a selected realization target: generated
source contains ordinary named implementations for the supplied targets, and
the evaluated Build records target-keyed provider rows. It emits neither
target-prefixed declarations nor a different file selected per realized target.
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

Build execution may inspect explicitly supplied
[verified component descriptions](component_publication.md#verified-component-descriptions)
without executing those components. Use the existing component and
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
checked initial Build stage and admission
  -> one resumable activation with atomic dependency registration
  -> dependency preparation, then checked/admitted continuations without replay
  -> final dependency join and frozen product-selection frontier
  -> generated-source incorporation and final target/component checks
  -> required-output completion check and committed result set
```

Previously completed independent components may feed this sequence as explicit
immutable input artifacts. Inspecting this invocation's final component during
its own build is a cycle. A topology project that inspects freshly built services
therefore uses a separate composition build after those services, not a hidden
post-compilation callback in their build entry. A build cannot rewrite its own
dependency or input frontier by serializing a new manifest as output.

## Implementation acceptance

| Case | Required result |
| --- | --- |
| Build dependency declared but never used by Build code | Request may complete without suspending; Build return still joins and records it. |
| Provider readiness requested before dependency work completes | Suspend at `Build::await_provider<R, Backing>`, install only the explicitly named admitted backing into the exact static slot, and resume the same continuation without replay. |
| Dependency declaration in a local helper or conditional | Execute normally; only the requests actually emitted belong to this Build result. |
| Deferred import of a newly registered Build dependency | Remains inert until preparation; check the dependent continuation before any use executes. |
| Unused import without a dependency request | Normally warn; acquire and execute nothing. |
| Alias mutation, duplicate registration or a later request used to justify earlier code | Reject without retargeting any previously checked declaration. |
| Branch-local dependency used at a join | Require preparation on every incoming use path or an explicit checked preparation before the use. |
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
| Alternative filesystem/network provider selected for a build helper | Actual backing and complete closure admission remain required; no host escape or automatic grant. |
| Two consumers prepare the same package with different backing views | Distinct execution contexts; no grant union or reuse of an inapplicable result. |
| Cow overlay changed files inspected or exported | Preserve exact base/view and destination authority; no implicit commit to parent or host. |
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

## Migration and retirement

`STAGED-BUILD-CAPABILITIES` in [TASKS.md](../../../TASKS.md) owns the complete
ratified migration. Deprecate the old authoring routes with actionable
diagnostics, migrate their customers and both producers, then delete them in
that outcome. There is no permanent legacy mode or parallel filesystem API.

The retired surface includes old dependency names and post-registration alias
mutation, unconditional pre-known-only Build import checking, BuildInputs,
BuildSource and BuildOutput filesystem operations, named per-dependency input
maps including the unimplemented with_dependency_inputs proposal, and the
global incoming-map agreement rule. Compiler-private source capture, exact
provider installation, linear artifact publication, accounting and authority
checks remain where required; changing names must not remove those guarantees.

The accepted public journey is ordinary inert imports, configured descriptions,
atomic registration, explicit preparation, checked continuation execution,
ordinary filesystem capabilities and explicit artifact/source handoff. Exact
library helper signatures and protocol encoding are implementation work, not
permission to invent additional authoring concepts. Change retained schema and
review identities where context or stage meaning changes; reject or explicitly
migrate old records rather than reinterpret their bytes. Current fixtures using
old facets do not prove the new surface implemented.

Demonstrate a generator reading a template through a narrowed snapshot and
completing a required file, a multi-file helper taking a restricted product entry
reference, and the topology composition build receiving prebuilt components.
Measure retained input/output state and compare the added compiler mechanisms
with a separate build-tool-package route. These prototypes validate usability and
enforcement, not implementation status inferred from the contract's detail.
They may expose a new design choice; raise any semantic or trust amendment in
[owner questions](../../../OWNER_QUESTIONS.md) before relying on it. These are
implementation acceptance gates, not prerequisites to the design's ratification.

Live-host grant extension, persistent writable caches, symbolic generated-root
binding, concurrent filesystem operations, dependency inference from imports,
and native plugin execution are deferred. This leaves code generation and
topology composition usable without requiring a virtual operating system or
action DSL.

## Comparison and mechanism choice

| Existing idea | Useful lesson | Omega-specific decision |
| --- | --- | --- |
| [Cargo build dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#build-dependencies) | Build and product dependency sets are separate; build dependencies follow the host. | Retain the purpose split, but enforce it through exact checked contexts and admitted execution. |
| [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) | Host configuration differs from target configuration; scripts generate files before product compilation. `OUT_DIR` may persist and change detection can use declared watched paths. | Keep the realization targets distinct from execution profile and out of the Build, use fresh private staging and conservative input commitments, and do not treat an output-directory convention as a sandbox. |
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

Implementation work follows [the execution board](../../../TASKS.md). Future
amendments, whether found during implementation or later review, use
[owner questions](../../../OWNER_QUESTIONS.md); unsupported behavior must not
silently weaken this contract.
