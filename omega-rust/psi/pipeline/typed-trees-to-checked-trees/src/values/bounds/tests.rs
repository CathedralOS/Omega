use super::*;

struct Bounds(Vec<IntegerRange>);

impl IntegerBoundsSource for Bounds {
    fn binding(&mut self, position: usize) -> Option<IntegerRange> {
        self.0.get(position).cloned()
    }

    fn storage(&mut self, _: SymbolHandle) -> Option<IntegerRange> {
        self.binding(0)
    }

    fn structural_field(
        &mut self,
        _: u32,
        _: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        self.binding(0)
    }

    fn indexed_field(
        &mut self,
        _: u32,
        _: &[CheckedStructuralPredicatePathSegment],
        _: Option<&IntegerRange>,
    ) -> Option<IntegerRange> {
        self.binding(0)
    }
}

fn range(minimum: i128, maximum: i128) -> IntegerRange {
    IntegerRange {
        minimum: BigInt::from_i128(minimum),
        maximum: BigInt::from_i128(maximum),
    }
}

fn parameter(position: usize, primitive_type: PrimitiveType) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    }
}

fn operation(
    kind: CheckedIntegerBinaryKind,
    primitive_type: PrimitiveType,
) -> CheckedScalarExpression {
    CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(parameter(0, primitive_type)),
        right: Box::new(parameter(1, primitive_type)),
    }
}

#[test]
fn remainder_bounds_require_nonnegative_dividends_and_positive_divisors() {
    use CheckedIntegerBinaryKind::*;
    for kind in [ExactRemainder, WrappingRemainder, SaturatingRemainder] {
        for (primitive, left, right, expected) in [
            (
                PrimitiveType::U8,
                range(0, 255),
                range(10, 10),
                Some(range(0, 9)),
            ),
            (
                PrimitiveType::U8,
                range(0, 3),
                range(5, 20),
                Some(range(0, 3)),
            ),
            (
                PrimitiveType::U8,
                range(0, 255),
                range(1, 1),
                Some(range(0, 0)),
            ),
            (PrimitiveType::U8, range(0, 255), range(0, 10), None),
            (PrimitiveType::I8, range(-1, 127), range(10, 10), None),
            (PrimitiveType::I8, range(0, 127), range(-10, -1), None),
            (
                PrimitiveType::I8,
                range(0, 127),
                range(2, 10),
                Some(range(0, 9)),
            ),
            (
                PrimitiveType::U64,
                range(0, i128::from(u64::MAX)),
                range(i128::from(u64::MAX), i128::from(u64::MAX)),
                Some(range(0, i128::from(u64::MAX) - 1)),
            ),
        ] {
            assert_eq!(
                evaluate(&operation(kind, primitive), &mut Bounds(vec![left, right])),
                expected,
                "{kind:?} {primitive:?}",
            );
        }
    }
}

#[test]
fn in_carrier_arithmetic_preserves_interval_bounds_for_each_policy() {
    use CheckedIntegerBinaryKind::*;
    for (kinds, left, right, expected) in [
        (
            [ExactAdd, WrappingAdd, SaturatingAdd],
            range(-5, 3),
            range(1, 7),
            range(-4, 10),
        ),
        (
            [ExactSubtract, WrappingSubtract, SaturatingSubtract],
            range(-5, 3),
            range(1, 7),
            range(-12, 2),
        ),
        (
            [ExactMultiply, WrappingMultiply, SaturatingMultiply],
            range(-5, 3),
            range(-2, 7),
            range(-35, 21),
        ),
    ] {
        for kind in kinds {
            assert_eq!(
                evaluate(
                    &operation(kind, PrimitiveType::I8),
                    &mut Bounds(vec![left.clone(), right.clone()]),
                ),
                Some(expected.clone()),
                "{kind:?}"
            );
        }
    }
}

#[test]
fn overflow_rejects_exact_widens_wrapping_and_clamps_saturating_bounds() {
    use CheckedIntegerBinaryKind::*;
    for (primitive, kinds, left, right, saturated) in [
        (
            PrimitiveType::I8,
            [ExactAdd, WrappingAdd, SaturatingAdd],
            range(120, 127),
            range(1, 10),
            range(121, 127),
        ),
        (
            PrimitiveType::I8,
            [ExactSubtract, WrappingSubtract, SaturatingSubtract],
            range(-128, -120),
            range(1, 10),
            range(-128, -121),
        ),
        (
            PrimitiveType::I8,
            [ExactMultiply, WrappingMultiply, SaturatingMultiply],
            range(-100, 100),
            range(2, 3),
            range(-128, 127),
        ),
        (
            PrimitiveType::U8,
            [ExactSubtract, WrappingSubtract, SaturatingSubtract],
            range(0, 3),
            range(2, 5),
            range(0, 1),
        ),
        (
            PrimitiveType::U8,
            [ExactMultiply, WrappingMultiply, SaturatingMultiply],
            range(200, 255),
            range(2, 3),
            range(255, 255),
        ),
    ] {
        let carrier = match primitive {
            PrimitiveType::I8 => range(-128, 127),
            PrimitiveType::U8 => range(0, 255),
            _ => unreachable!("fixture carriers"),
        };
        for (kind, expected) in kinds
            .into_iter()
            .zip([None, Some(carrier), Some(saturated)])
        {
            assert_eq!(
                evaluate(
                    &operation(kind, primitive),
                    &mut Bounds(vec![left.clone(), right.clone()]),
                ),
                expected,
                "{primitive:?} {kind:?}"
            );
        }
    }
}

