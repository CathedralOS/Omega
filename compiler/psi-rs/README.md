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

- `foundation/psi-arena`: typed arena handles and contiguous handle spans used
  by Psi-owned source representations;
- `foundation/psi-diagnostics`: target-neutral diagnostic values and phase
  snapshot contracts;
- `foundation/psi-language-core`: target-neutral atomic-ordering, cast-form,
  and operator-spelling vocabulary used by source representations;
- `foundation/psi-source`: loaded-source records and maps, source identities,
  coordinates, and source-backed text shared by the Psi frontend and temporary
  Omega compatibility exports;
- `foundation/psi-source-loader`: root-file loading into Psi-owned source maps;
- `foundation/psi-core`: stable terminal semantic identities and the initial
  typed proposition vocabulary;
- `representations/psi-tokens`: the spelling-level Omega token stream;
- `representations/psi-terminal`: the in-memory terminal semantic module and
  its first integer-constant / jump / return operation vocabulary;
- `pipeline/psi-source-files-to-tokens`: the Psi-owned Omega lexer;
- `semantics/psi-proof-kernel`: total primitive judgments, explicit proof
  checking, evidence envelopes, and sealed admission validation.
- `semantics/psi-terminal-verifier`: structural module validation,
  verifier-reconstructed operation/edge axioms, and exhaustive bodyful-contract
  evidence checking.

The old `omega-tokens` and `omega-source-files-to-tokens` package names are
compatibility re-exports for unmigrated parser consumers; they contain no
token or lexer implementation. New frontend work proceeds under this root.
