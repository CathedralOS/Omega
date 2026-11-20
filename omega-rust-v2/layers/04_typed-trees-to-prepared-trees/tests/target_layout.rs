use std::sync::Arc;

use omega_preparation::{
    CheckedPreparationEvidence, LayoutExtent, LayoutGeometry, PreparationAnswers, PreparationFailure, PreparationProgress, PreparedTrees, RefusalCause, SelectedLayout, TargetIdentity,
    TargetObservation, TargetSemantics, TargetSemanticsError, prepare_source_package, prepare_source_package_diagnosed_with_inputs, prepare_source_package_for_target,
    prepare_source_package_with_open_target, prepare_typing_pass,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{LayoutQuery, SourcePackage, TypingEnvironment, ValueType, resolve_source_package, type_source_package_for_preparation};

const BYTE: ValueType = ValueType::Integer { signed: false, width: Some(8) };
const WORD: ValueType = ValueType::Integer {
    signed: false,
    width: Some(16),
};

fn resolve(text: &str) -> SourcePackage<'_> {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text,
            tokens: &tokens,
            tree: &syntax,
        },
    }])
    .unwrap()
}

fn identity(profile: &str) -> TargetIdentity {
    TargetIdentity {
        profile: profile.into(),
        semantics_version: 1,
        realization: Arc::from(b"test-abi-v1".as_slice()),
    }
}

fn layout(
    target: &TargetIdentity,
    subject: ValueType,
    extent: LayoutExtent,
    alignment: Option<u64>,
) -> SelectedLayout {
    SelectedLayout {
        target: target.clone(),
        subject,
        schema: Arc::from(b"normalized-test-schema".as_slice()),
        plan: Arc::from(b"checked-test-layout-plan".as_slice()),
        geometry: LayoutGeometry { extent, alignment },
    }
}

fn parameter<'prepared>(
    prepared: &'prepared PreparedTrees<'_>,
    machine: &str,
) -> &'prepared ValueType {
    &prepared
        .typed()
        .signatures
        .iter()
        .find(|signature| signature.machine.path == machine)
        .unwrap()
        .parameters[0]
        .1
}

#[test]
fn a_selected_layout_survives_constant_aliases_and_type_formation() {
    let code = "use omega\\language\\core;
                machine read_size()->u64 reaches none crashes none terminates { core\\size_of::<u16>() }
                const WIDTH:u64 = read_size();
                const ALIAS:u64 = WIDTH;
                machine consume(bytes:[u8; ALIAS]){}";
    let source = resolve(code);
    let target = identity("selected-target");
    let size = layout(&target, WORD, LayoutExtent::Fixed(8), Some(8));
    let capsule = TargetSemantics::new(target.clone(), [size.clone()], []).unwrap();
    let prepared = prepare_source_package_for_target(&source, &[], &capsule).unwrap();
    assert_eq!(parameter(&prepared, "consume"), &ValueType::Array(Box::new(BYTE), 8));
    let alias = prepared
        .typed()
        .constants
        .iter()
        .find(|constant| constant.declaration.path == "ALIAS")
        .unwrap();
    assert_eq!(prepared.integer_constant(&alias.declaration), Some(&8.into()));
    assert_eq!(
        prepared.target_observations(),
        &[TargetObservation::Layout {
            target: target.clone(),
            projection: LayoutQuery::Size,
            layout: Arc::new(size.clone())
        }]
    );

    let observation = &prepared.target_observations()[0];
    assert!(capsule.matches_observation(observation));
    let competing_layout = SelectedLayout {
        plan: Arc::from(b"another-checked-plan".as_slice()),
        ..size
    };
    let capsule = TargetSemantics::new(target, [competing_layout], []).unwrap();
    assert!(!capsule.matches_observation(observation));
    let competing = prepare_source_package_for_target(&source, &[], &capsule).unwrap();
    assert_eq!(parameter(&prepared, "consume"), parameter(&competing, "consume"));
    assert_ne!(prepared.target_observations(), competing.target_observations());
}

