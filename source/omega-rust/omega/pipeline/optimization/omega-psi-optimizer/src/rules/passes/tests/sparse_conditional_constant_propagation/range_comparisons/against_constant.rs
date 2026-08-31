use super::*;

#[test]
fn integer_range_equality_proves_singleton_outside_and_declines_overlap() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for kind in [
        IntegerRangeComparisonKind::RangeEqualConstant,
        IntegerRangeComparisonKind::ConstantEqualRange,
    ] {
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(7),
            ),
            Some(true)
        );
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(6),
            ),
            Some(false)
        );
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(10),
            ),
            Some(false)
        );
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(8),
            ),
            None
        );
    }
}
