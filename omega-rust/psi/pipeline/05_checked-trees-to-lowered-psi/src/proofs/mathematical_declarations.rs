//! Mathematical declaration admission at lowering.
//!
//! Checked `let`/`boundary let` mathematical declarations land on
//! `ProofFacts::mathematical_declarations` beside the checked kernel
//! signature on `ProofFacts::mathematical_signature`
//! (`PROOF-CONTRACT-MIGRATION`). The signature reaches Terminal evidence
//! through the artifact envelope's signature section — terminal production
//! encodes it onto the canonical `PSISIG` wire at publication, where the
//! receiver's kernel re-decides it — so this gate no longer refuses a
//! declaration-carrying program. What it still refuses is an inconsistent
//! facts record: declarations without their checked signature (or a
//! signature without declarations) would publish an artifact that silently
//! omits the evidence, so the verdict stays loud.

use checked_trees::CheckedTrees;

use crate::lowering_error::{LoweringError, unsupported};

/// Admit the program's checked mathematical declarations for lowering,
/// requiring the checked signature evidence to accompany them.
///
/// The verdict is program-level: every public lowering entrance consults it
/// before per-machine work so a declaration-carrying program whose checked
/// facts lost the signature can never publish an artifact that drops it.
pub(crate) fn admit_mathematical_declarations(checked: &CheckedTrees) -> Result<(), LoweringError> {
    let declarations_present = !checked.facts.proof.mathematical_declarations.is_empty();
    let signature_present = checked.facts.proof.mathematical_signature.is_some();
    match (declarations_present, signature_present) {
        (true, true) | (false, false) => Ok(()),
        (true, false) => unsupported(
            "mathematical `let`/`boundary let` declarations lack their checked \
             signature evidence (PROOF-CONTRACT-MIGRATION)",
        ),
        (false, true) => unsupported(
            "checked mathematical signature evidence without declarations \
             (PROOF-CONTRACT-MIGRATION)",
        ),
    }
}
