#[path = "support/source.rs"]
mod sources;

use omega_lexer::SourceSpan;
use omega_lowering::{PredicateOperator, PredicateStep, ScalarType, TypeShape};
use omega_preparation::prepare_source_package;
use omega_typer::ValueType;

#[test]
fn published_audible_reaches_lowering_and_stops_at_the_record_domain_head() {
    let example = spec_corpus::published_chapter("language/modules.md")
        .unwrap()
        .into_iter()
        .find(|e| e.fence_line_number == 227)
        .expect("published Audio::Audible example at language/modules.md:227");
    let text = spec_corpus::parser_examples::parser_source(&example.source_text);
    let files = [("published.omg", text.as_str())];
    let package = sources::resolve(&files);
    let prepared = prepare_source_package(&package).expect("the published example type-checks and prepares");
    let (_, carrier) = prepared
        .typed()
        .domain_carriers
        .iter()
        .find(|(id, _)| id.package == "app" && id.path == "Audio::Audible")
        .expect("the typed domain declaration");
    assert!(matches!(carrier, ValueType::Nominal(id) if id.package == "app" && id.path == "Audio"));

    // Select the declaration, not a use of the same qualified name elsewhere in the fence.
    let span = sources::span_within(&text, "domain Audio::Audible", "Audio::Audible");
    let head = SourceSpan::new(span.start, span.end);
    let error = sources::refuse_prepared(&prepared, &files, "published.omg", span, "domain carrier requires a fixed scalar");
    spec_corpus::observe_handoff!(
        psi "psi-output/published-audible-refusal",
        assert_eq!(
            (error.site.package.as_str(), error.site.file.as_str(), error.site.span, error.message.as_str()),
            ("app", "published.omg", head, "domain carrier requires a fixed scalar")
        ),
    );
}

#[test]
fn a_scalar_audible_domain_lowers_with_its_carrier_and_predicate() {
    let package = sources::lower(&[("scalar.omg", "domain u8::Audible requires self > 0;")]).unwrap();
    let [domain] = package.domains.as_slice() else {
        panic!("expected one scalar domain");
    };
    let [clause] = domain.requires.as_slice() else {
        panic!("expected its requirement");
    };
    assert_eq!(domain.declaration.path, "u8::Audible");
    let byte = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 });
    assert_eq!(package.types.get(domain.carrier), Some(&byte));
    spec_corpus::observe_handoff!(
        psi "psi-output/scalar-audible-predicate",
        assert_eq!(
            clause.predicate,
            [PredicateStep::SelfValue, PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)]
        ),
    );
}
