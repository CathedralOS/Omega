# Package inputs for programmable Build stages

A project describes its direct dependencies in ordinary Omega source. Running
`omega build build.omg` uses the project's own public entry whether the
project is the root or a dependency. A consumer can build a board package
without knowing which libraries the board uses to configure itself.

This is the proposed replacement for the [version 4 project lock](locks.md).
This page governs the proposed source, update and reach rules. The Rust tools
currently use the lock workflow. The [remaining design decisions](#remaining-design-decisions)
matter before retiring that workflow, particularly for separately admitted
external assumptions, terminal permissions and compiler inputs.

## From source to the next stage

Each stage starts with a frozen source view and caller-supplied package
bindings. A binding names one admitted package instance under an alias for
a particular use. Generated local files travel through the source view.
Externally owned libraries keep their package identity, import scope and
transitive dependencies. Source acquisition and registering an alias are
distinct operations. Supplying either one grants no host services.

A stage that needs a library for its product registers a product dependency.
A stage that needs a library to compile the next Build supplies it before that
Build's entry. The following example shows the proposed library and stage API
spellings. The repository locations and full Git commit IDs are illustrative.
The stock distribution makes its Git request vocabulary available to the
preparing stage at startup.

```omega
// firmware/build.omg
use {omega}/build/dependencies/providers/git.omg as git;

machine build(build: @mutable Build) {
    Build::package(build, "firmware");
    let board: DependencyDescription = |git|::dependency(
        "board", "https://example.org/board.git", "refs/heads/stable",
        "72bc01f59de8a314c60155e8c3b7474b261490fe"
    );
    _ = Build::dependency(build, board);
}
```

```omega
// board/build.omg, from the selected board revision
use {omega}/build/dependencies/providers/git.omg as git;

machine build(build: @mutable Build) {
    let layout: DependencyDescription = |git|::dependency(
        "layout", "https://example.org/layout.git", "refs/heads/stable",
        "8e76d2a18f2ecb674b75670ab380aa1cf32f9d44"
    );
    Build::stage(build, "configure.omg", [layout]);
}
```

```omega
// board/configure.omg
use {layout}/api.omg as layout;

machine build(build: @mutable Build) {
    Build::package(build, "board");
    |layout|::configure_product(build);
}
```

The stage operation requests a separate compilation of the named Build source
with the supplied bindings and uses its resulting plan for this project. It
obeys ordinary dependency join, failure and publication rules. Only the board's
configuration stage sees the alias `layout`. The resulting product can use
dependencies registered in its own plan. Building the board alone starts at
the same board entry and follows the same revision literals. Each root supplies
its own authority; an approval shipped in the board repo cannot approve code
on behalf of firmware.

An intermediate stage explicitly forwards a received binding when a successor
needs that same instance. It need not import a machine from it, fetch it again
or repeat its revision. A dependency that prepares its own children keeps
their declarations in its own source. Bindings and host grants are scoped per
activation. Forwarding files or a library does not forward live networking or
process access.

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

The consumer may bound a dependency use with `max_reach`, a set written as an
array of reflected boundary-service identities. For example, a CRC library can
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

For example, the intended registration shape for a service-free library is
`|dependency|::add(build, checksum, max_reach: [])` (schematic spelling).
The array carries exact reflected service identities and uses set semantics.
Admission checks that the transitive reach attributable to this use is a
subset of the normalized allowed set. Updating a source literal leaves that
array as the consuming author wrote it. A full report may still highlight
custom critical services even when they fit the edge ceiling.

An update may introduce new behavior within an existing ceiling. The check
answers whether the candidate fits the current contract. A tool claiming a
historical change needs a comparable old result or sufficient old inputs.

The `Unsafe` family is denied by default in each package and on every
dependency edge. Each consuming Build must name the specific atoms. For
example, admitting `BorrowMutabilityUpgrade` permits an upgrade through an
existing borrow. Switching the dependency to `borrow_mutable_at` requires
`RawMemoryRead` and `RawMemoryWrite`. The compiler diagnoses a missing
admission even when the call is transitive. A reachable foreign call also
reports `Unsafe` through `ForeignCodeExecution`.
[Operations, admission and diagnostics](../resources/native_borrows.md)

## Installing and updating

The bundled workflow supports deterministic edits for conventional Git
declarations. Custom resolvers and arbitrary conditionals remain ordinary
Omega. The compiler imposes no generic update algorithm on them.

| Command in the selected stage | Behavior |
| --- | --- |
| `omega packages install x --git <repository> --track <ref>` | For a new alias, resolve the ref, verify the selected package, then propose the literal declaration, exact commit and registration edit together. |
| `omega packages install x` | Use the existing declaration at its selected revision, fetching verified missing content if permitted. An unknown alias requires a source. |
| `omega packages update x` | Resolve the declared tracking ref. If unchanged, leave the file alone. Otherwise verify the candidate and propose replacing only the commit literal. Without a tracking ref or an explicit new revision there is no inferred update target. |

Installing the same binding is idempotent. An alias already bound to a
different source is a conflict. In a simple unconditional stage, the editor
can insert a declaration beside the registration and append its binding to
the literal input list. The ordinary product registration is also editable.
An edit that would require inventing a new stage, choosing a branch, or tracing
an arbitrary helper stays with the author. Show the source location, a concrete
declaration or replacement and the call that would need wiring. Distinguish a
definitely absent alias from one the editor cannot trace. Leave the source
untouched on failure or if it changed under the proposed edit. A person or an
LLM can apply an ordinary source diff. Building the edited project never
requires an LLM.

The updater can show candidate reach by running the normal compiler checks
on the resolved tree in the consuming context. It can also just edit the pin.
The next check must enforce the same constraints. An update cannot silently
widen `max_reach` or approve host access. If any install, update or preview
executes Build code, admit that code before its first effect. Candidate-only
review requires no saved reach or historical source. If the old baseline is
unavailable, report current findings and mark the delta unknown. Optional caches
never supply approval.

For example, `update layout` in the board preparation stage above edits the
last argument of `|git|::dependency`. The resolver supplies the verified tree;
the compiler supplies the candidate's reach if the caller requests review.
Neither Git-specific syntax nor the updater determines the reach verdict.

## Remaining design decisions

1. **Caller approval and existing compiler inputs.** Which caller-owned inputs
   carry restricted Build approvals, terminal permissions, external assumptions and
   exact compiler/provider binding identities after version 4, including for
   unattended builds? Specify scope, invalidation and migration with consumer
   review. A missing required approval refuses the operation. Retired assumed
   provider contracts have no successor approval. Reject old registrations
   and never translate their receipts into unsafe reach grants.
2. **Stage bindings and forwarding.** Specify the public type for a resolved
   binding, how B refers to an alias supplied by A and forwards it to C, and
   the exact stage entry and completion signature. Forwarding must preserve
   identity, attenuation and the frozen view. Test a staged handoff beside a
   sibling with no binding.
3. **Reach policy defaults and composition.** Empty is explicit. Omission
   denies unsafe reach. Decide what omission means for other reach and how to attribute and
   diagnose a package used on multiple edges with different ceilings. Exercise the same library used as both
   Build code and product code, including an edge reached through a helper.
4. **Choosing an edit site.** A dependency may belong to a product or to an
   explicitly selected successor Build. Settle the CLI spelling for selecting
   that use and stage when the normal project entry is ambiguous. A filename
   alone cannot distinguish multiple occurrences of the same stage.
