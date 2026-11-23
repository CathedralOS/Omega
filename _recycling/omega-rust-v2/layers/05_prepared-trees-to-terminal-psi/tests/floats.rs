//! Canonical `f32` and `f64` operators, literals and exact casts in Psi.
//! [Floating formats](../../../wiki/spec/language/numeric_values.md#floating-formats-and-operations)
//! · [Carrier casts](../../../wiki/spec/language/domains.md#carrier-casts)
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{
    ArithmeticPolicy, BlockTerminator, FloatOperation, FloatSemantics, IeeeFloatBits, IeeeFloatFormat, IntegerObligation, IntegerPrimitive, LoweredPackage,
    MachineBody, OperationKind, OperationResult, Parameter, PlaceProjection, ProjectedPlace, ReturnEdge, ScalarLiteral, ScalarType, TypeShape, ValueId,
};
use psi_handoff_consumer::{FloatScalar, evaluate_float_body};

const NEAREST: FloatSemantics = FloatSemantics::Ieee754NearestEvenV1;

fn body(package: &LoweredPackage) -> &MachineBody {
    let [machine] = package.machines.as_slice() else { panic!("one machine") };
    &machine.body
}

fn scalar(
    package: &LoweredPackage,
    value: ValueId,
) -> ScalarType {
    match package.types.get(body(package).values[value.index()].ty) {
        Some(TypeShape::PrimitiveScalar(scalar)) => *scalar,
        other => panic!("scalar value expected, found {other:?}"),
    }
}

fn returned(body: &MachineBody) -> ValueId {
    let [block] = body.blocks.as_slice() else { panic!("one block") };
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(value),
        ..
    } = block.terminator
    else {
        panic!("scalar return")
    };
    value
}

fn result(operation: &omega_lowering::PsiOperation) -> ValueId {
    let OperationResult::Scalar(value) = operation.result else {
        panic!("scalar result")
    };
    value
}

const F32: ScalarType = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
const F64: ScalarType = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
fn f32_bits(bits: u32) -> FloatScalar {
    FloatScalar::Bits(IeeeFloatBits::Binary32(bits))
}

/// The original case: two shared `f32` views are read and added. Psi names
/// the canonical addition with its nearest-even contract on the two reads, in
/// source order, and returns the f32 sum. A Psi-only reader recovers the IEEE
/// results, including a tie that rounds to even and an overflow to infinity.
#[test]
fn arithmetic_on_float_reads_names_the_nearest_even_addition_of_both_reads() {
    let source = "machine add(a:@f32, b:@f32)->f32 { $a + $b }";
    let package = sources::lower(&[("recast.omg", source)]).unwrap();
    let body = body(&package);
    let [Parameter::Structural(a), Parameter::Structural(b)] = body.parameters[..] else {
        panic!("two reference formals")
    };
    let [left, right, sum] = body.operations.as_slice() else {
        panic!("two reads and one addition: {:#?}", body.operations)
    };
    for (read, place) in [(left, a), (right, b)] {
        assert_eq!(
            read.kind,
            OperationKind::ProjectedScalarRead {
                source: ProjectedPlace {
                    root: place,
                    path: vec![PlaceProjection::Referent]
                }
            }
        );
        assert_eq!(scalar(&package, result(read)), F32);
    }
    spec_corpus::observe_handoff!(
        psi "psi-output/float-read-addition",
        assert_eq!(sum.kind, OperationKind::Float { operation: FloatOperation::Add, semantics: NEAREST, operands: vec![result(left), result(right)] }),
    );
    assert_eq!(scalar(&package, result(sum)), F32);
    assert_eq!(returned(body), result(sum));

    let add = |x: u32, y: u32| evaluate_float_body(&package, body, &[f32_bits(x), f32_bits(y)]);
    // 0.1 + 0.2 in binary32.
    assert_eq!(add(0x3DCC_CCCD, 0x3E4C_CCCD), Some(f32_bits(0x3E99_999A)));
    // 1 + 2^-24 is halfway between 1 and its successor: ties go to the even 1.
    assert_eq!(add(0x3F80_0000, 0x3380_0000), Some(f32_bits(0x3F80_0000)));
    // 1 + 3 * 2^-24 is halfway between two neighbours: the even one is 1 + 2^-22.
    assert_eq!(add(0x3F80_0000, 0x3440_0000), Some(f32_bits(0x3F80_0002)));
    // Quiet semantics: overflow is infinity, not a trap.
    assert_eq!(add(0x7F7F_FFFF, 0x7F7F_FFFF), Some(f32_bits(0x7F80_0000)));

    // Integer control, same shape: still an exact integer addition with its fit question.
    let integer = sources::lower(&[("recast.omg", "machine add(a:@u32, b:@u32)->u32 { $a + $b }")]).unwrap();
    let [_, _, sum] = body_operations(&integer) else {
        panic!("two reads and one addition")
    };
    let OperationKind::Integer { operation, .. } = &sum.kind else {
        panic!("integer addition: {:?}", sum.kind)
    };
    assert_eq!(
        (operation.primitive, operation.policy, operation.obligations.as_slice()),
        (
            IntegerPrimitive::Add,
            ArithmeticPolicy::Exact,
            [IntegerObligation::ResultFitsCarrier].as_slice()
        )
    );
}

