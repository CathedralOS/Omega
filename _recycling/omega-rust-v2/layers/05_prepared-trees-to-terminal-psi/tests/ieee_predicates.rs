//! IEEE comparisons in contracts and domain predicates are typed propositions,
//! not executable operations.
//! [Comparisons](../../../../wiki/spec/language/numeric_values.md#meaning-comparisons-and-representation)
//! · [IEEE comparison kinds](../../../../source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#ieee-comparison-kinds)
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{
    CallTarget, ContractClause, FloatOperation, FloatSemantics, IeeeComparisonKind, IeeeFloatBits, IeeeFloatFormat, LoweredPackage, OperationKind,
    PredicateOperator, PredicateStep, ScalarLiteral,
};
use psi_handoff_consumer::{PredicateValue, evaluate_ieee_clause, float_formal};

const ONE: u32 = 0x3F80_0000;
const HALF: u32 = 0x3F00_0000;
const QUIET_NAN: u32 = 0x7FC0_0000;
const NEGATIVE_ZERO: u32 = 0x8000_0000;

fn bits32(bits: u32) -> PredicateValue {
    PredicateValue::Bits(IeeeFloatBits::Binary32(bits))
}

fn machine<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> &'a omega_lowering::LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == path)
        .unwrap_or_else(|| panic!("machine {path}"))
}

fn sole(clauses: &[ContractClause]) -> &ContractClause {
    let [clause] = clauses else { panic!("one clause: {clauses:#?}") };
    clause
}

fn requirement(source: &str) -> Vec<PredicateStep> {
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    sole(&package.machines[0].contract.requires).predicate.clone()
}

fn less(format: IeeeFloatFormat) -> PredicateStep {
    PredicateStep::IeeeComparison {
        kind: IeeeComparisonKind::LessThan,
        format,
    }
}

/// The exact input of `l5_package_cases.tsv` case `float comparison in a
/// contract` (`float-comparison-in-contract/case/case.omg`). `requires a < 1.0`
/// becomes one typed requirement: the formal, preparation's binary32 bits
/// of 1.0, and an IEEE `LessThan` in binary32. The body gains no executable
/// comparison. A Psi-only reader decides the requirement: NaN is unordered,
/// and -0.0 is below 1.0. The control, which has no precondition, publishes no
/// clause.
#[test]
fn the_float_contract_package_case_states_a_typed_ieee_requirement() {
    let source = include_str!("fixtures/package_cases/float-comparison-in-contract/case/case.omg");
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let l = machine(&package, "l");
    let clause = sole(&l.contract.requires);
    spec_corpus::observe_handoff!(
        psi "psi-output/float-contract-requirement",
        assert_eq!(
            (clause.predicate.as_slice(), l.body.operations.is_empty(), l.contract.ensures.is_empty()),
            ([PredicateStep::Formal(0), PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE)), less(IeeeFloatFormat::Binary32)].as_slice(), true, true)
        ),
    );
    assert_eq!(
        clause.site.span.get_start_offset(),
        source.find("requires a < 1.0").unwrap(),
        "the authored clause"
    );
    let decide = |a| evaluate_ieee_clause(clause, &[bits32(a)], None, None);
    spec_corpus::observe_handoff!(
        psi "psi-output/float-contract-requirement-consumer",
        assert_eq!(
            [decide(HALF), decide(ONE), decide(QUIET_NAN), decide(NEGATIVE_ZERO)],
            [Some(true), Some(false), Some(false), Some(true)]
        ),
    );

    let control = include_str!("fixtures/package_cases/float-comparison-in-contract/control/case.omg");
    let package = sources::lower(&[("case.omg", control)]).unwrap();
    assert!(package.machines[0].contract.requires.is_empty());

    // The same bytes lower to the same product.
    assert_eq!(
        sources::lower(&[("case.omg", source)]).unwrap(),
        sources::lower(&[("case.omg", source)]).unwrap()
    );
}