#[test]
fn a_dynamic_extent_can_have_alignment() {
    let code = "use omega\\language\\core; const VALUE:u64 = core\\align_of::<[u8]>();";
    let target = identity("layout-target");
    let subject = ValueType::Slice(Box::new(BYTE));
    let dynamic = layout(&target, subject, LayoutExtent::Dynamic, Some(1));
    let capsule = TargetSemantics::new(target.clone(), [dynamic.clone()], []).unwrap();
    let source = resolve(code);
    let prepared = prepare_source_package_for_target(&source, &[], &capsule).unwrap();
    assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&1.into()));
    assert_eq!(
        prepared.target_observations(),
        &[TargetObservation::Layout {
            target,
            projection: LayoutQuery::Alignment,
            layout: Arc::new(dynamic)
        }]
    );

    let code = code.replace("align_of", "size_of");
    let error = prepare_source_package_for_target(&resolve(&code), &[], &capsule).unwrap_err();
    assert!(error.message.contains("no fixed complete extent"), "{}", error.message);
    assert_eq!(error.span.slice(code.as_bytes()), Some(b"core\\size_of::<[u8]>()".as_slice()));
}

#[test]
fn target_and_plan_must_match_and_queries_do_not_guess() {
    let code = "use omega\\language\\core; const VALUE:u64 = core\\size_of::<u16>();";
    let source = resolve(code);
    let open = prepare_source_package(&source).unwrap();
    let constant = &open.typed().constants[0];
    assert!(open.constant(&constant.declaration).is_none());
    let error = open.open_target_constant(&constant.declaration).unwrap();
    assert!(error.message.contains("requires a selected target"), "{}", error.message);
    assert_eq!(error.span.slice(code.as_bytes()), Some(b"core\\size_of::<u16>()".as_slice()));

    let with_use = format!("{code} machine consume(bytes:[u8; VALUE]){{}}");
    let error = prepare_source_package(&resolve(&with_use)).unwrap_err();
    assert!(error.message.contains("requires a selected target"), "{}", error.message);

    let target = identity("selected");
    let capsule = TargetSemantics::new(target.clone(), [], []).unwrap();
    let error = prepare_source_package_for_target(&source, &[], &capsule).unwrap_err();
    assert!(error.message.contains("no validated layout"), "{}", error.message);

    let other_target = layout(&identity("different-target"), WORD, LayoutExtent::Fixed(2), Some(2));
    assert!(matches!(
        TargetSemantics::new(target.clone(), [other_target], []),
        Err(TargetSemanticsError::MismatchedLayoutTarget)
    ));
    let invalid_geometry = layout(&target, WORD, LayoutExtent::Fixed(2), Some(0));
    assert!(matches!(
        TargetSemantics::new(target, [invalid_geometry], []),
        Err(TargetSemanticsError::InvalidLayoutProjection)
    ));
}

#[test]
fn only_the_selected_core_declaration_is_a_layout_query() {
    let code = "machine size_of()->u64 reaches none crashes none terminates { 7 }
                const VALUE:u64 = size_of();";
    let source = resolve(code);
    let prepared = prepare_source_package(&source).unwrap();
    assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&7.into()));
    assert!(prepared.target_observations().is_empty());
}

#[test]
fn an_open_layout_recipe_closes_under_exact_type_arguments() {
    let open = "use omega\\language\\core; data Bytes<T> { bytes: [u8; core\\size_of::<T>()]; }";
    assert!(
        prepare_source_package(&resolve(open))
            .unwrap()
            .target_observations()
            .is_empty()
    );

    let code = format!("{open} machine consume(x:Bytes<u16>){{}}");
    let source = resolve(&code);
    let target = identity("selected");
    let size = layout(&target, WORD, LayoutExtent::Fixed(4), Some(4));
    let capsule = TargetSemantics::new(target, [size], []).unwrap();
    let prepared = prepare_source_package_for_target(&source, &[], &capsule).unwrap();
    let [answer] = prepared.specialized_indices() else {
        panic!("expected one closed layout application, got {:?}", prepared.specialized_indices());
    };
    assert_eq!(answer.arguments, [WORD]);
    assert_eq!(answer.value, omega_typer::StaticValue::Integer(4.into()));
    assert_eq!(prepared.target_observations().len(), 1);
}

