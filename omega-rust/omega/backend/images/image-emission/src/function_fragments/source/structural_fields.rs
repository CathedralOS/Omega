//! Publication joins field observations to their exact ordinary graph operations.
//! Mandatory object/source replay derives layout and validates the selected load,
//! including the original referent or owned input's current local home. No second
//! field-offset calculator or legacy expression-tree realization belongs here.

use abstract_operations::{AbstractFunction, AbstractOperation, AbstractResult};
use semantic_vocabulary::ScalarType;
use target_operations::{TargetFunction, TargetUnitOperation};
use terminal_psi::StructuralArgument;

#[cfg(test)]
mod tests;

pub(super) fn retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    let (identity, result, place, field) = match operation {
        AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => (*psi_operation, *result, *source, *field),
        AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => (
            *psi_operation,
            AbstractResult {
                value: *result,
                scalar_type: ScalarType::Boolean,
            },
            *source,
            *field,
        ),
        _ => return false,
    };
    let Some(access) = read_access(function, target, place) else {
        return false;
    };
    let expected_source = StructuralArgument {
        place,
        access,
        path: Vec::new(),
    };
    let mut reads = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation {
            TargetUnitOperation::StructuralScalarFieldRead {
                psi_operation,
                result,
                source,
                field,
            } if *psi_operation == identity => Some((result, source, field)),
            _ => None,
        });
    matches!(reads.next(), Some((actual_result, source, actual_field))
        if *actual_result == result && *source == expected_source && *actual_field == field)
        && reads.next().is_none()
}

/// Rejoin original storage declarations without manufacturing a parameter for
/// an operation result. Mandatory graph replay checks pointwise availability.
fn read_access(
    function: &AbstractFunction,
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<terminal_psi::StructuralAccess> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let mut access = None;
    for (position, parameter) in function
        .structural_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.place == place)
    {
        let retained = target.graph.parameters.get(position)?;
        if parameter.position as usize != position
            || !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
            )
            || parameter.multiplicity == StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || retained.place != place
            || retained.structural_type != parameter.structural_type
            || retained.access != parameter.access
            || retained.multiplicity != parameter.multiplicity
            || retained.projected_qualifications != parameter.projected_qualifications
            || access.replace(parameter.access).is_some()
        {
            return None;
        }
    }
    for block in &function.block_entries {
        for parameter in block
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.place == place)
        {
            if parameter.access != StructuralAccess::Owned
                || parameter.multiplicity == StructuralMultiplicity::Linear
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || !target.graph.blocks.iter().any(|candidate| {
                    candidate.block == block.block
                        && candidate
                            .structural_parameters
                            .iter()
                            .any(|retained| retained == parameter)
                })
                || access.replace(parameter.access).is_some()
            {
                return None;
            }
        }
    }
    for operation in &function.operations {
        let (producer, result) = match operation {
            AbstractOperation::EstablishRecord {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                result,
                ..
            } if result.place == place => (*psi_operation, result),
            _ => continue,
        };
        if result.multiplicity == StructuralMultiplicity::Linear
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !target
                .graph
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    let home = match operation {
                        TargetUnitOperation::EstablishRecord { result_home, .. }
                        | TargetUnitOperation::StructuralResultCall {
                            result_home: Some(result_home),
                            ..
                        } => result_home,
                        _ => return false,
                    };
                    home.operation_result()
                        .is_some_and(|(actual, declaration)| {
                            actual == producer && declaration == result
                        })
                })
            || access.replace(StructuralAccess::Owned).is_some()
        {
            return None;
        }
    }
    access
}
