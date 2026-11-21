#[path = "../support/typed_packages.rs"]
mod typed_packages;

use omega_preparation::{
    CheckedPreparationEvidence, PreparationAnswers, PreparationFailure, PreparationReason, PreparedValue, RefusalCause, prepare_source_package_diagnosed,
    prepare_typing_pass_with_cause,
};
use omega_typed::{ObligationKind, TypedTrees, ValueType};
use omega_typer::TypingProgress;

#[test]
fn unbounded_bitwise_source_is_refused_during_initial_typing() {
    let source = "machine combine(n:u,m:u)->u {n & m} const ANSWER:u=1 + combine(40,1);";
    let package = super::source_package([("numbers.omg", source)]);
    let failure = prepare_source_package_diagnosed(&package).unwrap_err();
    assert_eq!(failure.cause(), None);
    let PreparationFailure::Typing(error) = failure else {
        panic!("expected an L3 refusal: {failure:?}")
    };
    let start = source.find("n & m").unwrap();
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "numbers.omg"));
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (start, start + "n & m".len()));
    assert_eq!(error.message, "no matching operator & for u and u");
}

#[test]
fn an_unbounded_integer_has_no_selected_bitwise_execution_rule() {
    let library = "public machine combine(n:u,m:u)->u reaches none crashes none terminates {n + m}";
    let client = "const ANSWER:u=1 + geo\\combine(40,1);";
    let trees = typed_packages::typed(library, client);
    let answer = trees
        .constants
        .iter()
        .find(|c| c.declaration.path == "ANSWER")
        .unwrap()
        .declaration
        .clone();
    let control = TypingProgress::Complete(trees);
    let evaluate = |input: &TypingProgress<'_>| prepare_typing_pass_with_cause(input, &[], None, CheckedPreparationEvidence::default());
    let PreparationAnswers::Frozen { constants, .. } = evaluate(&control).unwrap().answers else {
        panic!("expected frozen constant")
    };
    assert_eq!(constants[&answer], PreparedValue::Integer(42u8.into()));

    let TypingProgress::Complete(trees) = control else { unreachable!() };
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    let operation = facts
        .operations
        .iter_mut()
        .find(|op| op.site.file == "geometry.omg")
        .unwrap();
    let carrier = ValueType::Integer { signed: false, width: None };
    assert_eq!(operation.token, "+");
    assert_eq!(operation.result, carrier);
    assert_eq!(operation.operands, [carrier.clone(), carrier]);
    // Change just the selected primitive. L3 publishes its token on both
    // records; leaving the contract unchanged would test a different refusal.
    let contract = facts
        .obligations
        .iter_mut()
        .find_map(|o| match &mut o.kind {
            ObligationKind::OperationContract { token, operands } if o.node == operation.node && operands == &operation.operands => Some(token),
            _ => None,
        })
        .unwrap();
    assert_eq!(contract, &operation.token);
    *contract = "&".into();
    operation.token = "&".into();

    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    let refusal = evaluate(&changed).unwrap_err();
    let call = "geo\\combine(40,1)";
    let start = client.find(call).unwrap();
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(PreparationReason::IntegerPrimitiveWithoutRule),
        "client",
        "client.omg",
        (start, start + call.len()),
        "static evaluation cannot admit this invocation: the selected integer primitive has no execution rule",
    );
}

#[test]
fn a_forward_constant_read_only_observes_an_in_range_initializer() {
    let good = "const COPY:u8=ANSWER; const BASE:u8=253u8; const ANSWER:u8=1u8 + (BASE + 1u8);";
    let package = super::source_package([("numbers.omg", good)]);
    let product = prepare_source_package_diagnosed(&package).unwrap();
    super::assert_integer_constant(&product, "ANSWER", 255);
    super::assert_integer_constant(&product, "COPY", 255);

    for (base, occurrence) in [("254u8", "1u8 + (BASE + 1u8)"), ("255u8", "BASE + 1u8")] {
        let bad = good.replace("253u8", base);
        let package = super::source_package([("numbers.omg", &bad)]);
        let failure = prepare_source_package_diagnosed(&package).unwrap_err();
        let start = bad.find(occurrence).unwrap();
        assert!(matches!(failure, PreparationFailure::Preparation(_)));
        spec_corpus::assert_preparation_refusal!(
            failure,
            RefusalCause::Preparation(PreparationReason::AdditionOutsideCarrier),
            "app",
            "numbers.omg",
            (start, start + occurrence.len()),
            "static evaluation cannot admit this invocation: exact addition exceeds its selected integer carrier",
        );
    }
}

#[test]
fn initializer_overflow_in_a_call_operand_precedes_entry_to_the_machine() {
    let source = "const BASE:u8=255u8; machine keep(n:u8)->u8 {n} const X:u8=keep(BASE + 1u8);";
    let package = super::source_package([("operand.omg", source)]);
    let failure = prepare_source_package_diagnosed(&package).unwrap_err();
    let operand = "BASE + 1u8";
    let start = source.find(operand).unwrap();
    spec_corpus::assert_preparation_refusal!(
        failure,
        RefusalCause::Preparation(PreparationReason::AdditionOutsideCarrier),
        "app",
        "operand.omg",
        (start, start + operand.len()),
        "static evaluation cannot admit this invocation: exact addition exceeds its selected integer carrier",
    );
}
