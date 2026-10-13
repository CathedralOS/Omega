//! Lossless conversion between proof magnitudes and fixed-width integer kernels.
//! Admission precedes evaluation: truncating an out-of-carrier magnitude would
//! turn an invalid operand into evidence about a different value.

use numerics::bignum::BigInt;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use typed_trees::types::PrimitiveType;

pub(crate) fn integer_type(primitive: PrimitiveType) -> Option<IntegerType> {
    let (sign, bits) = primitive.integer_sign_bits()?;
    IntegerType::new(sign, bits).ok()
}

pub(crate) fn admitted_integer(scalar_type: IntegerType, value: &BigInt) -> Option<IntegerValue> {
    let value = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(i128::from(value.to_i64()?)),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(value.to_u64()?)),
    };
    scalar_type.admits(value).then_some(value)
}

pub(crate) fn integer_magnitude(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Signed(value) => BigInt::from_i128(value),
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
    }
}
