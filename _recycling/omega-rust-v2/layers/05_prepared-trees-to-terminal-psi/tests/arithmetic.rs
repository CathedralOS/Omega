#[path = "support/source.rs"]
mod sources;

#[path = "support/returns.rs"]
mod returns;
use returns::sole_return;
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
use omega_typed::{ObligationKind, ValueType};

/// Receipt for the L5 readiness report, printed after the checks above it pass.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

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

/// Each numeric operator carries one `OperationContract` naming its token and
/// typed operand carriers at its occurrence. The Psi integer operation at that
/// same site takes its primitive and policy from the token and its questions
/// from the carriers: signed and unsigned division are twins in one package.
#[test]
fn integer_questions_distinguish_quotient_range_and_shift_count_policy() {
    use ArithmeticPolicy::{Exact, Saturating, Wrapping};
    use IntegerObligation::{NonzeroDivisor, QuotientFitsCarrier, ResultFitsCarrier, ShiftCountInRange};
    use IntegerPrimitive::{Divide, Remainder, ShiftLeft, ShiftRight};
    let lowered = pipeline::lower(
        "machine exact_divide(value: i8, divisor: i8) -> i8 { value / divisor }
         machine exact_remainder(value: i8, divisor: i8) -> i8 { value % divisor }
         machine exact_shift(value: u13, count: i8) -> u13 { value << count }
         machine saturating_shift(value: u13, count: i8) -> u13 { value >>| count }
         machine unsigned_divide(value: u8, divisor: u8) -> u8 { value / divisor }
         machine unsigned_remainder(value: u8, divisor: u8) -> u8 { value % divisor }
         machine wrapping_remainder(value: i8, divisor: i8) -> i8 { value %% divisor }
         machine wrapping_shift(value: u13, count: i8) -> u13 { value <<% count }",
    );
    let package = &lowered.package;
    let (s8, n8, n13) = ((true, 8), (false, 8), (false, 13));
    // (occurrence, token, typed operand carriers, primitive, policy, questions)
    let expected = [
        (
            "value / divisor",
            "/",
            [s8, s8],
            Divide,
            Exact,
            [NonzeroDivisor, QuotientFitsCarrier].as_slice(),
        ),
        (
            "value % divisor",
            "%",
            [s8, s8],
            Remainder,
            Exact,
            [NonzeroDivisor, QuotientFitsCarrier].as_slice(),
        ),
        (
            "value << count",
            "<<",
            [n13, s8],
            ShiftLeft,
            Exact,
            [ShiftCountInRange, ResultFitsCarrier].as_slice(),
        ),
        ("value >>| count", ">>|", [n13, s8], ShiftRight, Saturating, [ShiftCountInRange].as_slice()),
        ("value / divisor", "/", [n8, n8], Divide, Exact, [NonzeroDivisor].as_slice()),
        ("value % divisor", "%", [n8, n8], Remainder, Exact, [NonzeroDivisor].as_slice()),
        ("value %% divisor", "%%", [s8, s8], Remainder, Wrapping, [NonzeroDivisor].as_slice()),
        ("value <<% count", "<<%", [n13, s8], ShiftLeft, Wrapping, [].as_slice()),
    ];
    let contracts = lowered.obligations("OperationContract");
    assert_eq!(contracts.len(), expected.len(), "one formation question per numeric operator: {contracts:?}");
    assert_eq!(package.machines.len(), expected.len());
    for (machine, (occurrence, token, carriers, primitive, policy, obligations)) in package.machines.iter().zip(expected) {
        let [row] = machine.body.operations.as_slice() else {
            panic!("expected a single integer operation")
        };
        let site = &machine.body.diagnostics.operations[row.id.index()];
        assert_eq!(lowered.spelling(site), occurrence);
        let [contract] = contracts.iter().filter(|o| &o.site == site).collect::<Vec<_>>()[..] else {
            panic!("{occurrence}: expected one operation contract at the Psi operation")
        };
        assert_eq!(
            contract.kind,
            ObligationKind::OperationContract {
                token: token.into(),
                operands: carriers
                    .map(|(signed, width)| ValueType::Integer { signed, width: Some(width) })
                    .to_vec(),
            },
            "{occurrence}"
        );
        let OperationKind::Integer { operation, operands } = &row.kind else {
            panic!("expected integer operation")
        };
        for (operand, (signed, width)) in operands.iter().zip(carriers) {
            assert_eq!(
                package.types.get(machine.body.values[operand.index()].ty),
                Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed, width })),
                "{occurrence}"
            );
        }
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
    observed("obligation-output/operation-contract");
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
        spec_corpus::observe_handoff!(
            psi "psi-output/folded-comparison",
            assert_eq!(compare.kind, greater(vec![x, landed]), "{name}"),
        );
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

