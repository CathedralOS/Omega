//! Interned program and declaration identities, the vocabulary marker and semantic identity.

pub(crate) mod declarations;
mod private_inputs;
mod private_results;
pub(crate) mod table;
pub use declarations::{
    CheckedMachineCapture, CheckedMachineDeclaration, CheckedMachineSpecialization,
    NominalDeclarationIdentity,
};
pub use private_inputs::InvocationPrivateInputs;
pub use private_results::InvocationPrivateResult;
pub use psi_program_identity::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
pub use table::{
    InvocationCodeScope, TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder,
};
