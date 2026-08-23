# Proof kernel

[Lattice overview](bootstrap_lattice.md) | [Psi/Omega toolchain](omega_toolchain.md) | [Terminal Psi](../pipeline/terminal_psi.md)

> **Status: WORKING CROSS-CUTTING SERVICE.** Independent Beta and Gamma
> implementations accept valid certificates, reject invalid ones, and are
> exercised by logical, equality, operational-seam, fuzz, and
> cross-implementation gates.

The proof kernel is deliberately not a language rung. Programs do not elaborate
through it, and it adds no stage between Gamma and Delta. It is an assurance
service used by producers and artifact verifiers throughout the build lattice.

Its canonical owner is `bootstrap/assurance/proof-kernel/`;
`compiler/proof-kernel` is a compatibility path. Beta, Gamma, and executable
reference implementations live under `implementations/`; untrusted automation,
fixtures, and executable policy live under `tools/`, `corpus/`, and `gates/`.
The
product-specific `psi-proof-kernel` remains under Psi semantics; it checks Psi
judgments and is not this generic bootstrap derivation checker.

```text
Alpha → Beta → Gamma → Delta                   language spine
              ↘       ↙       ↓
                proof kernel   omega-bootstrap → production omega
                assurance      hosted compiler edge
```

## Judgment

The kernel answers one small question:

```text
Does certificate C derive proposition P under explicit premises Γ?
```

It accepts or rejects. It does not search for proofs, optimize programs, compile
source, reconstruct artifact obligations, or decide which claims are sufficient
for deployment. Those larger components may be arbitrarily sophisticated, but
they gain no authority by producing a candidate certificate.

## Implementations

The principal low-rung implementations are:

- `bootstrap/assurance/proof-kernel/implementations/beta/check.beta` — logical proof checking in Beta;
- `bootstrap/assurance/proof-kernel/implementations/beta/eq.beta` — fuel-bounded definitional equality;
- `bootstrap/assurance/proof-kernel/implementations/gamma/checker.gamma` — independently written Gamma checker;
- `bootstrap/assurance/proof-kernel/implementations/gamma/checker_typed.gamma` — typed Gamma form checked by Gamma's
  static type checker.

They are separately written implementations of a shared calculus. Shared
positive and negative corpora, cross-checks, fuzzers, and operational seams test
that they decide the same judgments. Agreement is evidence while the formal
soundness bridge matures; it does not grant either implementation
authority over artifact-specific obligation reconstruction.

## Proof checking is not artifact verification

A proof bundle can honestly prove the wrong proposition. Therefore the kernel
cannot, by itself, validate a terminal-Psi artifact.

The artifact-aware route is:

```text
canonical terminal-Psi bytes
    → validate canonical structure and identities
    → reconstruct the complete ordered semantic ledger
    → derive the exact obligation set for this artifact
    → require exactly the permitted evidence for every obligation
    → invoke the proof kernel on certificate-derived facts
    → VerifiedTerminalModule
```

The proposition carried for a certificate must exactly match an obligation
reconstructed independently from the fingerprinted artifact. A producer cannot
omit an access, weaken a contract, relabel a derived fact as admitted, or attach
a valid theorem to unrelated code.

This is why Gamma is not “the Psi proof checker.” Gamma hosts one generic kernel
implementation. A low-rung Psi-aware semantic-ledger generator supplies the
artifact-specific reconstruction assurance. A future Psi- or Omega-hosted kernel
may accelerate or independently cross-check validation, but it cannot replace
that reconstruction step merely by understanding the proof calculus.

## Canonical semantic ledger

The deployment endpoint consumes canonical terminal-Psi bytes and produces one
ordered semantic ledger containing every required goal, permitted premise,
identity, validity scope, establishment point, invalidation, and justification
dependency. Direct low-rung evaluation or a checked derivation of that same
ledger is authoritative; agreement with the current optimized Rust verifier is
diagnostic only.

Local operation denotations and goal shapes come from closed, typed declarative
schemas. Multi-operation algebraic reduction remains untrusted and must emit a
kernel-checked derivation from exact ledger premises to the unchanged canonical
goal. Adding a proof-bearing operation therefore does not add an opaque trusted
verification function.

The detailed operation inventories, arithmetic reconstruction families,
composition rules, canonical byte formats, and current Rust migration closure
belong to [Terminal Psi](../pipeline/terminal_psi.md) and the implementation
READMEs. They are intentionally not duplicated in this kernel-boundary document.

## Trust and soundness

Kernel acceptance is authoritative only for the kernel judgment. Connecting
that judgment to program execution requires the soundness bridge:

```text
kernel accepts C : P
    ⟹ P is true in the pinned operational semantics
```

That full metatheorem remains research work. Today every logical capability must
ship with an operational seam that compares kernel-provability with independent
evaluation or decision on a positive corpus and a negative battery. These seams
are strong regression evidence, not substitutes for the theorem.

Fuel bounds ensure normalization and checking terminate operationally. Fuel
exhaustion is rejection or an explicit incomplete result; it is never evidence
that a proposition is false and never termination evidence for the program being
proved.

## Producer and self-host boundaries

The Delta-built `omega-bootstrap` compiler and the production Omega compiler
may both emit certificates. Neither compiler decides whether its own evidence is
valid. A bug in the bridge can reproduce into production Omega, but it
cannot forge a derivation accepted by an independent sound kernel or choose a
different artifact obligation when canonical reconstruction is in place.

Self-hosting closes external compiler dependencies. Proof checking, canonical
meaning, artifact reconstruction, and translation validation establish semantic
assurance across the self-host edge.

## Scope discipline

- The kernel grows only when an actual obligation class requires a new logical
  rule.
- Every new rule lands in each independent implementation with positive,
  negative, differential, and operational-seam coverage.
- Proof search, SMT procedures, algebraic reducers, optimizers, and compilers are
  untrusted certificate producers.
- Artifact-specific semantics remain in the canonical ledger definition, not in
  generic kernel rules.
- Production optimization remains entirely outside the kernel.
