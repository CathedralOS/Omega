use omega_target_operations::TargetValueOperand;

use crate::remap;

/// The value operand is the one shared type across stages, so translating it to
/// the target arena is just remapping the recursive `Binary` handles -- the
/// non-recursive variants are copied verbatim by `map_handles`. (This replaced a
/// hand-written field-by-field match that had to be kept in sync with the enum.)
pub(crate) fn translate_runtime_value_operand(
    operand: &omega_abstract_operations::AbstractValueOperand,
) -> TargetValueOperand {
    operand.map_handles(remap::runtime_value_handle)
}
