use register_model::RegisterConstraintKey;

use super::MachineSemanticKind;
use crate::SelectedConstraintKeys;

impl SelectedConstraintKeys {
    pub fn in_identity_order(&self) -> Vec<RegisterConstraintKey> {
        [
            self.hosted_read_byte,
            self.hosted_write_byte_i32,
            self.hosted_exit_process_i32,
            self.store,
            self.address_offset,
            self.load64,
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
        .chain(self.call_i64.iter().copied())
        .chain([
            self.materialize_i64,
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
        ])
        .collect()
    }

    pub const fn for_semantic(
        &self,
        semantic: MachineSemanticKind,
    ) -> Option<RegisterConstraintKey> {
        Some(match semantic {
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
            MachineSemanticKind::Load32 => return self.load32,
            MachineSemanticKind::Store64 => return self.store64,
            MachineSemanticKind::FrameAddress => return self.frame_address,
            MachineSemanticKind::CallUnit => return None,
            MachineSemanticKind::CompareI64Zero => self.compare_i64_zero,
            MachineSemanticKind::MaterializeI64 => self.materialize_i64,
            MachineSemanticKind::CopyI64
            | MachineSemanticKind::ZeroExtendU8
            | MachineSemanticKind::ZeroExtendU32 => self.copy_i64,
            MachineSemanticKind::ByteViewAddress | MachineSemanticKind::ExactAddI64 => self.add_i64,
            MachineSemanticKind::ExactAddI64Immediate => self.add_i64_immediate,
            MachineSemanticKind::ExactSubtractI64 => self.subtract_i64,
            MachineSemanticKind::ExactSubtractI64Immediate => self.subtract_i64_immediate,
            MachineSemanticKind::ConditionalBranchNonZero => self.conditional_branch,
            MachineSemanticKind::ReturnI64 => self.return_i64,
            MachineSemanticKind::ReturnUnit => self.return_unit,
            MachineSemanticKind::CompareI64 => self.compare_i64,
            MachineSemanticKind::ConditionalBranchU64LessThan => self.conditional_branch,
            MachineSemanticKind::ConditionalBranchI64LessThan => self.conditional_branch,
            MachineSemanticKind::CallI64 => return None,
            MachineSemanticKind::Jump => self.jump,
        })
    }

    /// Canonical catalog coordinates: semantic order, then authored ABI arity.
    pub fn declaration_keys(&self) -> Vec<(MachineSemanticKind, RegisterConstraintKey)> {
        MachineSemanticKind::ALL
            .into_iter()
            .flat_map(|semantic| {
                if matches!(
                    semantic,
                    MachineSemanticKind::CallI64 | MachineSemanticKind::CallUnit
                ) {
                    (if semantic == MachineSemanticKind::CallUnit {
                        &self.call_unit
                    } else {
                        &self.call_i64
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
