use crate::terminal_module::TerminalIdentityIndex;
use crate::{
    StructuralAccess, StructuralDomainRequirement, StructuralMultiplicity,
    StructuralPathQualification,
};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
};

/// One exact checked provider candidate for a boundary requirement.
///
/// The candidate body is an ordinary terminal machine. The extra row binds it
/// to the requirement and records the structured signature/refinement witness
/// independently checked by the terminal verifier. A readable method spelling
/// is deliberately absent; `requirement_identity` is the canonical overload
/// identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderCandidateConformance {
    pub boundary: BoundaryMachineId,
    pub requirement_identity: TerminalIdentityIndex,
    /// Original checked callable coordinate selected by a provider-plan row.
    /// Equal to `requirement_identity` when no application closed it. This
    /// association belongs to the canonical catalog, not a public proposal:
    /// receivers enumerate the complete instance roster without source replay.
    pub source_requirement_identity: TerminalIdentityIndex,
    pub provider_identity: TerminalIdentityIndex,
    /// Exact package/toolchain-owned selecting declaration, independently of
    /// the callable it realizes. Named maps may share the same body.
    pub provider_declaration: crate::NominalDeclarationIdentity,
    /// Canonical checked-machine identity named by the selected
    /// `CheckedAdapter` row. The dense `candidate` ID is artifact-local.
    pub candidate_identity: TerminalIdentityIndex,
    /// The immediate checked specialization template's callable identity,
    /// or `candidate_identity` for an unspecialized body. Outer applications
    /// already present on that template remain part of this exact coordinate.
    pub source_candidate_identity: TerminalIdentityIndex,
    /// Package custody of that immediate source callable. Module-qualified
    /// overload text alone cannot establish package equality. `None` retains
    /// the existing toolchain/standalone coordinate, not a wildcard.
    pub source_candidate_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub candidate: MachineId,
    pub signature: ProviderSignature,
    pub refinement: ProviderRefinement,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderSignature {
    /// Structural parameter correspondence. Scalar parameters and the result
    /// are joined directly between the boundary and candidate declarations.
    pub parameters: Vec<ProviderSignatureParameter>,
}

impl ProviderSignature {
    /// A signature with no structural parameter correspondence.
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
    /// Complete dense positional correspondence between requirement and
    /// candidate parameters. Reordering cannot hide behind equal types.
    pub positional_parameters: Vec<ProviderParameterRefinement>,
    /// Exact boundary-domain premises inherited by the candidate.
    pub required_domains: Vec<StructuralDomainRequirement>,
    /// Exact checked candidate reach, proved to refine the boundary ceiling.
    pub realized_service_ceiling: Vec<ServiceId>,
}
impl ProviderRefinement {
    /// A refinement with no positional correspondence, inherited domains, or
    /// realized ceiling.
    pub fn empty() -> Self {
        Self {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        }
    }
}
