use crate::{
    StructuralAccess, StructuralDomainRequirement, StructuralMultiplicity,
    StructuralPathQualification,
};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
};

/// One exact checked provider candidate for a Unit boundary requirement.
///
/// The candidate body is an ordinary terminal machine. The extra row binds it
/// to the requirement and records the structured signature/refinement witness
/// independently checked by the terminal verifier. A readable method spelling
/// is deliberately absent; `requirement_identity` is the canonical overload
/// identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderCandidateConformance {
    pub boundary: BoundaryMachineId,
    pub requirement_identity: String,
    pub provider_identity: String,
    /// Canonical checked-machine identity named by the selected
    /// `CheckedAdapter` row. The dense `candidate` ID is artifact-local.
    pub candidate_identity: String,
    pub candidate: MachineId,
    pub signature: ProviderUnitSignature,
    pub refinement: ProviderUnitRefinement,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderUnitSignature {
    pub parameters: Vec<ProviderSignatureParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderSignatureParameter {
    pub position: u32,
    pub is_self: bool,
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
pub struct ProviderUnitRefinement {
    /// Complete dense positional correspondence between requirement and
    /// candidate parameters. Reordering cannot hide behind equal types.
    pub positional_parameters: Vec<ProviderParameterRefinement>,
    /// Exact boundary-domain premises inherited by the candidate.
    pub required_domains: Vec<StructuralDomainRequirement>,
    /// Exact checked candidate reach, proved to refine the boundary ceiling.
    pub realized_service_ceiling: Vec<ServiceId>,
}