/// Each source comparison names its canonical kind. `>` and `>=` read their
/// operands in reverse, so `a > b` and `b < a` state the same proposition.
/// `!=` keeps its own complementary kind, and `!(a < b)` stays a Boolean
/// negation, which differs from `a >= b` on NaN. The executable body keeps the
/// operation as written.
#[test]
fn contract_comparisons_keep_ieee_kind_and_operand_order() {
    use IeeeComparisonKind::*;
    use PredicateStep::{Formal, IeeeComparison};
    for (format, name) in [
        (IeeeFloatFormat::Binary32, "f32"),
        (IeeeFloatFormat::Binary64, "f64"),
    ] {
        let compare = |kind| IeeeComparison { kind, format };
        for (spelling, expected) in [
            ("a < b", vec![Formal(0), Formal(1), compare(LessThan)]),
            ("a <= b", vec![Formal(0), Formal(1), compare(LessOrEqual)]),
            ("a == b", vec![Formal(0), Formal(1), compare(Equal)]),
            ("a != b", vec![Formal(0), Formal(1), compare(NotEqual)]),
            ("b < a", vec![Formal(1), Formal(0), compare(LessThan)]),
            ("a > b", vec![Formal(1), Formal(0), compare(LessThan)]),
            ("a >= b", vec![Formal(1), Formal(0), compare(LessOrEqual)]),
            (
                "!(a < b)",
                vec![
                    Formal(0),
                    Formal(1),
                    compare(LessThan),
                    PredicateStep::Apply(PredicateOperator::Not),
                ],
            ),
        ] {
            let source = format!("machine c(a:{name}, b:{name})->bool requires {spelling} {{ true }}");
            assert_eq!(requirement(&source), expected, "{source}");
        }
    }

    // Decide the f32 forms on unordered and signed inputs using Psi alone.
    let decide = |spelling: &str, a: u32, b: u32| {
        let package = sources::lower(&[("case.omg", &format!("machine c(a:f32, b:f32)->bool requires {spelling} {{ true }}"))]).unwrap();
        evaluate_ieee_clause(sole(&package.machines[0].contract.requires), &[bits32(a), bits32(b)], None, None).unwrap()
    };
    assert!(!decide("a == a", QUIET_NAN, 0), "NaN is not equal to itself");
    assert!(decide("a != a", QUIET_NAN, 0));
    assert!(decide("a == b", NEGATIVE_ZERO, 0), "signed zeros compare equal");
    assert!(!decide("a < b", NEGATIVE_ZERO, 0));
    assert!(decide("!(a < b)", QUIET_NAN, ONE), "the negation is true on NaN");
    assert!(!decide("a >= b", QUIET_NAN, ONE), "but the ordered comparison is false");

    // A crash guard states the same kind of proposition over its entry formal.
    let package = sources::lower(&[("case.omg", "machine l(a:f32)->f32 crashes Trap a < 0.0 { a }")]).unwrap();
    let [route] = package.machines[0].contract.crash_routes.as_slice() else {
        panic!("one crash route")
    };
    let guard = route.guard.as_ref().expect("a guarded route");
    assert_eq!(
        guard.predicate,
        [
            Formal(0),
            PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(0)),
            less(IeeeFloatFormat::Binary32)
        ]
    );
    assert_eq!(
        evaluate_ieee_clause(guard, &[bits32(NEGATIVE_ZERO)], None, None),
        Some(false),
        "-0.0 is not below +0.0"
    );

    // Body and contract of one machine: the body executes `Float { Greater }`
    // on (a, b); the requirement states `LessThan` of (b, a).
    let package = sources::lower(&[("case.omg", "machine c(a:f32, b:f32)->bool requires a > b { a > b }")]).unwrap();
    let c = &package.machines[0];
    assert_eq!(sole(&c.contract.requires).predicate, [Formal(1), Formal(0), less(IeeeFloatFormat::Binary32)]);
    let [op] = c.body.operations.as_slice() else { panic!("one comparison") };
    let OperationKind::Float {
        operation,
        semantics,
        operands,
    } = &op.kind
    else {
        panic!("{:?}", op.kind)
    };
    assert_eq!(
        (*operation, *semantics, operands.len()),
        (FloatOperation::Greater, FloatSemantics::Ieee754NearestEvenV1, 2)
    );
}

