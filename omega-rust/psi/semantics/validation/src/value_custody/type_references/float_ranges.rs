//! Authored floating range endpoints keep their exact IEEE interchange
//! values at the declared carrier. The source spelling stays authoritative
//! upstream; these readers convert once, directly from the spelling, and
//! never normalize an exclusive end through integer predecessor arithmetic.

use numerics::literals::FloatFormat;
use semantic_vocabulary::IeeeFloatValue;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

/// Evaluate one authored floating range endpoint at its declared carrier.
/// A float literal reads its spelling directly at the carrier format; a
/// literal authored with a narrower landing keeps that value and widens
/// exactly, while a wider-authored literal cannot narrow through the f32
/// read. Integer endpoints keep their exact completed value converted once
/// into the carrier. Anything else cannot be retained and must fail closed
/// upstream.
pub fn closed_float_range_endpoint(
    program: &TypedTrees,
    expression: ExpressionHandle,
    primitive_type: PrimitiveType,
) -> Option<IeeeFloatValue> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Float(literal) => match primitive_type {
            PrimitiveType::F32 => (literal.landing() != Some(FloatFormat::F64))
                .then(|| IeeeFloatValue::Binary32(literal.value_f32().to_bits())),
            PrimitiveType::F64 => Some(IeeeFloatValue::Binary64(literal.landed_f64().to_bits())),
            _ => None,
        },
        // Mixed floating ranges may have integer endpoints; the completed
        // exact value converts once into the carrier, matching the proof
        // constraint read. A failed exact endpoint cannot fall back to its
        // raw literal text.
        ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
            let value =
                super::integer_ranges::closed_integer_range_bound(program, expression)?.to_i64()?;
            Some(match primitive_type {
                PrimitiveType::F32 => IeeeFloatValue::Binary32((value as f32).to_bits()),
                PrimitiveType::F64 => IeeeFloatValue::Binary64((value as f64).to_bits()),
                _ => return None,
            })
        }
        _ => None,
    }
}

/// IEEE `<=` over retained interchange values. NaN endpoints fail the
/// order, so an unordered or NaN-bounded window cannot be retained as a
/// nonempty requirement. Signed zeros order under IEEE equality.
pub fn ieee_float_range_ordered(minimum: IeeeFloatValue, maximum: IeeeFloatValue) -> bool {
    match (minimum, maximum) {
        (IeeeFloatValue::Binary32(low), IeeeFloatValue::Binary32(high)) => {
            f32::from_bits(low) <= f32::from_bits(high)
        }
        (IeeeFloatValue::Binary64(low), IeeeFloatValue::Binary64(high)) => {
            f64::from_bits(low) <= f64::from_bits(high)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExpressionHandle, IeeeFloatValue, PrimitiveType, TypedTrees, closed_float_range_endpoint,
        ieee_float_range_ordered,
    };
    use typed_trees::types::TypeConstraintNode;
    use typed_trees::types::TypeReferenceHandle;

    fn field(spelling: &str) -> (TypedTrees, TypeReferenceHandle) {
        let source = format!("data Carrier {{ value: {spelling}; }}");
        let typed = crate::front_end::typed_program(&source);
        let [typed_trees::data::DataMember::Field(field)] =
            typed.data_members(&typed.data_definitions()[0])
        else {
            panic!("one field");
        };
        let reference = field.type_reference;
        (typed, reference)
    }

    fn range(
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> (ExpressionHandle, ExpressionHandle, bool) {
        let mut current = reference;
        loop {
            match typed.type_reference_table.type_reference(current) {
                typed_trees::types::TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    if let [
                        TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        },
                    ] = typed.type_reference_table.constraints(*constraints)
                    {
                        return (*minimum, *maximum, *end_inclusive);
                    }
                    current = *base_type;
                }
                _ => panic!("range constraint"),
            }
        }
    }

    #[test]
    fn exclusive_endpoints_keep_authored_bits_at_the_declared_carrier() {
        let (typed, reference) = field("f64[0.0..1.5]");
        let (minimum, maximum, end_inclusive) = range(&typed, reference);
        assert!(!end_inclusive);
        assert_eq!(
            closed_float_range_endpoint(&typed, minimum, PrimitiveType::F64),
            Some(IeeeFloatValue::Binary64(0.0f64.to_bits()))
        );
        // The authored end is retained verbatim: an integer-predecessor
        // normalization would land one ulp below at 0x3FF7FFFFFFFFFFFF.
        assert_eq!(
            closed_float_range_endpoint(&typed, maximum, PrimitiveType::F64),
            Some(IeeeFloatValue::Binary64(1.5f64.to_bits()))
        );
    }

    #[test]
    fn f32_carrier_reads_the_spelling_directly_at_binary32() {
        let (typed, reference) = field("f32[0.5..2.25]");
        let (minimum, maximum, end_inclusive) = range(&typed, reference);
        assert!(!end_inclusive);
        assert_eq!(
            closed_float_range_endpoint(&typed, minimum, PrimitiveType::F32),
            Some(IeeeFloatValue::Binary32(0.5f32.to_bits()))
        );
        assert_eq!(
            closed_float_range_endpoint(&typed, maximum, PrimitiveType::F32),
            Some(IeeeFloatValue::Binary32(2.25f32.to_bits()))
        );
    }

    #[test]
    fn integer_endpoints_convert_once_into_the_carrier() {
        let (typed, reference) = field("f64[0..2]");
        let (minimum, maximum, _) = range(&typed, reference);
        assert_eq!(
            closed_float_range_endpoint(&typed, minimum, PrimitiveType::F64),
            Some(IeeeFloatValue::Binary64(0.0f64.to_bits()))
        );
        assert_eq!(
            closed_float_range_endpoint(&typed, maximum, PrimitiveType::F64),
            Some(IeeeFloatValue::Binary64(2.0f64.to_bits()))
        );
    }

    #[test]
    fn unrepresentable_endpoints_fail_closed() {
        let (typed, reference) = field("f64[0.0..1.5]");
        let (minimum, _, _) = range(&typed, reference);
        assert_eq!(
            closed_float_range_endpoint(&typed, ExpressionHandle::invalid(), PrimitiveType::F64),
            None
        );
        // A float endpoint cannot stand in for an integer carrier.
        assert_eq!(
            closed_float_range_endpoint(&typed, minimum, PrimitiveType::I32),
            None
        );
    }

    #[test]
    fn ieee_order_rejects_nan_and_reversed_windows() {
        let nan = IeeeFloatValue::Binary64(f64::NAN.to_bits());
        let one = IeeeFloatValue::Binary64(1.0f64.to_bits());
        let two = IeeeFloatValue::Binary64(2.0f64.to_bits());
        assert!(!ieee_float_range_ordered(nan, one));
        assert!(!ieee_float_range_ordered(one, nan));
        assert!(!ieee_float_range_ordered(two, one));
        assert!(ieee_float_range_ordered(one, two));
        assert!(ieee_float_range_ordered(one, one));
        // Signed zeros are equal under IEEE order; a different carrier is
        // never ordered against it.
        assert!(ieee_float_range_ordered(
            IeeeFloatValue::Binary64((-0.0f64).to_bits()),
            IeeeFloatValue::Binary64(0.0f64.to_bits()),
        ));
        assert!(!ieee_float_range_ordered(
            IeeeFloatValue::Binary32(0.0f32.to_bits()),
            one,
        ));
    }
}
