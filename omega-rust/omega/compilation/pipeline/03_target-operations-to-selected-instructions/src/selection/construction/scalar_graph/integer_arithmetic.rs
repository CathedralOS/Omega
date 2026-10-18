//! Integer arithmetic and representation operations: constants, bitwise
//! forms, and the Saturating, Trapping, Wrapping, and Exact binary families.
//! Each selects its carrier's instruction row and emits the scratch and
//! carrier normalization its realization needs.

use super::{
    Builder, IntegerSign, IntegerValue, LegalizedScalarInstructionKind, MachineSemanticKind,
    RegisterConstraintKey, SaturatingCarrier, ScalarType, SelectedConstraintKeys,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance, TrappingForm,
    TrappingOperation, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;

/// Select one integer arithmetic operation. The dispatcher routes only this
/// family's kinds here.
pub(super) fn emit(
    function: usize,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    operation: &legalized_operations::LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let result = operation.result.ok_or_else(invalid)?;
    let scalar_type = result.scalar_type;
    let constraints = builder.constraints;
    // Machine arithmetic is modulo 64 bits; a narrow result's carrier
    // normalization copies the raw register into the fresh result register
    // unless the identity `CopyI64` already makes it canonical.
    let normalized_result = |builder: &mut Builder<'_>,
                             normalization: SelectedInstructionKind,
                             raw: VirtualRegisterId|
     -> Result<VirtualRegisterId, SelectedInstructionError> {
        if normalization == SelectedInstructionKind::CopyI64 {
            Ok(raw)
        } else {
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            builder.emit_integer(
                normalization,
                constraints.keys.copy_i64,
                &[raw, output],
                SelectedInstructionProvenance {
                    values: vec![result.value],
                    ..Default::default()
                },
            )?;
            Ok(output)
        }
    };
    Ok(match &operation.kind {
        LegalizedScalarInstructionKind::Constant(value) => {
            if scalar_type == ScalarType::Boolean && !matches!(value, IntegerValue::Unsigned(0 | 1))
            {
                return Err(invalid());
            }
            if matches!(
                scalar_type,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32)
            ) && !matches!(value, IntegerValue::Unsigned(bits) if *bits <= u128::from(u32::MAX))
                || matches!(
                    scalar_type,
                    ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64)
                ) && !matches!(value, IntegerValue::Unsigned(bits) if *bits <= u128::from(u64::MAX))
            {
                return Err(invalid());
            }
            if scalar_type == ScalarType::Boolean && !matches!(value, IntegerValue::Unsigned(0 | 1))
            {
                return Err(invalid());
            }
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            builder.emit_materialize_i64(
                *value,
                output,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            output
        }
        LegalizedScalarInstructionKind::BitwiseAnd { left, right }
        | LegalizedScalarInstructionKind::BitwiseOr { left, right }
        | LegalizedScalarInstructionKind::BitwiseXor { left, right } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            if left_type != scalar_type
                || right_type != scalar_type
                || !matches!(scalar_type, ScalarType::Integer(integer)
                        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                        && matches!(integer.bits(), 8 | 16 | 32 | 64))
            {
                return Err(invalid());
            }
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            // Subtraction shares operand constraints and conservatively models
            // x64 flag clobbers; the distinct bitwise form carries the semantics.
            builder.emit_integer(
                match operation.kind {
                    LegalizedScalarInstructionKind::BitwiseOr { .. } => {
                        SelectedInstructionKind::BitwiseOrI64
                    }
                    LegalizedScalarInstructionKind::BitwiseXor { .. } => {
                        SelectedInstructionKind::BitwiseXorI64
                    }
                    _ => SelectedInstructionKind::BitwiseAndI64,
                },
                constraints.keys.subtract_i64,
                &[left_register, right_register, output],
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            output
        }
        LegalizedScalarInstructionKind::BitwiseNot { operand } => {
            let (_, input, _, operand_type) = builder.resolve(*operand).ok_or_else(invalid)?;
            if operand_type != scalar_type
                || !matches!(scalar_type, ScalarType::Integer(integer)
                        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                        && matches!(integer.bits(), 8 | 16 | 32 | 64))
            {
                return Err(invalid());
            }
            let raw = builder.register(result.value, result.definition_site, scalar_type)?;
            builder.emit_integer(
                SelectedInstructionKind::BitwiseNotI64,
                constraints.keys.copy_i64,
                &[input, raw],
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*operand, result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            // Complementing a normalized signed carrier stays
            // sign-extended, but a narrow unsigned result exposes
            // set high bits; re-normalize it for later source uses.
            let normalization = if matches!(scalar_type, ScalarType::Integer(integer)
                    if integer.sign() == IntegerSign::Unsigned)
            {
                crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type)
            } else {
                SelectedInstructionKind::CopyI64
            };
            normalized_result(builder, normalization, raw)?
        }
        LegalizedScalarInstructionKind::SaturatingAdd {
            carrier,
            left,
            right,
        }
        | LegalizedScalarInstructionKind::SaturatingSubtract {
            carrier,
            left,
            right,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            if left_type != scalar_type
                || right_type != scalar_type
                || !carries(scalar_type, *carrier)
            {
                return Err(invalid());
            }
            let adds = matches!(
                operation.kind,
                LegalizedScalarInstructionKind::SaturatingAdd { .. }
            );
            let (kind, constraint, scratch) =
                saturating_add_or_subtract_selection(adds, *carrier, &constraints.keys);
            // Normalized narrow carriers combine exactly in 64 bits
            // and the realization clamps the result to the carrier,
            // so it is already normalized for later source uses.
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let [left_register, right_register] = binary_operands(
                builder,
                constraint,
                (*left, left_register),
                (*right, right_register),
            )?;
            let mut operands = vec![left_register, right_register, output];
            if scratch == SaturatingScratch::Bound {
                operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?);
            }
            builder.emit_integer(
                kind,
                constraint,
                &operands,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            output
        }
        LegalizedScalarInstructionKind::SaturatingMultiply {
            carrier,
            left,
            right,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            if left_type != scalar_type
                || right_type != scalar_type
                || !carries(scalar_type, *carrier)
            {
                return Err(invalid());
            }
            // Every carrier's realization clamps or saturates in
            // place, so the result is already normalized for later
            // source uses and needs no carrier normalization.
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let constraint = saturating_multiply_selection(*carrier, &constraints.keys);
            let [left_register, right_register] = binary_operands(
                builder,
                constraint,
                (*left, left_register),
                (*right, right_register),
            )?;
            let scratch = instruction_scratch(builder, 3, IntegerSign::Signed)?;
            builder.emit_integer(
                SelectedInstructionKind::SaturatingMultiply { carrier: *carrier },
                constraint,
                &[left_register, right_register, output, scratch],
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            output
        }
        LegalizedScalarInstructionKind::TrappingBinary { form, left, right } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            // The carrier is re-derived from the declared result
            // type; a shift's count may be any fixed native type.
            if left_type != scalar_type
                || !carries(scalar_type, form.carrier)
                || !trapping_right_type(*form, scalar_type, right_type)
            {
                return Err(invalid());
            }
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let constraint = trapping_constraint(*form, &constraints.keys)?;
            let [left_register, right_register] = binary_operands(
                builder,
                constraint,
                (*left, left_register),
                (*right, right_register),
            )?;
            let scratch = instruction_scratch(builder, 3, IntegerSign::Unsigned)?;
            builder.emit_integer(
                SelectedInstructionKind::TrappingInteger { form: *form },
                constraint,
                &[left_register, right_register, output, scratch],
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            output
        }
        LegalizedScalarInstructionKind::TrappingConvert {
            form,
            source,
            operand,
        } => {
            let (_, input, _, operand_type) = builder.resolve(*operand).ok_or_else(invalid)?;
            if !carries(scalar_type, form.carrier)
                || !carries(operand_type, *source)
                || form.operation
                    != (TrappingOperation::Convert {
                        source: source.sign(),
                    })
            {
                return Err(invalid());
            }
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let scratch = instruction_scratch(builder, 2, IntegerSign::Unsigned)?;
            builder.emit_integer(
                SelectedInstructionKind::TrappingInteger { form: *form },
                trapping_constraint(*form, &constraints.keys)?,
                &[input, output, scratch],
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*operand, result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            output
        }
        LegalizedScalarInstructionKind::SaturatingDivide {
            carrier,
            left,
            right,
            obligation,
            accepted_fact,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            if left_type != scalar_type
                || right_type != scalar_type
                || !carries(scalar_type, *carrier)
            {
                return Err(invalid());
            }
            // The zero divisor stays a proof obligation carried by
            // the instruction; the realization handles the one
            // signed overflow quotient (MIN / -1) itself.
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let (constraint, scratch) = saturating_divide_selection(
                *carrier,
                &constraints.keys,
                environment.target().architecture,
            );
            let [left_register, right_register] = binary_operands(
                builder,
                constraint,
                (*left, left_register),
                (*right, right_register),
            )?;
            let mut operands = vec![left_register, right_register, output];
            match scratch {
                SaturatingScratch::None => {}
                SaturatingScratch::Bound => {
                    operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?)
                }
                SaturatingScratch::DivisionHighHalf => operands.push(division_scratch(builder)?),
            }
            builder.emit_integer(
                SelectedInstructionKind::SaturatingDivide {
                    carrier: *carrier,
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                constraint,
                &operands,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    obligations: vec![*obligation],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            if environment.target().architecture == target::Architecture::X86_64 {
                builder.copy(output, result.value, result.definition_site, scalar_type)?
            } else {
                output
            }
        }
        LegalizedScalarInstructionKind::SaturatingRemainder {
            carrier,
            left,
            right,
            obligation,
            accepted_fact,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            if left_type != scalar_type
                || right_type != scalar_type
                || !carries(scalar_type, *carrier)
            {
                return Err(invalid());
            }
            // The mathematical remainder already lies inside the
            // carrier, so the realization is the ordinary signed
            // or unsigned remainder row; the zero divisor stays a
            // proof obligation carried by the instruction.
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let constraint = if carrier.is_signed() {
                constraints.keys.remainder_i64
            } else {
                constraints.keys.remainder_u64
            };
            let [left_register, right_register] = binary_operands(
                builder,
                constraint,
                (*left, left_register),
                (*right, right_register),
            )?;
            let mut operands = vec![left_register, right_register, output];
            if environment.target().architecture == target::Architecture::X86_64 {
                operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?);
            }
            builder.emit_integer(
                SelectedInstructionKind::SaturatingRemainder {
                    carrier: *carrier,
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                constraint,
                &operands,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    obligations: vec![*obligation],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            if environment.target().architecture == target::Architecture::X86_64 {
                builder.copy(output, result.value, result.definition_site, scalar_type)?
            } else {
                output
            }
        }
        LegalizedScalarInstructionKind::WrappingRemainder {
            left,
            right,
            obligation,
            accepted_fact,
        }
        | LegalizedScalarInstructionKind::WrappingDivide {
            left,
            right,
            obligation,
            accepted_fact,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            let Some(integer) = scalar_type.as_integer() else {
                return Err(invalid());
            };
            if left_type != scalar_type
                || right_type != scalar_type
                || integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(integer.bits(), 8 | 16 | 32 | 64)
            {
                return Err(invalid());
            }
            let divides = matches!(
                operation.kind,
                LegalizedScalarInstructionKind::WrappingDivide { .. }
            );
            let selection = WrappingDivision::of(divides, integer);
            let (kind, key) =
                selection.kind_and_key(*obligation, *accepted_fact, &constraints.keys);
            let raw = builder.register(result.value, result.definition_site, scalar_type)?;
            let [left_register, right_register] = binary_operands(
                builder,
                key,
                (*left, left_register),
                (*right, right_register),
            )?;
            let mut operands = vec![left_register, right_register, raw];
            if environment.target().architecture == target::Architecture::X86_64 {
                if selection == WrappingDivision::DivideU64 {
                    operands.push(division_scratch(builder)?);
                } else {
                    operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?);
                }
            }
            builder.emit_integer(
                kind,
                key,
                &operands,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    obligations: vec![*obligation],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            if selection != WrappingDivision::DivideSignedNarrow {
                if environment.target().architecture == target::Architecture::X86_64 {
                    // The architectural result is local: retain its bits in a
                    // flexible value before another call or divide writes it.
                    builder.copy(raw, result.value, result.definition_site, scalar_type)?
                } else {
                    raw
                }
            } else {
                // The only quotient outside a narrow signed
                // carrier is MIN / -1, whose widened i64 value is
                // -MIN; truncating it back to the carrier is the
                // wrapped MIN. This private normalization adds no
                // source operation or fuel.
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit_integer(
                    crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type),
                    constraints.keys.copy_i64,
                    &[raw, output],
                    SelectedInstructionProvenance {
                        values: vec![result.value],
                        ..Default::default()
                    },
                )?;
                output
            }
        }
        LegalizedScalarInstructionKind::WrappingAdd { left, right }
        | LegalizedScalarInstructionKind::WrappingSubtract { left, right }
        | LegalizedScalarInstructionKind::WrappingMultiply { left, right } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            if left_type != scalar_type
                || right_type != scalar_type
                || !matches!(scalar_type, ScalarType::Integer(integer)
                        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64))
            {
                return Err(invalid());
            }
            let (kind, key) = match operation.kind {
                LegalizedScalarInstructionKind::WrappingSubtract { .. } => (
                    SelectedInstructionKind::WrappingSubtractI64,
                    constraints.keys.subtract_i64,
                ),
                LegalizedScalarInstructionKind::WrappingMultiply { .. } => (
                    SelectedInstructionKind::WrappingMultiplyI64,
                    constraints.keys.multiply_i64,
                ),
                _ => (
                    SelectedInstructionKind::WrappingAddI64,
                    constraints.keys.add_i64,
                ),
            };
            let raw = builder.register(result.value, result.definition_site, scalar_type)?;
            let [left_register, right_register] = binary_operands(
                builder,
                key,
                (*left, left_register),
                (*right, right_register),
            )?;
            builder.emit_integer(
                kind,
                key,
                &[left_register, right_register, raw],
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            // Machine addition is modulo 64 bits. Only the normalized
            // low-width result becomes available to later source uses;
            // this private normalization adds no source operation/fuel.
            let normalization =
                crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type);
            normalized_result(builder, normalization, raw)?
        }
        LegalizedScalarInstructionKind::WrappingShiftLeft { value, count }
        | LegalizedScalarInstructionKind::WrappingShiftRight { value, count }
        | LegalizedScalarInstructionKind::ExactShiftLeft { value, count, .. }
        | LegalizedScalarInstructionKind::ExactShiftRight { value, count, .. } => {
            let (_, value_register, _, resolved_value_type) =
                builder.resolve(*value).ok_or_else(invalid)?;
            let (_, mut count_register, count_site, count_type) =
                builder.resolve(*count).ok_or_else(invalid)?;
            let Some(value_integer) = scalar_type.as_integer() else {
                return Err(invalid());
            };
            let Some(count_integer) = count_type.as_integer() else {
                return Err(invalid());
            };
            // The result carries the shifted value's type; the
            // count keeps its own integer type and only needs a
            // native fixed carrier.
            if resolved_value_type != scalar_type
                || value_integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(value_integer.bits(), 8 | 16 | 32 | 64)
                || count_integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(count_integer.bits(), 8 | 16 | 32 | 64)
            {
                return Err(invalid());
            }
            let exact = matches!(
                operation.kind,
                LegalizedScalarInstructionKind::ExactShiftLeft { .. }
                    | LegalizedScalarInstructionKind::ExactShiftRight { .. }
            );
            // Both targets reduce the register count modulo 64.
            // A narrower value's wrapping semantics reduce modulo
            // its own width, so mask the count by `width - 1`
            // first. The exact count is proven inside [0, width),
            // which already satisfies the hardware reduction.
            if !exact && value_integer.bits() < 64 {
                let mask = builder.register(
                    *count,
                    count_site,
                    ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                            .map_err(|_| invalid())?,
                    ),
                )?;
                builder.emit_materialize_i64(
                    IntegerValue::Unsigned(u128::from(value_integer.bits() - 1)),
                    mask,
                    SelectedInstructionProvenance {
                        values: vec![*count],
                        ..Default::default()
                    },
                )?;
                let masked = builder.register(*count, count_site, count_type)?;
                builder.emit_integer(
                    SelectedInstructionKind::BitwiseAndI64,
                    constraints.keys.subtract_i64,
                    &[count_register, mask, masked],
                    SelectedInstructionProvenance {
                        values: vec![*count],
                        ..Default::default()
                    },
                )?;
                count_register = masked;
            }
            let signed = value_integer.sign() == IntegerSign::Signed;
            let (kind, key) = match &operation.kind {
                LegalizedScalarInstructionKind::WrappingShiftLeft { .. } => (
                    SelectedInstructionKind::WrappingShiftLeftI64,
                    constraints.keys.shift_i64,
                ),
                LegalizedScalarInstructionKind::WrappingShiftRight { .. } => (
                    if signed {
                        SelectedInstructionKind::WrappingShiftRightI64
                    } else {
                        SelectedInstructionKind::WrappingShiftRightU64
                    },
                    constraints.keys.shift_i64,
                ),
                LegalizedScalarInstructionKind::ExactShiftLeft {
                    obligation,
                    accepted_fact,
                    ..
                } => (
                    SelectedInstructionKind::ExactShiftLeftI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                    constraints.keys.shift_i64,
                ),
                LegalizedScalarInstructionKind::ExactShiftRight {
                    obligation,
                    accepted_fact,
                    ..
                } => (
                    if signed {
                        SelectedInstructionKind::ExactShiftRightI64 {
                            obligation: *obligation,
                            accepted_fact: *accepted_fact,
                        }
                    } else {
                        SelectedInstructionKind::ExactShiftRightU64 {
                            obligation: *obligation,
                            accepted_fact: *accepted_fact,
                        }
                    },
                    constraints.keys.shift_i64,
                ),
                _ => unreachable!("shift dispatch"),
            };
            let raw = builder.register(result.value, result.definition_site, scalar_type)?;
            let [value_register, count_register] = binary_operands(
                builder,
                key,
                (*value, value_register),
                (*count, count_register),
            )?;
            let operands = vec![value_register, count_register, raw];
            builder.emit_integer(
                kind,
                key,
                &operands,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*value, *count, result.value],
                    obligations: match &operation.kind {
                        LegalizedScalarInstructionKind::ExactShiftLeft { obligation, .. }
                        | LegalizedScalarInstructionKind::ExactShiftRight { obligation, .. } => {
                            vec![*obligation]
                        }
                        _ => vec![],
                    },
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            // The i64 shift leaves out-of-carrier bits in a narrow
            // wrapping result; re-normalize it like the other
            // wrapping arithmetic rows. Exact results are proven
            // canonical by their retained obligation.
            let normalization = if exact {
                SelectedInstructionKind::CopyI64
            } else {
                crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type)
            };
            normalized_result(builder, normalization, raw)?
        }
        LegalizedScalarInstructionKind::ExactBinary {
            operator,
            left,
            right,
            obligation,
            accepted_fact,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
            // Exact address offsets/distances keep independently typed operands.
            // Replay checks the signature again rather than treating equal
            // machine widths as source-level carrier equality.
            let operands_match = if *operator
                == legalized_operations::LegalizedExactIntegerOperator::Add
            {
                matches!((scalar_type, left_type, right_type),
                    (ScalarType::Integer(result), ScalarType::Integer(left), ScalarType::Integer(right))
                        if result.admits_exact_add_operands(left, right))
            } else if *operator == legalized_operations::LegalizedExactIntegerOperator::Subtract {
                matches!((scalar_type, left_type, right_type),
                    (ScalarType::Integer(result), ScalarType::Integer(left), ScalarType::Integer(right))
                        if result.admits_exact_subtract_operands(left, right))
            } else {
                left_type == scalar_type && right_type == scalar_type
            };
            if !operands_match {
                return Err(invalid());
            }
            let divide_operator = matches!(
                *operator,
                legalized_operations::LegalizedExactIntegerOperator::Divide
                    | legalized_operations::LegalizedExactIntegerOperator::Remainder
            );
            // Exact divide/remainder: u64 keeps the unsigned entry
            // (its dividend range escapes i64); every other fixed
            // 8/16/32/64 carrier selects the signed i64 entry
            // because scalar transport normalizes the operands and
            // the proven obligations exclude the faulting inputs
            // the wrapping forms guard against.
            if divide_operator
                && !matches!(
                    scalar_type,
                    ScalarType::Integer(integer)
                        if integer.carrier()
                            == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64)
                )
            {
                return Err(invalid());
            }
            let unsigned_divide = divide_operator
                && scalar_type
                    == ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                            .map_err(|_| invalid())?,
                    );
            let (kind, key) = match operator {
                legalized_operations::LegalizedExactIntegerOperator::Divide => {
                    if unsigned_divide {
                        (
                            SelectedInstructionKind::ExactDivideU64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            },
                            constraints.keys.divide_u64,
                        )
                    } else {
                        (
                            SelectedInstructionKind::ExactDivideI64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            },
                            constraints.keys.divide_i64,
                        )
                    }
                }
                legalized_operations::LegalizedExactIntegerOperator::Remainder => {
                    if unsigned_divide {
                        (
                            SelectedInstructionKind::ExactRemainderU64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            },
                            constraints.keys.remainder_u64,
                        )
                    } else {
                        (
                            SelectedInstructionKind::ExactRemainderI64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            },
                            constraints.keys.remainder_i64,
                        )
                    }
                }
                legalized_operations::LegalizedExactIntegerOperator::Add => (
                    SelectedInstructionKind::ExactAddI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                    constraints.keys.add_i64,
                ),
                legalized_operations::LegalizedExactIntegerOperator::Subtract => (
                    SelectedInstructionKind::ExactSubtractI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                    constraints.keys.subtract_i64,
                ),
                legalized_operations::LegalizedExactIntegerOperator::Multiply => (
                    SelectedInstructionKind::ExactMultiplyI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                    constraints.keys.multiply_i64,
                ),
            };
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let [left_register, right_register] = binary_operands(
                builder,
                key,
                (*left, left_register),
                (*right, right_register),
            )?;
            let mut operands = vec![left_register, right_register, output];
            if environment.target().architecture == target::Architecture::X86_64 {
                if unsigned_divide
                    && *operator == legalized_operations::LegalizedExactIntegerOperator::Divide
                {
                    operands.push(division_scratch(builder)?);
                } else if divide_operator {
                    operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?);
                }
            }
            builder.emit_integer(
                kind,
                key,
                &operands,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![*left, *right, result.value],
                    obligations: vec![*obligation],
                    fuel: operation.fuel.clone(),
                    ..Default::default()
                },
            )?;
            if divide_operator && environment.target().architecture == target::Architecture::X86_64
            {
                builder.copy(output, result.value, result.definition_site, scalar_type)?
            } else {
                output
            }
        }
        _ => return Err(invalid()),
    })
}