/// A literal in a predicate is the value preparation landed, rounded once
/// to nearest with ties to even: 16777217.0 is the binary32 2^24, so the
/// requirement is false at a = 2^24. Anonymous `-0.0` is rational zero and
/// lands as +0.0. A frozen constant keeps its bits.
#[test]
fn contract_literals_use_the_bits_preparation_landed() {
    let literal = |source: &str| match requirement(source).as_slice() {
        [
            _,
            PredicateStep::IeeeFloat(bits),
            PredicateStep::IeeeComparison { .. },
        ] => *bits,
        other => panic!("{source}: {other:?}"),
    };
    assert_eq!(
        literal("machine l(a:f32)->f32 requires a < 16777217.0 { a }"),
        IeeeFloatBits::Binary32(0x4B80_0000)
    );
    assert_eq!(literal("machine l(a:f32)->f32 requires a < 0.1 { a }"), IeeeFloatBits::Binary32(0x3DCC_CCCD));
    assert_eq!(
        literal("machine l(a:f64)->f64 requires a < 0.1 { a }"),
        IeeeFloatBits::Binary64(0x3FB9_9999_9999_999A)
    );
    assert_eq!(literal("machine l(a:f32)->f32 requires a == -0.0 { a }"), IeeeFloatBits::Binary32(0));
    assert_eq!(
        literal("machine l(a:f32)->f32 requires a == -(1.0) { a }"),
        IeeeFloatBits::Binary32(0xBF80_0000)
    );
    assert_eq!(
        literal("const k: f32 = 1.5f32; machine l(a:f32)->f32 requires a < k { a }"),
        IeeeFloatBits::Binary32(0x3FC0_0000)
    );

    let package = sources::lower(&[("case.omg", "machine l(a:f32)->f32 requires a < 16777217.0 { a }")]).unwrap();
    let clause = sole(&package.machines[0].contract.requires);
    assert_eq!(evaluate_ieee_clause(clause, &[bits32(0x4B80_0000)], None, None), Some(false));
    assert_eq!(evaluate_ieee_clause(clause, &[bits32(0x4B7F_FFFF)], None, None), Some(true));
}

/// Lowering decides nothing. A premise that no input satisfies and a guarantee
/// the body breaks reach Psi exactly as written, as checker input. A call
/// owes the callee's requirement at its own occurrence, with the actual it
/// passes: 0.5 satisfies it and 1.5 does not, and both calls still lower.
#[test]
fn false_premises_guarantees_and_call_debts_stay_checker_input() {
    assert_eq!(
        requirement("machine l(a:f32)->f32 requires 1.0f32 < 0.5f32 { a }"),
        [
            PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE)),
            PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(HALF)),
            less(IeeeFloatFormat::Binary32)
        ]
    );
    let package = sources::lower(&[("case.omg", "machine l(a:f32)->f32 ensures result < 0.0 { 1.0 }")]).unwrap();
    let l = &package.machines[0];
    let guarantee = sole(&l.contract.ensures);
    assert_eq!(
        guarantee.predicate,
        [
            PredicateStep::Result,
            PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(0)),
            less(IeeeFloatFormat::Binary32)
        ]
    );
    let [constant] = l.body.operations.as_slice() else { panic!("one constant") };
    assert_eq!(constant.kind, OperationKind::Constant(ScalarLiteral::IeeeFloat(IeeeFloatBits::Binary32(ONE))));
    assert_eq!(
        evaluate_ieee_clause(guarantee, &[bits32(HALF)], Some(&bits32(ONE)), None),
        Some(false),
        "the body breaks it"
    );

    for (actual, holds) in [("0.5", true), ("1.5", false)] {
        let source = format!("machine l(a:f32)->f32 requires a < 1.0 {{ a }} machine m()->f32 {{ l({actual}) }}");
        let package = sources::lower(&[("case.omg", &source)]).unwrap();
        let callee = machine(&package, "l");
        let caller = machine(&package, "m");
        let [argument, call] = caller.body.operations.as_slice() else {
            panic!("constant and call: {:#?}", caller.body.operations)
        };
        let OperationKind::Constant(ScalarLiteral::IeeeFloat(bits)) = argument.kind else {
            panic!("{:?}", argument.kind)
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(target),
            requirements,
            ..
        } = &call.kind
        else {
            panic!("{:?}", call.kind)
        };
        assert_eq!((*target, requirements.len(), requirements[0].clause), (callee.id, 1, 0));
        let clause = &callee.contract.requires[requirements[0].clause];
        assert_eq!(evaluate_ieee_clause(clause, &[PredicateValue::Bits(bits)], None, None), Some(holds), "{source}");
    }
}

