//! Exact mutable byte descriptors and once-evaluated scalar write operands.

use super::LiveDefinitions;
use crate::lowering::shared::*;

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::ByteSequenceWrite {
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
    } = operation
    else {
        return Err(invalid());
    };
    if live.lengths.get(length) != Some(destination)
        || !(prepared
            .parameters
            .iter()
            .any(|parameter| parameter.place == *destination)
            || live.block_views.contains(destination))
    {
        return Err(invalid());
    }
    let (destination, view) = crate::lowering::scalar::byte_views::mutable_parameter_view(
        function,
        structural_types,
        &prepared.parameters,
        *destination,
    )?;
    let index_value = *live.integers.get(index).ok_or_else(invalid)?;
    let byte_value = *live.integers.get(value).ok_or_else(invalid)?;
    for (scalar_type, bits) in [
        (index_value.scalar_type(), 64),
        (byte_value.scalar_type(), 8),
    ] {
        if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
            || scalar_type.bits() != bits
            || scalar_type.is_address()
        {
            return Err(invalid());
        }
    }
    operations.push(TargetUnitOperation::ByteSequenceWrite {
        psi_operation: *psi_operation,
        destination,
        view,
        index: index_value.into_target_source(*index),
        value: byte_value.into_target_source(*value),
        length: *length,
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
