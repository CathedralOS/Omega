//! Semantic identities and the closed vocabulary marker.

mod declarations;
mod semantic;
mod vocabulary;

pub use declarations::{
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
};
pub use semantic::{SemanticFingerprint, TerminalPsiIdentity};
pub use vocabulary::VocabularyMarker;
