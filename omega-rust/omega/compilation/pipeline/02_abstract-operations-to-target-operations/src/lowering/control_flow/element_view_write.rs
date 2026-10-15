//! Indexed scalar-element store through a mutable element view.
use super::{LiveDefinitions, observations};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::IntegerSign;
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::StructuralTypeShape;

/// An indexed element write names one view place verified mutable. The view
/// is a signature parameter or non-entry block parameter declared
/// `&mut [T]`, or a local whose establishment lent the collection under
/// `MutableBorrow` — the same descriptor derivation a read resolves, gated
/// here on the exclusive loan the verifier approved for the store.
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::ElementViewWrite {
        path,
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let values = observations::scalar_values(live, &prepared.scalar_parameters)?;
    if !crate::lowering::scalar::descriptor_operands::observes_count(
        function,
        &values,
        *length,
        destination,
        false,
    ) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (view_structural_type, indices) =
        crate::lowering::scalar::descriptor_operands::descriptor_projection(
            function,
            structural_types,
            live.element_view_backings(),
            destination,
        )?;
    let StructuralTypeShape::ElementView { element, .. } = &structural_types
        .get(&view_structural_type)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?
        .shape
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let Some(element_scalar) = terminal_semantics::element_view_leaf_scalar(
        structural_types.catalog().as_slice(),
        *element,
        path,
    ) else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    // Resolve the authored leaf value from the common dominating scalar roster.
    let index_value = *live
        .integers
        .get(index)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let element_value = super::scalar_sources::source(*value, function, live)?;
    if index_value.scalar_type().sign() != IntegerSign::Unsigned
        || index_value.scalar_type().bits() != 64
        || index_value.scalar_type().is_address()
        || element_scalar != element_value.scalar_type()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    operations.push(TargetUnitOperation::ElementViewWrite {
        path: path.clone(),
        psi_operation: *psi_operation,
        destination: destination.clone(),
        indices,
        index: index_value.into_target_source(*index),
        value: element_value,
        length: *length,
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
