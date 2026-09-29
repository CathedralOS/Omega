//! Replay one retained §5b representation re-view of a scalar place.
//!
//! Stage 04 retains `&place as &T` (inline, or read through a loan local bound
//! to one) as `CheckedScalarExpression::RepresentationView`: the source place's
//! own read at its declared carrier, wrapped in the view carrier. The source
//! read therefore still passes every declared-type check its ordinary lowering
//! applies -- a field observation keeps the carrier its declaration names --
//! and this module replays the carrier relation from the two types alone
//! instead of trusting the retained pairing.
//!
//! The relation replayed here is the one Terminal Psi can already spell:
//!
//! - Two fixed integer carriers of equal width hold the same two's-complement
//!   bits, so the view reads the source value's modular image at the view
//!   width. `wrapping_cast` composes that image from admitted Terminal
//!   operations; at equal width it never drops a bit.
//! - A retained view must change its carrier. A same-carrier recast only
//!   changes facts and stage 04 reads the place directly, so an identity pair
//!   here is a malformed judgment rather than a no-op to accept silently.
//! - A width change, and every pairing of a Boolean or IEEE float carrier
//!   with another carrier, reject. Equal byte counts do not make `bool` and
//!   `u8` or `f32` and `u32` interchangeable values: those views need a
//!   Terminal re-view vocabulary (bit reinterpretation, 0/1 landing) that
//!   Terminal Psi does not have yet, and no arithmetic spelling may stand in
//!   for it.
//!
//! The operand must observe one place. A re-view of a computed value would be
//! a conversion, which the recast contract forbids a cast to perform.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::ScalarType;

pub(crate) fn replay(
    operand: LoweredDirectExpression,
    view: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let source = operand.scalar_type();
    if source == view {
        return unsupported("a retained representation re-view must change its carrier");
    }
    match (source, view) {
        (ScalarType::Integer(source_type), ScalarType::Integer(view_type)) => {
            if source_type.bits() != view_type.bits() {
                return unsupported("a representation re-view cannot change its carrier width");
            }
            if !observes_one_place(&operand) {
                return unsupported("a representation re-view must observe one scalar place");
            }
            crate::expression_preparation::wrapping_cast::wrapping_cast(operand, source_type, view)
        }
        _ => unsupported(
            "a representation re-view between Boolean or IEEE float and other carriers has no \
             Terminal Psi vocabulary",
        ),
    }
}

fn observes_one_place(operand: &LoweredDirectExpression) -> bool {
    matches!(
        operand,
        LoweredDirectExpression::PrimitiveRead { .. }
            | LoweredDirectExpression::StructuralField { .. }
            | LoweredDirectExpression::IndexedPrimitiveRead { .. }
            | LoweredDirectExpression::IndexedElementFieldRead { .. }
            | LoweredDirectExpression::ElementViewRead { .. }
            | LoweredDirectExpression::Parameter { .. }
            | LoweredDirectExpression::Local { .. }
    )
}

#[cfg(test)]
mod tests;