/// A domain predicate and a machine requirement state the same IEEE
/// comparison about different subjects: the domain's `self` and the machine's
/// formal. A user-declared `<` on a record stays a call, which contracts
/// still refuse, while the builtin comparison of its `f32` field is IEEE.
#[test]
fn domain_predicates_formals_fields_and_user_operators_keep_their_subjects() {
    let package = sources::lower(&[(
        "case.omg",
        "domain f32::Small requires self < 1.0; machine l(a:f32)->f32 requires a < 1.0 { a }",
    )])
    .unwrap();
    let [domain] = package.domains.as_slice() else { panic!("one domain") };
    let in_domain = sole(&domain.requires);
    let one = PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE));
    assert_eq!(
        in_domain.predicate,
        [
            PredicateStep::SelfValue,
            one.clone(),
            less(IeeeFloatFormat::Binary32)
        ]
    );
    assert_eq!(
        sole(&package.machines[0].contract.requires).predicate,
        [
            PredicateStep::Formal(0),
            one.clone(),
            less(IeeeFloatFormat::Binary32)
        ]
    );
    assert_eq!(evaluate_ieee_clause(in_domain, &[], None, Some(&bits32(HALF))), Some(true));
    assert_eq!(evaluate_ieee_clause(in_domain, &[], None, Some(&bits32(QUIET_NAN))), Some(false));

    let field = "data T { v: f32; } machine l(a: T)->T requires a.v < 1.0 { a }";
    let package = sources::lower(&[("case.omg", field)]).unwrap();
    let clause = sole(&package.machines[0].contract.requires);
    let [PredicateStep::Formal(0), PredicateStep::Field(v), _, _] = clause.predicate.as_slice() else {
        panic!("{:?}", clause.predicate)
    };
    assert_eq!(clause.predicate[2..], [one, less(IeeeFloatFormat::Binary32)]);
    let record = |bits| PredicateValue::Record(vec![(v.clone(), bits32(bits))]);
    assert_eq!(evaluate_ieee_clause(clause, &[record(HALF)], None, None), Some(true));
    assert_eq!(evaluate_ieee_clause(clause, &[record(ONE)], None, None), Some(false));

    let user = "data T { v: f32; } machine < before(a: T, b: T) -> bool crashes none { true } machine l(a: T, b: T)->T requires a < b { a }";
    let twin = user.replace("machine < before", "machine before").replace("machine l(",
        "machine < after(a:T,b:T)->bool requires a.v <= b.v crashes none { false } machine l(").replace("requires a < b", "requires b < a");
    let observe = |source: &str| {
        let package = sources::lower(&[("case.omg", source)]).unwrap();
        let (callee, arguments, requirements, result) = psi_handoff_consumer::record_predicate_application(&package, "l").unwrap();
        assert!(machine(&package, "l").body.operations.iter().all(|op| !matches!(op.kind, OperationKind::Call { .. } | OperationKind::Float { .. })));
        (callee.path.clone(), arguments, requirements.len(), result)
    };
    spec_corpus::observe_handoff!(psi "psi-output/selected-record-predicate-operator", assert_eq!(
        [observe(user), observe(&twin)], [("before".into(), [0,1], 0, true), ("after".into(), [1,0], 1, false)]
    ));
    let body = "data T { v: f32; } machine < before(a: T, b: T) -> bool crashes none { true } machine l(a: T, b: T)->bool { a < b }";
    let package = sources::lower(&[("case.omg", body)]).unwrap();
    let operations = &machine(&package, "l").body.operations;
    assert!(
        operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Call { .. })),
        "{operations:#?}"
    );
    assert!(
        !operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Float { .. }))
    );
}