/// Fixed uses constrain this occurrence, not the source value's whole range.
/// A divide can overwrite its dividend's fixed view while that value is used
/// again later. Transport each pinned input to a fresh short-lived register;
/// the original remains independently homed. This also handles equal inputs
/// without the former divide/multiply/shift source-equality recognizers.
/// Early-clobber rows additionally need distinct participants even on targets
/// with flexible inputs. Copies retain the value, type and definition lineage;
/// operation obligations and fuel remain on the arithmetic instruction.
fn binary_operands(
    builder: &mut Builder<'_>,
    key: RegisterConstraintKey,
    left: (semantic_vocabulary::ValueId, VirtualRegisterId),
    right: (semantic_vocabulary::ValueId, VirtualRegisterId),
) -> Result<[VirtualRegisterId; 2], SelectedInstructionError> {
    let row = super::row(builder.catalog, key)?;
    let inputs = row
        .operands
        .get(..2)
        .ok_or_else(SelectedInstructionError::custody)?;
    if inputs
        .iter()
        .any(|operand| operand.access != register_model::RegisterOperandAccess::Use)
    {
        return Err(SelectedInstructionError::custody());
    }
    let pinned = [
        inputs[0].fixed_view.is_some(),
        inputs[1].fixed_view.is_some(),
    ];
    let distinct = row.operands.iter().any(|operand| operand.early_clobber);
    let mut registers = [left.1, right.1];
    for (index, (value, register)) in [left, right].into_iter().enumerate() {
        if pinned[index] || index == 1 && distinct && registers[0] == registers[1] {
            let (_, _, site, scalar_type) = builder
                .resolve(value)
                .ok_or_else(SelectedInstructionError::custody)?;
            registers[index] = builder.copy(register, value, site, scalar_type)?;
        }
    }
    Ok(registers)
}

