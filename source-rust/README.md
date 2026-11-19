# Rust Omega compiler

This directory holds the Rust implementation of the Omega compiler. It is a
reference compiler for the migration, not the source of the eventual
self-hosted compiler and not a bootstrap language rung.

- `application/` is the entry point for the `omega` command.
- `subsystems/` contains the compiler, interpreter, and package manager.
- `foundation/` provides arenas, exact arithmetic, record storage, byte encoding, and [host filesystem operations](foundation/filesystem/src/lib.rs) across subsystems.

These crates stay the working development compiler and a maintained
comparison compiler while the Epsilon-evaluator bootstrap path grows. Delta implements the Epsilon evaluator; Epsilon is used to
write the first Omega compiler D, which builds the Omega-written product. See
the [bootstrap source map](../bootstrap/README.md). Development builds may use
them, but they grant no trust; outputs earn it only through meaning,
refinement and artifact checks. They are neither a
bootstrap nor a release dependency.

The Omega-written compiler is split across the sibling `source/psi/` and
`source/omega/` packages. A product may omit this Rust compiler even while it
is kept for cross-compiler bug finding.

Omitting it is a property of what was produced, not of this workspace. A
toolchain, release or bootstrap input set omits the Rust compiler when its
closed set of dependencies has no `omega-rust/` artifact, build step or path
taken from this checkout (including the bundled `source/library/` location,
which a self-hosted product must find without this checkout). Admission rules
do not change: artifact bytes, manifest identities and proof sidecars are
checked against their own claims, never against which compiler emitted them.
Keeping or retiring this workspace is a separate decision; omitting it never
means deleting it.

## Implementation entrypoints

- [Pipeline and ownership](pipeline.md): the source-to-Terminal and
  Terminal-to-native route, and where new work goes.
- [Psi](psi/README.md) and its [frontend](psi/compilation/pipeline/README.md): source identity,
  syntax, typing, checking and portable publication.
- [Compiler coordination](omega/compilation/compiler/README.md): product boundaries,
  optional reports and reuse of source across targets.
- [Native realization](omega/compilation/compiler/README.md#native-realization):
  lowering that no longer reads source, with authority supplied separately.
- [Native representations](omega/compilation/representations/README.md) and
  [optimization](optimization.md): current data, exact rewrites and replay.
- [Package management](omega/packages/package-manager/README.md): package
  operations; source acquisition, resolver execution and review evidence are
  separate packages in `omega/packages/`.
- [Build evaluation](omega/build-system/build-settings/README.md): Build
  activation, declaration admission, consent for restricted requests and
  execution, separate from compiler coordination.
- [Psi interpreters](psi/interpreter): the Terminal Psi engine.
  [Build-time evaluation](psi/compilation/build-time-evaluation) runs
  target-neutral code without depending on the Omega compiler coordinator.

Omega has six domains: `application/`, `build-system/`, `compilation/`,
`interpreter/`, `packages/` and `foundation/`; see the [domain map](omega/README.md).
Each crate keeps its own manifest and API. The package manager may use
compiler services, but the compiler never depends on package management;
shared foundations stay separate packages. IRs live in
`psi/compilation/representations/` and `omega/compilation/representations/`,
apart from the stages that produce them. The separate optimization-unit
checker sits beside its IR at
[`omega/compilation/representations/abstract-operations/optimization-unit-semantics/`](omega/compilation/representations/abstract-operations/optimization-unit-semantics),
with no reverse dependency from the representation to its checker.

Run the product through the `omega` package in `omega/application/`;
repository commands and validation policy are in [AGENTS.md](../AGENTS.md#commands).
Programs the Rust tests read live in `../tests/fixtures/`, tests spanning
several crates in `../tests/`, and crate-specific tests beside their crate. A
crate's root file lists its responsibilities; tests and implementation
details go in named modules, not one huge root file.

# Omega domains

Each directory owns one kind of work:

| Directory | Responsibility |
| --- | --- |
| [application/](application/README.md) | The `omega` router: parse a command, hand it to its owner, report the outcome |
| [build-system/](build-system/README.md) | Runs a project: prepares it, executes its `build.omg`, calls the compilers and publishes their outputs |
| [compilation/](compilation/README.md) | The compilers and their machinery: pipeline, representations, realization and backend |
| `interpreter/` | Hosted interpretation: the Terminal runtime that runs programs and the native-provider runtime it hosts |
| [packages/](packages/README.md) | Package operations, acquisition, review and compilation inputs |
| [foundation/bounded-process/](foundation/bounded-process/README.md) | Host child-process execution, output capture, termination and cleanup |
| [foundation/filesystem/](foundation/filesystem/src/lib.rs) | Host file identity, access inspection, scratch directories, locking, and exact-byte publication |

The application is Cargo package `omega` in `application/`; run it from the
workspace root with `mbx run -p omega -- run <root.omg>`.

These directories group independent packages by responsibility. Each package
keeps its own manifest and API; Cargo dependencies enforce the boundaries.
The build system in `build-system/` calls the compilers; compiler machinery,
including its independent IR packages, is in `compilation/`. Hosted program
execution is in `interpreter/`, a library any embedder can use without the
`omega` executable. Generic primitives are in [Psi foundation](../psi/foundation/README.md).
Native identity and optimization records are in `compilation/representations/`;
product measurements and request records are in `compilation/products/`.

Source processing through Terminal Psi is in [Psi](../psi/README.md),
including [target-neutral interpretation](../psi/interpreter/README.md).
The [pipeline contract](../pipeline.md) defines the Psi/Omega firewall.
The bundled libraries are in [`../../source/library/`](../../source/library/);
the Rust tree holds compiler and runtime machinery, not copies of them.
