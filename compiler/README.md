# `compiler/` — product compilers and bootstrap compatibility paths

Omega is rebuilt from a small audited seed through increasingly capable
languages, then through one deliberate Omega self-host edge:

```text
Alpha → Beta → Gamma → Delta
                           ↓
              Omega (Delta-built, simple)
                           ↓
              Omega (Omega-built, optimized)
```

The first Omega compiler is required to be spec-compliant, not fast. It may use
simple lowering, omit advanced optimization, compile slowly, and emit slower
code. It is nevertheless a valid self-sufficient endpoint. The second compiler
is the full production implementation written in Omega and built by the first.
That single self-host dependency replaces a historical tower of external
implementation-language dependencies.

The repeated Omega does not claim self-hosting proves correctness. A defect in
bootstrap Omega can reproduce into production Omega. Canonical meaning routes,
artifact reconstruction, proof checking, operational cross-checks, and
translation validation supply the assurance.

## Language spine

| Language | Role | Principal gate |
| --- | --- | --- |
| **Alpha** | 21-opcode raw executor and native seed | written semantics, conformance, audited x64/arm64 realizations |
| **Beta** | small structured compiler language with Omega-shaped state graphs | `bc` self-host, language corpus, source-to-artifact refinement (incomplete for the whole compiler) |
| **Gamma** | pure ADTs, matching, types, and fuel-bounded definitional meaning | interpreter/type-checker gates and meaning corpora |
| **Delta** | systems/compiler-host language that can build bootstrap Omega | self-host, native corpus, Delta-to-Gamma meaning diamond |

The Greek names and order are fixed language roles. The Alpha assembler now
lives at `bootstrap/rungs/alpha/assembler/`; historical `compiler/beta` is only
a compatibility path. Beta proper is the language compiled by
`bootstrap/rungs/beta/bc.beta`; `compiler/beta-lang` is also only a
compatibility path.

## Proof kernel

The proof kernel is a cross-cutting assurance service, not a language rung.
`bootstrap/assurance/proof-kernel/check.beta` and
`compiler/gamma/checker.gamma` are
separately written implementations checked against shared positive, negative,
cross-check, fuzz, and operational-seam gates. Their agreement is useful
evidence while the soundness bridge matures; it is not DDC and does not replace
artifact-specific refinement.

The kernel answers only:

```text
Does this certificate derive proposition P from these explicit premises?
```

It does not decide what proposition a terminal-Psi artifact must prove. The
Psi-aware artifact verifier or canonical semantic-ledger generator reconstructs
the exact obligations from canonical artifact bytes; the proof kernel checks the
attached derivations. Proof search and optimization remain untrusted producers.

See [`bootstrap/assurance/proof-kernel/README.md`](../bootstrap/assurance/proof-kernel/README.md) and
[`wiki/architecture/bootstrap_lattice/proof_kernel.md`](../wiki/architecture/bootstrap_lattice/proof_kernel.md).

## Psi and the two Omega compilers

Psi owns Omega source semantics through terminal portable IR. Omega consumes
terminal Psi and performs target realization and native emission.

The hosted build has two stages:

1. Delta builds the minimal conforming Psi/Omega path and produces bootstrap
   Omega.
2. Bootstrap Omega compiles the full optimizing compiler from Omega source.

The current Rust implementations remain migration/reference producers while
that hosted path matures. `compiler/omega/` contains Rust-free meaning and
translation-validation experiments. `compiler/omega-rs/` and
`compiler/psi-rs/` contain the current production implementations.

## Trust and verification

Self-hosting establishes dependency closure and reproducibility, not semantic
correctness. Trust comes from explicit, independently checked boundaries:

- Alpha is audited against written small-step semantics; its native realizations
  are conformance-checked across supported platforms.
- Higher-language meaning is exposed through the lower reference route.
- Artifact-specific obligations are reconstructed rather than selected by the
  producer.
- The proof kernel validates certificate derivations after the artifact-aware
  layer reconstructs the claims the producer must establish.
- Native and optimized paths are compared or translation-validated against
  canonical meaning.

Rust is removed first from meaning and checking, where it affects soundness, and
later from producers, where it affects self-sufficiency. The Delta-built Omega
compiler is the point at which an external compiler can be omitted entirely;
the Omega-built production compiler improves performance rather than closing a
new language dependency.

## Entry points

```sh
sh compiler/verify-lattice.sh
sh bootstrap/rungs/beta/selfhost.sh
sh compiler/gamma/test-interp.sh
sh compiler/gamma/test-typeck.sh
sh bootstrap/assurance/proof-kernel/test.sh
```

Architecture and standing decisions live in
[`wiki/architecture/bootstrap_lattice/`](../wiki/architecture/bootstrap_lattice/).
The current flat `compiler/` directory is migration state; the role-based target
layout is documented in
[`repository_structure.md`](../wiki/architecture/bootstrap_lattice/repository_structure.md).
Live bootstrap work belongs in [`TASKS_BOOTSTRAP.md`](../TASKS_BOOTSTRAP.md),
while broader product work belongs in [`TASKS.md`](../TASKS.md). Exact corpus and
gate counts belong beside the scripts that produce them rather than in this
overview.
