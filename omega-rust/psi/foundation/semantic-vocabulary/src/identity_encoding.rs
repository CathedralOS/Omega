// Canonical identity encodings of scalar and integer types. A unit's identity
// and an execution plan's identity both bind these bytes, so both forms must
// encode a type the same way; the encoders live with the types' vocabulary.

use crate::{IntegerCarrier, IntegerSign, IntegerType, ScalarType};

pub fn encode_scalar_type<B>(
    bytes: &mut B,
    scalar_type: ScalarType,
    push_u8: impl Fn(&mut B, u8),
    encode_integer: impl FnOnce(&mut B, IntegerType),
) {
    match scalar_type {
        ScalarType::Boolean => push_u8(bytes, 1),
        ScalarType::Integer(integer) => {
            push_u8(bytes, 2);
            encode_integer(bytes, integer);
        }
        ScalarType::IeeeFloat(format) => {
            push_u8(bytes, 3);
            push_u8(
                bytes,
                match format {
                    crate::IeeeFloatFormat::Binary32 => 1,
                    crate::IeeeFloatFormat::Binary64 => 2,
                },
            );
        }
    }
}

pub fn encode_integer_type(canonical: &mut Vec<u8>, scalar_type: IntegerType) {
    canonical.push(match scalar_type.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    canonical.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    // Rewrite/range fingerprints describe raw metadata as well as closed facts.
    // Identity never grants a parameter the concrete interval needed for a fact.
    scalar_type
        .width()
        .write_identity(|part| canonical.extend_from_slice(part));
}
