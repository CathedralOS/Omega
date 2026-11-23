#[path = "support/source_package.rs"]
mod source_package;

use omega_preparation::{PreparationFailure, PreparationReason, RefusalCause, prepare_source_package_diagnosed};
use omega_typer::{TypingProgress, ValueType};
use source_package::source_package;

#[test]
fn signed_fixture_freezes_the_index_or_refuses_the_overflowing_call() {
    let source = include_str!("fixtures/signed_arithmetic/signed_wrap.omg");
    let package = source_package([("signed_wrap.omg", source)]);
    let round = omega_preparation::PreparationRound::begin(&package, &[], None).unwrap();
    let TypingProgress::Pending(pending) = round.typing() else {
        panic!("carry needs the value of OFFSET");
    };
    let [omega_typed::StaticValueDemand::Closed(request)] = pending.demands() else {
        panic!("expected one closed index request, got {:?}", pending.demands());
    };
    let occurrence = source.find("Indexed<OFFSET>").unwrap() + "Indexed<".len();
    let signed_byte = ValueType::Integer { signed: true, width: Some(8) };
    assert_eq!(request.carrier, signed_byte);
    assert_eq!(request.expression.file, "signed_wrap.omg");
    assert_eq!(request.expression.span, omega_lexer::SourceSpan::new(occurrence, occurrence + "OFFSET".len()));

    let prepared = prepare_source_package_diagnosed(&package).unwrap();
    let typed = prepared.typed();
    let offset = typed.constants.iter().find(|c| c.declaration.path == "OFFSET").unwrap();
    assert_eq!(prepared.integer_constant(&offset.declaration), Some(&(-128).into()));

    let carry = typed.signatures.iter().find(|sig| sig.machine.path == "carry").unwrap();
    let indexed = typed.resolved().declarations.iter().find(|id| id.package == "app" && id.path == "Indexed").unwrap();
    let expected = ValueType::Applied(
        Box::new(indexed.clone()),
        vec![ValueType::StaticInteger {
            carrier: Box::new(signed_byte),
            value: (-128).into(),
        }],
    );
    assert_eq!(carry.parameters[0].1, expected);
    assert_eq!(carry.result, expected);

    let source = include_str!("fixtures/signed_arithmetic/signed_overflow.omg");
    let package = source_package([("signed_overflow.omg", source)]);
    let failure = prepare_source_package_diagnosed(&package).unwrap_err();
    let PreparationFailure::Preparation(refusal) = failure else {
        panic!("expected L4 to refuse the exact addition, received {failure:?}");
    };
    let call = "adjust(127)";
    let start = source.find(call).unwrap();
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(PreparationReason::AdditionOutsideCarrier),
        "app",
        "signed_overflow.omg",
        (start, start + call.len()),
        "static evaluation cannot admit this invocation: exact addition exceeds its selected integer carrier",
    );
}
