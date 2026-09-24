//! The raw-width normalization a widening or an exact cast selects.
//!
//! Construction emits it and validation reconstructs it, so it belongs to
//! neither: an independent replay that read the producer would be checking
//! the producer against itself.

use legalized_operations::LegalizedScalarInstructionKind;
use semantic_vocabulary::ScalarType;

/// The one normalization a widening or a proof-bearing exact cast selects,
/// or `None` when its carriers are not fixed native integers. Both sides hold
/// the value full-register normalized, and the narrower carrier's
/// normalization states it exactly: a widening extends the source's sign or
/// zero bits, never the destination width's potentially uninitialized high
/// bits, and an exact narrowing is proven inside the target, so the target's
/// own normalization is exact. Construction and validation both read this.
pub(super) fn normalization(
    kind: &LegalizedScalarInstructionKind,
    scalar_type: ScalarType,
) -> Option<selected_instructions::SelectedInstructionKind> {
    let ScalarType::Integer(target) = scalar_type else {
        return None;
    };
    let fixed = |integer: semantic_vocabulary::IntegerType| {
        integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
            && matches!(integer.bits(), 8 | 16 | 32 | 64)
    };
    let narrower = match kind {
        LegalizedScalarInstructionKind::IntegerWiden { source_type, .. }
            if fixed(*source_type) && fixed(target) && source_type.can_widen_to(target) =>
        {
            *source_type
        }
        LegalizedScalarInstructionKind::IntegerExactCast { source_type, .. }
            if crate::legalization::exact_cast_has_native_carriers(*source_type, target) =>
        {
            if source_type.bits() < target.bits() {
                *source_type
            } else {
                target
            }
        }
        _ => return None,
    };
    Some(
        crate::selection::scalar_call_abi::integer_carrier_normalization(ScalarType::Integer(
            narrower,
        )),
    )
}