/// Whether the declared result type is exactly this saturating carrier,
/// checked here independently of legalization's admission.
fn carries(scalar_type: ScalarType, carrier: SaturatingCarrier) -> bool {
    matches!(scalar_type, ScalarType::Integer(integer)
        if SaturatingCarrier::from_integer(integer) == Some(carrier))
}

/// The scratch operand a saturating realization owns beyond its two inputs
/// and result.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SaturatingScratch {
    None,
    /// Operand 3: the early-clobber bound scratch the clamp compares against.
    Bound,
    /// The explicit zero high half of x86-64 division, pinned to `rdx`.
    DivisionHighHalf,
}

/// Operand shape follows the carrier class, not the width: the u64 add and
/// every unsigned subtract saturate on the carry flag in three operands (a
/// zero-normalized narrow difference borrows exactly when the u64 one does),
/// while every other carrier computes the exact 64-bit result, or detects
/// the i64 overflow flag, and clamps through the operand-3 bound scratch.
fn saturating_add_or_subtract_selection(
    adds: bool,
    carrier: SaturatingCarrier,
    keys: &SelectedConstraintKeys,
) -> (
    SelectedInstructionKind,
    RegisterConstraintKey,
    SaturatingScratch,
) {
    if adds {
        let kind = SelectedInstructionKind::SaturatingAdd { carrier };
        if carrier == SaturatingCarrier::U64 {
            (kind, keys.saturating_add_u64, SaturatingScratch::None)
        } else {
            (kind, keys.saturating_add_clamped, SaturatingScratch::Bound)
        }
    } else {
        let kind = SelectedInstructionKind::SaturatingSubtract { carrier };
        if carrier.is_signed() {
            (
                kind,
                keys.saturating_subtract_clamped,
                SaturatingScratch::Bound,
            )
        } else {
            (
                kind,
                keys.saturating_subtract_unsigned,
                SaturatingScratch::None,
            )
        }
    }
}

