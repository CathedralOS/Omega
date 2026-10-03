//! Element-view coordinates keep u64 counts; element reads carry whatever
//! scalar type the view's element declaration selected, so the result check
//! is definitional agreement, not a fixed width.
use abstract_operations::AbstractOperation as O;
use optimization_unit::{PsiOptimizationFunction, ValueDefinition};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
use std::collections::BTreeMap;

pub(super) fn types_match(
    function: &PsiOptimizationFunction,
    operation: &O,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
) -> bool {
    // Result contracts apply at production, including dead descriptors: no
    // later read or cleanup may be needed to expose invalid loan metadata.
    if matches!(operation, O::EstablishElementView { .. } | O::ElementViewSubslice { .. })
        && crate::unit_validation::operation_contracts::structural_access::structural_operation_result_contract(function, operation).is_none()
    {
        return false;
    }
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    let u64_scalar = |value: &ValueId| {
        matches!(scalar(*value), Some(ScalarType::Integer(integer))
            if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
    };
    match operation {
        O::ElementViewSubslice {
            start, end, length, ..
        } => u64_scalar(start) && u64_scalar(end) && u64_scalar(length),
        O::ElementViewRead {
            result,
            index,
            length,
            ..
        } => {
            scalar(result.value) == Some(result.scalar_type)
                && u64_scalar(index)
                && scalar(*index) == scalar(*length)
        }
        O::ElementViewWrite {
            index,
            value,
            length,
            ..
        } => u64_scalar(index) && scalar(*value).is_some() && scalar(*index) == scalar(*length),
        O::ElementViewLength { result, .. } => {
            matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
        }
        O::EstablishElementView { region, .. } => match region {
            terminal_psi::ElementViewRegion::WholeCollection => true,
            terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. } => {
                u64_scalar(start) && u64_scalar(end)
            }
        },
        _ => false,
    }
}
