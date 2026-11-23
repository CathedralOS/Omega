#[cfg(feature = "diagnostic-origins")]
#[path = "support/diagnostic_origin.rs"]
mod diagnostic_origin;

use omega_lexer::SourceSpan;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{StaticValue, StaticValueDemand, TypingEnvironment, TypingProgress, resolve_source_package, type_source_package_for_preparation};

#[test]
fn closed_record_answers_must_match_the_demanded_carrier() {
    const PACKAGE: &str = "app";
    const FILE: &str = "main.omg";
    // Review 39, good/main.omg. Both occurrences must agree for `carry` to return x.
    const SOURCE: &str = concat!(
        "data Key [copy]{count:u8; active:bool;}\n",
        "data Row<const K:Key>{}\n",
        "machine carry(x:Row<Key{count:7,active:true}>)->Row<Key{count:7,active:true}>{x}\n",
    );
    const LITERAL: &str = "Key{count:7,active:true}";
    let tokens = omega_lexer::lex(SOURCE.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(SOURCE.as_bytes(), &tokens).unwrap();
    let package = resolve_source_package(&[StageSource {
        package: PACKAGE,
        source: ParsedSource {
            logical_path: FILE,
            text: SOURCE,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    let resume = |answers: &[_]| {
        type_source_package_for_preparation(
            &package,
            &TypingEnvironment {
                evaluated_indices: answers,
                ..Default::default()
            },
        )
    };
    let TypingProgress::Pending(pass) = resume(&[]).unwrap() else {
        panic!("expected closed Key demands");
    };
    let key = package.declarations.iter().find(|id| id.path == "Key").unwrap();
    let spans: Vec<_> = SOURCE.match_indices(LITERAL).map(|(i, s)| SourceSpan::new(i, i + s.len())).collect();
    assert_eq!(spans.len(), 2);
    assert_eq!(pass.demands().len(), spans.len());
    let mut answers = Vec::new();
    for span in &spans {
        let request = pass
            .demands()
            .iter()
            .find_map(|d| match d {
                StaticValueDemand::Closed(r) if r.expression.span == *span => Some(r),
                _ => None,
            })
            .expect("expected a closed demand at each authored literal");
        assert_eq!(request.expression.package, PACKAGE);
        assert_eq!(request.expression.file, FILE);
        assert_eq!(request.carrier.nominal_parts(), Some((key, &[][..])));
        answers.push((
            request.clone(),
            StaticValue::Record {
                owner: key.clone(),
                fields: [StaticValue::Integer(7.into()), StaticValue::Bool(true)].into(),
            },
        ));
    }
    let TypingProgress::Complete(typed) = resume(&answers).unwrap() else {
        panic!("expected both closed answers to finish typing");
    };
    assert_eq!(typed.evaluated_indices.len(), answers.len());
    assert!(answers.iter().all(|answer| typed.evaluated_indices.contains(answer)));

    // Preserve both emitted keys and keep the two Row types equal. Only the
    // answer payload is invalid: a bool is not a Key. No specialized receipt.
    for (_, value) in &mut answers {
        *value = StaticValue::Bool(true);
    }
    let error = resume(&answers).unwrap_err();
    const REASON: &str = "evaluation result does not match its carrier";
    let first = spans[0];
    assert!(!error.waiting_for_static_value());
    assert_eq!(error.package, PACKAGE);
    assert_eq!(error.file, FILE);
    assert_eq!(error.span, first);
    assert_eq!(error.message, REASON);
    #[cfg(feature = "diagnostic-origins")]
    diagnostic_origin::report_return(&error, "", Some((PACKAGE, FILE, first.get_start_offset(), first.get_end_offset(), REASON)));
}
