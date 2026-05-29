use crate::{AssignedTargetOperationPlan, AssignedValueOperandKind};
use omega_target_operations::{RuntimeStorageRegion, StateGuardOperator};

impl omega_target_operations::RuntimeValueOperandSource for AssignedTargetOperationPlan {
    fn immediate_integer(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<i64> {
        match AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Immediate(value) => Some(value),
            _ => None,
        }
    }

    fn storage(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Storage {
                region,
                byte_offset,
                byte_size,
            } => Some((*region, *byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn pointee(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Some((*pointer_byte_offset, *field_byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn frame_indexed(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *descriptor_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
                *byte_size,
            )),
            _ => None,
        }
    }

    fn frame_base_indexed(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *base_byte_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
                *byte_size,
            )),
            _ => None,
        }
    }

    fn frame_fixed_indexed(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *descriptor_offset,
                *element_index,
                *element_byte_size,
                *field_byte_offset,
                *byte_size,
            )),
            _ => None,
        }
    }

    fn binary(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(
        omega_target_operations::RuntimeValueOperandHandle,
        StateGuardOperator,
        omega_target_operations::RuntimeValueOperandHandle,
    )> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Binary {
                left,
                operator,
                right,
            } => Some((*left, *operator, *right)),
            _ => None,
        }
    }
}