/// The reader is sensitive to each part of the published requirement:
/// changing the kind, swapping the operands or changing one literal bit
/// changes its verdict on some input, and a format the operands do not have
/// is ill-formed.
#[test]
fn corrupted_requirements_change_the_reader_verdict() {
    let source = include_str!("fixtures/package_cases/float-comparison-in-contract/case/case.omg");
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let clause = sole(&package.machines[0].contract.requires).clone();
    let inputs = [HALF, ONE, 0x3F80_0001, QUIET_NAN, NEGATIVE_ZERO, 0xBF80_0000];
    let verdicts = |clause: &ContractClause| {
        inputs
            .iter()
            .map(|&a| evaluate_ieee_clause(clause, &[bits32(a)], None, None))
            .collect::<Vec<_>>()
    };
    let original = verdicts(&clause);
    assert_eq!(
        original,
        [
            Some(true),
            Some(false),
            Some(false),
            Some(false),
            Some(true),
            Some(true)
        ]
    );
    let corrupt = |edit: &dyn Fn(&mut Vec<PredicateStep>)| {
        let mut changed = clause.clone();
        edit(&mut changed.predicate);
        verdicts(&changed)
    };
    let kind = corrupt(&|steps| {
        steps[2] = PredicateStep::IeeeComparison {
            kind: IeeeComparisonKind::LessOrEqual,
            format: IeeeFloatFormat::Binary32,
        }
    });
    let order = corrupt(&|steps| steps.swap(0, 1));
    let bit = corrupt(&|steps| steps[1] = PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE + 1)));
    let format = corrupt(&|steps| steps[2] = less(IeeeFloatFormat::Binary64));
    for (name, changed) in [("kind", &kind), ("order", &order), ("bit", &bit)] {
        assert_ne!(changed, &original, "{name}");
    }
    assert!(format.iter().all(Option::is_none));
}

#[test]
fn float_arithmetic_publishes_rounded_predicate_terms() {
    let add = "machine l(a:f32)->f32 requires a + 1.0 < 2.0 { a }";
    let neg = "machine l(a:f32)->f32 requires -a < 1.0 { a }";
    let format = IeeeFloatFormat::Binary32;
    let term = |operation| PredicateStep::IeeeArithmetic { operation, format, semantics: FloatSemantics::Ieee754NearestEvenV1 };
    let expected = [
        vec![PredicateStep::Formal(0), PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE)), term(FloatOperation::Add),
             PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(2.0f32.to_bits())), less(format)],
        vec![PredicateStep::Formal(0), term(FloatOperation::Negate), PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE)), less(format)],
    ];
    let mut outcomes = vec![];
    for (source, steps) in [add, neg].into_iter().zip(expected) {
        let package = sources::lower(&[("case.omg", source)]).unwrap();
        let l = machine(&package, "l");
        assert!(l.body.operations.is_empty());
        let clause = sole(&l.contract.requires);
        assert_eq!(clause.predicate, steps);
        outcomes.push([ONE - 2, ONE - 1, QUIET_NAN, NEGATIVE_ZERO, 2.0f32.to_bits()].map(|bits| {
            evaluate_ieee_clause(clause, &[bits32(bits)], None, None)
        }));
    }
    spec_corpus::observe_handoff!(
        psi "psi-output/float-arithmetic-predicate",
        assert_eq!(outcomes, [
            [Some(true), Some(false), Some(false), Some(true), Some(false)],
            [Some(true), Some(true), Some(false), Some(true), Some(true)],
        ])
    );
}

