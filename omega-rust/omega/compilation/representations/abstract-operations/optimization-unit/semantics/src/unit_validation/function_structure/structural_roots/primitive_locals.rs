//! Independent primitive referent admission in the current optimization IR.

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralOperationResult};

use crate::OptimizationUnitValidationError;

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let claim_free = |place| {
        function
            .entry_claim_declarations
            .iter()
            .all(|claim| claim.input != place)
            && function
                .content_entry_claims
                .iter()
                .all(|claim| claim.input.root != place)
    };
    let primitive = |structural_type, scalar_type| {
        types.get(&structural_type).is_some_and(|declaration| {
            matches!(declaration.shape, terminal_psi::StructuralTypeShape::PrimitiveScalar(actual)
                if actual == scalar_type)
        })
    };
    let local_matches = |result: &StructuralOperationResult, scalar_type| {
        result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.projected_qualifications.is_empty()
            && result.qualification_establishments.is_empty()
            && result.claims.is_empty()
            && claim_free(result.place)
            && primitive(result.structural_type, scalar_type)
    };
    let local = |place| {
        function
            .block_nodes()
            .find_map(|node| match &node.operation {
                O::EstablishPrimitiveLocal { result, .. } if result.place == place => Some(result),
                _ => None,
            })
    };
    let valid = match operation {
        O::EstablishPrimitiveLocal {
            psi_operation,
            result,
            value,
            ..
        } => {
            local_matches(result, value.scalar_type)
                && function.structural_places.iter().any(|place| {
                    place.id == result.place
                        && place.kind
                            == StructuralPlaceKind::OperationResult {
                                producer: *psi_operation,
                                structural_type: result.structural_type,
                            }
                })
        }
        O::PrimitiveLocalStore {
            destination, value, ..
        } => local(*destination).is_some_and(|result| {
            // This retained operation has no receiving obligation. A home's
            // valid initialization cannot authorize a later restricted write;
            // those writes must carry the common store's exact range evidence.
            local_matches(result, value.scalar_type)
                && result.qualifications.is_empty()
                && types
                    .get(&result.structural_type)
                    .is_some_and(|declaration| declaration.scalar_restriction.is_none())
        }),
        O::PrimitiveScalarRead {
            source,
            path,
            result,
            ..
        } => {
            (path.is_empty()
                && local(*source).is_some_and(|local| local_matches(local, result.scalar_type)))
                || (!path.is_empty()
                    && claim_free(*source)
                    && projected_local(function, *source).is_some_and(|local| {
                        terminal_semantics::primitive_read_projection_type(
                            types.values().copied(),
                            local.structural_type,
                            path,
                        ) == Some(result.scalar_type)
                    }))
                || function
                    .structural_parameters
                    .iter()
                    .chain(function.block_structural_parameters())
                    // Reads depend on the declared referent and access, not
                    // whether native transport uses an address or descriptor.
                    // The ownership walk independently checks availability.
                    .any(|parameter| {
                        parameter.place == *source
                            && (matches!(
                                parameter.access,terminal_psi::reading_borrow_pair_pattern!()) || (parameter.access == StructuralAccess::Owned
                                && !path.is_empty()))
                            && parameter.admits_scalar_read_at(path.is_empty())
                            && claim_free(*source)
                            // A read may end at a scalar field of a
                            // runtime-selected record element; stores keep
                            // the primitive-leaf rule.
                            && terminal_semantics::primitive_read_projection_type(
                                types.values().copied(),
                                parameter.structural_type,
                                path,
                            ) == Some(result.scalar_type)
                    })
        }
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(OptimizationUnitValidationError::InvalidPrimitiveLocal {
            machine: function.machine,
            block,
            node,
        })
    }
}

/// Reconstruct the current producer, rather than trusting a copied source
/// declaration or treating a local home as an incoming parameter. Availability
/// and loan checking remain with the common structural ownership walk.
fn projected_local(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Option<&StructuralOperationResult> {
    let declaration = function.structural_place(source)?;
    let (producer, structural_type) = declaration.kind.as_operation_result()?;
    function.block_nodes().find_map(|node| {
        let (operation, result) = node.operation.structural_result()?;
        (operation == producer
            && result.place == source
            && result.structural_type == structural_type
            && matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty())
        .then_some(result)
    })
}
