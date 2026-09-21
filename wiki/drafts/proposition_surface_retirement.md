# NEW-PCM-PROPOSITION-SURFACE-INVENTORY — proposition declaration surface at 97be15c1b592

Status: retirement inventory for the dedicated `proposition` declaration under
**PROOF-CONTRACT-MIGRATION** (TASKS.md:2605). The migration moves the proof
surface to ordinary machine contracts and trait bundles elaborating to
PROOF-KERNEL-CORE terms; `wiki/spec/proofs/mathematical_bindings.md:8` already
records the settled direction — "No `forall`, `exists`, `claim`, `proposition`,
or proposition-returning machines remain optional, unaccepted naming
proposals." This note inventories every surface the retirement must drain. It
authorizes no new gate and changes no fence.

## 1. Source grammar

`tokens-to-syntax-trees/src/declarations/proposition.rs`
(`parse_proposition_definition`) admits two bodies:

- `proposition Name(params) = expr;` — transparent formula
  (`PropositionBody` transparent arm); and
- `proposition Name(params);` — primitive body
  (`PropositionBody::Primitive`).

Static binders only: lifetime parameters are refused at parse
("proposition binders are proof-static and cannot declare lifetime
parameters"). `TypeParameterKind::Proposition { contract }`
(`syntax-trees-to-symbol-resolved-trees`/`typed-trees` data declarations) is
the companion surface — a data type parameter carrying a proposition
contract — and `SymbolKind::{Proposition, PropositionParameter}` give the
declaration and its parameters resolved identities.

## 2. Pipeline owners

| Stage | Owning file |
|---|---|
| Parse | `psi/pipeline/tokens-to-syntax-trees/src/declarations/proposition.rs` |
| Resolve | `psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/lowering/proposition.rs` |
| Type | `psi/pipeline/symbol-resolved-trees-to-typed-trees/src/declarations/proposition.rs` |
| Check | `psi/pipeline/typed-trees-to-checked-trees/src/proof/proposition_vocabulary.rs`, `proof/proof_output_calls.rs` (named-witness call lanes), `proof/contracts.rs`, `proof/contracts/inherited.rs`, `proof/contracts/operators.rs`, `proof/evidence_forwarding.rs`, `proof/outcome_arms.rs`, `proof/contract_entailment.rs` |

## 3. Representations

- `psi/representations/symbol-resolved-trees/src/symbol_resolved_trees/evidence/proposition.rs`
- `psi/representations/typed-trees/src/typed_trees/evidence/proposition.rs`
- `psi/representations/checked-trees/src/checked_trees/proof/propositions.rs`
  (`CheckedPropositionVocabulary`: `CheckedPropositionDeclaration` +
  `CheckedPropositionApplication` rosters)
- `psi/representations/terminal-psi/src/terminal_module/proof/*` and
  `artifacts/proof_bundle/*` — proposition-shaped proof content and witnesses
  on the Terminal boundary.

## 4. Library carriers

`proposition` declarations in the bundled core library:

- `source/library/core/relation.omg`
- `source/library/core/int.omg` (e.g. `int_pair_equivalent`)
- `source/library/core/extent.omg`

## 5. Corpus carriers

43 files under `tests/omega/` name `proposition`:

- `fail/proofs` — 27 fixtures (the `quotient_*` rejection family plus
  `proposition_relation_inherited_law_rejected`,
  `proposition_relation_*` pins)
- `pass/proofs` — 12 fixtures (`named_witness_*` ×6,
  `proposition_relation_hierarchy_compile`, `quotient_*` ×5)
- `pass/borrows` — `borrow_proposition_index_disequality_mut`
- `pass/dependent` — `case_where_match_contributes`
- `pass/terminal_psi` — `integer_control_contract`
- `fail/borrows` — `borrow_proposition_opaque_mut`

`proofs/named_witness_*` and `proposition_relation_hierarchy_compile` are the
pass polarity pins for the named-witness call lanes;
`fail/proofs/proposition_*`/`quotient_*` pin the rejection controls the
migration must preserve.

## 6. Spec surface

- `wiki/language_guide/chapter_10_compile_time_proofs.md` — the user-facing
  `proposition` declaration semantics (propositions mention linear values
  without custody; a containing proposition cannot justify its own
  formation; partial applications stay propositions).
- `wiki/spec/proofs/foundation.md` — kernel-side `Strict`/`Squash`
  proposition vocabulary (`P : Strict v`, proof irrelevance).
- `wiki/spec/proofs/mathematical_bindings.md` — settled exclusion: no
  `proposition` keyword survives the migration.
- `wiki/spec/proofs/inductive_profile.md:139` — propositional laws change a
  contract and themselves need proof.

## 7. Retirement edge

PROOF-CONTRACT-MIGRATION's second bullet owns the replacement: formula
declarations and hidden-witness calls migrate to ordinary contracts and
named witness/law bundles, preserving substitution, result/path
availability, erasure, validity, and transitive assumptions across trait
calls and artifacts. The corpus split above is the drain order: the
`pass/proofs/named_witness_*` lanes and `proposition_relation_hierarchy`
migrate to contract/witness-bundle spellings first; the `fail/proofs`
rejection controls rewrite their pinned diagnostics on the new surface; the
library `proposition` declarations in `core/{relation,int,extent}.omg`
migrate last since every quotient fixture depends on `equivalent`.

Open edges the parent row already records: Terminal-side
`terminal_module/proof/*` + `proof_bundle` propositions stay until the
kernel elaboration's evidence encoding exists (`terminal-codec`'s
certificate wire still has no production caller), and the checked records
retire only when `ProofFacts::mathematical_declarations` carries the
elaborated `Declaration` list end to end.
