use super::*;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;

#[test]
fn a_repeated_typing_demand_stops_at_the_already_answered_expression() {
    let source = "machine width()->u64{3u64} machine f(x:[u8;width()]){}";
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let package = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &syntax,
        },
    }])
    .unwrap();
    let begin = || PreparationRound::begin(&package, &[], None).unwrap();
    let next = || {
        let round = begin();
        let answers = round.evaluate(CheckedPreparationEvidence::default()).unwrap();
        let PreparationStep::Evaluate(next) = round.advance(answers).unwrap() else {
            panic!("the extent must be returned to typing")
        };
        *next
    };

    let PreparationProgress::Complete(product) = next().finish_without_external_evidence().unwrap() else {
        panic!("the closed width must prepare without a target")
    };
    assert_eq!(product.typed().evaluated_indices[0].1, StaticValue::Integer(3.into()));

    // Replay L3's original demand after L4 has supplied the real answer.
    // This is the guard against a stalled producer at the resume boundary.
    let mut repeated = next();
    repeated.progress = begin().progress;
    let answers = repeated
        .evaluate(CheckedPreparationEvidence::default())
        .unwrap();
    let Err(PreparationAdvanceError::Refused(PreparationFailure::Preparation(refusal))) = repeated.advance(answers) else {
        panic!("a completed demand must not start a third evaluation")
    };
    let start = source.rfind("width()").unwrap();
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(PreparationReason::TypingDidNotAdvance),
        "app",
        "main.omg",
        (start, start + "width()".len()),
        "static evaluation cannot admit this invocation: typing did not advance after receiving the evaluation result"
    );
}