/// Unsigned division never overflows and shares the exact unsigned divide
/// row; signed division takes the signed row. x86-64 pins both to `rax`
/// with the explicit zero `rdx` input the divide discards and its clamp or
/// guard reuses, so the divisor stays out of that register; AArch64 signed
/// division clamps through the bound scratch and unsigned division needs
/// no scratch at all.
fn saturating_divide_selection(
    carrier: SaturatingCarrier,
    keys: &SelectedConstraintKeys,
    architecture: target::Architecture,
) -> (RegisterConstraintKey, SaturatingScratch) {
    let x86 = architecture == target::Architecture::X86_64;
    let key = if carrier.is_signed() {
        keys.saturating_divide_signed
    } else {
        keys.divide_u64
    };
    let scratch = match (x86, carrier.is_signed()) {
        (true, _) => SaturatingScratch::DivisionHighHalf,
        (false, true) => SaturatingScratch::Bound,
        (false, false) => SaturatingScratch::None,
    };
    (key, scratch)
}

/// Every saturating multiplication owns an operand-3 scratch: the clamp
/// bound for narrow carriers, the saturated value or product high half for
/// i64, and the product high half for u64. The selected constraint row owns
/// target-specific fixed inputs, transported by `binary_operands`.
fn saturating_multiply_selection(
    carrier: SaturatingCarrier,
    keys: &SelectedConstraintKeys,
) -> RegisterConstraintKey {
    if carrier == SaturatingCarrier::U64 {
        keys.saturating_multiply_u64
    } else {
        keys.saturating_multiply_clamped
    }
}

