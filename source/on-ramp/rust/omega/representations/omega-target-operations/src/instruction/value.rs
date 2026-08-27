use crate::{RuntimeStorageRegion, StateGuardOperator};
use psi_arena::{Arena, Handle};

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
    fn bit_field(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        RuntimeStorageRegion,
        usize,
        usize,
        Vec<omega_abstract_operations::RuntimeBitFieldFragment>,
    )>;
    fn pointee(&self, handle: RuntimeValueOperandHandle) -> Option<(usize, usize, usize)>;
    fn frame_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        usize,
        RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    )>;
    fn frame_base_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize, usize)>;
    fn frame_fixed_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)>;
    /// A `MachineIndexed` operand: `(base_byte_offset, index_region,
    /// index_offset, index_byte_size, element_byte_size, field_byte_offset,
    /// byte_size)`.
    fn machine_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        usize,
        RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    )>;
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
    /// The decision-17 arithmetic domain of a `Binary` operand and whether its
    /// operands are SIGNED integers, both resolved at build time from the
    /// operands' declared types. Drives the Saturating/Trapping
    /// operand-position lowering (clamp bounds / overflow flag choice).
    /// `None` for non-binary operands.
    fn binary_arithmetic_domain(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(psi_numerics::arithmetic::ArithmeticDomain, bool)>;
    /// A `Convert` (numeric cast) operand: `(source, source_byte_size,
    /// target_byte_size, source_is_float, target_is_float, source_signed)`.
    fn convert(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, usize, usize, bool, bool, bool)>;
    /// F4: whether a `Convert` operand is a TRAPPING float->int cast (traps
    /// on NaN/out-of-range before converting). False for non-convert
    /// operands. Kept separate from `convert()` so the existing tuple
    /// accessor (and its many callers) stays unchanged.
    fn convert_trapping(&self, handle: RuntimeValueOperandHandle) -> bool;
    /// F4: whether a `Convert` operand is a SATURATING float->int cast.
    fn convert_saturating(&self, handle: RuntimeValueOperandHandle) -> bool;
    /// Whether a `Convert` operand's integer target is signed.
    fn convert_target_signed(&self, handle: RuntimeValueOperandHandle) -> bool;
    /// A `TextEquals` (value-position text content compare) operand:
    /// `(left_region, left_offset, left_is_bounded_buffer, right_region,
    /// right_offset, right_is_bounded_buffer)` of the two text places.
    /// Evaluates to bool 0/1.
    fn text_equals(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        RuntimeStorageRegion,
        usize,
        bool,
        RuntimeStorageRegion,
        usize,
        bool,
    )>;
    /// A `TextEqualsLiteral` (guard-position text content compare against an
    /// inline literal) operand: `(place, literal, place_is_bounded_buffer)` where
    /// `place` is the text side's place operand and the bool flags an owned
    /// `[u8; N]` carrier (`{len, bytes}` inline) so the encoder uses carrier
    /// addressing. Evaluates to bool 0/1.
    fn text_equals_literal(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, std::sync::Arc<[u8]>, bool)>;
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

    fn bit_field(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        RuntimeStorageRegion,
        usize,
        usize,
        Vec<omega_abstract_operations::RuntimeBitFieldFragment>,
    )> {
        match self.get(handle) {
            RuntimeValueOperand::BitField {
                region,
                base_byte_offset,
                value_byte_size,
                fragments,
            } => Some((
                *region,
                *base_byte_offset,
                *value_byte_size,
                fragments.clone(),
            )),
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
    ) -> Option<(
        usize,
        RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    )> {
        match self.get(handle) {
            RuntimeValueOperand::FrameIndexed {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *descriptor_offset,
                *index_region,
                *index_offset,
                *index_byte_size,
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
    ) -> Option<(usize, usize, usize, usize, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *base_byte_offset,
                *index_offset,
                *index_byte_size,
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

    fn machine_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        usize,
        RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    )> {
        match self.get(handle) {
            RuntimeValueOperand::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *base_byte_offset,
                *index_region,
                *index_offset,
                *index_byte_size,
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

    fn binary_arithmetic_domain(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(psi_numerics::arithmetic::ArithmeticDomain, bool)> {
        match self.get(handle) {
            RuntimeValueOperand::Binary {
                arithmetic_domain,
                operands_signed,
                ..
            } => Some((*arithmetic_domain, *operands_signed)),
            _ => None,
        }
    }

    fn text_equals(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(
        RuntimeStorageRegion,
        usize,
        bool,
        RuntimeStorageRegion,
        usize,
        bool,
    )> {
        match self.get(handle) {
            RuntimeValueOperand::TextEquals {
                left_region,
                left_offset,
                left_is_bounded_buffer,
                right_region,
                right_offset,
                right_is_bounded_buffer,
            } => Some((
                *left_region,
                *left_offset,
                *left_is_bounded_buffer,
                *right_region,
                *right_offset,
                *right_is_bounded_buffer,
            )),
            _ => None,
        }
    }

    fn text_equals_literal(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, std::sync::Arc<[u8]>, bool)> {
        match self.get(handle) {
            RuntimeValueOperand::TextEqualsLiteral {
                place,
                literal,
                place_is_bounded_buffer,
            } => Some((*place, literal.clone(), *place_is_bounded_buffer)),
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
                ..
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

    fn convert_trapping(&self, handle: RuntimeValueOperandHandle) -> bool {
        matches!(
            self.get(handle),
            RuntimeValueOperand::Convert { trapping: true, .. }
        )
    }

    fn convert_saturating(&self, handle: RuntimeValueOperandHandle) -> bool {
        matches!(
            self.get(handle),
            RuntimeValueOperand::Convert {
                saturating: true,
                ..
            }
        )
    }

    fn convert_target_signed(&self, handle: RuntimeValueOperandHandle) -> bool {
        matches!(
            self.get(handle),
            RuntimeValueOperand::Convert {
                target_signed: true,
                ..
            }
        )
    }
}
