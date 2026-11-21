//! IEEE comparisons in contracts and domain predicates are typed propositions,
//! not executable operations.
//! [Comparisons](../../../../wiki/spec/language/numeric_values.md#meaning-comparisons-and-representation)
//! · [IEEE comparison kinds](../../../../wiki/spec/terminal-psi/encoding.md#ieee-comparison-kinds)
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{
    CallTarget, ContractClause, FloatOperation, FloatSemantics, IeeeComparisonKind, IeeeFloatBits, IeeeFloatFormat, LoweredPackage, OperationKind,
    PredicateOperator, PredicateStep, ScalarLiteral,
};
use psi_handoff_consumer::{PredicateValue, evaluate_ieee_clause};

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
    sources::refuse(
        &[("case.omg", user)],
        "case.omg",
        sources::span_within(user, "requires a < b", "a < b"),
        "unsupported machine call `before` in a Psi contract predicate",
    );
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

/// The parts this repair does not supply still refuse. A qualified formal is
/// owned by its domain carrier. IEEE arithmetic has no predicate term. An
/// anonymous rational comparison is exact arithmetic, not an IEEE comparison.
#[test]
fn qualified_formals_float_arithmetic_and_rational_comparisons_still_refuse() {
    for (source, at, message) in [
        (
            "domain f32::Small; machine l(a:f32 in Small)->f32 requires (a as -Small) < 1.0 { a as -Small }",
            "a as -Small) <",
            "contract binding is not a supported formal of this machine",
        ),
        (
            "machine l(a:f32)->f32 requires a + 1.0 < 2.0 { a }",
            "a + 1.0",
            "float arithmetic in a contract predicate has no Psi term",
        ),
        (
            "machine l(a:f32)->f32 requires -a < 1.0 { a }",
            "-a",
            "float arithmetic in a contract predicate has no Psi term",
        ),
        (
            "machine l(a:f32)->f32 requires 1.0 < 0.5 { a }",
            "1.0 < 0.5",
            "unsupported contract predicate operator",
        ),
    ] {
        let start = source.find(at).unwrap();
        let span = if at.starts_with("a as") { start..start + 1 } else { start..start + at.len() };
        sources::refuse(&[("case.omg", source)], "case.omg", span, message);
    }
}