fn body_operations(package: &LoweredPackage) -> &[omega_lowering::PsiOperation] {
    &body(package).operations
}

/// The original case: `bits as f32` on a `u32` is a carrier cast. It keeps the
/// denoted value, so Psi publishes one exact numeric cast from the u32 formal
/// to an f32 result; the checker owes the proof that the value is exactly
/// representable. It is not a bit reinterpretation: the reference recast
/// `@bits as @f32` is still a same-storage view. A Psi-only reader converts 2^24
/// and the integer whose bits spell 1.0, and has no value for 2^24 + 1, which
/// binary32 cannot hold exactly.
#[test]
fn value_level_integer_to_float_cast_is_an_exact_numeric_cast() {
    let source = "machine build(bits:u32)->f32 { bits as f32 }";
    let package = sources::lower(&[("recast.omg", source)]).unwrap();
    let body = body(&package);
    let [Parameter::Scalar(bits)] = body.parameters[..] else {
        panic!("one scalar formal")
    };
    let [cast] = body.operations.as_slice() else {
        panic!("one cast: {:#?}", body.operations)
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/integer-float-exact-cast",
        assert_eq!((&cast.kind, scalar(&package, bits), scalar(&package, result(cast))), (&OperationKind::ExactNumericCast { operand: bits }, ScalarType::Integer { signed: false, width: 32 }, F32)),
    );
    assert_eq!(returned(body), result(cast));
    let convert = |value: i128| evaluate_float_body(&package, body, &[FloatScalar::Integer(value)]);
    assert_eq!(convert(16_777_216), Some(f32_bits(0x4B80_0000)));
    assert_eq!(convert(0x3F80_0000), Some(f32_bits(0x4E7E_0000)), "the value, not the bits");
    assert_eq!(convert(16_777_217), None, "not exactly representable: the checker must refuse it");

    let view = "machine build(bits:u32)->f32 { let view: @f32 = @bits as @f32; $view }";
    let package = sources::lower(&[("recast.omg", view)]).unwrap();
    let operations = body_operations(&package);
    assert!(
        operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::ScalarBitRecast { .. })),
        "{operations:#?}"
    );
    assert!(
        !operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::ExactNumericCast { .. }))
    );
}

