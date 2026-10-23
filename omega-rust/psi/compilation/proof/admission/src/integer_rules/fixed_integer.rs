//! One reading of a fixed native integer carrier and of the exact values its
//! terms carry.
//!
//! Every integer rule asks the same two questions before it applies its own
//! policy: does this carrier belong to the address-free 8/16/32/64-bit widths
//! the hosted targets hold, and what exact value does this term carry in the
//! mathematical window? Each family had grown its own spelling of the
//! answers — a carrier predicate, an interval reader, a signed-only literal
//! reader and a type-checked one, seven copies in all — so a carrier-policy
//! change had to be repeated in every one of them. This owner answers both
//! questions once; each rule keeps its own step vocabulary, error shape and
//! admission policy beside the answer.

use semantic_vocabulary::{IntegerType, IntegerValue, ScalarTerm};

/// An address-free 8/16/32/64-bit carrier: the fixed widths every hosted
/// target holds natively, so a rule may read its endpoints in the host's
/// `i128` window. Address carriers and other widths are refused by their
/// callers rather than widened into the window.
pub(crate) fn fixed_native(integer_type: IntegerType) -> bool {
    !integer_type.is_address() && matches!(integer_type.bits(), 8 | 16 | 32 | 64)
}

/// The exact `(minimum, maximum)` interval of a fixed native carrier, or
/// `None` for a carrier the rules do not read natively.
pub(crate) fn fixed_native_interval(integer_type: IntegerType) -> Option<(i128, i128)> {
    if !fixed_native(integer_type) {
        return None;
    }
    Some((
        integer_value_as_i128(integer_type.minimum_value())?,
        integer_value_as_i128(integer_type.maximum_value())?,
    ))
}

/// An integer value in the mathematical window. An unsigned value too large
/// for `i128` has no mathematical literal: `None`, never a wrapped reading.
pub(crate) fn integer_value_as_i128(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

/// The exact value a term carries when its closed integer value belongs to
/// the expected carrier; a term typed for another carrier, or carrying no
/// integer value, has none.
pub(crate) fn fixed_integer_as_i128(term: &ScalarTerm, integer_type: IntegerType) -> Option<i128> {
    let (actual_type, value) = term.integer_value()?;
    (actual_type == integer_type)
        .then(|| integer_value_as_i128(value))
        .flatten()
}
