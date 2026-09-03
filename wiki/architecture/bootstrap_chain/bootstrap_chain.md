# The bootstrap chain

> **Status: selected topology, incomplete compiler edges.**

The trust-minimizing lattice is:

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator -> gamma_evaluator_bytecode.tape
  -> Gamma compiler -> canonical Beta -> gamma_compiler_bytecode.tape
  -> Delta compiler -> canonical Gamma -> canonical Beta -> delta_compiler_bytecode.tape
  -> Epsilon compiler -> canonical Delta -> ... -> epsilon_compiler_bytecode.tape
  -> Omega compiler D -> canonical Epsilon -> ... -> omega0_compiler_bytecode.tape
  -> Omega compiler C -> canonical Epsilon -> ... -> omega_compiler_bytecode.tape
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
cold-start tape. No later intermediate rung needs self-hosting: each validates
its successor and emits canonical source for the selected lower compilers.
Only Omega closes a further self-host edge because `omega0` must compile the
production Omega-written compiler closure `C`.

## Edge discipline

Beta alone encodes Alpha. Each higher compiler accepts exactly one source
language and emits canonical source in the immediately prior rung. A lower rung
may not parse its successor's successor. Host scripts may invoke, stamp, compare, and report; they may not
parse source, lower programs, manufacture semantic evidence, or decide trust.

Every accepted edge binds:

```text
exact source + canonical prior-rung receipt + exact Alpha tape
  + adjacent language semantics + Alpha semantics
  + observation/resource profile
  + independently reconstructed obligation + checked certificate
  -> source-to-tape refinement
```

The Beta compiler tape is admitted at the Alpha boundary; its exact Beta
self-reconstruction is part of that authority story. Later sources derive
their tapes through the immediately preceding trusted language.

## Current state

Alpha conformance, trusted Beta compiler reconstruction, and selected
Gamma-to-Beta compiler reconstruction are executable. The Beta-written Gamma
evaluator has a passing development slice but no admitted tape. The selected
Gamma-written Delta compiler has a composed scalar slice, but its full-language
implementation and admitted tape remain absent. The Delta-written Epsilon
compiler and Epsilon-written Omega `D` are incomplete and have no canonical
tapes. Omega-written `C` is also incomplete. No compatibility route fills these
gaps.

See the [manifest](chain_manifest.md), [repository map](repository_structure.md),
and [`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
