# Rust Omega compiler on-ramp

This directory contains the current Rust implementation of the Omega compiler.
It is a migration/reference producer, not the canonical source of the eventual
self-hosted compiler and not a language rung.

- `psi/` implements source processing, language judgments, and terminal Psi.
- `omega/` consumes terminal Psi and performs target lowering and artifact
  emission.
- `apps/omega-cli/` exposes the current Rust implementation as the `omega`
  development command.

The crates remain the working development compiler while the Delta-built Omega
path grows. Their outputs gain authority only through the lattice's meaning,
refinement, and artifact checks.

The eventual Omega-written implementations belong under `compiler/psi/` and
`compiler/omega/`. Once the hosted path replaces this producer, the entire
on-ramp can be omitted.
