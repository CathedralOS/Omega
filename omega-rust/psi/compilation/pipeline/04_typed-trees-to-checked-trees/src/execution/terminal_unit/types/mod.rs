//! Structural type, shape, claim, and return-custody helpers.
use super::{
    BTreeMap, BTreeSet, CheckFacts, CheckedBoundaryMachineResultPlan, CheckedStructuralAccess,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, ContractProofFactKind,
    ContractProofFactOwner, DataMember, DataShapeKind, ExpressionNode, Multiplicity,
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PrimitiveType, ProofFact, SemanticDomainId, SignatureContractKind, StateParameter,
    SymbolHandle, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
};
use checked_trees::{
    CheckedStructuralPathQualification, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::type_identity::TypeIdentityRequest;

mod scalar_fields;
pub(super) use scalar_fields::retain_scalar_field;

#[cfg(test)]
mod partial_affine_ownership_tests;

#[cfg(test)]
mod arithmetic_policy_array_tests;

pub(super) use validation::has_plain_owned_contents;

mod affine_discards;
mod identities;
mod parameters;
mod shape_collector;
mod signature_contracts;
mod views;

pub(crate) use affine_discards::*;
pub(crate) use identities::*;
pub(crate) use parameters::*;
pub(crate) use shape_collector::*;
pub(in crate::execution::terminal_unit) use signature_contracts::*;
pub(crate) use views::*;