/// How one wrapping divide or remainder is realized at its fixed native
/// carrier. Scalar transport keeps narrow operands sign- or zero-normalized
/// in 64-bit registers, so every carrier but u64 shares the signed i64
/// division rows: a zero-extended narrow operand is a non-negative i64, the
/// i64 guard owns the one faulting i64::MIN / -1 pair, and no remainder or
/// unsigned quotient leaves its carrier. The only widened result outside a
/// carrier is a narrow signed MIN / -1 quotient, which the carrier
/// normalization after the divide truncates back to the wrapped MIN. u64
/// operands escape i64, so they take the unsigned rows: unsigned division
/// never overflows, which makes it the same machine operation the exact u64
/// forms realize under the same nonzero-divisor proof.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WrappingDivision {
    DivideSignedNarrow,
    DivideI64,
    DivideU64,
    RemainderI64,
    RemainderU64,
}

impl WrappingDivision {
    fn of(divides: bool, integer: semantic_vocabulary::IntegerType) -> Self {
        let u64_carrier = integer.sign() == IntegerSign::Unsigned && integer.bits() == 64;
        match (divides, u64_carrier) {
            (true, true) => Self::DivideU64,
            (false, true) => Self::RemainderU64,
            (false, false) => Self::RemainderI64,
            (true, false) if integer.sign() == IntegerSign::Signed && integer.bits() < 64 => {
                Self::DivideSignedNarrow
            }
            (true, false) => Self::DivideI64,
        }
    }