#[test]
fn an_open_package_retains_exact_demands_and_closes_like_direct_preparation() {
    let code = "use omega\\language\\core;
                public const WIDTH: u64 = core\\size_of::<u16>();
                public const ALIAS: u64 = WIDTH;
                public data Packet { payload: [u8; ALIAS]; }
                machine portable()->u64 reaches none crashes none terminates { 2 }
                machine consume(bytes:[u8; portable()]){}";
    let source = resolve(code);
    let progress = type_source_package_for_preparation(&source, &TypingEnvironment::default()).unwrap();
    let pass = prepare_typing_pass(&progress, &[], None, CheckedPreparationEvidence::default()).unwrap();
    let PreparationAnswers::OpenTarget { demands, answers } = pass.answers else {
        panic!("expected a target demand and an independent answer");
    };
    let [answer] = answers.as_slice() else {
        panic!("expected one independent answer, got {answers:?}")
    };
    assert_eq!(answer.1, omega_typer::StaticValue::Integer(2.into()));
    assert_eq!(demands.len(), 1);

    let PreparationProgress::OpenTarget(open) = prepare_source_package_with_open_target(&source, &[]).unwrap() else {
        panic!("a type using the open constant still needs a target");
    };
    assert_eq!(open.source(), &source);
    assert_eq!(open.evaluated_indices().len(), 1);
    assert_eq!(open.evaluated_indices()[0].1, omega_typer::StaticValue::Integer(2.into()));
    let [demand] = open.demands() else {
        panic!("expected one open target demand")
    };
    assert_eq!(demand.demand.request().expression.span.slice(code.as_bytes()), Some(b"ALIAS".as_slice()));
    assert_eq!(demand.diagnostic.span.slice(code.as_bytes()), Some(b"core\\size_of::<u16>()".as_slice()));
    let pending = open.pending_typing();
    spec_corpus::observe_handoff!("open-target-and-selected-layout/producer", assert_eq!(pending.demands(), std::slice::from_ref(&demand.demand)));
    let width = pending
        .trees()
        .constants
        .iter()
        .find(|constant| constant.declaration.path == "WIDTH")
        .unwrap();
    let layout_call = pending
        .trees()
        .calls
        .iter()
        .find(|call| call.node == width.initializer.node)
        .unwrap();
    assert_eq!(layout_call.layout_query, Some(LayoutQuery::Size));
    assert_eq!(layout_call.type_arguments, [WORD]);
    let reads = open
        .dependencies()
        .inputs(&omega_preparation::PreparedValueOrigin::StaticIndex(demand.demand.clone()))
        .unwrap();
    assert_eq!(reads.constants[0].path, "ALIAS");

    let target = identity("selected");
    let capsule = TargetSemantics::new(target.clone(), [layout(&target, WORD, LayoutExtent::Fixed(4), Some(4))], []).unwrap();
    let direct = prepare_source_package_for_target(&source, &[], &capsule).unwrap();
    let resumed = open.close(&capsule).unwrap();
    assert_eq!(resumed, direct);
    assert_eq!(parameter(&resumed, "consume"), &ValueType::Array(Box::new(BYTE), 2));
}

