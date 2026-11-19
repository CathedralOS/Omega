/*
Interned program identities (`TerminalIdentityTable`, `TerminalIdentityIndex`), nominal declaration
identities, semantic fingerprints (`SemanticFingerprint`, `TerminalPsiIdentity`), and artifact
manifest metadata (`TerminalArtifactManifest`, `OptimizationReceipt`) in `TerminalModule`.
*/

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
