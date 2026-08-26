# Psi Compiler Workspace

Psi owns Omega-file parsing and all target-neutral language semantics through
the immutable terminal-Psi module. Crates under this directory must not depend
on Omega backend, target, ABI, storage, instruction, object, or installation
representations.

Frontend ownership has migrated completely: Omega consumes Psi-owned source
and semantic representations directly, and no Omega-named frontend adapter or
`omega-core` re-export sits between them. Terminal-Psi coverage still grows in
vertical slices; constructs outside that vocabulary continue from checked Psi
semantics into Omega lowering until their terminal form lands.

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
  content algebra/projection plans, built-in value domains, and normalized
  wire scalar ranges;
- `foundation/psi-layout-plans`: normalized author-selected layout geometry,
  relocation identity, and materialization plans;
- `foundation/psi-numerics`: exact integers/rationals, host-independent float
  semantics, arithmetic domains, and source-literal payloads;
- `foundation/psi-source`: loaded-source records and maps, source identities,
  coordinates, and source-backed text shared by the Psi frontend and Omega
  orchestration;
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
- `representations/psi-facts`: durable target-neutral places, contexts,
  propositions, and checked-fact plans;
- `representations/psi-effects`: target-neutral operational ceilings, service
  reach, synchronous invocation summaries, and capability-flow facts;
- `representations/psi-checked-trees`: checked proof, borrow, flow, reach,
  value-origin, carried semantic-dependency, and admissibility evidence;
- `representations/psi-terminal`: the self-contained terminal semantic module,
  closed operation vocabulary, contracts, claims, and proof-facing identities;
- `pipeline/psi-source-files-to-tokens`: the Psi-owned Omega lexer;
- `pipeline/psi-tokens-to-syntax-trees`: the Psi-owned unresolved Omega parser;
- `pipeline/psi-syntax-trees-to-symbol-resolved-trees`: Psi-owned name lookup,
  source-scope resolution, and stable symbol stamping;
- `pipeline/psi-symbol-resolved-trees-to-typed-trees`: Psi-owned type identity,
  compatibility, and signature normalization;
- `pipeline/psi-typed-trees-to-checked-trees`: Psi-owned semantic checking and
  checked-fact construction;
- `pipeline/psi-checked-trees-to-terminal`: fail-closed vertical-slice
  production from checked semantics into terminal Psi, including current
  scalar/control/call/crash and content-evidence slices;
- `semantics/psi-types`: unresolved source type-surface analysis;
- `semantics/psi-validation`: target-neutral cross-semantic source validation;
- `semantics/psi-proof`: source proof-surface collection, obligation planning,
  and checking;
- `semantics/psi-proof-admission`: currently named product-local admission and
  judgment checking (rename tracked in
  [`TASKS.md`](../../../../TASKS.md)), explicit proof checking, evidence
  envelopes, and sealed admission validation;
- `semantics/psi-checked-interpreter`: build-time and differential reference
  execution of checked/source-shaped semantics not yet represented in terminal
  Psi;
- `semantics/psi-terminal-verifier`: structural module validation,
  verifier-reconstructed operation/edge axioms, and exhaustive bodyful-contract
  evidence checking;
- `semantics/psi-terminal-interpreter`: canonical decoding, verification, and
  fuel-bounded reference execution of terminal-Psi artifacts.

Every workspace harness invokes the Psi source-to-checked stages directly.
Omega begins at provider selection and realization: it consumes terminal Psi
where that vocabulary exists and otherwise lowers checked Psi semantics while
the remaining terminal slices are implemented. Cross-layer interpreter/native
comparisons live in an Omega test-only harness; both reference interpreters
remain Psi-owned.
