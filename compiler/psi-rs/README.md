# Psi Compiler Workspace

Psi owns Omega-file parsing and all target-neutral language semantics through
the immutable terminal-Psi module. Crates under this directory must not depend
on Omega backend, target, ABI, storage, instruction, object, or installation
representations.

The migration is incremental. Existing target-neutral bootstrap crates remain
under `compiler/omega-rs` until their ownership is moved or renamed with a
compatibility adapter. New terminal semantic identities and proof machinery
land here so the eventual boundary does not acquire a reverse dependency on
the old Omega pipeline.

Current roots:

- `foundation/psi-core`: stable terminal semantic identities and the initial
  typed proposition vocabulary;
- `representations/psi-terminal`: the in-memory terminal semantic module and
  its first integer-constant / jump / return operation vocabulary;
- `semantics/psi-proof-kernel`: total primitive judgments, explicit proof
  checking, evidence envelopes, and sealed admission validation.
- `semantics/psi-terminal-verifier`: structural module validation,
  verifier-reconstructed operation/edge axioms, and exhaustive bodyful-contract
  evidence checking.

Canonical serialization and semantic fingerprints are intentionally deferred
until the in-memory vocabulary has both interpreter and Omega-lowering
customers, as required by the terminal-Psi architecture.
