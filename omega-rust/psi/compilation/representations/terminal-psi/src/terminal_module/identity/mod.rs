//! Semantic identities and the closed vocabulary marker.

mod declarations;
mod semantic;
mod table;
mod vocabulary;

pub use declarations::{
    CheckedMachineCapture, CheckedMachineDeclaration, CheckedMachineSpecialization,
    NominalDeclarationIdentity,
};
pub use semantic::{SemanticFingerprint, TerminalPsiIdentity};
pub use table::{
    InvocationCodeScope, TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder,
};
pub use vocabulary::VocabularyMarker;
