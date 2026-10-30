use crate::{SaturatingCarrier, TrappingOperation};
use register_model::RegisterConstraintKey;

use super::MachineSemanticKind;
use crate::SelectedConstraintKeys;

impl SelectedConstraintKeys {
    /// One stable physical ABI roster, independently of invocation completion.
    pub fn ordinary_call_constraints(&self) -> Vec<RegisterConstraintKey> {
        let mut rows = Vec::new();
        for key in self
            .call_unit
            .iter()
            .chain(&self.call_unit_mixed)
            .chain(&self.call_scalar)
            .chain(&self.call_aggregate)
        {
            if !rows.contains(key) {
                rows.push(*key);
            }
        }
        rows
    }

    pub fn in_identity_order(&self) -> Vec<RegisterConstraintKey> {
        [
            self.placed_read,
            self.placed_write,
            self.port_write,
            self.system_register_read,
            self.system_register_write,
            self.control_register_read,
            self.control_register_write,
            self.port_register_read,
            self.port_register_write,
            self.model_specific_read,
            self.model_specific_write,
            self.processor_halt,
            self.interrupt_control,
            self.memory_fence,
            self.atomic_fence,
            self.atomic_load,
            self.atomic_store,
            self.atomic_update,
            self.atomic_compare_exchange,
            // LSE uses the ordinary update row; a software-retry target can
            // select its separate interference contract without duplicating
            // an identical hardware row in the inventory.
            if self.atomic_bitwise_update == self.atomic_update {
                None
            } else {
                self.atomic_bitwise_update
            },
            self.copy_bytes,
            self.load_encoded_scalar,
            self.store_encoded_scalar,
            self.zero_bytes,
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
            if self.ieee_float_directed_unary == self.ieee_float_unary {
                None
            } else {
                self.ieee_float_directed_unary
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
            MachineSemanticKind::PlacedScalarAccess(form) => {
                return if form.write {
                    self.rows.placed_write
                } else {
                    self.rows.placed_read
                };
            }
            MachineSemanticKind::PortWrite => return self.rows.port_write,
            MachineSemanticKind::SystemRegisterRead => return self.rows.system_register_read,
            MachineSemanticKind::SystemRegisterWrite => return self.rows.system_register_write,
            MachineSemanticKind::ControlRegisterRead => return self.rows.control_register_read,
            MachineSemanticKind::ControlRegisterWrite => return self.rows.control_register_write,
            MachineSemanticKind::PortRegisterRead => return self.rows.port_register_read,
            MachineSemanticKind::PortRegisterWrite => return self.rows.port_register_write,
            MachineSemanticKind::ModelSpecificRead => return self.rows.model_specific_read,
            MachineSemanticKind::ModelSpecificWrite => return self.rows.model_specific_write,
            MachineSemanticKind::ProcessorHalt => return self.rows.processor_halt,
            MachineSemanticKind::InterruptControl => return self.rows.interrupt_control,
            MachineSemanticKind::MemoryFence => return self.rows.memory_fence,
            MachineSemanticKind::AtomicFence => return self.rows.atomic_fence,
            MachineSemanticKind::AtomicAccess(form) => {
                return match form.operation {
                    crate::AtomicOperation::Load => self.rows.atomic_load,
                    crate::AtomicOperation::Store => self.rows.atomic_store,
                    crate::AtomicOperation::CompareExchange => self.rows.atomic_compare_exchange,
                    crate::AtomicOperation::FetchAnd
                    | crate::AtomicOperation::FetchOr
                    | crate::AtomicOperation::FetchXor => self.rows.atomic_bitwise_update,
                    _ => self.rows.atomic_update,
                };
            }
            MachineSemanticKind::NearestIeeeFloatArithmetic(form) => {
                if form.operation.rounding_direction().is_some() {
                    return match form.operand_count() {
                        1 => self.rows.ieee_float_directed_unary,
                        2 => self.rows.ieee_float_directed_binary,
                        _ => None,
                    };
                }
                return match form.operand_count() {
                    1 => match form.operation {
                        crate::IeeeFloatArithmeticOperation::IsNan
                        | crate::IeeeFloatArithmeticOperation::IsInfinite
                        | crate::IeeeFloatArithmeticOperation::IsFinite
                        | crate::IeeeFloatArithmeticOperation::IsNormal
                        | crate::IeeeFloatArithmeticOperation::IsSubnormal
                        | crate::IeeeFloatArithmeticOperation::Classify
                        | crate::IeeeFloatArithmeticOperation::FromInteger(
                            crate::IeeeFloatIntegerCarrier::U64,
                        )
                        | crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                            target: crate::IeeeFloatIntegerCarrier::U64,
                            ..
                        }
                        | crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                            domain: crate::IeeeFloatToIntegerDomain::Trapping,
                            ..
                        } => self.rows.ieee_float_conversion_control,
                        _ => self.rows.ieee_float_unary,
                    },
                    2 => self.rows.ieee_float_arithmetic,
                    3 => self.rows.ieee_float_ternary,
                    _ => None,
                };
            }
            MachineSemanticKind::FloatResultPolicy(form) => {
                return match form.policy {
                    crate::FloatResultPolicyKind::Trapping => self.rows.float_result_trapping,
                    crate::FloatResultPolicyKind::Saturating => self.rows.float_result_saturating,
                };
            }
            MachineSemanticKind::CopyBytes => return self.rows.copy_bytes,
            MachineSemanticKind::ZeroBytes => return self.rows.zero_bytes,
            MachineSemanticKind::ScanByteSequence => return self.rows.scan_byte_sequence,
            MachineSemanticKind::LoadEncodedScalar => return self.rows.load_encoded_scalar,
            MachineSemanticKind::StoreEncodedScalar => return self.rows.store_encoded_scalar,
            MachineSemanticKind::CompareByteSequences => return self.rows.compare_byte_sequences,
            MachineSemanticKind::Float32ToBits => return self.rows.float32_to_bits,
            MachineSemanticKind::Float64ToBits => return self.rows.float64_to_bits,
            MachineSemanticKind::BitsToFloat32 => return self.rows.bits_to_float32,
            MachineSemanticKind::BitsToFloat64 => return self.rows.bits_to_float64,
            MachineSemanticKind::HostedReadByte => return self.rows.hosted_read_byte,
            MachineSemanticKind::HostedWriteByteI32 => return self.rows.hosted_write_byte_i32,
            MachineSemanticKind::SaveFloatingControl => return self.rows.save_floating_control,
            MachineSemanticKind::RestoreFloatingControl => {
                return self.rows.restore_floating_control;
            }
            MachineSemanticKind::Crash => self.rows.crash,
            MachineSemanticKind::HostedExitProcessI32 => return self.rows.hosted_exit_process_i32,
            MachineSemanticKind::Store => return self.rows.store,
            MachineSemanticKind::AddressOffset => return self.rows.address_offset,
            MachineSemanticKind::Load8Indexed => return self.rows.load8_indexed,
            MachineSemanticKind::Load64 => return self.rows.load64,
            MachineSemanticKind::LoadPacked3
            | MachineSemanticKind::LoadPacked5
            | MachineSemanticKind::LoadPacked6
            | MachineSemanticKind::LoadPacked7 => return self.rows.load_packed,
            MachineSemanticKind::StorePacked => return self.rows.store_packed,
            MachineSemanticKind::Load8 => return self.rows.load8,
            MachineSemanticKind::Load16 => return self.rows.load16,
            MachineSemanticKind::Load32 => return self.rows.load32,
            MachineSemanticKind::Store64 => return self.rows.store64,
            MachineSemanticKind::FrameAddress => return self.rows.frame_address,
            MachineSemanticKind::ImageAddress => {
                return self.rows.byte_literal_image_address;
            }
            MachineSemanticKind::CallUnit | MachineSemanticKind::InvokeNoNormal => return None,
            MachineSemanticKind::CompareI64Zero => self.rows.compare_i64_zero,
            MachineSemanticKind::MaterializeI64 => self.rows.materialize_i64,
            MachineSemanticKind::MaterializeBooleanEqual => self.rows.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanU64LessThan => self.rows.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanI64LessThan => self.rows.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanU64LessOrEqual => self.rows.materialize_boolean,
            MachineSemanticKind::MaterializeBooleanI64LessOrEqual => self.rows.materialize_boolean,

            MachineSemanticKind::CopyI64
            | MachineSemanticKind::ZeroExtendU8
            | MachineSemanticKind::ZeroExtendU16
            | MachineSemanticKind::SignExtendI8
            | MachineSemanticKind::SignExtendI16
            | MachineSemanticKind::SignExtendI32
            | MachineSemanticKind::ZeroExtendU32 => self.rows.copy_i64,
            MachineSemanticKind::ByteViewAddress
            | MachineSemanticKind::ExactAddI64
            | MachineSemanticKind::WrappingAddI64 => self.rows.add_i64,
            // x64 AND/OR/XOR clobber flags, unlike the flag-preserving LEA
            // addition; NOT and its guarding copy do not.
            MachineSemanticKind::BitwiseAndI64 => self.rows.subtract_i64,
            MachineSemanticKind::BitwiseOrI64 => self.rows.subtract_i64,
            MachineSemanticKind::BitwiseXorI64 => self.rows.subtract_i64,
            MachineSemanticKind::BitwiseNotI64 => self.rows.copy_i64,
            MachineSemanticKind::ExactAddI64Immediate => self.rows.add_i64_immediate,
            MachineSemanticKind::ExactSubtractI64 => self.rows.subtract_i64,
            MachineSemanticKind::WrappingSubtractI64 => self.rows.subtract_i64,
            MachineSemanticKind::ExactMultiplyI64 => self.rows.multiply_i64,
            MachineSemanticKind::WrappingMultiplyI64 => self.rows.multiply_i64,
            MachineSemanticKind::ExactDivideU64 => self.rows.divide_u64,
            MachineSemanticKind::ExactRemainderU64 => self.rows.remainder_u64,
            MachineSemanticKind::WrappingRemainderI64 => self.rows.remainder_i64,
            MachineSemanticKind::WrappingDivideI64 => self.rows.divide_i64,
            // The exact signed forms share the signed divide rows: the same
            // RAX/RDX/RCX pinning serves both policies.
            MachineSemanticKind::ExactDivideI64 => self.rows.divide_i64,
            MachineSemanticKind::ExactRemainderI64 => self.rows.remainder_i64,
            // Every shift reads its value and count sources and defines the
            // result through the same two-source row; x86-64 pins the count
            // to `rcx` while AArch64 leaves all three allocatable.
            MachineSemanticKind::WrappingShiftLeftI64
            | MachineSemanticKind::WrappingShiftRightI64
            | MachineSemanticKind::WrappingShiftRightU64
            | MachineSemanticKind::ExactShiftLeftI64
            | MachineSemanticKind::ExactShiftRightI64
            | MachineSemanticKind::ExactShiftRightU64 => self.rows.shift_i64,
            // Operand shape, not carrier, selects the constraint row: the u64
            // add and every unsigned subtract are three-operand forms, unsigned
            // division shares the exact unsigned divide row, and everything
            // else clamps through the early-clobber bound scratch.
            MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64) => {
                self.rows.saturating_add_u64
            }
            MachineSemanticKind::SaturatingAdd(_) => self.rows.saturating_add_clamped,
            MachineSemanticKind::SaturatingSubtract(carrier) if !carrier.is_signed() => {
                self.rows.saturating_subtract_unsigned
            }
            MachineSemanticKind::SaturatingSubtract(_) => self.rows.saturating_subtract_clamped,
            MachineSemanticKind::SaturatingDivide(carrier) if !carrier.is_signed() => {
                self.rows.divide_u64
            }
            MachineSemanticKind::SaturatingDivide(_) => self.rows.saturating_divide_signed,
            // The mathematical remainder always lies inside its carrier, so
            // saturating remainder shares the ordinary signed and unsigned
            // remainder rows rather than owning a clamped form.
            MachineSemanticKind::SaturatingRemainder(carrier) if !carrier.is_signed() => {
                self.rows.remainder_u64
            }
            MachineSemanticKind::SaturatingRemainder(_) => self.rows.remainder_i64,
            // Only u64 needs the full product's high half beyond what the
            // clamped four-operand row provides; on x86-64 that is `MUL` on
            // fixed RAX:RDX.
            MachineSemanticKind::SaturatingMultiply(SaturatingCarrier::U64) => {
                self.rows.saturating_multiply_u64
            }
            MachineSemanticKind::SaturatingMultiply(_) => self.rows.saturating_multiply_clamped,
            // Operand shape selects the Trapping row: division and the u64
            // multiply need the x86-64 RAX:RDX pair, shifts need the x86-64
            // CL count, a conversion reads one input, and every other form
            // is the allocatable four-operand row.
            MachineSemanticKind::TrappingInteger(form) => match form.operation {
                TrappingOperation::Divide | TrappingOperation::Remainder => {
                    self.rows.trapping_fixed_pair
                }
                TrappingOperation::Multiply if matches!(form.carrier, SaturatingCarrier::U64) => {
                    self.rows.trapping_fixed_pair
                }
                TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
                    self.rows.trapping_shift
                }
                TrappingOperation::Convert { .. } => self.rows.trapping_convert,
                TrappingOperation::Add
                | TrappingOperation::Subtract
                | TrappingOperation::Multiply => self.rows.trapping_binary,
            },
            MachineSemanticKind::ExactSubtractI64Immediate => self.rows.subtract_i64_immediate,
            MachineSemanticKind::ConditionalBranchNonZero => self.rows.conditional_branch,
            MachineSemanticKind::ReturnScalar => self.rows.return_i64,
            MachineSemanticKind::ReturnUnit => self.rows.return_unit,
            MachineSemanticKind::CompareI64 => self.rows.compare_i64,
            MachineSemanticKind::CompareI64Immediate => self.rows.compare_i64_immediate,
            MachineSemanticKind::ConditionalBranchU64LessThan => self.rows.conditional_branch,
            MachineSemanticKind::ConditionalBranchI64LessThan => self.rows.conditional_branch,
            MachineSemanticKind::CallScalar
            | MachineSemanticKind::CallAggregate
            | MachineSemanticKind::ReturnAggregate => return None,
            // Normalized foreign calls carry their ABI views on the per-plan
            // `call_normalized_foreign` roster, not a single fixed key; the
            // evaluated boundary plan selects the row, so the semantic keeps
            // failing closed at single-key lookup.
            MachineSemanticKind::NormalizedForeignCall => return None,
            MachineSemanticKind::Jump => self.rows.jump,
        })
    }

    /// Canonical catalog coordinates of these selected keys, in semantic
    /// order and then authored ABI arity. The catalog declares every form the
    /// target can encode, x86 FMA3 included; only a realization carrying
    /// admitted AVX+FMA3 custody can select that form.
    pub fn declaration_keys(&self) -> Vec<(MachineSemanticKind, RegisterConstraintKey)> {
        MachineSemanticKind::ALL
            .into_iter()
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
                        | MachineSemanticKind::InvokeNoNormal
                        | MachineSemanticKind::CallUnit
                        | MachineSemanticKind::CallAggregate
                        | MachineSemanticKind::ReturnAggregate
                        | MachineSemanticKind::NormalizedForeignCall
                ) {
                    (if semantic == MachineSemanticKind::InvokeNoNormal {
                        &self.invoke_no_normal
                    } else if semantic == MachineSemanticKind::CallAggregate {
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
