# The bootstrap chain

> **Status: selected topology, incomplete compiler edges.**

The trust-minimizing lattice is:

```text
audited Alpha VM + directly audited Beta evaluator tape
  -> Beta-written Gamma compiler -> gamma_compiler_bytecode.tape
  -> Gamma-written Delta compiler -> delta_compiler_bytecode.tape
  -> Delta-written Omega compiler D -> omega0_compiler_bytecode.tape
  -> Omega-written Omega compiler C -> omega_compiler_bytecode.tape
```

Alpha is unchanged. Beta is a strict first-order functional S-expression
calculus interpreted by one directly audited Alpha tape. Gamma is the former
Delta typed functional language. Delta is the former Epsilon fixed-storage
compiler-host language. Epsilon is no longer a source owner or rung. Alpha Tape
Assembly is retained only as off-chain tooling under `tools/alpha/`.

## Purpose

Every intermediate language has one job: express the compiler for the rung
above it, plus a named small tool such as the derivation checker when that is
cheaper than another root artifact. It is not a public general-purpose trust
platform. Features require a concrete customer and a favorable whole-chain
audit.

Intermediate self-hosting has no value in this architecture. A Beta evaluator
need not be written in Beta; Gamma need not compile Gamma; Delta need not
compile Delta. Only Omega closes a meaningful self-host edge because `omega0`
must compile the production Omega-written compiler closure `C`.

## Edge discipline

Each compiler accepts exactly one source language and emits one
platform-independent Alpha tape. A lower rung may not parse its successor's
successor. Host scripts may invoke, stamp, compare, and report; they may not
parse source, lower programs, manufacture semantic evidence, or decide trust.

Every accepted edge binds:

```text
exact source + exact Alpha tape
  + source semantics + Alpha semantics
  + observation/resource profile
  + independently reconstructed obligation + checked certificate
  -> source-to-tape refinement
```

Reproducibility and differential agreement are diagnostics, not authority.
The directly audited Beta evaluator is part of the root and therefore receives
instruction-level audit rather than invented lower-language pedigree.

## Current state

Alpha conformance and off-chain assembler reconstruction are executable. The
Beta evaluator and Beta-written Gamma compiler are absent. The Gamma-written
Delta compiler source and Delta-written Omega `D` source are incomplete and
have no canonical tapes. Omega-written `C` is also incomplete. No old compiler,
interpreter, or compatibility route fills these gaps.

See the [manifest](chain_manifest.md), [repository map](repository_structure.md),
and [`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
