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
comparator while the direct Delta-produced compiler path grows. Development
builds may use them, but they grant no authority; outputs gain authority only
through meaning, refinement, and artifact checks. They are neither a bootstrap
nor a release dependency.

The eventual Omega-written implementations belong under `source/psi/` and
`source/omega/`. This Rust producer may be omitted even if retained for
cross-compiler bug finding.
