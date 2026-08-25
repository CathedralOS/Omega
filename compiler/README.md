# `compiler/` — product compiler source

Omega is rebuilt from a small audited seed through increasingly capable
languages, then through one deliberately profile-limited hosted edge:

```text
Alpha → Beta → Gamma → Delta
Delta compiler source ──[Delta→Gamma + Gamma execution]──▶ delta compiler
Delta bridge source ──[delta compiler]───────────────────▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

Delta is an independently specified compiler-host language; shared Omega-like
spelling does not make it an Omega subset. `omega-bootstrap` accepts only the
Omega product-compiler source
profile `Ωself` required by the exact product source closure and rejects the
rest. The product source is normal Omega constrained to that profile; the
resulting compiler implements full Omega. This hosted dependency replaces a
historical tower of external implementation-language dependencies.

“Full Omega” refers to accepted language and artifact semantics, not a mandate
to import every adjacent tool. Standalone Terminal-Psi interpreters, REPLs,
proof explorers, viewers, and debuggers stay outside the hosted compiler source
closure unless the compiler executable actually depends on them.

The bridge may run slowly and lower the product compiler conservatively. It
must compile the `Ωself` source that implements the production optimizer and
advanced lowering, but need not run those passes itself. A further product
`omega` → `omega` rebuild can optimize the compiler binary and add fixed-point
evidence; it is optional for functionality and dependency closure. The required
bridge → product edge is a cross-language hosted build; only that optional edge
is a strict self-rebuild.

The hosted edge does not claim self-hosting proves correctness. A defect in
`omega-bootstrap` can reproduce into production Omega. Canonical meaning routes,
artifact reconstruction, proof checking, operational cross-checks, and
translation validation supply the assurance.

## Language spine

| Language | Role | Principal gate |
| --- | --- | --- |
| **Alpha** | 21-opcode raw executor and native seed | written semantics, conformance, audited x64/arm64 realizations |
| **Beta** | small structured compiler language with Omega-shaped state graphs | `bc` self-host, language corpus, complete `B_bc1` source-to-artifact refinement |
| **Gamma** | pure ADTs, matching, types, and fuel-bounded definitional meaning | interpreter/type-checker gates and meaning corpora |
| **Delta** | independent systems/compiler-host language that can build `omega-bootstrap` | self-host, native corpus, Delta-to-Gamma meaning diamond |

The Greek names and order are fixed language roles. The Alpha assembler now
lives at `bootstrap/rungs/alpha/assembler/`. Beta proper is the language
compiled by `bootstrap/rungs/beta/bc.beta`.
The disposable Rust producer of Alpha VM tapes lives at
`bootstrap/onramps/alpha-assembler-rust/` and has no Beta-language role.
The disposable Beta-language diagnostic/reference producer lives at
`bootstrap/onramps/beta-rust/`.
Delta's language corpus, Delta-written compiler, and lattice-built artifacts
live under `bootstrap/rungs/delta/`; its disposable Rust producer lives under
`bootstrap/onramps/delta-rust/`.

## Proof kernel

The proof kernel is a cross-cutting assurance service, not a language rung.
`bootstrap/assurance/proof-kernel/implementations/beta/check.beta` and
`bootstrap/assurance/proof-kernel/implementations/gamma/checker.gamma` are
separately written implementations checked against shared positive, negative,
cross-check, fuzz, and operational-seam gates. Their agreement is useful
evidence while the soundness bridge matures; it does not replace
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

## Psi, omega-bootstrap, and production Omega

Psi owns Omega source semantics through terminal portable IR. Omega consumes
terminal Psi and performs target realization and native emission.

The hosted build has two source surfaces:

1. Delta source implements `omega-bootstrap`, including exact `Ωself`
   acceptance and correct conservative lowering of the product source.
2. `omega-bootstrap` compiles the `Ωself`-constrained Omega product source into
   the full-spec compiler containing the optimizer and advanced lowering. That
   compiler's own binary may be conservatively lowered until an optional
   self-rebuild.

`Ωself` has no private semantics and is not another language rung. The feature
budget and enforcement contract live in
[`compiler_source_profile.md`](../wiki/architecture/bootstrap_lattice/compiler_source_profile.md).

The current Rust implementation remains a migration/reference producer under
`bootstrap/onramps/omega-rust/{psi,omega}/` while that hosted path matures.
`bootstrap/omega-bootstrap/` owns the Rust-free meaning, current Delta-written compiler
slices/profiles, and validation gates. The roots here at
`compiler/{psi,omega}/` own the Omega-written product source. The first coherent
checkpoint now implements Psi source-to-token processing under `compiler/psi/`;
`compiler/omega/` and later Psi phases remain open. Exact checkpoint closures
and provisional `Ωself` censuses live in `compiler/source-checkpoints/`.

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
later from producers, where it affects self-sufficiency. The current Rust
Psi/Omega compiler may remain maintained in parallel as a differential
reference, but it grants no authority and is never required to bootstrap or
release the product. `omega-bootstrap` is the point at which an external
compiler can be omitted entirely.

## Entry points

```sh
sh bootstrap/verify-lattice.sh
sh bootstrap/rungs/beta/selfhost.sh
sh bootstrap/rungs/gamma/test-interp.sh
sh bootstrap/rungs/gamma/test-typeck.sh
sh bootstrap/assurance/proof-kernel/gates/test.sh
```

Architecture and standing decisions live in
[`wiki/architecture/bootstrap_lattice/`](../wiki/architecture/bootstrap_lattice/).
The product roots have role-based names; external-language producers are
explicitly suffixed on-ramps. The retired flat bootstrap facade and canonical
ownership map are documented in
[`repository_structure.md`](../wiki/architecture/bootstrap_lattice/repository_structure.md).
Live bootstrap work belongs in [`TASKS_BOOTSTRAP.md`](../TASKS_BOOTSTRAP.md),
while broader product work belongs in [`TASKS.md`](../TASKS.md). Exact corpus and
gate counts belong beside the scripts that produce them rather than in this
overview.
