//! A typing refusal names whether L4 had already evaluated a demanded identity.
//! Initial typing, typing resumed with evaluated answers, and static evaluation
//! are distinct producer phases with the same source diagnostics. A demand that
//! needs an unselected target is a fourth phase: the open package succeeds, while
//! the diagnosed entry point reports missing target input at the layout query.
use std::sync::Arc;

use omega_preparation::{
    CheckedPreparationEvidence, PreparationAdvanceError, PreparationFailure, PreparationProgress, PreparationRound, PreparationStep, TargetIdentity,
    TargetSemantics, prepare_source_package, prepare_source_package_diagnosed, prepare_source_package_for_target, prepare_source_package_with_open_target,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_typed::TypeError;
use omega_typer::{SourcePackage, resolve_source_package};

const DIVIDE: &str = "machine divide(n:u64,d:u64)->u64 requires d != 0 reaches none crashes none terminates {n / d}\nconst SIZE:u64=divide(12,3);\nmachine carry(x:[u8; SIZE])->[u8; 4]{x}";
// The constant alone is retained as an open recipe. `EXTENT_USE` makes a type depend on it.
const LAYOUT: &str = r"use omega\language\core; const VALUE:u64=core\size_of::<u16>();";
const EXTENT_USE: &str = " machine consume(bytes:[u8; VALUE]){}";
const LAYOUT_QUERY: &str = r"core\size_of::<u16>()";
const NO_TARGET: &str = "static evaluation cannot admit this invocation: the layout query requires a selected target";

fn with_package<R>(
    source: &str,
    inspect: impl FnOnce(&SourcePackage<'_>) -> R,
) -> R {
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
    inspect(&package)
}

fn diagnose(source: &str) -> Result<(), PreparationFailure> {
    with_package(source, diagnose_package)
}

fn diagnose_package(package: &SourcePackage<'_>) -> Result<(), PreparationFailure> {
    let diagnosed = prepare_source_package_diagnosed(package).map(drop);
    // The legacy entry point reports the same diagnostic without the phase.
    let legacy = prepare_source_package(package).map(drop);
    match (&diagnosed, legacy) {
        (Ok(()), Ok(())) => {}
        (Err(failure), Err(error)) => assert_eq!(failure.diagnostic(), &error, "legacy conversion changed the diagnostic"),
        (diagnosed, legacy) => panic!("diagnosed {diagnosed:?} and legacy {legacy:?} disagree"),
    }
    let mut round = match PreparationRound::begin(package, &[], None) {
        Ok(round) => round,
        Err(error) => {
            assert_eq!(diagnosed.as_ref().unwrap_err(), &error);
            return diagnosed;
        }
    };
    loop {
        let answers = match round.evaluate(CheckedPreparationEvidence::default()) {
            Ok(answers) => answers,
            Err(error) => {
                assert!(matches!(&diagnosed, Err(PreparationFailure::Preparation(e)) if e.diagnostic == error), "round evaluation: {error:?}");
                return diagnosed;
            }
        };
        match round.advance(answers) {
            Ok(PreparationStep::Evaluate(next)) => round = *next,
            Ok(PreparationStep::Finished(PreparationProgress::Complete(_))) => {
                assert!(diagnosed.is_ok(), "the round prepared a refused package");
                return diagnosed;
            }
            // Without a target the round stops open. The diagnosed entry point reports its first demand.
            Ok(PreparationStep::Finished(PreparationProgress::OpenTarget(open))) => {
                let first = &open
                    .demands()
                    .first()
                    .expect("an open round has a target demand")
                    .diagnostic;
                assert!(matches!(&diagnosed, Err(PreparationFailure::MissingTarget(e)) if &e.diagnostic == first), "open round: {first:?}");
                return diagnosed;
            }
            Err(PreparationAdvanceError::Refused(error)) => {
                assert_eq!(diagnosed.as_ref().unwrap_err(), &error);
                return diagnosed;
            }
            Err(other) => panic!("unexpected advance failure {other:?}"),
        }
    }
}

fn assert_at(
    error: &TypeError,
    source: &str,
    occurrence: &str,
    reason: &str,
) {
    let start = source.rfind(occurrence).unwrap();
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "main.omg"));
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (start, start + occurrence.len()));
    assert_eq!(error.message, reason);
}