/// Every canonical operator spelling selects its own operation, for both
/// formats, with operands in source order. Comparisons produce a Boolean.
#[test]
fn each_float_operator_selects_its_canonical_operation_in_source_order() {
    use FloatOperation::*;
    for (format, ty) in [("f32", F32), ("f64", F64)] {
        for (token, operation) in [
            ("+", Add),
            ("-", Subtract),
            ("*", Multiply),
            ("/", Divide),
            ("==", Equal),
            ("!=", NotEqual),
            ("<", Less),
            ("<=", LessEqual),
            (">", Greater),
            (">=", GreaterEqual),
        ] {
            let comparison = !matches!(operation, Add | Subtract | Multiply | Divide);
            let result_type = if comparison { "bool" } else { format };
            let source = format!("machine op(a:{format}, b:{format})->{result_type} {{ a {token} b }}");
            let package = sources::lower(&[("op.omg", &source)]).unwrap();
            let body = body(&package);
            let [Parameter::Scalar(a), Parameter::Scalar(b)] = body.parameters[..] else {
                panic!("two scalar formals")
            };
            let [op] = body.operations.as_slice() else {
                panic!("{source}: one operation")
            };
            assert_eq!(
                op.kind,
                OperationKind::Float {
                    operation,
                    semantics: NEAREST,
                    operands: vec![a, b]
                },
                "{source}"
            );
            assert_eq!(scalar(&package, result(op)), if comparison { ScalarType::Boolean } else { ty }, "{source}");
        }
        let source = format!("machine op(a:{format})->{format} {{ -a }}");
        let package = sources::lower(&[("op.omg", &source)]).unwrap();
        let body = body(&package);
        let [Parameter::Scalar(a)] = body.parameters[..] else {
            panic!("one scalar formal")
        };
        let [op] = body.operations.as_slice() else { panic!("one negation") };
        assert_eq!(
            op.kind,
            OperationKind::Float {
                operation: Negate,
                semantics: NEAREST,
                operands: vec![a]
            }
        );
        assert_eq!(scalar(&package, result(op)), ty);
    }

    // IEEE comparison, not identity: NaN is unequal to itself and signed zeros are equal.
    let package = sources::lower(&[("op.omg", "machine op(a:f32, b:f32)->bool { a == b }")]).unwrap();
    let equal = |x, y| evaluate_float_body(&package, body(&package), &[f32_bits(x), f32_bits(y)]);
    assert_eq!(equal(0x7FC0_0000, 0x7FC0_0000), Some(FloatScalar::Bool(false)));
    assert_eq!(equal(0x8000_0000, 0x0000_0000), Some(FloatScalar::Bool(true)));
}