#[test]
fn nested_double_precision_predicates_preserve_operand_order_and_width() {
    let source = "machine d(a:f64, b:f64)->f64 ensures (a - b) / (a * b) < result { a }";
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let clause = sole(&machine(&package, "d").contract.ensures);
    let bits = |x:f64| PredicateValue::Bits(IeeeFloatBits::Binary64(x.to_bits()));
    // (3 - 2) / 6 is below 0.2, while 6 / (3 - 2) is above it.
    assert_eq!(evaluate_ieee_clause(clause, &[bits(3.0), bits(2.0)], Some(&bits(0.2)), None), Some(true));
    assert_eq!(evaluate_ieee_clause(clause, &[bits(2.0), bits(3.0)], Some(&bits(0.0)), None), Some(true));
    assert_eq!(evaluate_ieee_clause(clause, &[bits(3.0), bits(2.0)], Some(&bits(0.0)), None), Some(false));
    // Widening inputs or producing a comparison instead of an arithmetic result
    // would make the predicate ill-formed at this point.
    let mut changed = clause.clone();
    let PredicateStep::IeeeArithmetic { format, .. } = &mut changed.predicate[2] else { panic!("expected subtraction") };
    *format = IeeeFloatFormat::Binary32;
    assert_eq!(evaluate_ieee_clause(&changed, &[bits(3.0), bits(2.0)], Some(&bits(0.2)), None), None);
}

/// The body and the predicate must agree on nearest-even ties and unordered NaN.
#[test]
fn executable_twins_distinguish_rounded_addition_and_unordered_negation() {
    use psi_handoff_consumer::{FloatScalar, evaluate_float_body};
    for (expression, results) in [
        ("a + 1.0 < 2.0", [true, false, false, true]),
        ("-a < 1.0", [true, true, false, true]),
    ] {
        let source = format!("machine twin(a:f32)->bool {{ {expression} }}");
        let package = sources::lower(&[("case.omg", &source)]).unwrap();
        let twin = machine(&package, "twin");
        assert!(twin.contract.requires.is_empty());
        for (bits, expected) in [ONE - 2, ONE - 1, QUIET_NAN, NEGATIVE_ZERO].into_iter().zip(results) {
            let value = FloatScalar::Bits(IeeeFloatBits::Binary32(bits));
            assert_eq!(evaluate_float_body(&package, &twin.body, &[value]), Some(FloatScalar::Bool(expected)), "{expression}, 0x{bits:08x}");
        }
    }
}

#[test]
fn a_qualified_float_formal_is_compared_on_its_unchanged_carrier() {
    let original = "domain f32::Small; machine l(a:f32 in Small)->f32 requires (a as -Small) < 1.0 { a as -Small }";
    let package = sources::lower(&[("case.omg", original)]).unwrap();
    let l = machine(&package, "l");
    let clause = sole(&l.contract.requires);
    let bare = include_str!("fixtures/package_cases/float-comparison-in-contract/case/case.omg");
    let control = sources::lower(&[("case.omg", bare)]).unwrap();
    let misplaced = original.replace("l(a:", "l(b:f32, a:");
    let twin = sources::lower(&[("case.omg", &misplaced)]).unwrap();
    let twin_clause = sole(&machine(&twin, "l").contract.requires);
    let [small] = package.domains.as_slice() else { panic!("expected Small") };
    assert_eq!(small.declaration.path, "f32::Small");
    let atom = omega_psi::DomainAtom { declaration: small.declaration.clone(), arguments: vec![] };
    let format = IeeeFloatFormat::Binary32;
    let expected = [PredicateStep::Formal(0), PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE)), less(format)];
    let control_clause = sole(&machine(&control, "l").contract.requires);
    spec_corpus::observe_handoff!(
        psi "psi-output/qualified-float-contract",
        assert_eq!(
            (float_formal(&package, "l", 0), float_formal(&control, "l", 0), clause.predicate.as_slice(), control_clause.predicate.as_slice()),
            (Some((format, vec![atom])), Some((format, vec![])), expected.as_slice(), expected.as_slice())
        )
    );
    spec_corpus::observe_handoff!(
        psi "psi-output/qualified-float-contract-consumer",
        assert_eq!(
            ([HALF, ONE, QUIET_NAN, NEGATIVE_ZERO].map(|bits| evaluate_ieee_clause(clause, &[bits32(bits)], None, None)),
             evaluate_ieee_clause(twin_clause, &[bits32(ONE), bits32(HALF)], None, None),
             evaluate_ieee_clause(twin_clause, &[bits32(HALF), bits32(ONE)], None, None)),
            ([Some(true), Some(false), Some(false), Some(true)], Some(true), Some(false))
        )
    );
    // The separately authored cast in the body erases the domain on its jump.
    use omega_psi::{BlockTerminator, Parameter, QualificationDirection, ReturnEdge, ScalarQualificationChange, ScalarType, TypeShape, ValueOrigin};
    let [Parameter::Scalar(input)] = l.body.parameters[..] else { panic!("expected scalar input") };
    let [entry, arrival] = &l.body.blocks[..] else { panic!("expected the cast jump and its arrival") };
    let BlockTerminator::Jump(edge) = &entry.terminator else { panic!("expected cast jump") };
    let [dest] = arrival.scalar_parameters[..] else { panic!("expected erased result") };
    assert!(l.body.operations.is_empty());
    let cast_site = &l.body.diagnostics.edges[edge.id.get() - 1].span;
    assert_eq!(cast_site.get_start_offset()..cast_site.get_end_offset(), sources::span_within(original, "{ a as -Small }", "a as -Small"));
    assert_eq!((edge.target, &edge.arguments[..]), (arrival.id, &[input][..]));
    assert_eq!(edge.scalar_qualification_changes, [ScalarQualificationChange { argument_index: 0, direction: QualificationDirection::Erase }]);
    assert!(edge.domain_completions.is_empty());
    assert_eq!(l.body.values[dest.index()].origin, ValueOrigin::BlockParameter(arrival.id));
    assert_eq!(package.types.get(l.body.values[dest.index()].ty), Some(&TypeShape::PrimitiveScalar(ScalarType::IeeeFloat(format))));
    assert_eq!(l.result_type, Some(l.body.values[dest.index()].ty));
    assert!(matches!(arrival.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(id), .. } if id == dest));
}

