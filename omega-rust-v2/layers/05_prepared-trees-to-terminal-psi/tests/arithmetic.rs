#[path = "support/source.rs"]
mod sources;

#[path = "support/returns.rs"]
mod returns;
use returns::sole_return;

use omega_lowering::{
    ArithmeticPolicy, BooleanPrimitive, IntegerObligation, IntegerOperation, IntegerPrimitive, LoweredPackage, OperationKind, OperationResult, Parameter,
    PsiOperation, ReturnEdge, ScalarLiteral, ScalarType, TypeShape, ValueId, ValueOrigin,
};

fn lower(source: &str) -> LoweredPackage {
    sources::lower(&[("arithmetic.omg", source)]).unwrap()
}

fn scalar_result(operation: &PsiOperation) -> ValueId {
    let OperationResult::Scalar(value) = operation.result else {
        panic!("expected a scalar operation result");
    };
    value
}

#[test]
fn nested_integer_operations_keep_evaluation_order_and_local_policy() {
    let package = {
        let source = String::from("machine add(left: u8, right: u8) -> u8 { (left +% 1) + (right +| 2u8) }");
        lower(&source)
    };
    let body = &package.machines[0].body;
    let [Parameter::Scalar(left), Parameter::Scalar(right)] = *body.parameters.as_slice() else {
        panic!("expected two scalar parameters")
    };
    let [one, wrapping, two, saturating, exact] = body.operations.as_slice() else {
        panic!("expected five ordered operations")
    };
    assert!(matches!(&one.kind, OperationKind::Constant(_)));
    assert!(matches!(&two.kind, OperationKind::Constant(_)));
    for (row, policy, operands, obligations) in [
        (wrapping, ArithmeticPolicy::Wrapping, [left, scalar_result(one)], [].as_slice()),
        (saturating, ArithmeticPolicy::Saturating, [right, scalar_result(two)], [].as_slice()),
        (
            exact,
            ArithmeticPolicy::Exact,
            [scalar_result(wrapping), scalar_result(saturating)],
            &[IntegerObligation::ResultFitsCarrier],
        ),
    ] {
        assert_eq!(
            row.kind,
            OperationKind::Integer {
                operation: IntegerOperation {
                    primitive: IntegerPrimitive::Add,
                    policy,
                    obligations: obligations.to_vec()
                },
                operands: operands.to_vec(),
            }
        );
    }
    for (index, row) in body.operations.iter().enumerate() {
        assert_eq!(row.id.index(), index);
        assert_eq!(body.values[scalar_result(row).index()].origin, ValueOrigin::Operation(row.id));
    }
    assert_eq!(sole_return(body), ReturnEdge::Scalar(scalar_result(exact)));
}

