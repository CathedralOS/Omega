//! The raw-width normalization a widening, an exact cast or a scalar
//! representation re-view selects.
//!
//! Construction emits it and validation reconstructs it, so it belongs to
//! neither: an independent replay that read the producer would be checking
//! the producer against itself.
//!
//! A re-view needs no floating-point register move. Every scalar this graph
//! holds lives in a general register as its stored bits normalized to its
//! carrier: an integer extended by its sign, a Boolean as the byte `0` or
//! `1`, and an IEEE value as its raw interchange bits zero-extended (a
//! binary32 constant materializes its `u32` bits, a field load zero-extends
//! four bytes, and IEEE comparison and arithmetic read those bits). Reading
//! the same bits at the other carrier is therefore exactly the result
//! carrier's normalization of the operand register: `f32` to `i32`
//! sign-extends bit 31, `i32` to `f32` clears the high half, and the 64-bit
//! and Boolean-byte pairs are already in their target form.

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
    if let LegalizedScalarInstructionKind::ScalarRepresentationView { source_type, .. } = kind {
        terminal_semantics::ScalarRepresentationViewPair::classify(*source_type, scalar_type)?;
        return Some(register_form_normalization(scalar_type));
    }
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

/// The normalization that leaves a general register holding `scalar_type`'s
/// stored bits in this graph's register form.
fn register_form_normalization(scalar_type: ScalarType) -> selected_instructions::SelectedInstructionKind {
    match scalar_type {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            selected_instructions::SelectedInstructionKind::ZeroExtendU32
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            selected_instructions::SelectedInstructionKind::CopyI64
        }
        other => crate::selection::scalar_call_abi::integer_carrier_normalization(other),
    }
}
