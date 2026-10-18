//! Boundary machine plans: the boundary machine, its result and its
//! provider attachment requirements.

use crate::checked_trees::CheckedMemberIdentity;
use crate::checked_trees::flow::terminal::{
    CheckedStructuralScalarParameterPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralParameterPlan,
};
use language_semantics::{Multiplicity, SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::PrimitiveType;

/// Exact result role of one bodyless boundary declaration. Unit, primitive,
/// and structural results are distinct identities; an absent result is never
/// reused to mean an empty structural result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBoundaryMachineResultPlan {
    Unit,
    Scalar(PrimitiveType),
    Structural {
        type_identity: TypeIdentityId,
        multiplicity: Multiplicity,
        qualifications: Vec<SemanticDomainId>,
        /// Declared descendant memberships, distinct from root domains and
        /// stronger facts private to a selected implementation.
        projected_qualifications: Vec<super::CheckedStructuralPathQualification>,
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

    pub fn structural_identity(&self) -> Option<TypeIdentityId> {
        match self {
            Self::Structural { type_identity, .. } => Some(*type_identity),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProviderAttachmentRequirementPlan {
    pub field_identity: CheckedMemberIdentity,
    pub provider_type_identity: TypeIdentityId,
    pub boundary: SymbolHandle,
}

/// A normal-return fact in the boundary's input/result scalar namespace.
/// Conditional consequences retain their premise as proof structure rather
/// than converting it into executable Boolean control or losing it in a union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBoundaryScalarGuarantee {
    Predicate(crate::CheckedBooleanExpression),
    Implication {
        premise: crate::CheckedBooleanExpression,
        conclusion: crate::CheckedBooleanExpression,
    },
}

/// Normal completion promises a reference, not a spatial relation to an input.
/// Parameter positions here are authored positions, before lane packing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryReferenceOutput {
    pub destination: CheckedBoundaryReferenceDestination,
    pub path: Vec<super::CheckedUnitStructuralPathSegment>,
    pub access: language_semantics::ReferenceAccess,
    pub dependencies: Vec<CheckedBoundaryLifetimeDependency>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedBoundaryReferenceDestination {
    Parameter(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBoundaryLifetimeDependency {
    Referent {
        parameter_index: u32,
        access: language_semantics::ReferenceAccess,
    },
    Stored {
        parameter_index: u32,
        path: Vec<super::CheckedUnitStructuralPathSegment>,
        access: language_semantics::ReferenceAccess,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryMachinePlan {
    pub kind: semantic_vocabulary::BoundaryRequirementKind,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// Exact owner of the canonical contract carrier. For an attached
    /// boundary declaration this is `machine`; for a boundary-trait
    /// requirement it is the declaring trait that owns the crash capsule.
    pub contract_owner: SymbolHandle,
    /// Present for a bodyless attached boundary declaration; absent for a
    /// static boundary-trait requirement, which has no runtime provider value.
    pub attachment_type_identity: Option<TypeIdentityId>,
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
    pub scalar_requires: Vec<crate::ClosedScalarContractValue>,
    /// Retained normal-return scalar guarantees. The result follows all inputs
    /// in the same dense scalar namespace.
    pub scalar_ensures: Vec<CheckedBoundaryScalarGuarantee>,
    pub reference_outputs: Vec<CheckedBoundaryReferenceOutput>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
}

impl CheckedBoundaryMachinePlan {
    /// Borrow the declared callable without inventing an executable body or
    /// selecting the provider that will eventually realize this requirement.
    pub fn callable_entry(&self) -> super::CheckedUnitCallableEntry<'_> {
        super::CheckedUnitCallableEntry {
            machine: self.machine,
            state: self.state,
            returns_unit: self.result.is_unit(),
            attachment_type_identity: self.attachment_type_identity,
            structural_parameters: &self.structural_parameters,
            scalar_parameters: &self.scalar_parameters,
            erased_scalar_parameters: &[],
            erased_proof_parameters: &[],
            entry_claims: &[],
            contract_report_fingerprint: self.contract_report_fingerprint,
            contract_service_reach: self.contract_service_reach,
        }
    }
}
