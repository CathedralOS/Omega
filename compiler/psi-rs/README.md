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

- `foundation/psi-access-plans`: normalized placed-view access demand and
  authorization semantics;
- `foundation/psi-arena`: typed dense, paged, generational, hierarchy, and
  ordered-root arena storage used by Psi-owned source representations;
- `foundation/psi-diagnostics`: target-neutral diagnostic values and phase
  snapshot contracts;
- `foundation/psi-extents`: target-neutral extent geometry, lineage, rights,
  and admitted-provider identities;
- `foundation/psi-language-core`: target-neutral grammar semantics plus
  atomic-ordering, cast-form, operator-spelling, and source-assembly contract
  vocabulary used by source representations;
- `foundation/psi-language-semantics`: target-neutral resolved semantic
  identities, service/domain tables, termination and supply plans,
  establishment routes, byte-sequence predicates, canonical const-value atoms,
  and normalized wire scalar ranges;
- `foundation/psi-layout-plans`: normalized author-selected layout geometry,
  relocation identity, and materialization plans;
- `foundation/psi-numerics`: exact integers/rationals, host-independent float
  semantics, arithmetic domains, and source-literal payloads;
- `foundation/psi-source`: loaded-source records and maps, source identities,
  coordinates, and source-backed text shared by the Psi frontend and temporary
  Omega compatibility exports;
- `foundation/psi-source-loader`: root-file loading into Psi-owned source maps;
- `foundation/psi-symbols`: stable source symbol identities, names, paths, and
  hierarchy storage used by resolution and later semantic stages;
- `foundation/psi-core`: stable terminal semantic identities and the initial
  typed proposition vocabulary;
- `representations/psi-tokens`: the spelling-level Omega token stream;
- `representations/psi-syntax-trees`: parsed Omega source shape before name and
  symbol resolution;
- `representations/psi-symbol-resolved-trees`: source-shaped trees carrying
  resolved symbol identities;
- `representations/psi-typed-trees`: target-neutral typed source trees and
  canonical semantic boundary identities;
- `representations/psi-terminal`: the in-memory terminal semantic module and
  its first integer-constant / jump / return operation vocabulary;
- `pipeline/psi-source-files-to-tokens`: the Psi-owned Omega lexer;
- `pipeline/psi-tokens-to-syntax-trees`: the Psi-owned unresolved Omega parser;
- `pipeline/psi-syntax-trees-to-symbol-resolved-trees`: Psi-owned name lookup,
  source-scope resolution, and stable symbol stamping;
- `semantics/psi-proof-kernel`: total primitive judgments, explicit proof
  checking, evidence envelopes, and sealed admission validation.
- `semantics/psi-terminal-verifier`: structural module validation,
  verifier-reconstructed operation/edge axioms, and exhaustive bodyful-contract
  evidence checking.

The old `omega-tokens` and `omega-source-files-to-tokens` package names are
compatibility re-exports for unmigrated parser consumers; they contain no
token or lexer implementation. New frontend work proceeds under this root.