#[test]
fn supplying_the_layout_later_preserves_the_refusing_phase_and_occurrence() {
    let code = r"use omega\language\core;
const WIDTH:u64=core\size_of::<u16>();
machine carry(bytes:[u8; WIDTH])->[u8; 4]{bytes}";
    let source = resolve(code);
    let target = identity("selected");
    for width in [None, Some(2), Some(4)] {
        let layouts = width.map(|size| layout(&target, WORD, LayoutExtent::Fixed(size), Some(2)));
        let capsule = TargetSemantics::new(target.clone(), layouts, []).unwrap();
        let PreparationProgress::OpenTarget(open) = prepare_source_package_with_open_target(&source, &[]).unwrap() else {
            panic!("the parameter type needs the target's layout");
        };
        let resumed = open.close(&capsule);
        let direct = prepare_source_package_diagnosed_with_inputs(&source, &[], Some(&capsule));
        assert_eq!(resumed, direct);
        let (failure, occurrence, message) = match width {
            Some(4) => {
                assert_eq!(parameter(&resumed.unwrap(), "carry"), &ValueType::Array(Box::new(BYTE), 4));
                continue;
            }
            Some(_) => {
                let failure = resumed.unwrap_err();
                assert!(matches!(failure, PreparationFailure::ResumedTyping(_)));
                assert_eq!(failure.cause(), None);
                (failure, "bytes", "expected [u8; 4], found [u8; 2]")
            }
            None => {
                let failure = resumed.unwrap_err();
                assert!(matches!(failure, PreparationFailure::Preparation(_)));
                assert_eq!(failure.cause(), Some(RefusalCause::TargetLayout("the selected target has no validated layout for the type")));
                (failure, r"core\size_of::<u16>()", "static evaluation cannot admit this invocation: the selected target has no validated layout for the type")
            }
        };
        let diagnostic = failure.diagnostic();
        let start = code.rfind(occurrence).unwrap();
        assert_eq!((diagnostic.package.as_str(), diagnostic.file.as_str()), ("app", "main.omg"));
        assert_eq!((diagnostic.span.get_start_offset(), diagnostic.span.get_end_offset()), (start, start + occurrence.len()));
        assert_eq!(diagnostic.message, message);
    }
}

#[test]
fn a_layout_cannot_supply_the_extent_that_defines_its_own_subject() {
    let code = "use omega\\language\\core;
                data Ring { bytes: [u8; core\\size_of::<Ring>()]; }";
    let source = resolve(code);
    let (_, declaration) = source
        .trees()
        .next()
        .unwrap()
        .declaration_nodes()
        .find(|(_, declaration)| declaration.path == "Ring")
        .unwrap();
    let target = identity("selected");
    let ring = layout(&target, ValueType::Nominal(Box::new(declaration.clone())), LayoutExtent::Fixed(4), Some(4));
    let capsule = TargetSemantics::new(target, [ring], []).unwrap();
    let error = prepare_source_package_for_target(&source, &[], &capsule).unwrap_err();
    assert!(error.message.contains("typing dependency cycle"), "{}", error.message);
}

#[test]
fn a_specialized_initializer_closes_the_call_before_entering_the_layout_helper() {
    let code = "use omega\\language\\core;
                machine byte_count<Left, Right>()->u64 {
                    core\\size_of::<Left>() *% 10 +% core\\size_of::<Right>()
                }
                data Outer<T> { data Inner<U> { payload: [u8; byte_count::<T, U>()]; } }
                machine consume(first:Outer<u16>::Inner<u8>, second:Outer<u8>::Inner<u16>){}";
    let source = resolve(code);
    let target = identity("selected");
    let capsule = TargetSemantics::new(
        target.clone(),
        [
            layout(&target, WORD, LayoutExtent::Fixed(4), Some(4)),
            layout(&target, BYTE, LayoutExtent::Fixed(1), Some(1)),
        ],
        [],
    )
    .unwrap();
    let prepared = prepare_source_package_for_target(&source, &[], &capsule).unwrap();
    let answers = prepared.specialized_indices();
    assert_eq!(answers.len(), 2);
    for (arguments, size) in [([WORD, BYTE], 41), ([BYTE, WORD], 14)] {
        let answer = answers
            .iter()
            .find(|answer| answer.arguments == arguments)
            .unwrap();
        spec_corpus::observe_handoff!("collection-layout-and-indirect-call-selections/consumer", assert_eq!(answer.value, omega_typer::StaticValue::Integer(size.into())));
    }
    // `Inner<U>` captures `T` from `Outer<T>`. Each frozen size answers the
    // nested field under the captured owner argument first and its own second.
    spec_corpus::observe_handoff!(
        "captured-owner-applications/consumer",
        assert!([([WORD, BYTE], 41), ([BYTE, WORD], 14)].iter().all(|(arguments, size)| {
            answers
                .iter()
                .filter(|answer| answer.arguments == *arguments && answer.value == omega_typer::StaticValue::Integer((*size).into()))
                .count()
                == 1
        }))
    );
    assert_eq!(prepared.target_observations().len(), 2);
}

