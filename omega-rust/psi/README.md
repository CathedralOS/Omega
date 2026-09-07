# Psi Compiler Workspace

Psi owns Omega-file parsing and all target-neutral language semantics through
the immutable terminal-Psi module. That module is a publishable compilation
product: an interpreter or native lowerer may consume it later under separate
realization authority. Crates under this directory must not depend on Omega
backend, target, ABI, storage, instruction, object, or installation
representations.

Frontend ownership has migrated completely: Omega consumes Psi-owned source
and semantic representations directly, and no Omega-named frontend adapter or
`core` re-export sits between them. Terminal-Psi coverage still grows in
vertical slices; constructs outside that vocabulary continue from checked Psi
semantics into Omega lowering until their terminal form lands.

Current roots:

- `foundation/access-plans`: normalized placed-view access demand and
  authorization semantics;
- `foundation/arena`: typed dense, paged, generational, hierarchy, and
  ordered-root arena storage used by Psi-owned source representations;
- `foundation/diagnostics`: target-neutral diagnostic values and phase
  snapshot contracts;
- `foundation/extents`: target-neutral extent geometry, lineage, rights,
  and admitted-provider identities;
- `foundation/language-core`: target-neutral grammar semantics plus
  atomic-ordering, cast-form, operator-spelling, and source-assembly contract
  vocabulary used by source representations;
- `foundation/language-semantics`: target-neutral resolved semantic
  identities, service/domain tables, termination and supply plans,
  establishment routes, byte-sequence predicates, canonical const-value atoms,
  content algebra/projection plans, built-in value domains, and normalized
  wire scalar ranges;
- `foundation/layout-plans`: normalized author-selected layout geometry,
  relocation identity, and materialization plans;
- `foundation/numerics`: exact integers/rationals, host-independent float
  semantics, arithmetic domains, and source-literal payloads;
- `foundation/source`: loaded-source records and maps, source identities,
  coordinates, and source-backed text shared by the Psi frontend and Omega
  orchestration;
- `foundation/symbols`: stable source symbol identities, names, paths, and
  hierarchy storage used by resolution and later semantic stages;
- `foundation/semantic-vocabulary`: stable terminal semantic identities and the initial
  typed proposition vocabulary;
- `representations/tokens`: the spelling-level Omega token stream;
- `representations/syntax-trees`: parsed Omega source shape before name and
  symbol resolution;
- `representations/symbol-resolved-trees`: source-shaped trees carrying
  resolved symbol identities;
- `representations/typed-trees`: target-neutral typed source trees and
  canonical semantic boundary identities;
- `representations/facts`: durable target-neutral places, contexts,
  propositions, and checked-fact plans;
- `representations/flow-effects`: target-neutral operational ceilings, service
  reach, synchronous invocation summaries, and capability-flow facts;
- `representations/checked-trees`: checked proof, borrow, flow, reach,
  value-origin, carried semantic-dependency, and admissibility evidence;
- `representations/terminal`: the self-contained terminal semantic module,
  closed operation vocabulary, contracts, claims, and proof-facing identities;
- `pipeline/source-files-to-tokens`: the Psi-owned Omega lexer;
- `pipeline/tokens-to-syntax-trees`: the Psi-owned unresolved Omega parser;
- `pipeline/syntax-trees-to-symbol-resolved-trees`: Psi-owned name lookup,
  source-scope resolution, and stable symbol stamping;
- `pipeline/symbol-resolved-trees-to-typed-trees`: Psi-owned type identity,
  compatibility, and signature normalization;
- `pipeline/typed-trees-to-checked-trees`: Psi-owned semantic checking and
  checked-fact construction;
- `pipeline/checked-trees-to-lowered-psi`: fail-closed vertical-slice
  production from checked semantics into terminal Psi, including current
  scalar/control/call/crash and content-evidence slices;
- `semantics/validation`: target-neutral cross-semantic source validation;
- `semantics/proof`: source proof-surface collection, obligation planning,
  and checking;
- `semantics/proof-admission`: product-local Psi judgment and admission
  checking, explicit proof checking, evidence envelopes, and sealed admission
  validation; it is distinct from the Alpha-owned derivation checker;
- `semantics/checked-interpreter`: build-time and differential reference
  execution of checked/source-shaped semantics not yet represented in terminal
  Psi;
- `semantics/terminal-verifier`: structural module validation,
  verifier-reconstructed operation/edge axioms, and exhaustive bodyful-contract
  evidence checking;
- `semantics/terminal-interpreter`: canonical decoding, verification, and
  fuel-bounded reference execution of terminal-Psi artifacts.

Every workspace harness invokes the Psi source-to-checked stages directly.
Omega begins at provider selection and realization: it consumes terminal Psi
where that vocabulary exists and otherwise lowers checked Psi semantics while
the remaining terminal slices are implemented. Cross-layer interpreter/native
comparisons live in an Omega test-only harness; both reference interpreters
remain Psi-owned.

The shared free/attached Unit state-graph producer retains the authored
`Slice::Length` witness as `TerminalRankedScc::Natural`, using actual immutable
byte-view lengths rather than a synthetic counter. Ordinary Terminal verification
reconstructs every SCC and internal edge, checks exact successor-rank substitution,
and accepts preserving staging edges only when strict edges meet every cycle.
`terminal-verifier/src/control_cycles` owns those checks; the lowerer's
`attached_unit/composed_control/state_graph/ranking.rs` retains the source witness.
Grouped `proof_bundle.control_cycles` evidence uses the shared component checker
for one well-foundedness citation and every edge comparison. Serialized writer
coverage includes raw bytes, optional newline, caller continuation, and resumable
interpretation. Terminal semantic format/vocabulary markers are 79/85; the proof
format marker is 30.

This natural-rank slice also accepts fixed unsigned scalar parameter ranks at
the Terminal boundary. Mutable/owned cyclic custody, general projections and
ranking views, callee-progress composition, and source-to-Psi correspondence
remain separate limitations. Native byte views and Natural fixed-fuel/native
routes remain fenced; only the legacy unsigned countdown has the special ranked
native and fixed-fuel routes. Source `requires bytes.len > 0` still needs
contract-level byte-length observation and exact caller substitution. See
[Terminal Psi architecture](../../wiki/architecture/pipeline/terminal_psi.md#borrowed-byte-writer-composition)
for the supported writer composition and remaining boundaries.
