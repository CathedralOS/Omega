# Bootstrap minimization

> **Status: topology selected; enforcement continues.**

The selected lattice is `Alpha -> Beta -> Gamma -> Delta -> Epsilon -> Omega`.
Alpha is the unchanged tape machine. Beta is the trusted imperative tape-
assembly language with an admitted self-reconstructing compiler tape. Gamma,
Delta, and Epsilon are single-customer evaluator/compiler languages. Only Omega
has a meaningful later self-host edge.

## Objective

Minimize the complete audited path to the first full Omega compiler, not any
single source file. Count written semantics, root artifact bytes, mutable state,
compiler source, proof obligations, checker rules, resource profiles, wire
formats, permanent tests, and host plumbing. Complexity nearer Alpha costs more
because every later edge inherits it.

## Retention test

A language feature, runtime mechanism, checker rule, sidecar, or tool survives
only when it serves one exact customer and reduces total audit cost:

- Beta: Gamma evaluator;
- Gamma: Delta compiler or derivation checker;
- Delta: Epsilon compiler;
- Epsilon: Omega compiler `D`;
- Omega `D`: first complete Omega compiler;
- Omega `C`: production compiler and the sole self-host closure.

Familiarity, general-purpose usefulness, current implementation, historical
investment, compatibility, and hypothetical reuse are not reasons.

## Non-negotiable properties

- deterministic written semantics for every retained construct;
- exact identity for every admitted seed and generated tape;
- closed source envelopes and explicit bounded implementation profiles;
- distinct invalid-source, authored-trap, incomplete-capacity, and internal-
  contradiction outcomes;
- exact successful bytes and no partial publication on failure;
- independent reconstruction of every checked source/tape proposition; and
- no hidden host semantic stage or source-specific accelerator.

The admitted Beta compiler tape is legitimate root material. Its exact Beta
self-reconstruction is part of the authority argument because the measured
Gamma evaluator prototype is too large for credible opaque bytecode review.

## Feature review

For each proposed addition:

1. Name the exact customer source that needs it.
2. Show why existing forms or a source refactor are worse.
3. Account for evaluator/compiler/checker and proof complexity, not source lines
   alone.
4. Define deterministic failure and resource behavior.
5. Add a focused positive and mutation control.
6. Delete the feature when its customer disappears.

A tiny functional language stops being tiny when closures, macros, general GC,
polymorphism, continuations, exceptions, packages, or ambient effects arrive by
convenience. Those remain excluded unless a new whole-chain comparison reverses
the selected ruling.

## Completion

The minimization program closes when every retained mechanism has a named
customer, every selected edge reconstructs exactly, the chain rebuilds offline
from an audited Alpha seed and repository bytes, and no retired rung, alternate
compiler, compatibility adapter, or machine-readable sidecar survives without
a current consumer.
