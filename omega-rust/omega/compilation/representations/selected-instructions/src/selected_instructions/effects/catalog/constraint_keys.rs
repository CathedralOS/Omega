use crate::{SaturatingCarrier, TrappingOperation};
use register_model::RegisterConstraintKey;

use super::MachineSemanticKind;
use crate::SelectedConstraintKeys;

impl SelectedConstraintKeys {
    pub fn in_identity_order(&self) -> Vec<RegisterConstraintKey> {
        [
            self.atomic_load,
            self.atomic_store,
            self.atomic_update,
            self.atomic_compare_exchange,
            self.copy_bytes,
            self.hosted_read_byte,
            self.hosted_write_byte_i32,
            self.save_floating_control,
            self.restore_floating_control,
            self.hosted_exit_process_i32,
            self.store,
            self.address_offset,
            self.load64,
            self.load8,
            self.load16,
            self.load32,
            self.load8_indexed,
            self.store64,
            self.frame_address,
            self.byte_literal_image_address,
            self.float32_to_bits,
            self.float64_to_bits,
            self.bits_to_float32,
            self.bits_to_float64,
            self.ieee_float_arithmetic,
            if self.ieee_float_directed_binary == self.ieee_float_arithmetic {
                None
            } else {
                self.ieee_float_directed_binary
            },
            self.ieee_float_unary,
            // AArch64's conversion forms are flag-free and intentionally
            // alias the ordinary unary row. Only a distinct target-owned row
            // contributes another inventory key.
            if self.ieee_float_conversion_control == self.ieee_float_unary {
                None
            } else {
                self.ieee_float_conversion_control
            },
            self.ieee_float_ternary,
        ]
        .into_iter()
        .flatten()
        .chain(self.call_unit.iter().copied())
        .chain(self.call_unit_mixed.iter().copied())
        .chain(self.call_scalar.iter().copied())
        .chain(self.call_aggregate.iter().copied())
        .chain(self.call_normalized_foreign.iter().copied())
        .chain(self.return_aggregate.iter().copied())
        .chain(self.return_float.iter().copied())
        .chain([
            self.crash,
            self.materialize_i64,
            self.materialize_boolean,
            self.copy_i64,
            self.add_i64,
            self.add_i64_immediate,
            self.subtract_i64,
            self.subtract_i64_immediate,
            self.compare_i64_zero,
            self.conditional_branch,
            self.return_i64,
            self.return_unit,
            self.compare_i64,
            self.jump,
            self.compare_i64_immediate,
            self.saturating_subtract_unsigned,
            self.saturating_add_u64,
            self.divide_u64,
            self.remainder_u64,
            self.remainder_i64,
            self.divide_i64,
            self.shift_i64,
            self.saturating_add_clamped,
            self.saturating_subtract_clamped,
            self.saturating_divide_signed,
            self.multiply_i64,
            self.saturating_multiply_clamped,
            self.saturating_multiply_u64,
            self.trapping_binary,
            self.trapping_fixed_pair,
            self.trapping_shift,
            self.trapping_convert,
        ])
        .chain(
            [self.float_result_trapping, self.float_result_saturating]
                .into_iter()
                .flatten(),
        )
        .collect()
    }

