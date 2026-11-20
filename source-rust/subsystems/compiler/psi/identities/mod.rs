//! Interned program and declaration identities, the vocabulary marker and semantic identity.

pub(crate) mod declarations;
pub(crate) mod table;
pub use declarations::{
    CheckedMachineCapture, CheckedMachineDeclaration, CheckedMachineSpecialization,
    NominalDeclarationIdentity,
};
pub use psi_program_identity::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
pub use table::{
    InvocationCodeScope, TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder,
};