#[test]
fn a_float_domain_formal_preserves_its_declared_tag() {
    let case = include_str!("fixtures/package_cases/float-domain-predicate/case/case.omg");
    let control = include_str!("fixtures/package_cases/float-domain-predicate/control/case.omg");
    let qualified = sources::lower(&[("case.omg", case)]).unwrap();
    let bare = sources::lower(&[("case.omg", control)]).unwrap();
    let [unit] = &qualified.domains[..] else { panic!("expected Unit") };
    let [control_unit] = &bare.domains[..] else { panic!("expected Unit in the control") };
    assert_eq!(unit.declaration.path, "f32::Unit");
    assert_eq!(unit.declaration, control_unit.declaration);
    assert!(unit.requires.is_empty() && control_unit.requires.is_empty());
    let format = IeeeFloatFormat::Binary32;
    let carrier = omega_psi::TypeShape::PrimitiveScalar(omega_psi::ScalarType::IeeeFloat(format));
    assert_eq!(qualified.types.get(unit.carrier), Some(&carrier));
    assert_eq!(bare.types.get(control_unit.carrier), Some(&carrier));
    let atom = omega_psi::DomainAtom { declaration: unit.declaration.clone(), arguments: vec![] };
    spec_corpus::observe_handoff!(
        psi "psi-output/float-domain-formal",
        assert_eq!(
            (float_formal(&qualified, "f", 0), float_formal(&bare, "f", 0)),
            (Some((format, vec![atom])), Some((format, vec![])))
        )
    );
}

/// A nonempty binary64 domain supplies its predicate on the qualified
/// formal even when the program has no authored requires clause.
#[test]
fn a_qualified_float_carries_its_domain_predicate_to_entry() {
    let source = "domain f64::Positive requires self > 0.0; machine l(b:f64, a:f64 in Positive)->f64 { a as -Positive }";
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let [domain] = &package.domains[..] else { panic!("expected Positive") };
    let atom = omega_psi::DomainAtom { declaration: domain.declaration.clone(), arguments: vec![] };
    let format = IeeeFloatFormat::Binary64;
    assert_eq!(float_formal(&package, "l", 0), Some((format, vec![])));
    assert_eq!(float_formal(&package, "l", 1), Some((format, vec![atom])));
    let entry = sole(&machine(&package, "l").contract.requires);
    let definition = sole(&domain.requires);
    let zero = PredicateStep::IeeeFloat(IeeeFloatBits::Binary64(0));
    assert_eq!(definition.predicate, [zero.clone(), PredicateStep::SelfValue, less(format)]);
    assert_eq!(entry.predicate, [zero, PredicateStep::Formal(1), less(format)]);
    let bits = |v: f64| PredicateValue::Bits(IeeeFloatBits::Binary64(v.to_bits()));
    for (first, second, expected) in [(0.0, 1.0, true), (1.0, -0.0, false), (1.0, f64::NAN, false)] {
        assert_eq!(evaluate_ieee_clause(entry, &[bits(first), bits(second)], None, None), Some(expected));
    }
}