    pub const fn for_semantic(
        &self,
        semantic: MachineSemanticKind,
    ) -> Option<RegisterConstraintKey> {
        Some(match semantic {
            MachineSemanticKind::AtomicAccess(form) => {
                return match form.operation {
                    crate::AtomicOperation::Load => self.atomic_load,
                    crate::AtomicOperation::Store => self.atomic_store,
                    crate::AtomicOperation::CompareExchange => self.atomic_compare_exchange,
                    _ => self.atomic_update,
                };
            }
            MachineSemanticKind::NearestIeeeFloatArithmetic(form) => {
                if form.operation.rounding_direction().is_some() {
                    return self.ieee_float_directed_binary;
                }
                return match form.operand_count() {
                    1 => match form.operation {
                        crate::IeeeFloatArithmeticOperation::FromInteger(
                            crate::IeeeFloatIntegerCarrier::U64,
                        )
                        | crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                            target: crate::IeeeFloatIntegerCarrier::U64,
                            ..
                        }
                        | crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                            domain: crate::IeeeFloatToIntegerDomain::Trapping,
                            ..
                        } => self.ieee_float_conversion_control,
                        _ => self.ieee_float_unary,
                    },
                    2 => self.ieee_float_arithmetic,
                    3 => self.ieee_float_ternary,
                    _ => None,
                };
            }
            MachineSemanticKind::FloatResultPolicy(form) => {
                return match form.policy {
                    crate::FloatResultPolicyKind::Trapping => self.float_result_trapping,
                    crate::FloatResultPolicyKind::Saturating(_) => self.float_result_saturating,
                };
            }
            MachineSemanticKind::CopyBytes => return self.copy_bytes,
            MachineSemanticKind::ScanByteSequence => return self.scan_byte_sequence,
            MachineSemanticKind::Float32ToBits => return self.float32_to_bits,
            MachineSemanticKind::Float64ToBits => return self.float64_to_bits,
            MachineSemanticKind::BitsToFloat32 => return self.bits_to_float32,
            MachineSemanticKind::BitsToFloat64 => return self.bits_to_float64,
            MachineSemanticKind::HostedReadByte => return self.hosted_read_byte,
            MachineSemanticKind::HostedWriteByteI32 => return self.hosted_write_byte_i32,
            MachineSemanticKind::SaveFloatingControl => return self.save_floating_control,
            MachineSemanticKind::RestoreFloatingControl => return self.restore_floating_control,
            MachineSemanticKind::Crash => self.crash,
            MachineSemanticKind::HostedExitProcessI32 => return self.hosted_exit_process_i32,
            MachineSemanticKind::Store => return self.store,
            MachineSemanticKind::AddressOffset => return self.address_offset,
            MachineSemanticKind::Load8Indexed => return self.load8_indexed,
            MachineSemanticKind::Load64 => return self.load64,
            MachineSemanticKind::LoadPacked3
            | MachineSemanticKind::LoadPacked5
            | MachineSemanticKind::LoadPacked6
            | MachineSemanticKind::LoadPacked7 => return self.load_packed,
            MachineSemanticKind::StorePacked => return self.store_packed,
            MachineSemanticKind::Load8 => return self.load8,
            MachineSemanticKind::Load16 => return self.load16,
            MachineSemanticKind::Load32 => return self.load32,
            MachineSemanticKind::Store64 => return self.store64,
            MachineSemanticKind::FrameAddress => return self.frame_address,
            MachineSemanticKind::ByteLiteralImageAddress => {
                return self.byte_literal_image_address;
            }
            MachineSemanticKind::CallUnit => return None,
            MachineSemanticKind::CompareI64Zero => self.compare_i64_zero,
            MachineSemanticKind::MaterializeI64 => self.materialize_i64,
            MachineSemanticKind::MaterializeBooleanEqual => self.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanU64LessThan => self.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanI64LessThan => self.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanU64LessOrEqual => self.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanI64LessOrEqual => self.materialize_boolean,

            MachineSemanticKind::CopyI64
            | MachineSemanticKind::ZeroExtendU8
            | MachineSemanticKind::ZeroExtendU16
            | MachineSemanticKind::SignExtendI8
            | MachineSemanticKind::SignExtendI16
            | MachineSemanticKind::SignExtendI32
            | MachineSemanticKind::ZeroExtendU32 => self.copy_i64,
            MachineSemanticKind::ByteViewAddress
            | MachineSemanticKind::ExactAddI64
            | MachineSemanticKind::WrappingAddI64 => self.add_i64,
            // x64 AND/OR/XOR clobber flags, unlike the flag-preserving LEA
            // addition; NOT and its guarding copy do not.
            MachineSemanticKind::BitwiseAndI64 => self.subtract_i64,
            MachineSemanticKind::BitwiseOrI64 => self.subtract_i64,
            MachineSemanticKind::BitwiseXorI64 => self.subtract_i64,
            MachineSemanticKind::BitwiseNotI64 => self.copy_i64,
            MachineSemanticKind::ExactAddI64Immediate => self.add_i64_immediate,
            MachineSemanticKind::ExactSubtractI64 => self.subtract_i64,
            MachineSemanticKind::WrappingSubtractI64 => self.subtract_i64,
            MachineSemanticKind::ExactMultiplyI64 => self.multiply_i64,
            MachineSemanticKind::WrappingMultiplyI64 => self.multiply_i64,
            MachineSemanticKind::ExactDivideU64 => self.divide_u64,
            MachineSemanticKind::ExactRemainderU64 => self.remainder_u64,
            MachineSemanticKind::WrappingRemainderI64 => self.remainder_i64,
            MachineSemanticKind::WrappingDivideI64 => self.divide_i64,
            // The exact signed forms share the signed divide rows: the same
            // RAX/RDX/RCX pinning serves both policies.
            MachineSemanticKind::ExactDivideI64 => self.divide_i64,
            MachineSemanticKind::ExactRemainderI64 => self.remainder_i64,
            // Every shift reads its value and count sources and defines the
            // result through the same two-source row; x86-64 pins the count
            // to `rcx` while AArch64 leaves all three allocatable.
            MachineSemanticKind::WrappingShiftLeftI64
            | MachineSemanticKind::WrappingShiftRightI64
            | MachineSemanticKind::WrappingShiftRightU64
            | MachineSemanticKind::ExactShiftLeftI64
            | MachineSemanticKind::ExactShiftRightI64
            | MachineSemanticKind::ExactShiftRightU64 => self.shift_i64,
            // Operand shape, not carrier, selects the constraint row: the u64
            // add and every unsigned subtract are three-operand forms, unsigned
            // division shares the exact unsigned divide row, and everything
            // else clamps through the early-clobber bound scratch.
            MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64) => self.saturating_add_u64,
            MachineSemanticKind::SaturatingAdd(_) => self.saturating_add_clamped,
            MachineSemanticKind::SaturatingSubtract(carrier) if !carrier.is_signed() => {
                self.saturating_subtract_unsigned
            }
            MachineSemanticKind::SaturatingSubtract(_) => self.saturating_subtract_clamped,
            MachineSemanticKind::SaturatingDivide(carrier) if !carrier.is_signed() => {
                self.divide_u64
            }
            MachineSemanticKind::SaturatingDivide(_) => self.saturating_divide_signed,
            // The mathematical remainder always lies inside its carrier, so
            // saturating remainder shares the ordinary signed and unsigned
            // remainder rows rather than owning a clamped form.
            MachineSemanticKind::SaturatingRemainder(carrier) if !carrier.is_signed() => {
                self.remainder_u64
            }
            MachineSemanticKind::SaturatingRemainder(_) => self.remainder_i64,
            // Only u64 needs the full product's high half beyond what the
            // clamped four-operand row provides; on x86-64 that is `MUL` on
            // fixed RAX:RDX.
            MachineSemanticKind::SaturatingMultiply(SaturatingCarrier::U64) => {
                self.saturating_multiply_u64
            }
            MachineSemanticKind::SaturatingMultiply(_) => self.saturating_multiply_clamped,
            // Operand shape selects the Trapping row: division and the u64
            // multiply need the x86-64 RAX:RDX pair, shifts need the x86-64
            // CL count, a conversion reads one input, and every other form
            // is the allocatable four-operand row.
            MachineSemanticKind::TrappingInteger(form) => match form.operation {
                TrappingOperation::Divide | TrappingOperation::Remainder => {
                    self.trapping_fixed_pair
                }
                TrappingOperation::Multiply if matches!(form.carrier, SaturatingCarrier::U64) => {
                    self.trapping_fixed_pair
                }
                TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => self.trapping_shift,
                TrappingOperation::Convert { .. } => self.trapping_convert,
                TrappingOperation::Add
                | TrappingOperation::Subtract
                | TrappingOperation::Multiply => self.trapping_binary,
            },
            MachineSemanticKind::ExactSubtractI64Immediate => self.subtract_i64_immediate,
            MachineSemanticKind::ConditionalBranchNonZero => self.conditional_branch,
            MachineSemanticKind::ReturnScalar => self.return_i64,
            MachineSemanticKind::ReturnUnit => self.return_unit,
            MachineSemanticKind::CompareI64 => self.compare_i64,
            MachineSemanticKind::CompareI64Immediate => self.compare_i64_immediate,
            MachineSemanticKind::ConditionalBranchU64LessThan => self.conditional_branch,
            MachineSemanticKind::ConditionalBranchI64LessThan => self.conditional_branch,
            MachineSemanticKind::CallScalar
            | MachineSemanticKind::CallAggregate
            | MachineSemanticKind::ReturnAggregate => return None,
            // Normalized foreign calls carry their ABI views on the per-plan
            // `call_normalized_foreign` roster, not a single fixed key; the
            // evaluated boundary plan selects the row, so the semantic keeps
            // failing closed at single-key lookup.
            MachineSemanticKind::NormalizedForeignCall => return None,
            MachineSemanticKind::Jump => self.jump,
        })
    }

    /// Canonical catalog coordinates supported by this target, in semantic
    /// order and then authored ABI arity. Optional x86 FMA3 cannot enter the
    /// baseline catalog through its shared three-operand register constraint.
    pub fn declaration_keys(
        &self,
        target: target::NativeTarget,
    ) -> Vec<(MachineSemanticKind, RegisterConstraintKey)> {
        MachineSemanticKind::ALL
            .into_iter()
            .filter(|semantic| match semantic {
                MachineSemanticKind::NearestIeeeFloatArithmetic(form) => {
                    target_operations::NearestIeeeFloatArithmeticRealization {
                        operation: form.operation,
                        format: form.format,
                    }
                    .supports_target(target)
                }
                _ => true,
            })
            .flat_map(|semantic| {
                if semantic == MachineSemanticKind::ReturnScalar {
                    return std::iter::once(self.return_i64)
                        .chain(self.return_float.iter().copied())
                        .map(|key| (semantic, key))
                        .collect();
                }
                if matches!(
                    semantic,
                    MachineSemanticKind::CallScalar
                        | MachineSemanticKind::CallUnit
                        | MachineSemanticKind::CallAggregate
                        | MachineSemanticKind::ReturnAggregate
                        | MachineSemanticKind::NormalizedForeignCall
                ) {
                    (if semantic == MachineSemanticKind::CallAggregate {
                        &self.call_aggregate
                    } else if semantic == MachineSemanticKind::ReturnAggregate {
                        &self.return_aggregate
                    } else if semantic == MachineSemanticKind::CallUnit {
                        &self.call_unit
                    } else if semantic == MachineSemanticKind::NormalizedForeignCall {
                        &self.call_normalized_foreign
                    } else {
                        &self.call_scalar
                    })
                    .iter()
                    .chain(if semantic == MachineSemanticKind::CallUnit {
                        self.call_unit_mixed.iter()
                    } else {
                        [].iter()
                    })
                    .copied()
                    .map(|key| (semantic, key))
                    .collect()
                } else {
                    self.for_semantic(semantic)
                        .map(|key| (semantic, key))
                        .into_iter()
                        .collect::<Vec<_>>()
                }
            })
            .collect()
    }
}
