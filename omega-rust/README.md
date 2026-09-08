# Rust Omega compiler

This directory contains the current Rust implementation of the Omega compiler.
It is a migration/reference producer, not the canonical source of the eventual
self-hosted compiler and not a language rung.

- `psi/` implements source processing, language judgments, and terminal Psi.
- `omega/` consumes terminal Psi and performs target lowering and artifact
  emission.
- `omega/` is the `omega` product package and development command. Its nested
  crates implement target realization, orchestration, and artifact emission.

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

## Implementation entrypoints

- [Pipeline and ownership](pipeline.md): the connected source-to-Terminal and
  Terminal-to-native route, and placement rules for new work.
- [Psi](psi/README.md) and its [frontend](psi/pipeline/README.md): source identity,
  syntax, typing, checking, and portable publication.
- [Compiler coordination](omega/compiler/compiler/README.md): product boundaries,
  optional reports, and multi-target source reuse.
- [Native realization](omega/compiler/native-realization/README.md): source-free
  lowering with separately supplied authority.
- [Native representations](omega/representations/README.md) and
  [optimization](optimization.md): current data, exact rewrites, and replay.
- [Packages](omega/packages/README.md),
  [build evaluation](omega/build/build-evaluation/README.md), and
  [component deployment](omega/build/component-deployment/README.md): their own
  loading, execution, policy, and runtime coordination boundaries.

Run the product through `omega/`; repository commands and validation policy are
in [AGENTS.md](../AGENTS.md#commands). Language examples live in `../tests/omega/`,
cross-owner tests in `../tests/`, and crate-specific tests beside their owner.
Public crate roots map responsibilities; test families and implementation
details belong in named modules, not giant entrypoint files.
