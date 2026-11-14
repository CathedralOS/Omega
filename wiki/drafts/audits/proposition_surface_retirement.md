# NEW-PCM-PROPOSITION-SURFACE-INVENTORY — proposition declaration surface at 97be15c1b592

Historical record of the cited revisions. Superseded gates, artifact formats
and checker names below are not current requirements or work to resume.
Use [verification placement](../../../omega-rust/pipeline.md#verification-placement)
and the [current task board](../../../TASKS.md) for the maintained model.

Status: retirement inventory for the dedicated `proposition` declaration under
**PROOF-KERNEL-CORE** in TASKS.md. The migration moves the proof
surface to ordinary machine contracts and trait bundles elaborating to
PROOF-KERNEL-CORE terms; `wiki/spec/proofs/mathematical_bindings.md:8` already
records the settled direction — "No `forall`, `exists`, `claim`, `proposition`,
or proposition-returning machines remain optional, unaccepted naming
proposals." This note inventories every surface the retirement must drain. It
authorizes no new gate and changes no fence. Delete this inventory once
PROOF-CONTRACT-MIGRATION drains every surface listed here and no `proposition`
declaration remains.

## 1. Source grammar

`01_tokens-to-syntax-trees/src/declarations.rs`
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

## 2. Library carriers

`proposition` declarations in the bundled core library:

- `source/library/core/assembly_model.omg` (3)
- `source/library/core/int.omg` (1)

## 3. Spec surface

- `wiki/language_guide/chapter_10_compile_time_proofs.md` — the user-facing
  `proposition` declaration semantics (propositions mention linear values
  without custody; a containing proposition cannot justify its own
  formation; partial applications stay propositions).
- `wiki/spec/proofs/foundation.md` — kernel-side `Strict`/`Squash`
  proposition vocabulary (`P : Strict v`, proof irrelevance).
- `wiki/spec/proofs/mathematical_bindings.md` — settled exclusion: no
  `proposition` keyword survives the migration.
- `wiki/spec/proofs/inductive_profile.md:138` — propositional laws change a
  contract and themselves need proof.

## 4. Retirement edge

PROOF-KERNEL-CORE owns the replacement: formula
declarations and hidden-witness calls migrate to ordinary contracts and
named witness/law bundles, preserving substitution, result/path
availability, erasure, validity, and transitive assumptions across trait
calls and artifacts.
