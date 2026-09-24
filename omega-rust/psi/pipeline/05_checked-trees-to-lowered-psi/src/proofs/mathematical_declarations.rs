//! Mathematical declaration admission at lowering.
//!
//! Checked `let`/`boundary let` mathematical declarations land on
//! `ProofFacts::mathematical_declarations` (`PROOF-CONTRACT-MIGRATION`). No
//! Terminal evidence encoding carries them yet — `terminal-codec`'s
//! mathematical certificate wire exists but has no production caller — so the
//! admission verdict refuses loudly rather than emit a module that silently
//! omits the declaration. Once the encoding lands this gate becomes the
//! producer that lowers the checked signature onto the module.

use checked_trees::CheckedTrees;

use crate::lowering_error::{LoweringError, unsupported};

/// Admit the program's checked mathematical declarations for lowering,
/// refusing the module while no Terminal evidence encoding carries them.
///
/// The verdict is program-level: every public lowering entrance consults it
/// before per-machine work so a declaration-carrying program can never
/// publish an artifact that drops them.
pub(crate) fn admit_mathematical_declarations(checked: &CheckedTrees) -> Result<(), LoweringError> {
    if checked.facts.proof.mathematical_declarations.is_empty() {
        return Ok(());
    }
    unsupported(
        "mathematical `let`/`boundary let` declarations are not admitted for lowering: \
         no Terminal evidence encoding carries them yet (PROOF-CONTRACT-MIGRATION)",
    )
}
