//! Publication joins field observations to their exact ordinary graph operations.
//! Mandatory object/source replay derives layout and validates the selected load,
//! including its original referent. No second field-offset calculator or legacy
//! expression-tree realization belongs at this publication boundary.

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
        } => {
            if !function.structural_parameters.contains(source) {
                return false;
            }
            (*psi_operation, *result, source.place, *field)
        }
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
    let mut parameters = function
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.place == place);
    let Some(parameter) = parameters.next() else {
        return false;
    };
    if parameters.next().is_some() {
        return false;
    }
    let expected_source = StructuralArgument {
        place,
        access: parameter.access,
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
