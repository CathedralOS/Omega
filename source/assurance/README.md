# `source/assurance/` — cross-rung checking

This directory owns assurance that does not belong to any language rung:

- [`proof-kernel/`](proof-kernel/) checks derivations from explicit premises. It
  has independently written Beta and Gamma implementations, but is not a stage
  in the Alpha → Beta → Gamma → Delta build chain.
- [`refinement/`](refinement/) reconstructs and checks claims across concrete
  compiler edges. `beta/` relates Beta source to Alpha artifacts;
  `omega-bootstrap/` carries meaning and artifact checks for the bridge.

Artifact-specific obligation reconstruction remains distinct from generic proof
checking. The current Rust realization of Psi judgments lives under
`source/compiler/rust/psi/`; the bootstrap proof kernel accepts only the
derivations for obligations an independent, artifact-aware layer reconstructs.

Canonical paths are resolved through [`../paths.sh`](../paths.sh). The former
compatibility entries under `compiler/` have been retired.
