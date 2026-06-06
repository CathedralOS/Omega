use crate::{RuntimeStorageRegion, StateGuardOperator};
use omega_core::arena::{Arena, Handle};

// The value operand is structurally identical across stages, so the target layer
// shares the ONE canonical definition from omega-abstract-operations rather than
// re-declaring it. `TargetValueOperand`/`RuntimeValueOperand` are just this layer's
// names for it; the per-stage arenas still differ (handles are remapped between
// them by the abstract->target conversion).
pub use omega_abstract_operations::ValueOperand;
pub type TargetValueOperand = ValueOperand;
pub type RuntimeValueOperand = ValueOperand;

pub type TargetValueOperandHandle = Handle<TargetValueOperand>;
pub type RuntimeValueOperandHandle = TargetValueOperandHandle;

pub trait RuntimeValueOperandSource {
    fn immediate_integer(&self, handle: RuntimeValueOperandHandle) -> Option<i64>;
    fn storage(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, usize)>;
    fn pointee(&self, handle: RuntimeValueOperandHandle) -> Option<(usize, usize, usize)>;
    fn frame_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)>;
    fn frame_base_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)>;
    fn frame_fixed_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)>;
    fn binary(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        RuntimeValueOperandHandle,
        StateGuardOperator,
        RuntimeValueOperandHandle,
    )>;
    /// Whether a `Binary` operand is floating-point (SSE op) rather than integer.
    /// Returns false for non-binary operands. Kept separate from `binary()` so the
    /// existing tuple accessor (and its many callers) stays unchanged.
    fn binary_is_float(&self, handle: RuntimeValueOperandHandle) -> bool;
    /// A `Convert` (numeric cast) operand: `(source, source_byte_size,
    /// target_byte_size, source_is_float, target_is_float, source_signed)`.
    fn convert(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, usize, usize, bool, bool, bool)>;
}

impl RuntimeValueOperandSource for Arena<RuntimeValueOperand> {
    fn immediate_integer(&self, handle: RuntimeValueOperandHandle) -> Option<i64> {
        match self.get(handle) {
            RuntimeValueOperand::Immediate(value) => Some(*value),
            _ => None,
        }
    }

    fn storage(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::Storage {
                region,
                byte_offset,
                byte_size,
            } => Some((*region, *byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn pointee(&self, handle: RuntimeValueOperandHandle) -> Option<(usize, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Some((*pointer_byte_offset, *field_byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn frame_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::FrameIndexed {
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
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::FrameBaseIndexed {
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
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::FrameFixedIndexed {
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
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        RuntimeValueOperandHandle,
        StateGuardOperator,
        RuntimeValueOperandHandle,
    )> {
        match self.get(handle) {
            RuntimeValueOperand::Binary {
                left,
                operator,
                right,
                ..
            } => Some((*left, *operator, *right)),
            _ => None,
        }
    }

    fn binary_is_float(&self, handle: RuntimeValueOperandHandle) -> bool {
        matches!(
            self.get(handle),
            RuntimeValueOperand::Binary { is_float: true, .. }
        )
    }

    fn convert(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, usize, usize, bool, bool, bool)> {
        match self.get(handle) {
            RuntimeValueOperand::Convert {
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
