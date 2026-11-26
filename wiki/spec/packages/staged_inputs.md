# Package inputs for programmable Build stages

A project describes its direct dependencies in ordinary Omega source. Running
`omega build build.omg` uses the project's own public entry whether the
project is the root or a dependency. A consumer can build a board package
without knowing which libraries the board uses to configure itself.

This page specifies the source-based replacement for the [version 4 project lock](locks.md).
The Rust driver follows authored successor entries and forwards explicitly
registered package bindings. Install and update still use the lock workflow. The [binding and
admission rules](#binding-identity-and-admission) govern the staged workflow.

## Packages available to the initial Build

For `omega build build.omg`, the driver supplies the toolchain's core and
Build libraries by convention. The project declares additional packages in
its checked-in Build source. The CLI has no option to inject a package
binding into the initial Build. An embedding host can supply incoming
bindings directly.

If only the application imports `{system}/console.omg`, one `build.omg`
registers `system` and its allowed reach for the application. If Build code
itself needs to call `Filesystem::read_all` from `system`, the first entry
registers that package for a successor such as `configure.omg`. The successor
imports it when compiled with those bindings. In both cases the user runs
the same command.

## From source to the next stage

Each stage starts with a frozen source view and caller-supplied package
bindings. A binding names one admitted package instance under an alias for
a particular use. Generated local files travel through the source view.
Externally owned libraries keep their package identity, import scope and
transitive dependencies. Source acquisition and registering an alias are
distinct operations. Supplying either one grants no host services.

Every entry describes its immediate product. A package can supply code to an
interpreted Build, a native executable, or both. Each program receives its
own bindings. In this example the board uses a layout library to configure
itself. Building the board alone or as a firmware dependency starts from the
same board entry and uses the same revision.

```omega
// firmware/build.omg
use {omega}/build/dependency.omg;
use {omega}/build/dependencies/providers/git.omg;

machine build(build: @mutable BuildPlan) {
    build.product = BuildProduct::Package { name: "firmware" };
    let board: DependencyDescription = |git|::dependency(
        "board", "https://example.org/board.git", "refs/heads/stable",
        "72bc01f59de8a314c60155e8c3b7474b261490fe"
    );
    _ = |dependency|::add(build, board);
}
```

```omega
// board/build.omg
use {omega}/build/dependency.omg;
use {omega}/build/dependencies/providers/git.omg;

machine build(build: @mutable BuildPlan) {
    let layout: DependencyDescription = |git|::dependency(
        "layout", "https://example.org/layout.git", "refs/heads/stable",
        "8e76d2a18f2ecb674b75670ab380aa1cf32f9d44"
    );
    let service_free: [BoundaryServiceReference; 0] = [];
    _ = |dependency|::add(build, layout, @service_free[..]);
    build.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };
}
```

```omega
// board/configure.omg
use {layout}/api.omg as layout;

machine build(build: @mutable BuildPlan) {
    build.product = BuildProduct::Package { name: "board" };
    |layout|::configure_product(build);
}
```

The driver runs `configure.omg` after the preparing entry returns. Its
`layout` import resolves before entry. If the board's product also imports
that package, the configuration stage explicitly forwards it:

```omega
// In configure.omg, with use {omega}/build/dependency.omg;
let layout_input: PackageBinding = |dependency|::input(build, "layout");
_ = |dependency|::add(build, layout_input);
```

`input` looks up an alias in this invocation's frozen incoming bindings.
A missing alias rejects. The returned binding identifies the exact admitted
package instance and its frozen source and dependency closure. Forwarding
it preserves that identity and attenuation without reacquisition. `add`
registers the package for the immediate product with its own checked edge.
Siblings receive only their own inputs. Provider selections, live network
access, process grants and filesystem backing have their own activation scope.

A simple project needs no preparatory stage. A project can name an ordinary
preparing file `build.lock.omg` and invoke it explicitly. No stage searches
for an adjacent lock or builds through a companion file by convention.
[Fixed Build imports](../build/scoped_execution.md#checked-build-stages) still
apply. Two unchanged compilations of the same file both check its file-scoped
imports. An unresolved import cannot be postponed until a later entry. Use
separate source files or a prepared successor source view.

## Acquisition and authority

The driver consumes frozen package inputs and their identities. Source-specific
resolution belongs in libraries. The bundled Git library resolves references,
verifies the selected commit and tree, and supplies an immutable source view
under the [resolver custody rules](resolver_security.md). The package manager
and Build tooling can share it. The Rust implementation can retain its existing
Git backend during migration.

The conventional Git declaration describes a request. A normal build selects
the literal commit. A commit fixes its verified tree, even across independent
subrepositories. Updating a transitive pin in board source publishes a new
board revision. Firmware can then update its direct board pin. The tracking
reference tells the updater where to look
when asked. A Build author can deliberately select a moving reference or
compute a revision, in which case the build has the corresponding variability.
A local path needs no authored checksum. Snapshotting the actual bytes still
gives the compiler an input identity. Other libraries can use other source
identities. None requires a compiler-wide pin type.

Package-manager acquisition uses the resolver's own authorized resources.
If an authored preparation stage performs a live fetch itself, its checked
reach, actual network backing and grants must permit it. `--allow Network`
raises the root ceiling. A delegated child starts offline by default. A verified
cached tree at the requested commit or a caller-supplied frozen tree can feed
the next stage without live access. Resolver credentials stay with the resolver.
The driver cannot treat an arbitrary directory and a revision label as evidence
that Git verified those bytes.

## Dependency reach

Every dependency use has a ceiling. Omission means `max_reach: []`, including
for ordinary reach atoms. Check each edge independently, including uses of
the same instance in two stages. Report the failing edge, atom and reachable
call path. An unused export contributes no reach. The consumer writes
`max_reach` as an array of reflected boundary-service identities. For example, a CRC library can
be supplied with `max_reach: []`. Its checked use, including transitive calls
through selected contracts, must have empty service reach. A new call from
`crc32` to a telemetry helper reaching `NetworkSend` fails the build and reports
the dependency edge and call path. The checker enforces the ceiling on the
candidate regardless of how the source arrived or which tool last edited it.

Use the same identity, substitution and parent-closure rules as
[ordinary reach](../language/effects.md#declarations). Custom services such
as `MotorControl` and concrete applications such as `Sensor<17>` are eligible.
Registering a service for [critical audit highlighting](review.md#critical-service-audit-policy)
is independent. A permitted reach contributes no capability or provider backing.
A library-use ceiling applies to code admitted for that use. Acquisition, the
dependency's own Build, and its eventual product have distinct execution
contexts. In particular the CRC code can be service-free even when fetching
its source required networking.

The two-argument `|dependency|::add(build, checksum)` registers an empty
ceiling. A third positional argument supplies a shared view of service
references. Both forms accept a dependency description or a binding obtained
from `|dependency|::input`. A forwarded binding's ceiling must fit its incoming
ceiling.

```omega
let allowed: [BoundaryServiceReference; 1] = [Build::service::<Sensor<17u32>>(build)];
_ = |dependency|::add(build, telemetry, @allowed[..]);
```

`Build::service::<S>(build)` reflects the concrete boundary application
selected in the current Build scope. Copies retain the application identity.
The array uses set semantics.
Admission checks that the transitive reach attributable to this use is a
subset of the normalized allowed set. Updating a source literal leaves that
array as the consuming author wrote it. A full report may still highlight
custom critical services even when they fit the edge ceiling.

An update may introduce new behavior within an existing ceiling. The check
answers whether the candidate fits the current contract. A tool claiming a
historical change needs a comparable old result or sufficient old inputs.

The `Unsafe` family is denied by default in each package and on every
dependency edge. Each product, including a consuming product with transitive
reach, must call `Build::enable_unsafe::<Atom>(build)` for each reachable
specific child. Its dependency-edge ceiling independently bounds each use. For
example, admitting `BorrowMutabilityUpgrade` permits an upgrade through an
existing borrow. Switching the dependency to `borrow_mutable_at` requires
`RawMemoryRead` and `RawMemoryWrite`. The compiler diagnoses a missing
admission even when the call is transitive. A reachable foreign call also
reports `Unsafe` through `ForeignCodeExecution`.
[Operations, admission and diagnostics](../resources/native_borrows.md)

## Installing and updating

The source-based workflow ships package-management guidance. `omega packages`
and `omega packages help add` or `help update` explain how a person or an
agent can edit ordinary Build source. The bundled Git library owns resolution
and revision verification. A custom resolver can document its own edits.
A build checks the edited source independently of who wrote it.

For an add, identify the consuming stage, alias and intended use, then add
the dependency and its reach ceiling there. For an update, locate that
declaration, verify the candidate and replace its revision. When control flow
or repeated occurrences make the choice ambiguous, show the candidate sites
and proposed source edit for the author to choose. The version 4 install and
update commands belong to the lock-based workflow.

The audit tool can show candidate reach by running the normal compiler checks
on the resolved tree in the consuming context. The author may also just edit
the pin.
The next check must enforce the same constraints. An update cannot silently
widen `max_reach` or approve host access. If any install, update or preview
executes Build code, admit that code before its first effect. Candidate-only
review requires no saved reach or historical source. If the old baseline is
unavailable, report current findings and mark the delta unknown. Audit the selected occurrence and
target. Flag changes to reachable high-authority code even within its ceiling.
A caller can request a separate full-package review. Optional caches
never supply approval.

For example, updating `layout` in the board preparation stage above changes the
last argument of `|git|::dependency`. The resolver supplies the verified tree;
the compiler supplies the candidate's reach if the caller requests review.
Neither Git-specific syntax nor the updater determines the reach verdict.

## Binding identity and admission

The target profile designates the exact entry-contract package and declaration.
Build binds the product's entry machine to that slot. For provider and native
bindings, Build selects the implementation or native locator for the exact
requirement. A host installing an interpreter service identifies its declaration
and supplies its backing. The compiler checks each selection in its frozen
context and derives its [semantic binding](toolchain.md#semantic-bindings-in-a-staged-build).
An unattended build uses the same inputs. A missing required selection, wrong
owner, incompatible schema or ambiguous resolution fails at the selection.

The invocation and per-child delegation supply restricted Build grants.
Selecting an output authorizes driver publication of that output. Filesystem
operations inside a Build use its admitted backing.

The receiver owns the decision to load native code and any policy for accepting
its physical authority or external premises. Staged builds require no replacement
lock approval for plugins. Loading and entering native code retain the
[Unsafe reach rules](../resources/native_borrows.md#build-admission-and-diagnostics).
A receiver requesting [component verification](../build/component_publication.md#verified-component-descriptions)
must match the selected subject and profile and supply any premises required
for the claim. Ordinary compilation makes no claim about a loader's enforcement.
