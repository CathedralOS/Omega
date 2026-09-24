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
    let normalization = normalization(&operation.kind, scalar_type).ok_or_else(invalid)?;
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

/// The one normalization a widening or a proof-bearing exact cast selects,
/// or `None` when its carriers are not fixed native integers. Both sides hold
/// the value full-register normalized, and the narrower carrier's
/// normalization states it exactly: a widening extends the source's sign or
/// zero bits, never the destination width's potentially uninitialized high
/// bits, and an exact narrowing is proven inside the target, so the target's
/// own normalization is exact. Construction and validation both read this.
pub(in crate::selection) fn normalization(
    kind: &LegalizedScalarInstructionKind,
    scalar_type: ScalarType,
) -> Option<selected_instructions::SelectedInstructionKind> {
    let ScalarType::Integer(target) = scalar_type else {
        return None;
    };
    let fixed = |integer: semantic_vocabulary::IntegerType| {
        integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
            && matches!(integer.bits(), 8 | 16 | 32 | 64)
    };
    let narrower = match kind {
        LegalizedScalarInstructionKind::IntegerWiden { source_type, .. }
            if fixed(*source_type) && fixed(target) && source_type.can_widen_to(target) =>
        {
            *source_type
        }
        LegalizedScalarInstructionKind::IntegerExactCast { source_type, .. }
            if crate::legalization::exact_cast_has_native_carriers(*source_type, target) =>
        {
            if source_type.bits() < target.bits() {
                *source_type
            } else {
                target
            }
        }
        _ => return None,
    };
    Some(
        crate::selection::scalar_call_abi::integer_carrier_normalization(ScalarType::Integer(
            narrower,
        )),
    )
}
