# The bootstrap chain

> **Status: selected topology, incomplete compiler edges.**

The trust-minimizing lattice is:

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator -> gamma_evaluator_bytecode.tape
  -> Gamma-written Delta compiler -> delta_compiler_bytecode.tape
  -> Delta-written Epsilon compiler -> epsilon_compiler_bytecode.tape
  -> Epsilon-written Omega compiler D -> omega0_compiler_bytecode.tape
  -> Omega-written Omega compiler C -> omega_compiler_bytecode.tape
```

Alpha is unchanged. Beta is the trusted imperative tape-assembly language whose
self-reconstructing compiler has an admitted Alpha tape. Gamma is the bounded
concatenative compiler machine above it. Delta is the former Gamma typed
functional language, and Epsilon is the former Delta fixed-storage compiler
host.

## Purpose

Every intermediate language has one job: express the compiler for the rung
above it, plus a named small tool such as the derivation checker when that is
cheaper than another root artifact. It is not a public general-purpose trust
platform. Features require a concrete customer and a favorable whole-chain
audit.

Beta self-reconstruction binds its readable compiler source to the admitted
cold-start tape. No later intermediate rung needs self-hosting: Beta evaluates
Gamma, Gamma compiles Delta, Delta compiles Epsilon, and Epsilon compiles Omega.
Only Omega closes a further self-host edge because `omega0` must compile the
production Omega-written compiler closure `C`.

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

The Beta compiler tape is admitted at the Alpha boundary; its exact Beta
self-reconstruction is part of that authority story. Later sources derive
their tapes through the immediately preceding trusted language.

## Current state

Alpha conformance and trusted Beta compiler reconstruction are executable. The
Beta-written Gamma evaluator has a passing development slice but no admitted
tape. The Gamma-written Delta compiler is absent. The Delta-written Epsilon
compiler and Epsilon-written Omega `D` are incomplete and have no canonical
tapes. Omega-written `C` is also incomplete. No compatibility route fills these
gaps.

See the [manifest](chain_manifest.md), [repository map](repository_structure.md),
and [`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
