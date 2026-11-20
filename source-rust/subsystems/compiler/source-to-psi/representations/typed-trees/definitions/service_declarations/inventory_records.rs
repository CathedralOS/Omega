//! Source declarations retained by a typed provider inventory.
use symbols::SymbolHandle;

pub use symbols::trust_declarations::{
    AcceptedContractDeclaration, AcceptedMachineDeclaration, AcceptedTemplateDeclaration,
    TrustDeclarationCardinality, TrustDeclarations,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTargetMachineOrigin {
    pub machine: SymbolHandle,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRequirementApplication {
    pub requirement: String,
    pub receiving_application: String,
}