#[test]
fn full_u64_bounds_and_products_do_not_narrow_through_signed_host_values() {
    use CheckedIntegerBinaryKind::*;
    let maximum = i128::from(u64::MAX);
    for (kind, left, right, expected) in [
        (
            ExactAdd,
            range(maximum - 2, maximum - 1),
            range(1, 1),
            Some(range(maximum - 1, maximum)),
        ),
        (
            ExactSubtract,
            range(0, maximum),
            range(0, 0),
            Some(range(0, maximum)),
        ),
        (ExactMultiply, range(1 << 63, maximum), range(2, 2), None),
        (
            WrappingMultiply,
            range(1 << 63, maximum),
            range(2, 2),
            Some(range(0, maximum)),
        ),
        (
            SaturatingMultiply,
            range(1 << 63, maximum),
            range(2, 2),
            Some(range(maximum, maximum)),
        ),
    ] {
        assert_eq!(
            evaluate(
                &operation(kind, PrimitiveType::U64),
                &mut Bounds(vec![left, right])
            ),
            expected,
            "{kind:?}"
        );
    }
}

#[test]
fn every_source_leaf_rejects_reversed_outside_or_absent_carrier_bounds() {
    let path = vec![CheckedStructuralPredicatePathSegment::Field("byte".into())];
    for expression in [
        parameter(0, PrimitiveType::U8),
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U8,
        },
        CheckedScalarExpression::StorageRead {
            symbol: SymbolHandle::from_arena_index(1),
            primitive_type: PrimitiveType::U8,
        },
        CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path: path.clone(),
            primitive_type: PrimitiveType::U8,
        },
        CheckedScalarExpression::StructuralParameterIndexedRead {
            parameter_position: 0,
            path,
            index: Box::new(parameter(1, PrimitiveType::U64)),
            primitive_type: PrimitiveType::U8,
        },
    ] {
        assert_eq!(
            evaluate(&expression, &mut Bounds(vec![range(0, 255)])),
            Some(range(0, 255))
        );
        for invalid in [range(65, 64), range(-1, 65), range(65, 256)] {
            assert_eq!(
                evaluate(&expression, &mut Bounds(vec![invalid])),
                None,
                "{expression:?}"
            );
        }
        assert_eq!(evaluate(&expression, &mut Bounds(Vec::new())), None);
    }
}

#[test]
fn casts_require_valid_source_bounds_and_their_exact_destination_contract() {
    let source = parameter(0, PrimitiveType::I16);
    let exact = |range| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(source.clone()),
        range,
    };
    let trapping = CheckedScalarExpression::IntegerTrappingCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(source.clone()),
    };
    for expression in [exact(range(0, 255)), trapping] {
        assert_eq!(
            evaluate(&expression, &mut Bounds(vec![range(65, 90)])),
            Some(range(65, 90))
        );
        for invalid in [
            range(-1, 65),
            range(65, 256),
            range(90, 65),
            range(0, 32768),
        ] {
            assert_eq!(evaluate(&expression, &mut Bounds(vec![invalid])), None);
        }
        assert_eq!(evaluate(&expression, &mut Bounds(Vec::new())), None);
    }
    for declared in [range(70, 90), range(65, 80), range(90, 65)] {
        assert_eq!(
            evaluate(&exact(declared), &mut Bounds(vec![range(65, 90)])),
            None
        );
    }
    // A forged wide cast constraint cannot override the destination carrier.
    assert_eq!(
        evaluate(&exact(range(0, 500)), &mut Bounds(vec![range(256, 300)])),
        None
    );

    for (source_type, target_type, accepted) in [
        (PrimitiveType::U8, PrimitiveType::I16, true),
        (PrimitiveType::I16, PrimitiveType::U8, false),
        (PrimitiveType::I8, PrimitiveType::U8, false),
        (PrimitiveType::U8, PrimitiveType::Bool, false),
    ] {
        let expression = CheckedScalarExpression::IntegerWiden {
            primitive_type: target_type,
            operand: Box::new(parameter(0, source_type)),
        };
        assert_eq!(
            evaluate(&expression, &mut Bounds(vec![range(65, 90)])),
            accepted.then(|| range(65, 90)),
            "{source_type:?} to {target_type:?}"
        );
    }
}

#[test]
fn selected_operand_carriers_and_unserved_operations_cannot_supply_bounds() {
    let mut expression = operation(CheckedIntegerBinaryKind::ExactAdd, PrimitiveType::U8);
    let CheckedScalarExpression::IntegerBinary { right, .. } = &mut expression else {
        unreachable!("constructed binary expression");
    };
    **right = parameter(1, PrimitiveType::I8);
    assert_eq!(
        evaluate(&expression, &mut Bounds(vec![range(1, 2), range(3, 4)])),
        None
    );
    assert_eq!(
        evaluate(
            &operation(CheckedIntegerBinaryKind::ExactDivide, PrimitiveType::U8),
            &mut Bounds(vec![range(4, 8), range(2, 2)]),
        ),
        None,
    );
}
