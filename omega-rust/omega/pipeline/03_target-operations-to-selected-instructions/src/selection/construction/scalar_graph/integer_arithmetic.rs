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
    environment: &crate::register_environment::ValidatedTargetRegisterEnvironment,
    operation: &crate::legalized_operations::LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let result = operation.result.ok_or_else(invalid)?;
    let scalar_type = result.scalar_type;
    let constraints = builder.constraints;
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
            builder.emit(
                SelectedInstructionKind::MaterializeI64 { value: *value },
                constraints.keys.materialize_i64,
                &[output],
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
            builder.emit(
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
            builder.emit(
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
            if normalization == SelectedInstructionKind::CopyI64 {
                raw
            } else {
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit(
                    normalization,
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
            let mut operands = vec![left_register, right_register, output];
            if scratch == SaturatingScratch::Bound {
                operands.push(saturation_scratch(builder)?);
            }
            builder.emit(
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
            let (_, mut right_register, right_site, right_type) =
                builder.resolve(*right).ok_or_else(invalid)?;
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
            let (constraint, fixed_pair) = saturating_multiply_selection(
                *carrier,
                &constraints.keys,
                environment.target().architecture,
            );
            if fixed_pair && right_register == left_register {
                // x86-64 `MUL` pins the left operand to RAX and the
                // right one to RCX; one register cannot carry both
                // fixed views, so a squared operand gets its own copy.
                right_register = builder.copy(right_register, *right, right_site, scalar_type)?;
            }
            let scratch = saturation_scratch(builder)?;
            builder.emit(
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
            let (_, mut right_register, right_site, right_type) =
                builder.resolve(*right).ok_or_else(invalid)?;
            // The carrier is re-derived from the declared result
            // type; a shift's count may be any fixed native type.
            if left_type != scalar_type
                || !carries(scalar_type, form.carrier)
                || !trapping_right_type(*form, scalar_type, right_type)
            {
                return Err(invalid());
            }
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            if right_register == left_register {
                // Every Trapping row has two early-clobber
                // outputs, and liveness admits independent early
                // outputs only over distinct participants, so a
                // shared operand (`x + x`, `x << x`) gets its own
                // copy on every target. On x86-64 the copy also
                // lets RCX hold the divisor or shift count alone.
                right_register = builder.copy(right_register, *right, right_site, right_type)?;
            }
            let scratch = trapping_scratch(builder, 3)?;
            builder.emit(
                SelectedInstructionKind::TrappingInteger { form: *form },
                trapping_constraint(*form, &constraints.keys)?,
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
            let scratch = trapping_scratch(builder, 2)?;
            builder.emit(
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
            let mut operands = vec![left_register, right_register, output];
            match scratch {
                SaturatingScratch::None => {}
                SaturatingScratch::Bound => operands.push(saturation_scratch(builder)?),
                SaturatingScratch::DivisionHighHalf => operands.push(division_scratch(builder)?),
            }
            builder.emit(
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
            output
        }
        LegalizedScalarInstructionKind::SaturatingRemainder {
            carrier,
            left,
            right,
            obligation,
            accepted_fact,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, mut right_register, right_site, right_type) =
                builder.resolve(*right).ok_or_else(invalid)?;
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
            let mut operands = vec![left_register, right_register, output];
            if environment.target().architecture == target::Architecture::X86_64 {
                // The realized form pins the divisor to RCX so its
                // RDX zeroing cannot read a live divisor. A shared
                // dividend/divisor register cannot carry RAX and
                // RCX fixed views at once, so give the divisor its
                // own copy first.
                if right_register == left_register {
                    right_register =
                        builder.copy(right_register, *right, right_site, scalar_type)?;
                    operands[1] = right_register;
                }
                operands.push(remainder_scratch(builder)?);
            }
            builder.emit(
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
            output
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
            let (_, mut right_register, right_site, right_type) =
                builder.resolve(*right).ok_or_else(invalid)?;
            let ScalarType::Integer(integer) = scalar_type else {
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
            let mut operands = vec![left_register, right_register, raw];
            if environment.target().architecture == target::Architecture::X86_64 {
                if selection == WrappingDivision::DivideU64 {
                    operands.push(division_scratch(builder)?);
                } else {
                    // The realized form pins the divisor to RCX so
                    // its RDX extension or zeroing cannot read a
                    // live divisor. A shared dividend/divisor
                    // register cannot carry RAX and RCX fixed
                    // views at once, so give the divisor its own
                    // copy first.
                    if right_register == left_register {
                        right_register =
                            builder.copy(right_register, *right, right_site, scalar_type)?;
                        operands[1] = right_register;
                    }
                    operands.push(remainder_scratch(builder)?);
                }
            }
            builder.emit(
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
                raw
            } else {
                // The only quotient outside a narrow signed
                // carrier is MIN / -1, whose widened i64 value is
                // -MIN; truncating it back to the carrier is the
                // wrapped MIN. This private normalization adds no
                // source operation or fuel.
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit(
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
            builder.emit(
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
            if normalization == SelectedInstructionKind::CopyI64 {
                raw
            } else {
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit(
                    normalization,
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
        LegalizedScalarInstructionKind::WrappingShiftLeft { value, count }
        | LegalizedScalarInstructionKind::WrappingShiftRight { value, count }
        | LegalizedScalarInstructionKind::ExactShiftLeft { value, count, .. }
        | LegalizedScalarInstructionKind::ExactShiftRight { value, count, .. } => {
            let (_, value_register, _, resolved_value_type) =
                builder.resolve(*value).ok_or_else(invalid)?;
            let (_, mut count_register, count_site, count_type) =
                builder.resolve(*count).ok_or_else(invalid)?;
            let ScalarType::Integer(value_integer) = scalar_type else {
                return Err(invalid());
            };
            let ScalarType::Integer(count_integer) = count_type else {
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
                builder.emit(
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(u128::from(value_integer.bits() - 1)),
                    },
                    constraints.keys.materialize_i64,
                    &[mask],
                    SelectedInstructionProvenance {
                        values: vec![*count],
                        ..Default::default()
                    },
                )?;
                let masked = builder.register(*count, count_site, count_type)?;
                builder.emit(
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
            let mut operands = vec![value_register, count_register, raw];
            if environment.target().architecture == target::Architecture::X86_64
                && count_register == value_register
            {
                // The realized form copies the value into the
                // early-clobber result before the shift reads CL.
                // A shared value/count register must reach RCX
                // through its own copy so the fixed-view and
                // early-clobber rules stay disjoint.
                count_register = builder.copy(count_register, *count, count_site, count_type)?;
                operands[1] = count_register;
            }
            builder.emit(
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
            if normalization == SelectedInstructionKind::CopyI64 {
                raw
            } else {
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit(
                    normalization,
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
        LegalizedScalarInstructionKind::ExactBinary {
            operator,
            left,
            right,
            obligation,
            accepted_fact,
        } => {
            let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
            let (_, mut right_register, right_site, right_type) =
                builder.resolve(*right).ok_or_else(invalid)?;
            if left_type != scalar_type || right_type != scalar_type {
                return Err(invalid());
            }
            let divide_operator = matches!(
                *operator,
                crate::legalized_operations::LegalizedExactIntegerOperator::Divide
                    | crate::legalized_operations::LegalizedExactIntegerOperator::Remainder
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
                crate::legalized_operations::LegalizedExactIntegerOperator::Divide => {
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
                crate::legalized_operations::LegalizedExactIntegerOperator::Remainder => {
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
                crate::legalized_operations::LegalizedExactIntegerOperator::Add => (
                    SelectedInstructionKind::ExactAddI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                    constraints.keys.add_i64,
                ),
                crate::legalized_operations::LegalizedExactIntegerOperator::Subtract => (
                    SelectedInstructionKind::ExactSubtractI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                    constraints.keys.subtract_i64,
                ),
                crate::legalized_operations::LegalizedExactIntegerOperator::Multiply => (
                    SelectedInstructionKind::ExactMultiplyI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                    constraints.keys.multiply_i64,
                ),
            };
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
            let mut operands = vec![left_register, right_register, output];
            if environment.target().architecture == target::Architecture::X86_64 {
                if unsigned_divide
                    && *operator
                        == crate::legalized_operations::LegalizedExactIntegerOperator::Divide
                {
                    operands.push(division_scratch(builder)?);
                } else if divide_operator {
                    // The realized form pins the divisor to RCX so
                    // its RDX extension cannot read a live divisor.
                    // A shared dividend/divisor register cannot
                    // carry RAX and RCX fixed views at once, so
                    // give the divisor its own copy first.
                    if right_register == left_register {
                        right_register =
                            builder.copy(right_register, *right, right_site, scalar_type)?;
                        operands[1] = right_register;
                    }
                    operands.push(remainder_scratch(builder)?);
                }
            }
            builder.emit(
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
            output
        }
        _ => return Err(invalid()),
    })
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
/// bound for narrow carriers, the saturated value (x86-64) or product high
/// half (AArch64) for i64, and the product high half for u64. Only x86-64
/// u64 needs fixed registers, because `MUL` is the one baseline instruction
/// that produces the unsigned high half; it pins RAX, RCX, and RDX, and the
/// returned flag asks for a distinct right-operand register.
fn saturating_multiply_selection(
    carrier: SaturatingCarrier,
    keys: &SelectedConstraintKeys,
    architecture: target::Architecture,
) -> (RegisterConstraintKey, bool) {
    if carrier == SaturatingCarrier::U64 {
        (
            keys.saturating_multiply_u64,
            architecture == target::Architecture::X86_64,
        )
    } else {
        (keys.saturating_multiply_clamped, false)
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
        .ok_or_else(SelectedInstructionError::custody)
}

// A Trapping form's early-clobber scratch: operand 3 of a binary form or
// operand 2 of a conversion, defined by the encoding itself on every target.
fn trapping_scratch(
    builder: &mut Builder<'_>,
    operand: u16,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::custody();
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                .map_err(|_| invalid())?,
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

// The saturating i32 forms clamp through a bound held in operand 3, an
// early-clobber scratch the encoding defines itself on every target.
fn saturation_scratch(
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::custody();
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(IntegerSign::Signed, 64)
                .map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand: 3,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}

// The signed remainder encoding defines its high-half scratch itself. Unlike
// unsigned division, there is no incoming zero value to materialize or trust.
fn remainder_scratch(
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::custody();
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(IntegerSign::Signed, 64)
                .map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand: 3,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}

// The x86 high half is an explicit zero-valued implementation register.
fn division_scratch(
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::custody();
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                .map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand: 0,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        builder.constraints.keys.materialize_i64,
        &[id],
        Default::default(),
    )?;
    Ok(id)
}
