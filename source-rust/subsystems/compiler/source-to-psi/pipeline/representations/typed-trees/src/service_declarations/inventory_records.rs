//! Source declarations retained by a typed provider inventory.
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustDeclarationCardinality {
    Missing,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedTemplateDeclaration {
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedContractDeclaration {
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedMachineDeclaration {
    pub machine: SymbolHandle,
    pub declaration_identity: String,
    pub package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub name: String,
    pub template: Result<Option<AcceptedTemplateDeclaration>, TrustDeclarationCardinality>,
    pub contract: Result<AcceptedContractDeclaration, TrustDeclarationCardinality>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustDeclarations {
    pub machines: Box<[AcceptedMachineDeclaration]>,
}

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