/// `language/numeric_values.md` admits `((5 / 2) * 2) as u32` as 5: the cast
/// acknowledges the fractional `5 / 2` and lands the exact final value. The
/// historical `rational_arg_landing` sample passes `(7 / 2 * 2) as i32` to a
/// call the same way. Without the cast, typing refuses the same calculation.
#[test]
fn explicit_cast_lands_an_acknowledged_fractional_calculation_as_its_exact_constant() {
    let u32_type = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 32 });
    let i32_type = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: true, width: 32 });
    let package = lower(
        "machine exact() -> u32 { ((5 / 2) * 2) as u32 }
         machine identity(value: i32) -> i32 { value }
         machine anonymous() -> i32 { identity((7 / 2 * 2) as i32) }",
    );
    let machine = |name: &str| {
        package
            .machines
            .iter()
            .find(|machine| machine.declaration.path == name)
            .unwrap_or_else(|| panic!("missing machine {name}"))
    };
    let body = &machine("exact").body;
    let [constant] = body.operations.as_slice() else {
        panic!("expected one landed constant, got {:?}", body.operations);
    };
    assert_eq!(constant.kind, OperationKind::Constant(ScalarLiteral::Integer(5.into())));
    assert_eq!(
        package
            .types
            .get(body.values[scalar_result(constant).index()].ty),
        Some(&u32_type)
    );
    assert_eq!(sole_return(body), ReturnEdge::Scalar(scalar_result(constant)));

    let identity = machine("identity").id;
    let body = &machine("anonymous").body;
    let [constant, call] = body.operations.as_slice() else {
        panic!("expected the landed argument and its call, got {:?}", body.operations);
    };
    assert_eq!(constant.kind, OperationKind::Constant(ScalarLiteral::Integer(7.into())));
    assert_eq!(
        package
            .types
            .get(body.values[scalar_result(constant).index()].ty),
        Some(&i32_type)
    );
    let OperationKind::Call {
        callee: omega_lowering::CallTarget::Machine(callee),
        arguments,
        ..
    } = &call.kind
    else {
        panic!("expected a call to identity, got {:?}", call.kind);
    };
    assert_eq!(*callee, identity);
    assert_eq!(arguments.as_slice(), [omega_lowering::CallArgument::Scalar(scalar_result(constant))]);

    let implicit = "machine implicit() -> u32 { (5 / 2) * 2 }";
    let error = omega_preparation::prepare_source_package(&sources::resolve(&[("arithmetic.omg", implicit)])).unwrap_err();
    assert!(
        error
            .message
            .starts_with("fractional intermediate at bytes 29..34 yields 5"),
        "{}",
        error.message
    );
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
    assert_eq!(cast.kind, OperationKind::ExactNumericCast { operand: selected });
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

