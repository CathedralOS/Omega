//! Exact zero and one values for signed and unsigned integer types.

use psi_core::{IntegerSign, IntegerType, IntegerValue};

pub(in crate::rules::passes) fn integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}

pub(in crate::rules::passes) fn integer_one(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    }
}
