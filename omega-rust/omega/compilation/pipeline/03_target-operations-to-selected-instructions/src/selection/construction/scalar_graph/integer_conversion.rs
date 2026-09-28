//! Select proof-bearing exact casts and infallible widening with explicit raw-width normalization.
use super::{
    Builder, LegalizedScalarInstructionKind, ScalarType, SelectedInstructionProvenance,
    VirtualRegisterId,
};
use crate::SelectedInstructionError;
use legalized_operations::LegalizedScalarInstruction;

pub(super) fn emit(
    operation: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    function: usize,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let result = operation.result.as_ref().ok_or_else(invalid)?;
    let scalar_type = result.scalar_type;
    let constraints = builder.constraints;
    let (LegalizedScalarInstructionKind::IntegerWiden {
        operand,
        source_type,
    }
    | LegalizedScalarInstructionKind::IntegerExactCast {
        operand,
        source_type,
        ..
    }) = &operation.kind
    else {
        return Err(invalid());
    };
    let (_, input, _, actual_type) = builder.resolve(*operand).ok_or_else(invalid)?;
    if actual_type != ScalarType::Integer(*source_type) {
        return Err(invalid());
    }
    let normalization =
        crate::selection::integer_conversion::normalization(&operation.kind, scalar_type)
            .ok_or_else(invalid)?;
    let output = builder.register(result.value, result.definition_site, scalar_type)?;
    builder.emit(
        normalization,
        constraints.keys.copy_i64,
        &[input, output],
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![*operand, result.value],
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