/// A clause is already being used to state a proof obligation. The arithmetic
/// inside it can introduce further obligations before the clause denotes a fact.
#[test]
fn contract_integer_terms_keep_their_operands_and_formation_questions() {
    use ArithmeticPolicy::{Exact, Saturating, Wrapping};
    use IntegerObligation::{NonzeroDivisor, QuotientFitsCarrier, ResultFitsCarrier, ShiftCountInRange};
    use IntegerPrimitive::{Add, BitwiseAnd, BitwiseNot, Divide, Negate, Remainder, ShiftLeft};
    use omega_lowering::{PredicateOperator, PredicateStep};

    let u8_ty = ScalarType::Integer { signed: false, width: 8 };
    let i8_ty = ScalarType::Integer { signed: true, width: 8 };
    let u13_ty = ScalarType::Integer { signed: false, width: 13 };
    for (parameters, spelling, primitive, policy, carriers, obligations) in [
        ("x:u8,y:u8", "x + y", Add, Exact, &[u8_ty, u8_ty][..], &[ResultFitsCarrier][..]),
        ("x:u8,y:u8", "x +% y", Add, Wrapping, &[u8_ty, u8_ty], &[]),
        ("x:u8,y:u8", "x +| y", Add, Saturating, &[u8_ty, u8_ty], &[]),
        ("x:i8,y:i8", "x % y", Remainder, Exact, &[i8_ty, i8_ty], &[NonzeroDivisor, QuotientFitsCarrier]),
        ("x:u8,y:u8", "x / y", Divide, Exact, &[u8_ty, u8_ty], &[NonzeroDivisor]),
        ("x:u13,y:i8", "x << y", ShiftLeft, Exact, &[u13_ty, i8_ty], &[ShiftCountInRange, ResultFitsCarrier]),
        ("x:u13,y:i8", "x <<% y", ShiftLeft, Wrapping, &[u13_ty, i8_ty], &[]),
        ("x:u8,y:u8", "x & y", BitwiseAnd, Exact, &[u8_ty, u8_ty], &[]),
        ("x:u8", "~x", BitwiseNot, Exact, &[u8_ty], &[]),
        ("x:i8", "-x", Negate, Exact, &[i8_ty], &[ResultFitsCarrier]),
    ] {
        let source = format!("machine test({parameters}) requires ({spelling}) == x {{}}");
        let package = lower(&source);
        let machine = &package.machines[0];
        assert!(machine.body.operations.is_empty(), "{spelling}");
        let [clause] = machine.contract.requires.as_slice() else { panic!("missing requirement for {spelling}") };
        let (inputs, tail) = clause.predicate.split_at(carriers.len());
        for (index, input) in inputs.iter().enumerate() {
            assert_eq!(input, &PredicateStep::Formal(index), "{spelling}");
        }
        let [PredicateStep::IntegerArithmetic { operation, operands, site }, PredicateStep::Formal(0),
             PredicateStep::Apply(PredicateOperator::Equal)] = tail else { panic!("unexpected terms for {spelling}: {tail:?}") };
        assert_eq!((operation.primitive, operation.policy), (primitive, policy), "{spelling}");
        assert_eq!(operation.obligations, obligations, "{spelling}");
        assert_eq!(operands, carriers, "{spelling}");
        assert_eq!(site.span.slice(source.as_bytes()), Some(spelling.as_bytes()));
    }
}

/// At the u8 boundary, substitution distinguishes an unformed exact term,
/// a wrapping result of zero, and a saturated result of 255 in the same clause.
#[test]
fn read_nested_addition_policies_from_the_published_clause() {
    use omega_lowering::{PredicateOperator, PredicateStep};
    fn substitute(clause: &[PredicateStep], x: u8) -> Option<bool> {
        let mut values = Vec::new();
        let [terms @ .., PredicateStep::Apply(PredicateOperator::Equal)] = clause else { return None };
        for step in terms {
            match step {
                PredicateStep::Formal(0) => values.push(x),
                PredicateStep::Integer(n) => values.push(u8::try_from(n).ok()?),
                PredicateStep::IntegerArithmetic { operation, operands, .. } => {
                    let u8_ty = ScalarType::Integer { signed: false, width: 8 };
                    if operands != &[u8_ty, u8_ty] || operation.primitive != IntegerPrimitive::Add { return None; }
                    let right = values.pop()?;
                    let left = values.pop()?;
                    values.push(match (operation.policy, operation.obligations.as_slice()) {
                        (ArithmeticPolicy::Exact, [IntegerObligation::ResultFitsCarrier]) => left.checked_add(right)?,
                        (ArithmeticPolicy::Wrapping, []) => left.wrapping_add(right),
                        (ArithmeticPolicy::Saturating, []) => left.saturating_add(right),
                        _ => return None,
                    });
                }
                _ => return None,
            }
        }
        let [left, right] = values.as_slice() else { return None };
        Some(left == right)
    }
    for (expression, expected) in [
        ("x + 1u8", None),
        ("x +% 1u8", Some(true)),
        ("x +| 1u8", Some(false)),
        ("(x +| 1u8) +% 1u8", Some(true)),
        ("(x +% 1u8) +| 1u8", Some(false)),
    ] {
        let source = format!("machine test(x:u8) requires {expression} == 0u8 {{}}");
        let package = lower(&source);
        let mut terms = package.machines[0].contract.requires[0].predicate.clone();
        assert_eq!(substitute(&terms, 255), expected, "{expression}");
        if let Some(PredicateStep::IntegerArithmetic { operation, .. }) = terms.iter_mut().find(|s| matches!(s, PredicateStep::IntegerArithmetic { .. })) {
            operation.obligations = vec![IntegerObligation::NonzeroDivisor];
        }
        assert_eq!(substitute(&terms, 1), None, "the reader accepted a foreign formation obligation");
    }
}
