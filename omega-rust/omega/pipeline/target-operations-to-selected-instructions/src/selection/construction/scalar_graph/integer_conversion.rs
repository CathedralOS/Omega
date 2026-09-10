//! Select proof-bearing exact casts and infallible widening with explicit raw-width normalization.
use super::*;
use legalized_operations::LegalizedScalarInstruction;

pub(super) fn emit(
    operation: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    function: usize,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
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
    if actual_type != ScalarType::Integer(*source_type)
        || !matches!(scalar_type, ScalarType::Integer(integer)
        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
            && matches!(integer.bits(), 8 | 16 | 32 | 64)
            && if matches!(operation.kind, LegalizedScalarInstructionKind::IntegerExactCast { .. }) {
                source_type.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                    && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                    && source_type.can_exact_cast_to(integer)
                    && !(source_type.sign() == IntegerSign::Signed
                        && integer.sign() == IntegerSign::Signed
                        && (source_type.bits() != 64 || integer.bits() != 64))
                    && !(source_type.bits() == 16 && integer.bits() > 16)
            } else {
                source_type.sign() == IntegerSign::Unsigned && source_type.bits() == 8
                    && matches!(integer.bits(), 16 | 32 | 64) && source_type.can_widen_to(integer)
            })
    {
        return Err(invalid());
    }
    let output = builder.register(result.value, result.definition_site, scalar_type)?;
    builder.emit(
        if matches!(
            operation.kind,
            LegalizedScalarInstructionKind::IntegerExactCast { .. }
        ) {
            crate::selection::scalar_call_abi::integer_abi_normalization(scalar_type)
        } else {
            SelectedInstructionKind::CopyI64
        },
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
