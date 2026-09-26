//! Boundary machine plans: the boundary machine, its result and its
//! provider attachment requirements.

use crate::checked_trees::checked_trees::flow::terminal::{
    CheckedStructuralScalarParameterPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralParameterPlan,
};
use language_semantics::{Multiplicity, SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
use symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType;
use symbols::SymbolHandle;

/// Exact result role of one bodyless boundary declaration. Unit, primitive,
/// and structural results are distinct identities; an absent result is never
/// reused to mean an empty structural result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBoundaryMachineResultPlan {
    Unit,
    Scalar(PrimitiveType),
    Structural {
        type_identity: String,
        multiplicity: Multiplicity,
        qualifications: Vec<SemanticDomainId>,
    },
}

impl CheckedBoundaryMachineResultPlan {
    pub const fn scalar(&self) -> Option<PrimitiveType> {
        match self {
            Self::Scalar(scalar) => Some(*scalar),
            Self::Unit | Self::Structural { .. } => None,
        }
    }

    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub fn structural_identity(&self) -> Option<&str> {
        match self {
            Self::Structural { type_identity, .. } => Some(type_identity),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProviderAttachmentRequirementPlan {
    pub field_identity: String,
    pub provider_type_identity: String,
    pub boundary: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// Exact owner of the canonical contract carrier. For an attached
    /// boundary declaration this is `machine`; for a boundary-trait
    /// requirement it is the declaring trait that owns the crash capsule.
    pub contract_owner: SymbolHandle,
    /// Present for a bodyless attached boundary declaration; absent for a
    /// static boundary-trait requirement, which has no runtime provider value.
    pub attachment_type_identity: Option<String>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Primitive parameters in authored order after removing structural
    /// parameters into their independent custody namespace.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub result: CheckedBoundaryMachineResultPlan,
    /// Canonical `(argument_index, domain)` order derived from exact normalized
    /// membership facts in the boundary contract.
    pub domain_requirements: Vec<CheckedUnitStructuralDomainRequirementPlan>,
    /// The contract's scalar `requires` predicates in checked order: each
    /// authored clause that is not wholly structural membership, then the
    /// scalar parameter ranges. `Parameter` positions index
    /// `scalar_parameters`. Callers owe one obligation per lowered row.
    pub scalar_requires: Vec<crate::checked_trees::ClosedScalarContractValue>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::checked_trees::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
}