/// An anonymous number lands once in the destination format, rounding to
/// nearest with ties to even, and reaches Psi as exact bits. A folded
/// anonymous expression lands its exact rational value, so `-0.0` is rational
/// zero and lands as +0.0. A frozen float constant keeps preparation's bits.
#[test]
fn float_literals_and_constants_reach_psi_as_their_landed_bits() {
    let constant = |source: &str| {
        let package = sources::lower(&[("lit.omg", source)]).unwrap();
        let constants = body_operations(&package)
            .iter()
            .filter_map(|op| match op.kind {
                OperationKind::Constant(ScalarLiteral::IeeeFloat(bits)) => {
                    assert_eq!(scalar(&package, result(op)), ScalarType::IeeeFloat(bits.format()));
                    Some(bits)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [bits] = constants[..] else { panic!("{source}: one float constant") };
        bits
    };
    use IeeeFloatBits::{Binary32, Binary64};
    assert_eq!(constant("machine l(a:f32)->f32 { a * 0.1 }"), Binary32(0x3DCC_CCCD));
    assert_eq!(constant("machine l(a:f64)->f64 { a * 0.1 }"), Binary64(0x3FB9_9999_9999_999A));
    assert_eq!(constant("machine l()->f32 { 16777217.0 }"), Binary32(0x4B80_0000));
    assert_eq!(constant("machine l()->f32 { 16777219.0 }"), Binary32(0x4B80_0002));
    assert_eq!(constant("machine l(a:f64)->f64 { a * -(1.0 / 3.0) }"), Binary64(0xBFD5_5555_5555_5555));
    assert_eq!(constant("machine l()->f32 { 1.5f32 }"), Binary32(0x3FC0_0000));
    assert_eq!(constant("machine l()->f32 { 0.1 as f32 }"), Binary32(0x3DCC_CCCD));
    assert_eq!(constant("machine l(a:f32)->f32 { a + -0.0 }"), Binary32(0));
    assert_eq!(constant("const k: f32 = 1.5f32;\nmachine l()->f32 { k }"), Binary32(0x3FC0_0000));

    // The literal operand follows the read in source order.
    let package = sources::lower(&[("lit.omg", "machine l(a:f32)->f32 { a - 0.5 }")]).unwrap();
    let body = body(&package);
    let [Parameter::Scalar(a)] = body.parameters[..] else {
        panic!("one scalar formal")
    };
    let [half, difference] = body.operations.as_slice() else {
        panic!("constant then subtraction")
    };
    assert_eq!(
        difference.kind,
        OperationKind::Float {
            operation: FloatOperation::Subtract,
            semantics: NEAREST,
            operands: vec![a, result(half)]
        }
    );
    assert_eq!(evaluate_float_body(&package, body, &[f32_bits(0x3F80_0000)]), Some(f32_bits(0x3F00_0000)));
}

/// A NaN computed while preparing a constant has no exact bits to store, so
/// lowering refuses to materialize it rather than invent a payload.
#[test]
fn an_unspecified_nan_constant_is_not_materialized() {
    use omega_preparation::{FloatFormat, FloatRealization, TargetIdentity, TargetSemantics, prepare_source_package_for_target};
    let source = "const k: f32 = 0.0f32 / 0.0f32;\nmachine l()->f32 { k }";
    let files = [("nan.omg", source)];
    let target = TargetSemantics::new(
        TargetIdentity {
            profile: "float-test".into(),
            semantics_version: 1,
            realization: vec![1].into(),
        },
        [],
        [FloatRealization {
            format: FloatFormat::Binary32,
            operation: FloatOperation::Divide,
            semantics: NEAREST,
        }],
    )
    .unwrap();
    let prepared = prepare_source_package_for_target(&sources::resolve(&files), &[], &target).unwrap();
    let error = omega_lowering::lower_package(&prepared).unwrap_err();
    assert_eq!(error.message, "an unspecified NaN constant has no exact bits to materialize");
    assert_eq!(error.site.span.get_start_offset(), source.rfind('k').unwrap());
}

/// Every numeric carrier change is one exact cast: the operand and result
/// types tell the checker which representability fact to prove.
#[test]
fn exact_casts_cover_integer_and_float_carriers() {
    let byte = ScalarType::Integer { signed: false, width: 8 };
    let int = ScalarType::Integer { signed: true, width: 32 };
    for (source, from, to) in [
        ("machine c(x:u8)->f32 { x as f32 }", byte, F32),
        ("machine c(x:i32)->f64 { x as f64 }", int, F64),
        ("machine c(x:f32)->f64 { x as f64 }", F32, F64),
        ("machine c(x:f64)->f32 { x as f32 }", F64, F32),
        ("machine c(x:f32)->i32 { x as i32 }", F32, int),
        ("machine c(x:i32)->u8 { x as u8 }", int, byte),
    ] {
        let package = sources::lower(&[("cast.omg", source)]).unwrap();
        let body = body(&package);
        let [Parameter::Scalar(x)] = body.parameters[..] else {
            panic!("one scalar formal")
        };
        let [cast] = body.operations.as_slice() else { panic!("{source}: one cast") };
        assert_eq!(cast.kind, OperationKind::ExactNumericCast { operand: x }, "{source}");
        assert_eq!((scalar(&package, x), scalar(&package, result(cast))), (from, to), "{source}");
    }
}

/// Floats have no Wrapping candidate, a policy operator spelling has no float
/// meaning, and the two formats do not mix: typing refuses before lowering.
#[test]
fn policy_spellings_and_mixed_formats_have_no_float_operator() {
    for (source, message) in [
        ("machine s(a:f32, b:f32)->f32 { a +| b }", "no matching operator +| for f32 and f32"),
        ("machine s(a:f32, b:f32)->f32 { a +! b }", "no matching operator +! for f32 and f32"),
        ("machine s(a:f32, b:f32)->f32 { a +% b }", "no matching operator +% for f32 and f32"),
        ("machine s(a:f32, b:f64)->f64 { a + b }", "no matching operator + for f32 and f64"),
    ] {
        let error = omega_preparation::prepare_source_package(&sources::resolve(&[("s.omg", source)])).unwrap_err();
        assert_eq!(error.message, message, "{source}");
    }
}

/// The exact input of `l5_package_cases.tsv` case `float comparison of two
/// formals` (`float-comparison-of-formals/case/case.omg`), which the package
/// command measured as a refusal before float operations lowered. `a < b` on
/// two `f32` formals is one canonical `Less` with the nearest-even contract on
/// the formals in source order, returning a Boolean. A Psi-only reader computes
/// IEEE ordering, where a NaN operand compares false.
#[test]
fn the_float_comparison_package_case_publishes_the_canonical_less() {
    let source = include_str!("fixtures/package_cases/float-comparison-of-formals/case/case.omg");
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let body = body(&package);
    let [Parameter::Scalar(a), Parameter::Scalar(b)] = body.parameters[..] else { panic!("two scalar formals") };
    let [op] = body.operations.as_slice() else { panic!("one comparison") };
    spec_corpus::observe_handoff!(
        psi "psi-output/float-formal-comparison",
        assert_eq!(
            (&op.kind, scalar(&package, a), scalar(&package, b), scalar(&package, result(op)), returned(body) == result(op)),
            (&OperationKind::Float { operation: FloatOperation::Less, semantics: NEAREST, operands: vec![a, b] }, F32, F32, ScalarType::Boolean, true)
        ),
    );
    let compare = |x: u32, y: u32| evaluate_float_body(&package, body, &[f32_bits(x), f32_bits(y)]);
    spec_corpus::observe_handoff!(
        psi "psi-output/float-formal-comparison-consumer",
        assert_eq!(
            [compare(0x3F80_0000, 0x4000_0000), compare(0x4000_0000, 0x3F80_0000), compare(0x7FC0_0000, 0x3F80_0000)],
            [Some(FloatScalar::Bool(true)), Some(FloatScalar::Bool(false)), Some(FloatScalar::Bool(false))]
        ),
    );
}

/// The exact input of `l5_package_cases.tsv` case `float literal result`
/// (`float-literal-result/case/case.omg`). L4 lands `1.0` at `f32`, and Psi
/// returns one constant with its exact binary32 bits; lowering does not round
/// again. A Psi-only reader returns those bits.
#[test]
fn the_float_literal_package_case_returns_its_landed_bits() {
    let source = include_str!("fixtures/package_cases/float-literal-result/case/case.omg");
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let body = body(&package);
    let [op] = body.operations.as_slice() else { panic!("one constant") };
    spec_corpus::observe_handoff!(
        psi "psi-output/float-literal-result",
        assert_eq!(
            (&op.kind, scalar(&package, result(op)), returned(body) == result(op)),
            (&OperationKind::Constant(ScalarLiteral::IeeeFloat(IeeeFloatBits::Binary32(0x3F80_0000))), F32, true)
        ),
    );
    spec_corpus::observe_handoff!(
        psi "psi-output/float-literal-result-consumer",
        assert_eq!(evaluate_float_body(&package, body, &[]), Some(f32_bits(0x3F80_0000))),
    );
}
