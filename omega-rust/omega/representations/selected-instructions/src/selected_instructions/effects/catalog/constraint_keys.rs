use crate::SaturatingCarrier;
use register_model::RegisterConstraintKey;

use super::MachineSemanticKind;
use crate::SelectedConstraintKeys;

impl SelectedConstraintKeys {
    pub fn in_identity_order(&self) -> Vec<RegisterConstraintKey> {
        [
            self.copy_bytes,
            self.hosted_read_byte,
            self.hosted_write_byte_i32,
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
            self.float32_to_bits,
            self.float64_to_bits,
            self.bits_to_float32,
            self.bits_to_float64,
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
            self.remainder_i64,
            self.saturating_add_clamped,
            self.saturating_subtract_clamped,
            self.saturating_divide_signed,
            self.multiply_i64,
        ])
        .collect()
    }

    pub const fn for_semantic(
        &self,
        semantic: MachineSemanticKind,
    ) -> Option<RegisterConstraintKey> {
        Some(match semantic {
            MachineSemanticKind::CopyBytes => return self.copy_bytes,
            MachineSemanticKind::Float32ToBits => return self.float32_to_bits,
            MachineSemanticKind::Float64ToBits => return self.float64_to_bits,
            MachineSemanticKind::BitsToFloat32 => return self.bits_to_float32,
            MachineSemanticKind::BitsToFloat64 => return self.bits_to_float64,
            MachineSemanticKind::HostedReadByte => return self.hosted_read_byte,
            MachineSemanticKind::HostedWriteByteI32 => return self.hosted_write_byte_i32,
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
            // x64 AND clobbers flags, unlike the flag-preserving LEA addition.
            MachineSemanticKind::BitwiseAndI64 => self.subtract_i64,
            MachineSemanticKind::BitwiseXorI64 => self.subtract_i64,
            MachineSemanticKind::ExactAddI64Immediate => self.add_i64_immediate,
            MachineSemanticKind::ExactSubtractI64 => self.subtract_i64,
            MachineSemanticKind::ExactMultiplyI64 => self.multiply_i64,
            MachineSemanticKind::ExactDivideU64 => self.divide_u64,
            MachineSemanticKind::WrappingRemainderI64 => self.remainder_i64,
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

    /// Canonical catalog coordinates: semantic order, then authored ABI arity.
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
