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
                ..
            } => Some((*left, *operator, *right)),
            _ => None,
        }
    }

    fn binary_is_float(&self, handle: omega_target_operations::RuntimeValueOperandHandle) -> bool {
        AssignedTargetOperationPlan::runtime_value_operand(self, handle).is_some_and(|operand| {
            matches!(
                operand.kind,
                AssignedValueOperandKind::Binary { is_float: true, .. }
            )
        })
    }

    fn binary_byte_width(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<usize> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Binary { byte_width, .. } => Some(*byte_width),
            _ => None,
        }
    }

    fn text_equals(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, RuntimeStorageRegion, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::TextEquals {
                left_region,
                left_offset,
                right_region,
                right_offset,
            } => Some((*left_region, *left_offset, *right_region, *right_offset)),
            _ => None,
        }
    }

    fn text_equals_literal(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(omega_target_operations::RuntimeValueOperandHandle, String)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::TextEqualsLiteral { place, literal } => {
                Some((*place, literal.clone()))
            }
            _ => None,
        }
    }

    fn convert(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(
        omega_target_operations::RuntimeValueOperandHandle,
        usize,
        usize,
        bool,
        bool,
        bool,
    )> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Convert {
                source,
                source_byte_size,
                target_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
            } => Some((
                *source,
                *source_byte_size,
                *target_byte_size,
                *source_is_float,
                *target_is_float,
                *source_signed,
            )),
            _ => None,
        }
    }
}
