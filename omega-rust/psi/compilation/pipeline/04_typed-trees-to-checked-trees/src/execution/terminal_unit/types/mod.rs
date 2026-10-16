//! Structural type, shape, claim, and return-custody helpers.
use super::{
    BTreeMap, BTreeSet, CheckFacts, CheckedBoundaryMachineResultPlan, CheckedStructuralAccess,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, ContractProofFactKind,
    ContractProofFactOwner, DataMember, DataShapeKind, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PrimitiveType, ProofFact, SemanticDomainId, SignatureContractKind,
    StateParameter, SymbolHandle, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{
    CheckedExtent, CheckedStructuralPathQualification, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::type_identity::TypeIdentityRequest;

mod domain_predicates;
pub(super) use domain_predicates::retain_domain_predicates;
mod scalar_fields;
pub(crate) use scalar_fields::field_requires_place_observation;
pub(super) use scalar_fields::{retain_scalar_field, supports_primitive_storage};

mod affine_discards;
mod identities;
mod parameters;
mod shape_collector;
mod signature_contracts;
mod views;

pub(in crate::execution::terminal_unit) use affine_discards::*;
pub(crate) use identities::*;
pub(crate) use parameters::*;
pub(crate) use shape_collector::*;
pub(in crate::execution::terminal_unit) use signature_contracts::*;
pub(crate) use views::*;

/// The boundary-trait symbol a field spells as `&'a mut <boundary trait>`:
/// unwrap qualifications and mutable borrows to the named or trait symbol.
/// A shared `&` borrow is not a provider handle.
pub(crate) fn mutable_boundary_trait_symbol(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let mut unwrapped = type_reference;
    loop {
        match program.type_reference_table.type_reference(unwrapped) {
            TypeReferenceNode::Constrained {
                base_type: inner, ..
            } => unwrapped = *inner,
            TypeReferenceNode::Reference {
                referee: inner,
                access: language_core::ReferenceAccess::Mutable,
                ..
            } => unwrapped = *inner,
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. } => break Some(*symbol),
            _ => break None,
        }
    }
}
