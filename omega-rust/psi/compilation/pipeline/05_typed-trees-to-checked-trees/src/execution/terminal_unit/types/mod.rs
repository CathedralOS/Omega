//! Structural type, shape, claim, and return-custody helpers.
use super::{
    BTreeMap, BTreeSet, CheckFacts, CheckedBoundaryMachineResultPlan, CheckedStructuralAccess,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, DataMember, DataShapeKind,
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PrimitiveType, ProofFact,
    SemanticDomainId, SignatureContractKind, StateParameter, SymbolHandle, TypeConstraintNode,
    TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{
    CheckedExtent, CheckedStructuralPathQualification, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::type_identity::TypeIdentityRequest;

mod call_qualifications;
pub(super) use call_qualifications::retain_call_qualification_domains;
mod declared_result;
pub(super) use declared_result::{declared_result_plan, declared_result_type};
mod domain_predicates;
pub(super) use domain_predicates::retain_domain_predicates;
mod scalar_fields;
pub(crate) use scalar_fields::field_requires_place_observation;
pub(super) use scalar_fields::{retain_scalar_field, supports_primitive_storage};

mod affine_discards;
mod boundary_domains;
mod identities;
mod layout_observations;
pub(super) use layout_observations::retain_layout_observation_types;
mod parameters;
mod shape_collector;
mod signature_contracts;
pub(super) use boundary_domains::boundary_domain_requirements;
mod views;

pub(super) use affine_discards::*;
pub(crate) use identities::*;
pub(crate) use parameters::*;
pub(crate) use shape_collector::*;
pub(super) use signature_contracts::*;
pub(crate) use views::*;

