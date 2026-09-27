# Rust Omega compiler

This directory contains the current Rust implementation of the Omega compiler.
It is a migration/reference producer, not the canonical source of the eventual
self-hosted compiler and not a language rung.

- `psi/` implements source processing, language judgments, and terminal Psi.
- `omega/` consumes terminal Psi and performs target lowering and artifact
  emission.
- `omega/omega/` is the `omega` product package and development command. Its
  sibling crates own compiler coordination, package management and realization.
- `library/` contains Rust reference implementations of ordinary Omega library
  contracts; these do not become compiler subsystems.

The crates remain the working development compiler and a maintained parallel
comparator while the selected Epsilon-evaluator bootstrap path grows. Delta
implements the Epsilon evaluator; Epsilon authors the first Omega compiler D,
which builds the Omega-written product. See the [bootstrap source map](../bootstrap/README.md).
Development
builds may use them, but they grant no authority; outputs gain authority only
through meaning, refinement, and artifact checks. They are neither a bootstrap
nor a release dependency.

The Omega-written implementation is split across sibling `source/psi/` and
`source/omega/` packages. This Rust producer may be
omitted even if retained for cross-compiler bug finding.

Omission is a property of a produced closure, not of this workspace: a
toolchain, release, or bootstrap input set omits the Rust producer when its
closed dependency set contains no `omega-rust/` artifact, build step, or
checkout-derived path (including the bundled `source/library/` location, which
a self-hosted product must resolve without this checkout). An omitted
producer's products keep identical admission rules — artifact bytes, manifest
identities, and proof sidecars are checked against their own claims, never
against which implementation emitted them. Whether the workspace itself is
retained as a comparator or retired is a separate policy; omission alone never
implies deletion.

## Implementation entrypoints

- [Pipeline and ownership](pipeline.md): the connected source-to-Terminal and
  Terminal-to-native route, and placement rules for new work.
- [Psi](psi/README.md) and its [frontend](psi/pipeline/README.md): source identity,
  syntax, typing, checking, and portable publication.
- [Compiler coordination](omega/compiler/README.md): product boundaries,
  optional reports, and multi-target source reuse.
- [Native realization](omega/compiler/README.md#native-realization): source-free
  lowering with separately supplied authority.
- [Native representations](omega/representations/README.md) and
  [optimization](optimization.md): current data, exact rewrites, and replay.
- [Package management](omega/package-manager/README.md): package operations,
  with independent source acquisition, resolver execution, and review evidence
  packages beneath the subsystem owner.
- [Build evaluation](omega/compiler/build-evaluation/README.md): one of the
  independent supporting packages beneath compiler coordination, alongside
  declarations, output custody, package inputs, selection, and trust policy.
- [Psi interpreters](psi/interpreter): separate checked-tree and Terminal Psi
  engines. [Build-time evaluation](psi/interpreter/build-time-evaluation) uses
  target-neutral execution without depending on the Omega compiler coordinator.
- [Runtime component installation](omega/backend/runtime/component-deployment/README.md):
  custody composition under backend runtime ownership.
- [Topology library](library/topology/README.md): an ordinary package-defined
  plan, policy, and installation contract, separate from compiler ownership.

Subsystem folders show the operation owner and its supporting crates. Each
supporting crate retains its own manifest and API. The package manager can use
compiler services without the compiler depending back on package management;
shared foundations and utilities remain independently owned. Durable IRs stay
in `representations/`, separate from their producing pipeline stages. The
independent optimization-unit checker sits beside its IR at
[`omega/representations/abstract-operations/optimization-unit/semantics/`](omega/representations/abstract-operations/optimization-unit/semantics),
with no reverse dependency from the representation to its checker.

Run the product through the `omega` package in `omega/omega/`; repository commands and validation policy are
in [AGENTS.md](../AGENTS.md#commands). Language examples live in `../tests/omega/`,
cross-owner tests in `../tests/`, and crate-specific tests beside their owner.
Public crate roots map responsibilities; test families and implementation
details belong in named modules, not giant entrypoint files.
