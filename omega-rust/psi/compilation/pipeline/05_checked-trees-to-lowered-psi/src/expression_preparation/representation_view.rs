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
//! - A wider integer view whose operand is one runtime-selected `u8` element
//!   is a byte-region view (`&self.buf[offset] as &u32`): the region's bytes
//!   are that element and its successors, read one by one and assembled
//!   little-endian (`byte_region`). Each byte read owes its own Terminal
//!   index obligation, so the region's bounds are re-proved rather than taken
//!   from the recast judgment.
//! - A retained view must change its carrier. A same-carrier recast only
//!   changes facts and stage 04 reads the place directly, so an identity pair
//!   here is a malformed judgment rather than a no-op to accept silently.
//! - Any other width change, and every pairing of a Boolean or IEEE float carrier
//!   with another carrier, reject. Equal byte counts do not make `bool` and
//!   `u8` or `f32` and `u32` interchangeable values: those views need a
//!   Terminal re-view vocabulary (bit reinterpretation, 0/1 landing) that
//!   Terminal Psi does not have yet, and no arithmetic spelling may stand in
//!   for it.
//!
//! The operand must observe one place. A re-view of a computed value would be
//! a conversion, which the recast contract forbids a cast to perform.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};

pub(crate) fn replay(
    operand: LoweredDirectExpression,
    view: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let source = operand.scalar_type();
    if source == view {
        return unsupported("a retained representation re-view must change its carrier");
    }
    match (source, view) {
        (ScalarType::Integer(source_type), ScalarType::Integer(view_type))
            if source_type.bits() == 8
                && source_type.sign() == IntegerSign::Unsigned
                && view_type.bits() > 8 =>
        {
            byte_region(operand, view_type)
        }
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

/// Replay a byte-region view from its first runtime-selected `u8` element:
/// the view's `B / 8` bytes are that element and its successors, assembled
/// little-endian on the unsigned `B`-bit carrier -- the byte order of every
/// admitted layout, and the order the checked interpreter assembles. Each
/// byte is its own element read, so Terminal proves `index + j < extent` for
/// every byte of the footprint instead of trusting the recast judgment's
/// bound. A signed view then reads that carrier's modular image.
fn byte_region(
    operand: LoweredDirectExpression,
    view_type: IntegerType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let LoweredDirectExpression::IndexedPrimitiveRead {
        source,
        path,
        index,
        inner_indexes,
        element_path,
        scalar_type,
    } = operand
    else {
        return unsupported("a byte-region re-view must start at one runtime-selected byte");
    };
    if !inner_indexes.is_empty() || !element_path.is_empty() {
        return unsupported("a byte-region re-view must select a byte of one flat array");
    }
    let bits = view_type.bits();
    if bits % 8 != 0 {
        return unsupported("a byte-region re-view needs a whole-byte carrier");
    }
    let region_type = IntegerType::new(IntegerSign::Unsigned, bits)
        .map_err(|_| LoweringError::Unsupported("a byte-region carrier has no unsigned twin"))?;
    let region = ScalarType::Integer(region_type);
    let index_type = index.scalar_type();
    let mut assembled: Option<LoweredDirectExpression> = None;
    for byte in 0..u128::from(bits / 8) {
        let byte_index = if byte == 0 {
            index.clone()
        } else {
            Box::new(LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactAdd,
                scalar_type: index_type,
                left: index.clone(),
                right: Box::new(LoweredDirectExpression::IntegerLiteral {
                    value: IntegerValue::Unsigned(byte),
                    scalar_type: index_type,
                }),
            })
        };
        let widened = LoweredDirectExpression::IntegerWiden {
            scalar_type: region,
            operand: Box::new(LoweredDirectExpression::IndexedPrimitiveRead {
                source,
                path: path.clone(),
                index: byte_index,
                inner_indexes: Vec::new(),
                element_path: Vec::new(),
                scalar_type,
            }),
        };
        let placed = if byte == 0 {
            widened
        } else {
            LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::WrappingShiftLeft,
                scalar_type: region,
                left: Box::new(widened),
                right: Box::new(LoweredDirectExpression::IntegerLiteral {
                    value: IntegerValue::Unsigned(byte * 8),
                    scalar_type: region,
                }),
            }
        };
        assembled = Some(match assembled {
            None => placed,
            Some(lower) => LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::BitwiseOr,
                scalar_type: region,
                left: Box::new(lower),
                right: Box::new(placed),
            },
        });
    }
    let assembled = assembled.ok_or(LoweringError::Unsupported(
        "a byte-region re-view spans no bytes",
    ))?;
    if region_type == view_type {
        return Ok(assembled);
    }
    crate::expression_preparation::wrapping_cast::wrapping_cast(
        assembled,
        region_type,
        ScalarType::Integer(view_type),
    )
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
