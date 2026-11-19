use crate::terminal_module::TerminalIdentityIndex;
use crate::{
    StructuralAccess, StructuralDomainRequirement, StructuralMultiplicity,
    StructuralPathQualification,
};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInstanceRequirements {
    pub template: TerminalIdentityIndex,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub selecting_declaration: Option<crate::NominalDeclarationIdentity>,
    pub source_candidate_identity: TerminalIdentityIndex,
    pub required_instances: Vec<RequiredProviderInstance>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequiredProviderInstance {
    pub requirement_identity: TerminalIdentityIndex,
    pub candidate_identity: TerminalIdentityIndex,
    pub declaration_application: TerminalIdentityIndex,
    pub receiving_application: TerminalIdentityIndex,
    pub receiving_region_arguments: Vec<TerminalIdentityIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderCandidateConformance {
    pub boundary: BoundaryMachineId,
    pub requirement_identity: TerminalIdentityIndex,
    pub source_requirement_identity: TerminalIdentityIndex,
    pub declaration_application: TerminalIdentityIndex,
    pub receiving_application: TerminalIdentityIndex,
    pub receiving_region_arguments: Vec<TerminalIdentityIndex>,
    pub provider_identity: TerminalIdentityIndex,
    pub provider_declaration: crate::NominalDeclarationIdentity,
    pub candidate_identity: TerminalIdentityIndex,
    pub source_candidate_identity: TerminalIdentityIndex,
    pub source_candidate_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub candidate: MachineId,
    pub signature: ProviderSignature,
    pub refinement: ProviderRefinement,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderSignature {
    pub parameters: Vec<ProviderSignatureParameter>,
}

impl ProviderSignature {
    pub fn empty() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderSignatureParameter {
    pub position: u32,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    pub qualifications: Vec<StructuralDomainId>,
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderParameterRefinement {
    pub boundary_index: u32,
    pub candidate_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderRefinement {
    pub positional_parameters: Vec<ProviderParameterRefinement>,
    pub required_domains: Vec<StructuralDomainRequirement>,
    pub realized_service_ceiling: Vec<ServiceId>,
}
impl ProviderRefinement {
    pub fn empty() -> Self {
        Self {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        }
    }
}
