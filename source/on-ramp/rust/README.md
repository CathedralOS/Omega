# Rust Omega compiler on-ramp

This directory contains the current Rust implementation of the Omega compiler.
It is a migration/reference producer, not the canonical source of the eventual
self-hosted compiler and not a language rung.

- `psi/` implements source processing, language judgments, and terminal Psi.
- `omega/` consumes terminal Psi and performs target lowering and artifact
  emission.
- `omega/` is the `omega` product package and development command. Its nested
  crates implement target realization, orchestration, and artifact emission.

The crates remain the working development compiler and a maintained parallel
comparator while the Delta-built `omega-bootstrap` path grows. Migration builds
may use them, but they grant no authority; outputs gain authority only through
the lattice's meaning, refinement, and artifact checks. Once the hosted path
closes, they are neither a bootstrap nor a release dependency.

The eventual Omega-written implementations belong under `source/psi/` and
`source/omega/`. Once the hosted path replaces this producer, the on-ramp can
be omitted from the build even if retained for cross-compiler bug finding.