#[test]
fn integer_questions_distinguish_quotient_range_and_shift_count_policy() {
    use ArithmeticPolicy::{Exact, Saturating, Wrapping};
    use IntegerObligation::{NonzeroDivisor, QuotientFitsCarrier, ResultFitsCarrier, ShiftCountInRange};
    use IntegerPrimitive::{Remainder, ShiftLeft, ShiftRight};
    let package = lower(
        "machine exact_remainder(value: i8, divisor: i8) -> i8 { value % divisor }
         machine exact_shift(value: u13, count: i8) -> u13 { value << count }
         machine saturating_shift(value: u13, count: i8) -> u13 { value >>| count }
         machine wrapping_remainder(value: i8, divisor: i8) -> i8 { value %% divisor }
         machine wrapping_shift(value: u13, count: i8) -> u13 { value <<% count }",
    );
    for (machine, (primitive, policy, obligations)) in package.machines.iter().zip([
        (Remainder, Exact, [NonzeroDivisor, QuotientFitsCarrier].as_slice()),
        (ShiftLeft, Exact, [ShiftCountInRange, ResultFitsCarrier].as_slice()),
        (ShiftRight, Saturating, [ShiftCountInRange].as_slice()),
        (Remainder, Wrapping, [NonzeroDivisor].as_slice()),
        (ShiftLeft, Wrapping, [].as_slice()),
    ]) {
        let [row] = machine.body.operations.as_slice() else {
            panic!("expected a single integer operation")
        };
        let OperationKind::Integer { operation, operands } = &row.kind else {
            panic!("expected integer operation")
        };
        assert_eq!(
            *operation,
            IntegerOperation {
                primitive,
                policy,
                obligations: obligations.to_vec()
            }
        );
        assert_eq!(
            machine.body.parameters,
            operands
                .iter()
                .copied()
                .map(Parameter::Scalar)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn integer_comparison_preserves_both_operand_carriers() {
    let package = lower("machine compare(left: i8, right: u13) -> bool { left > right }");
    let [row] = package.machines[0].body.operations.as_slice() else {
        panic!("expected a single operation")
    };
    let OperationKind::Integer { operation, operands } = &row.kind else {
        panic!("expected integer comparison")
    };
    assert_eq!(operation.primitive, IntegerPrimitive::GreaterThan);
    assert!(operation.obligations.is_empty());
    let body = &package.machines[0].body;
    assert_eq!(
        package.types.get(body.values[operands[0].index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: true, width: 8 }))
    );
    assert_eq!(
        package.types.get(body.values[operands[1].index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 13 }))
    );
    assert_eq!(
        package.types.get(body.values[scalar_result(row).index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Boolean))
    );
}

#[test]
fn landed_exact_operator_supplies_its_signed_constant_to_the_comparison() {
    let package = lower(
        "machine negative(x: i8) -> bool { x > -1 }
         machine positive(x: i8) -> bool { x > 1 }
         machine minimum(x: i8) -> bool { x > -128 }
         machine grouped(x: i8) -> bool { x > (-(1)) }
         machine folded(x: i8) -> bool { x > 1 - 2 }
         machine runtime(x: i8, y: i8) -> bool { x > -y }",
    );
    let machine = |name: &str| {
        package
            .machines
            .iter()
            .find(|machine| machine.declaration.path == name)
            .unwrap_or_else(|| panic!("missing machine {name}"))
    };
    let i8_type = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: true, width: 8 });
    let greater = |operands: Vec<ValueId>| OperationKind::Integer {
        operation: IntegerOperation {
            primitive: IntegerPrimitive::GreaterThan,
            policy: ArithmeticPolicy::Exact,
            obligations: Vec::new(),
        },
        operands,
    };
    for (name, expected) in [
        ("negative", -1),
        ("positive", 1),
        ("minimum", -128),
        ("grouped", -1),
        ("folded", -1),
    ] {
        let body = &machine(name).body;
        let [Parameter::Scalar(x)] = body.parameters[..] else {
            panic!("{name}: expected one scalar parameter");
        };
        let [constant, compare] = body.operations.as_slice() else {
            panic!("{name}: expected one landed constant and one comparison, got {:?}", body.operations);
        };
        assert_eq!(constant.kind, OperationKind::Constant(ScalarLiteral::Integer(expected.into())), "{name}");
        let landed = scalar_result(constant);
        assert_eq!(package.types.get(body.values[landed.index()].ty), Some(&i8_type), "{name}");
        assert_eq!(compare.kind, greater(vec![x, landed]), "{name}");
        assert_eq!(sole_return(body), ReturnEdge::Scalar(scalar_result(compare)), "{name}");
    }

    // Negating a runtime operand stays an executable operation with its own
    // range question. Only anonymous exact operands fold into a landing.
    let body = &machine("runtime").body;
    let [Parameter::Scalar(x), Parameter::Scalar(y)] = body.parameters[..] else {
        panic!("expected two scalar parameters");
    };
    let [negate, compare] = body.operations.as_slice() else {
        panic!("expected a runtime negation and one comparison, got {:?}", body.operations);
    };
    assert_eq!(
        negate.kind,
        OperationKind::Integer {
            operation: IntegerOperation {
                primitive: IntegerPrimitive::Negate,
                policy: ArithmeticPolicy::Exact,
                obligations: vec![IntegerObligation::ResultFitsCarrier],
            },
            operands: vec![y],
        }
    );
    assert_eq!(compare.kind, greater(vec![x, scalar_result(negate)]));
}

#[test]
fn exact_integer_cast_preserves_the_source_operand_and_destination_carrier() {
    let package = lower("machine narrow(first:u16, selected:u16)->u8{selected as u8}");
    let machine = &package.machines[0];
    let body = &machine.body;
    let [Parameter::Scalar(first), Parameter::Scalar(selected)] = body.parameters[..] else {
        panic!("expected two scalar parameters");
    };
    assert_ne!(first, selected);
    let [cast] = body.operations.as_slice() else {
        panic!("expected one exact integer cast");
    };
    assert_eq!(cast.kind, OperationKind::ExactIntegerCast { operand: selected });
    let result = scalar_result(cast);
    assert_eq!(sole_return(body), ReturnEdge::Scalar(result));
    assert_eq!(
        package.types.get(body.values[selected.index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 16 })),
    );
    assert_eq!(
        package.types.get(body.values[result.index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 })),
    );
}

#[test]
fn boolean_not_and_equality_preserve_their_operand_order() {
    let package = lower("machine compare(first:bool, second:bool)->bool{!first == second}");
    let body = &package.machines[0].body;
    let [Parameter::Scalar(first), Parameter::Scalar(second)] = body.parameters[..] else {
        panic!("expected two Boolean parameters");
    };
    let [negated, equal] = body.operations.as_slice() else {
        panic!("expected Boolean negation and equality");
    };
    assert_eq!(
        negated.kind,
        OperationKind::Boolean {
            primitive: BooleanPrimitive::Not,
            operands: vec![first]
        },
    );
    assert_eq!(
        equal.kind,
        OperationKind::Boolean {
            primitive: BooleanPrimitive::Equal,
            operands: vec![scalar_result(negated), second]
        },
    );
    assert_eq!(sole_return(body), ReturnEdge::Scalar(scalar_result(equal)));
}