#[test]
fn a_typed_layout_query_requires_one_closed_subject() {
    use omega_preparation::{PreparationReason, PreparedValue, prepare_typing_pass_with_cause};
    use omega_typed::TypedTrees;
    use omega_typer::TypingProgress;

    let code = r"use omega\language\core;
const ANSWER:u64 = core\size_of::<u16>();
machine probe<T>()->u64 { core\size_of::<T>() }";
    let occurrence = r"core\size_of::<u16>()";
    let start = code.find(occurrence).unwrap();
    let source = resolve(code);
    let target = identity("selected");
    let capsule = TargetSemantics::new(target.clone(), [layout(&target, WORD, LayoutExtent::Fixed(2), Some(2))], []).unwrap();

    for expected in [
        None,
        Some((
            PreparationReason::NotOneLayoutQuery,
            "static evaluation cannot admit this invocation: the request does not select one core layout query",
        )),
        Some((
            PreparationReason::LayoutQueryNeedsClosedType,
            "static evaluation cannot admit this invocation: the layout query needs a closed type application",
        )),
    ] {
        let TypingProgress::Complete(trees) = type_source_package_for_preparation(&source, &TypingEnvironment::default()).unwrap() else {
            panic!("expected complete typing");
        };
        let [constant] = trees.constants.as_slice() else {
            panic!("expected one constant");
        };
        let answer = constant.declaration.clone();
        let root = constant.initializer.node;
        let resolved = trees.resolved().clone();
        let mut facts = trees.into_facts();
        let donor = facts
            .calls
            .iter()
            .find(|call| call.node != root && call.layout_query == Some(LayoutQuery::Size))
            .expect("typed query in probe");
        let [open] = donor.type_arguments.as_slice() else {
            panic!("expected one donor subject");
        };
        assert!(open.contains_parameter());
        let open = open.clone();

        let mut calls = facts.calls.iter_mut().filter(|call| call.node == root);
        let call = calls.next().unwrap();
        assert!(calls.next().is_none());
        assert_eq!(call.layout_query, Some(LayoutQuery::Size));
        assert_eq!(call.type_arguments, [WORD]);
        match expected {
            None => {}
            Some((PreparationReason::NotOneLayoutQuery, _)) => call.type_arguments.clear(),
            Some(_) => call.type_arguments[0] = open,
        }

        // Library obligations retain node IDs in the library forest.
        let first_local = facts
            .obligations
            .iter()
            .position(|obligation| obligation.site.package == "app")
            .unwrap_or(facts.obligations.len());
        let progress = TypingProgress::Complete(TypedTrees::new(resolved, facts, first_local));
        let result = prepare_typing_pass_with_cause(&progress, &[], Some(&capsule), CheckedPreparationEvidence::default());
        if let Some((reason, message)) = expected {
            let Err(error) = result else { panic!("expected preparation refusal") };
            spec_corpus::assert_preparation_cause!(
                error,
                RefusalCause::Preparation(reason),
                "app",
                "main.omg",
                (start, start + occurrence.len()),
                message,
            );
        } else {
            let PreparationAnswers::Frozen { constants, .. } = result.unwrap().answers else {
                panic!("expected frozen constants");
            };
            assert_eq!(constants[&answer], PreparedValue::Integer(2.into()));
        }
    }
}
