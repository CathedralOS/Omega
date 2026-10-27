use crate::current_ownership::CurrentOwnership;
use crate::current_ownership::residuals::partial_affine_residuals;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

pub(super) fn place_structural_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    function
        .structural_parameters
        .iter()
        .chain(function.block_structural_parameters())
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            function
                .result
                .structural()
                .filter(|result| result.place == place)
                .map(|result| result.structural_type)
        })
        .or_else(|| {
            function
                .structural_places
                .iter()
                .find_map(|candidate| (candidate.id == place).then_some(candidate.kind))
                .and_then(|kind| match kind {
                    semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        structural_type,
                        ..
                    }
                    | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type,
                        ..
                    }
                    | semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                        structural_type,
                        ..
                    } => Some(structural_type),
                    semantic_vocabulary::StructuralPlaceKind::ProviderAttachment {
                        attachment,
                        ..
                    } => Some(attachment),
                    semantic_vocabulary::StructuralPlaceKind::Parameter { .. }
                    | semantic_vocabulary::StructuralPlaceKind::BlockParameter { .. }
                    | semantic_vocabulary::StructuralPlaceKind::Result => None,
                })
        })
}

/// Cleanup custody requires an actual owner, not just a typed place declaration.
pub(crate) fn partial_affine_root_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if function.entry_claim_declared(place) || function.content_entry_claim_declared(place) {
        return None;
    }
    if let Some(parameter) = function.arrival_parameter(place) {
        return (parameter.is_unqualified_owned_affine()).then_some(parameter.structural_type);
    }
    let semantic_vocabulary::StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = function.structural_place(place)?.kind
    else {
        return None;
    };
    function
        .block_nodes()
        .find_map(|node| match &node.operation {
            O::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | O::BoundaryCall {
                psi_operation,
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            }
            | O::EstablishStructuralArray {
                psi_operation,
                result,
                ..
            }
            | abstract_operations::establish_record_or_case_pattern!(psi_operation, result, ..)
                if *psi_operation == producer
                    && result.place == place
                    && result.structural_type == structural_type
                    && result.multiplicity == StructuralMultiplicity::Affine
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty() =>
            {
                Some(structural_type)
            }
            _ => None,
        })
}

/// Whether projected moves have taken every affine child of an owned
/// partial-affine root, leaving no residual complement for cleanup.
pub(crate) fn partial_affine_root_is_drained(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    place: PlaceId,
    moved: &BTreeSet<Vec<StructuralPathSegment>>,
) -> bool {
    partial_affine_root_type(function, place).is_some_and(|root_type| {
        partial_affine_residuals(structural_types, root_type, moved, 0)
            .is_some_and(|residuals| residuals.is_empty())
    })
}

pub(super) fn projected_root_is_fully_consumed(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    frontier: &CurrentOwnership,
    place: PlaceId,
) -> bool {
    if frontier
        .claims
        .values()
        .any(|claim| claim.input == Some(place))
        || function.content_entry_claim_declared(place)
    {
        return false;
    }
    let Some(moved) = frontier.partial_custody_paths.get(&place) else {
        return false;
    };
    projected_paths_are_fully_consumed(function, structural_types, place, moved)
}

/// Authenticate zero disposal debt from the retained exact moved paths. Live
/// claim and carrier absence is checked by the consuming frontier join.
pub(crate) fn projected_paths_are_fully_consumed(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    place: PlaceId,
    moved: &BTreeSet<Vec<StructuralPathSegment>>,
) -> bool {
    if function.content_entry_claim_declared(place) {
        return false;
    }
    if partial_affine_root_type(function, place).is_some() {
        return partial_affine_root_is_drained(function, structural_types, place, moved);
    }
    let Some(parameter) = function.structural_parameter(place) else {
        return false;
    };
    if parameter.multiplicity != StructuralMultiplicity::Linear {
        return false;
    }
    let Some(StructuralTypeShape::FixedArray { length, .. }) = structural_types
        .get(&parameter.structural_type)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(length) = usize::try_from(*length).ok() else {
        return false;
    };
    moved.len() == length
        && (0..length).all(|index| {
            moved.contains(&vec![StructuralPathSegment::FixedIndex(
                u64::try_from(index).expect("a usize index fits u64"),
            )])
        })
}

/// Both aggregate constructors bind the same finished structural operands.
pub(crate) fn constructor_arguments(
    operation: &O,
) -> impl Iterator<Item = &terminal_psi::StructuralArgument> {
    let fields = match operation {
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => fields.as_slice(),
        _ => &[],
    };
    let elements = match operation {
        O::EstablishStructuralArray { elements, .. } => elements.as_slice(),
        _ => &[],
    };
    fields
        .iter()
        .filter_map(|initializer| match &initializer.value {
            terminal_psi::RecordFieldValue::Structural(argument) => Some(argument),
            terminal_psi::RecordFieldValue::Scalar { .. } => None,
        })
        .chain(elements)
}
