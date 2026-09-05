//! Optimizer module role: stage group. Versioned selected structural-function payload codec.
//!
//! This entrance fixes the signature -> settlements -> optional call -> return
//! order. Leaves own the exhaustive semantic, ABI, provider, and effect fields.

mod call;
mod calling;
mod declarations;
mod function;
mod projected_qualifications;
mod provider;
mod settlements;
mod signature;

use selected_instructions::SelectedStructuralUnitFunction;

use crate::FixedViewCopyDecodeError;

use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::Cursor;

pub(super) fn encode_structural_function_v5(
    bytes: &mut Vec<u8>,
    function: &SelectedStructuralUnitFunction,
) {
    function::encode(bytes, function, false);
}

pub(super) fn encode_structural_function_v6(
    bytes: &mut Vec<u8>,
    function: &SelectedStructuralUnitFunction,
) {
    function::encode(bytes, function, true);
}

pub(super) fn decode_structural_function_v5(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitFunction, FixedViewCopyDecodeError> {
    function::decode(cursor, false)
}

pub(super) fn decode_structural_function_v6(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitFunction, FixedViewCopyDecodeError> {
    function::decode(cursor, true)
}