#[test]
fn exact_comparisons_publish_the_truth_in_each_psi_context() {
    let original = "machine l(a:f32)->f32 requires 1.0 < 0.5 { a }";
    let package = sources::lower(&[("case.omg", original)]).unwrap();
    let requirement = sole(&machine(&package, "l").contract.requires);
    let source = "machine g()->bool requires (1 / 3) == (2 / 6) ensures (1 / 3) != (2 / 6) { 1.000000000000000000000000000001 > 1.0 }";
    let controls = sources::lower(&[("case.omg", source)]).unwrap();
    let g = machine(&controls, "g");
    let equal = sole(&g.contract.requires);
    let unequal = sole(&g.contract.ensures);
    spec_corpus::observe_handoff!(
        psi "psi-output/exact-predicate",
        assert_eq!(
            (&requirement.predicate[..], &equal.predicate[..], &unequal.predicate[..],
             g.body.operations.iter().map(|operation| &operation.kind).collect::<Vec<_>>()),
            (&[PredicateStep::Boolean(false)][..], &[PredicateStep::Boolean(true)][..], &[PredicateStep::Boolean(false)][..],
             vec![&OperationKind::Constant(ScalarLiteral::Boolean(true))])
        )
    );
    spec_corpus::observe_handoff!(
        psi "psi-output/exact-predicate-consumer",
        assert_eq!(
            [requirement, equal, unequal].map(|clause| evaluate_ieee_clause(clause, &[], None, None)),
            [Some(false), Some(true), Some(false)]
        )
    );
}

#[test]
fn a_predicate_follows_both_references_to_the_exact_field() {
    let text = "data Reading { sample:f32; other:f32; }
                data View<'a> { reading:@'a Reading; }
                machine check<'a>(view:@View<'a>) requires view.reading.sample < 1.0 {}";
    let package = sources::lower(&[("view.omg", text)]).unwrap();
    let field = |owner: &str, name: &str| {
        package
            .data
            .iter()
            .find(|d| d.declaration.path == owner)
            .unwrap()
            .fields
            .iter()
            .find(|f| f.declaration.path == name)
            .unwrap()
            .declaration
            .clone()
    };
    let stored = field("View", "View::reading");
    let sample = field("Reading", "Reading::sample");
    let other = field("Reading", "Reading::other");
    let clause = sole(&machine(&package, "check").contract.requires);
    assert_eq!(
        clause.predicate,
        [
            PredicateStep::Formal(0),
            PredicateStep::Referent,
            PredicateStep::Field(stored.clone()),
            PredicateStep::Referent,
            PredicateStep::Field(sample.clone()),
            PredicateStep::IeeeFloat(IeeeFloatBits::Binary32(ONE)),
            less(IeeeFloatFormat::Binary32),
        ]
    );
    let reference = |value| PredicateValue::ReadableReference(Box::new(value));
    let input = |value| {
        reference(PredicateValue::Record(vec![(
            stored.clone(),
            reference(PredicateValue::Record(vec![(sample.clone(), bits32(value)), (other.clone(), bits32(ONE))])),
        )]))
    };
    let decide = |clause: &ContractClause, bits| evaluate_ieee_clause(clause, &[input(bits)], None, None);
    assert_eq!((decide(clause, HALF), decide(clause, ONE)), (Some(true), Some(false)));
    for crossing in [1, 3] {
        let mut missing = clause.clone();
        missing.predicate.remove(crossing);
        assert_eq!(decide(&missing, HALF), None);
    }
}
