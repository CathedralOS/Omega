//! Semantic identities and the closed vocabulary marker.

mod declarations;
mod semantic;
mod table;
mod vocabulary;

pub use declarations::{
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
};
pub use semantic::{SemanticFingerprint, TerminalPsiIdentity};
pub use table::{
    TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder,
    TerminalLifetimeDeclaration, TerminalLifetimeFormalRole,
};
pub use vocabulary::VocabularyMarker;
