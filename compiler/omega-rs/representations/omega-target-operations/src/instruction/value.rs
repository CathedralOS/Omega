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
    /// The resolved scalar byte width of a `Binary` operand's result, threaded
    /// from build time so the float emission picks single (`addss`) vs double
    /// (`addsd`) precision instead of re-deriving/hardcoding it. `None` for
    /// non-binary operands; callers default to 8 (the historical width).
    fn binary_byte_width(&self, handle: RuntimeValueOperandHandle) -> Option<usize>;
    /// A `Convert` (numeric cast) operand: `(source, source_byte_size,
    /// target_byte_size, source_is_float, target_is_float, source_signed)`.
    fn convert(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, usize, usize, bool, bool, bool)>;
    /// A `TextEquals` (value-position text content compare) operand:
    /// `(left_region, left_offset, right_region, right_offset)` of the two
    /// `{ptr, len}` text descriptor places. Evaluates to bool 0/1.
    fn text_equals(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, RuntimeStorageRegion, usize)>;
    /// A `TextEqualsLiteral` (guard-position text content compare against an
    /// inline literal) operand: `(place, literal)` where `place` is the String
    /// side's 16-byte text descriptor place operand. Evaluates to bool 0/1.
    fn text_equals_literal(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, String)>;
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

    fn binary_byte_width(&self, handle: RuntimeValueOperandHandle) -> Option<usize> {
        match self.get(handle) {
            RuntimeValueOperand::Binary { byte_width, .. } => Some(*byte_width),
            _ => None,
        }
    }

    fn text_equals(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, RuntimeStorageRegion, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::TextEquals {
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
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, String)> {
        match self.get(handle) {
            RuntimeValueOperand::TextEqualsLiteral { place, literal } => {
                Some((*place, literal.clone()))
            }
            _ => None,
        }
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