#[test]
fn positive_twin_prepares() {
    diagnose(DIVIDE).unwrap();
}

#[test]
fn entry_typing_refusal_is_initial_typing() {
    let source = "machine f()->u32 { true }";
    spec_corpus::observe_handoff!("preparation-refusal/initial-typing", match diagnose(source) {
        Err(PreparationFailure::Typing(error)) => assert_at(&error, source, "true", "expected u32, found bool"),
        other => panic!("expected initial typing refusal, found {other:?}"),
    });
}

#[test]
fn retype_after_evaluated_identity_is_resumed_typing() {
    let source = DIVIDE.replace("12,3", "12,2");
    spec_corpus::observe_handoff!("preparation-refusal/resumed-typing", match diagnose(&source) {
        Err(PreparationFailure::ResumedTyping(error)) => assert_at(&error, &source, "x", "expected [u8; 4], found [u8; 6]"),
        other => panic!("expected resumed typing refusal, found {other:?}"),
    });
}

#[test]
fn failed_static_evaluation_is_preparation() {
    let source = DIVIDE.replace("12,3", "12,0");
    spec_corpus::observe_handoff!("preparation-refusal/evaluation", match diagnose(&source) {
        Err(PreparationFailure::Preparation(error)) => {
            assert_at(&error.diagnostic, &source, "divide(12,0)", "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments")
        }
        other => panic!("expected static evaluation refusal, found {other:?}"),
    });
}

#[test]
fn freezing_after_a_successful_retype_is_still_preparation() {
    use omega_preparation::{PreparationReason, RefusalCause};
    use omega_typer::TypingProgress;

    let healthy = format!("{DIVIDE}\nconst EXTRA:u64=divide(9,1);");
    with_package(&healthy, |package| {
        let product = prepare_source_package_diagnosed(package).unwrap();
        for (name, value) in [("SIZE", 4), ("EXTRA", 9)] {
            let constant = product.typed().constants.iter().find(|c| c.declaration.path == name).unwrap();
            assert_eq!(product.integer_constant(&constant.declaration), Some(&value.into()));
        }
    });

    // EXTRA is never demanded by a type. Both variants can evaluate SIZE; only
    // if L3 accepts its answer may L4 go on to freeze the bad initializer.
    let bad_freeze = healthy.replace("9,1", "9,0");
    let bad_retype = bad_freeze.replace("12,3", "12,2");
    for (source, width_matches) in [(&bad_freeze, true), (&bad_retype, false)] {
        with_package(source, |package| {
            let direct = prepare_source_package_diagnosed(package).unwrap_err();
            let round = PreparationRound::begin(package, &[], None).unwrap();
            assert!(matches!(round.typing(), TypingProgress::Pending(_)));
            let answers = round.evaluate_with_cause(CheckedPreparationEvidence::default()).unwrap();
            match (width_matches, round.advance(answers)) {
                (true, Ok(PreparationStep::Evaluate(next))) => {
                    assert!(matches!(next.typing(), TypingProgress::Complete(_)));
                    let Err(refusal) = next.evaluate_with_cause(CheckedPreparationEvidence::default()) else {
                        panic!("freezing EXTRA must reject its divisor");
                    };
                    assert_eq!(direct, PreparationFailure::Preparation(refusal));
                    let call = "divide(9,0)";
                    let start = source.find(call).unwrap();
                    spec_corpus::assert_preparation_refusal!(
                        direct, RefusalCause::Preparation(PreparationReason::IntegerPreconditionFalse), "app", "main.omg",
                        (start, start + call.len()),
                        "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments"
                    );
                }
                (false, Err(PreparationAdvanceError::Refused(PreparationFailure::ResumedTyping(error)))) => {
                    assert_at(&error, source, "x", "expected [u8; 4], found [u8; 6]");
                    assert_eq!(direct, PreparationFailure::ResumedTyping(error));
                    assert_eq!(direct.cause(), None);
                }
                _ => panic!("expected {}", if width_matches { "final freezing pass" } else { "resumed L3 refusal" }),
            }
        });
    }
}