    fn kind_and_key(
        self,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        keys: &SelectedConstraintKeys,
    ) -> (SelectedInstructionKind, RegisterConstraintKey) {
        match self {
            Self::DivideSignedNarrow | Self::DivideI64 => (
                SelectedInstructionKind::WrappingDivideI64 {
                    obligation,
                    accepted_fact,
                },
                keys.divide_i64,
            ),
            Self::DivideU64 => (
                SelectedInstructionKind::ExactDivideU64 {
                    obligation,
                    accepted_fact,
                },
                keys.divide_u64,
            ),
            Self::RemainderI64 => (
                SelectedInstructionKind::WrappingRemainderI64 {
                    obligation,
                    accepted_fact,
                },
                keys.remainder_i64,
            ),
            Self::RemainderU64 => (
                SelectedInstructionKind::ExactRemainderU64 {
                    obligation,
                    accepted_fact,
                },
                keys.remainder_u64,
            ),
        }
    }
}

/// A Trapping binary form's second operand: the result carrier for
/// arithmetic, any fixed native integer for a shift count.
fn trapping_right_type(
    form: TrappingForm,
    scalar_type: ScalarType,
    right_type: ScalarType,
) -> bool {
    match form.operation {
        TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
            matches!(right_type, ScalarType::Integer(count)
                if SaturatingCarrier::from_integer(count).is_some())
        }
        _ => right_type == scalar_type,
    }
}

