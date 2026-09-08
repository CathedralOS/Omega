//! Independent primitive referent admission in the current optimization IR.

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, StructuralPlaceKind, StructuralTypeId};
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
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && claim_free(result.place)
            && primitive(result.structural_type, scalar_type)
    };
    let local = |place| {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
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
        } => local(*destination).is_some_and(|result| local_matches(result, value.scalar_type)),
        O::PrimitiveScalarRead { source, result, .. } => {
            local(*source).is_some_and(|local| local_matches(local, result.scalar_type))
                || function.structural_parameters.iter().any(|parameter| {
                    parameter.place == *source
                        && matches!(
                            parameter.access,
                            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                        )
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty()
                        && claim_free(*source)
                        && primitive(parameter.structural_type, result.scalar_type)
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