#[test]
fn an_extent_without_a_selected_target_is_missing_target_input() {
    let source = format!("{LAYOUT}{EXTENT_USE}");
    spec_corpus::observe_handoff!("preparation-refusal/missing-target", match diagnose(&source) {
        // The diagnostic stays at the layout query, not at the `VALUE` read in the array type.
        Err(PreparationFailure::MissingTarget(error)) => assert_at(&error.diagnostic, &source, LAYOUT_QUERY, NO_TARGET),
        other => panic!("expected missing target input, found {other:?}"),
    });
}

#[test]
fn an_open_target_recipe_prepares_and_the_open_entry_point_retains_its_demand() {
    // No type depends on the constant, so the complete package keeps its open initializer.
    spec_corpus::observe_handoff!("preparation-refusal/open-target-retained", with_package(LAYOUT, |package| {
        let prepared = prepare_source_package_diagnosed(package).unwrap();
        let constant = &prepared.typed().constants[0];
        assert!(prepared.constant(&constant.declaration).is_none());
        let retained = prepared
            .open_target_constant(&constant.declaration)
            .expect("the open initializer is retained");
        assert_at(retained, LAYOUT, LAYOUT_QUERY, NO_TARGET);
    }));
    // The open-target entry point accepts the type use and keeps the same diagnostic for its demand.
    let source = format!("{LAYOUT}{EXTENT_USE}");
    with_package(&source, |package| match prepare_source_package_with_open_target(package, &[]) {
        Ok(PreparationProgress::OpenTarget(open)) => {
            let [demand] = open.demands() else {
                panic!("expected one open target demand, found {:?}", open.demands())
            };
            assert_at(&demand.diagnostic, &source, LAYOUT_QUERY, NO_TARGET);
            let request = &demand.demand.request().expression;
            let start = source.rfind("VALUE").unwrap();
            assert_eq!((request.span.get_start_offset(), request.span.get_end_offset()), (start, start + "VALUE".len()));
        }
        other => panic!("expected an open target package, found {other:?}"),
    });
}

#[test]
fn a_selected_target_without_the_layout_refuses_evaluation_at_the_query() {
    let source = format!("{LAYOUT}{EXTENT_USE}");
    let identity = TargetIdentity {
        profile: "selected".into(),
        semantics_version: 1,
        realization: Arc::from(b"test-abi-v1".as_slice()),
    };
    let target = TargetSemantics::new(identity, [], []).unwrap();
    spec_corpus::observe_handoff!("preparation-refusal/selected-target-evaluation", with_package(&source, |package| {
        let round = PreparationRound::begin(package, &[], Some(&target)).unwrap();
        let error = match round.evaluate(CheckedPreparationEvidence::default()) {
            Err(error) => error,
            Ok(_) => panic!("a target without the queried layout cannot answer the demand"),
        };
        assert_at(
            &error,
            &source,
            LAYOUT_QUERY,
            "static evaluation cannot admit this invocation: the selected target has no validated layout for the type",
        );
        assert_eq!(prepare_source_package_for_target(package, &[], &target).unwrap_err(), error);
    }));
}

#[test]
fn resumed_typing_uses_the_selected_closed_record_field_for_the_parameter_extent() {
    let program = |wide, result| format!(
        "data Part [copy]{{n:u8; wide:u64;}} machine width<const P:Part>()->u64 {{P.wide}}
         machine read(x:[u8;width::<Part{{wide:{wide}u64,n:42u8}}>()])->[u8;{result}] {{x}}"
    );
    let signatures = [3, 5].map(|wide| {
        let source = program(wide, wide);
        with_package(&source, |package| {
            let prepared = prepare_source_package_diagnosed(package).unwrap();
            let read = prepared.typed().signatures.iter().find(|sig| sig.machine.path == "read").unwrap();
            let [(_, parameter)] = read.parameters.as_slice() else { panic!("expected the read parameter") };
            (parameter.to_string(), read.result.to_string())
        })
    });
    spec_corpus::observe_handoff!(
        "closed-record-field-in-resumed-typing/consumer",
        assert_eq!(signatures, [("[u8; 3]".to_owned(), "[u8; 3]".to_owned()), ("[u8; 5]".to_owned(), "[u8; 5]".to_owned())]),
    );

    for wide in [3, 5] {
        let source = program(wide, 9);
        let Err(PreparationFailure::ResumedTyping(error)) = diagnose(&source) else { panic!("expected the resumed return-type check") };
        assert_at(&error, &source, "x", &format!("expected [u8; 9], found [u8; {wide}]"));
    }
}