fn trapping_constraint(
    form: TrappingForm,
    keys: &SelectedConstraintKeys,
) -> Result<RegisterConstraintKey, SelectedInstructionError> {
    keys.for_semantic(MachineSemanticKind::TrappingInteger(form))
        .ok_or(SelectedInstructionError::custody())
}

// An encoding-defined early-clobber scratch register: a Trapping form's
// bound (operand 3 of a binary form, operand 2 of a conversion), the
// saturating i32 forms' clamp bound, and the signed remainder's high half
// all share the same i64 scratch the encoding defines itself on every
// target. `sign` selects the carrier the form's bound reads.
#[track_caller]
pub(super) fn instruction_scratch(
    builder: &mut Builder<'_>,
    operand: u16,
    sign: IntegerSign,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let id = VirtualRegisterId(
        builder
            .registers
            .len()
            .try_into()
            .map_err(|_| SelectedInstructionError::custody())?,
    );
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| SelectedInstructionError::custody())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(sign, 64)
                .map_err(|_| SelectedInstructionError::custody())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}

// The x86 high half is an explicit zero-valued implementation register.
pub(super) fn division_scratch(
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let id = instruction_scratch(builder, 0, IntegerSign::Unsigned)?;
    builder.emit_materialize_i64(IntegerValue::Unsigned(0), id, Default::default())?;
    Ok(id)
}
