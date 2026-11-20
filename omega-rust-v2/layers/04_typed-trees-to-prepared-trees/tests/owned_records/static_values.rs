use super::{declaration, package, prepare};
use omega_preparation::{
    CheckedPreparationEvidence, PreparationAnswers, PreparationFailure, PreparationReason, PreparationRefusal, PreparationRound, PreparedFloat, PreparedValue,
    RefusalCause, prepare_source_package_diagnosed, prepare_typing_pass_with_cause,
};
use omega_typed::{ObligationKind, TypedTrees, ValueType};
use omega_typer::TypingProgress;

#[path = "../support/typed_packages.rs"]
mod typed_packages;

fn range(
    source: &str,
    occurrence: &str,
) -> (usize, usize) {
    let start = source.rfind(occurrence).unwrap();
    (start, start + occurrence.len())
}

fn refused(
    source: &str,
    occurrence: &str,
    reason: PreparationReason,
    text: &str,
) {
    let failure = prepare_source_package_diagnosed(&package(source)).unwrap_err();
    assert!(matches!(failure, PreparationFailure::Preparation(_)), "{failure:?}");
    spec_corpus::assert_preparation_refusal!(
        failure,
        RefusalCause::Preparation(reason),
        "app",
        "main.omg",
        range(source, occurrence),
        format!("static evaluation cannot admit this invocation: {text}"),
    );
}

#[test]
fn named_integer_reads_do_not_defer_source_overflow_until_the_read() {
    let source = "machine next(n:u8)->u8 {n + 1} const BASE:u8=next(254); const READ:u8=BASE;";
    let prepared = prepare(source).unwrap();
    assert_eq!(prepared.integer_constant(declaration(&prepared, "READ")), Some(&255.into()));
    refused(
        &source.replace("next(254)", "next(255)"),
        "next(255)",
        PreparationReason::AdditionOutsideCarrier,
        "exact addition exceeds its selected integer carrier",
    );

    let literal = source.replace("next(254)", "256");
    let failure = prepare_source_package_diagnosed(&package(&literal)).unwrap_err();
    assert_eq!(failure.cause(), None);
    let PreparationFailure::Typing(error) = failure else {
        panic!("literal landing belongs to initial typing: {failure:?}")
    };
    assert_eq!(error.message, "expected u8, found 256");
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "main.omg"));
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), range(&literal, "256"));
}

fn evaluate(progress: &TypingProgress<'_>) -> Result<PreparationAnswers, PreparationRefusal> {
    Ok(prepare_typing_pass_with_cause(progress, &[], None, CheckedPreparationEvidence::default())?.answers)
}

#[test]
fn aliased_constant_records_cannot_freeze_another_initializers_value() {
    for source in [
        "const OTHER:u8=17u8; const BYTE:u8=42u8; const READ:u8=0u8 +% BYTE;",
        "const READ:u8=0u8 +% BYTE; const BYTE:u8=42u8; const OTHER:u8=17u8;",
    ] {
        let progress = TypingProgress::Complete(typed_packages::typed("", source));
        let PreparationAnswers::Frozen { constants, .. } = evaluate(&progress).unwrap() else {
            panic!("expected frozen constants")
        };
        let TypingProgress::Complete(trees) = progress else { unreachable!() };
        let values = ["OTHER", "BYTE", "READ"].map(|name| {
            let constant = trees.constants.iter().find(|c| c.declaration.path == name).unwrap();
            &constants[&constant.declaration]
        });
        spec_corpus::observe_handoff!(
            "named-constant-initializers/producer",
            assert_eq!(values, [17, 42, 42].map(|n| PreparedValue::Integer(n.into())).each_ref()),
        );

        // Change only an ID; both requests and their landings still agree.
        let resolved = trees.resolved().clone();
        let mut facts = trees.into_facts();
        let byte = facts
            .constants
            .iter()
            .find(|c| c.declaration.path == "BYTE")
            .unwrap()
            .declaration
            .clone();
        facts
            .constants
            .iter_mut()
            .find(|c| c.declaration.path == "OTHER")
            .unwrap()
            .declaration = byte;
        let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
        let refusal = evaluate(&changed).unwrap_err();
        spec_corpus::assert_preparation_cause!(
            refusal,
            RefusalCause::Preparation(PreparationReason::ConstantInitializerDeclarationMismatch),
            "client",
            "client.omg",
            range(source, "17u8"),
            "static evaluation cannot admit this invocation: the selected expression is not the initializer of this constant declaration",
        );
    }
}

#[test]
fn unique_keys_do_not_authorize_exchanged_initializers() {
    let source = "const A:u8=17u8; const B:u8=42u8;";
    let trees = typed_packages::typed("", source);
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    let [a, b] = facts.constants.as_mut_slice() else { panic!("expected A and B") };
    std::mem::swap(&mut a.initializer, &mut b.initializer);
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    let refusal = evaluate(&changed).unwrap_err();
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(PreparationReason::ConstantInitializerDeclarationMismatch),
        "client",
        "client.omg",
        range(source, "42u8"),
        "static evaluation cannot admit this invocation: the selected expression is not the initializer of this constant declaration",
    );
}

#[test]
fn a_second_record_for_the_actual_initializer_is_a_duplicate() {
    let source = "const BYTE:u8=42u8;";
    let trees = typed_packages::typed("", source);
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    facts.constants.push(facts.constants[0].clone());
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    let refusal = evaluate(&changed).unwrap_err();
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(PreparationReason::DuplicateConstantInitializer),
        "client",
        "client.omg",
        range(source, "42u8"),
        "static evaluation cannot admit this invocation: the constant declaration has more than one typed initializer record",
    );
}

#[test]
fn unrelated_late_record_does_not_mask_a_consumed_initializer_failure() {
    let source = "machine next(n:u8)->u8 {n + 1} const BAD:u8=next(255); const LATE:u8=0u8;";
    let trees = typed_packages::typed("", source);
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    facts
        .constants
        .iter_mut()
        .find(|c| c.declaration.path == "LATE")
        .unwrap()
        .initializer
        .node
        .file = usize::MAX;
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    spec_corpus::assert_preparation_cause!(
        evaluate(&changed).unwrap_err(),
        RefusalCause::Preparation(PreparationReason::AdditionOutsideCarrier),
        "client",
        "client.omg",
        range(source, "next(255)"),
        "static evaluation cannot admit this invocation: exact addition exceeds its selected integer carrier",
    );
}

#[test]
fn forged_site_is_not_evidence_of_a_duplicate_or_owner_mismatch() {
    let source = "const BYTE:u8=42u8; const OTHER:u8=17u8;";
    for duplicate in [true, false] {
        let trees = typed_packages::typed("", source);
        let resolved = trees.resolved().clone();
        let mut facts = trees.into_facts();
        let [byte, other] = facts.constants.as_slice() else {
            panic!("expected BYTE and OTHER")
        };
        let mut forged = byte.clone();
        forged.initializer.expression = other.initializer.expression.clone();
        if !duplicate {
            forged.declaration = other.declaration.clone();
        }
        facts.constants.push(forged);
        let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
        spec_corpus::assert_preparation_cause!(
            evaluate(&changed).unwrap_err(),
            RefusalCause::Preparation(PreparationReason::RequestSiteMismatch),
            "client",
            "client.omg",
            range(source, "17u8"),
            "static evaluation cannot admit this invocation: the evaluation request selects a different source occurrence",
        );
    }
}

#[test]
fn a_forward_read_rejects_its_conflicting_definition_before_an_unrelated_failure() {
    let source = "const READ:u8=0u8 +% BYTE; machine next(n:u8)->u8 {n + 1} const BAD:u8=next(255); const BYTE:u8=42u8;";
    let trees = typed_packages::typed("", source);
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    facts.constants.push(
        facts
            .constants
            .iter()
            .find(|c| c.declaration.path == "BYTE")
            .unwrap()
            .clone(),
    );
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    spec_corpus::assert_preparation_cause!(
        evaluate(&changed).unwrap_err(),
        RefusalCause::Preparation(PreparationReason::DuplicateConstantInitializer),
        "client",
        "client.omg",
        range(source, "42u8"),
        "static evaluation cannot admit this invocation: the constant declaration has more than one typed initializer record",
    );
}

#[test]
fn an_open_recipe_requires_the_same_unique_initializer_as_a_frozen_value() {
    let source = r"use omega\language\core; const READ:u64=WIDTH; const WIDTH:u64=core\size_of::<u16>();";
    let input = package(source);
    let progress = omega_typer::type_source_package_for_preparation(&input, &Default::default()).unwrap();
    let PreparationAnswers::Frozen {
        constants,
        open_target_constants,
    } = evaluate(&progress).unwrap()
    else {
        panic!("expected open constant recipes")
    };
    let TypingProgress::Complete(trees) = progress else {
        panic!("no pending indices")
    };
    for name in ["READ", "WIDTH"] {
        let id = &trees
            .constants
            .iter()
            .find(|c| c.declaration.path == name)
            .unwrap()
            .declaration;
        assert!(!constants.contains_key(id));
        assert_eq!(
            open_target_constants[id].span.slice(source.as_bytes()),
            Some(br"core\size_of::<u16>()".as_slice())
        );
    }

    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    facts.constants.push(
        facts
            .constants
            .iter()
            .find(|c| c.declaration.path == "WIDTH")
            .unwrap()
            .clone(),
    );
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    spec_corpus::assert_preparation_cause!(
        evaluate(&changed).unwrap_err(),
        RefusalCause::Preparation(PreparationReason::DuplicateConstantInitializer),
        "app",
        "main.omg",
        range(source, r"core\size_of::<u16>()"),
        "static evaluation cannot admit this invocation: the constant declaration has more than one typed initializer record",
    );
}

#[test]
fn zero_requests_do_not_bypass_complete_type_admission() {
    let source = "data Packet [copy] {slot:u8;} const VALUE:Packet=Packet{..};";
    let prepared = prepare(source).unwrap();
    let packet = declaration(&prepared, "Packet");
    assert_eq!(
        prepared.constant(declaration(&prepared, "VALUE")),
        Some(&PreparedValue::Record {
            owner: packet.clone(),
            fields: [PreparedValue::Integer(0.into())].into(),
        })
    );
    let borrowed = source.replace("slot:u8", "slot:@u8");
    let input = package(&borrowed);
    let round = PreparationRound::begin(&input, &[], None).unwrap();
    let TypingProgress::Complete(trees) = round.typing() else {
        panic!("expected a complete typed package")
    };
    assert!(trees.obligations.iter().any(|o| matches!(
        &o.kind, ObligationKind::ZeroField { field, ty: ValueType::Reference(..) }
            if field.path == "Packet::slot" && o.site.span.slice(borrowed.as_bytes()) == Some(b"Packet{..}")
    )));
    refused(
        &borrowed,
        "Packet{..}",
        PreparationReason::ConstantTypeOutsideFragment,
        "the complete constant type is outside the prepared value fragment",
    );
}

#[test]
fn an_omitted_array_needs_established_elements_only_if_nonempty() {
    let source = "data Part [copy] requires false {n:u8;} data Packet [copy]{parts:[Part;0];} const VALUE:Packet=Packet{..};";
    let prepared = prepare(source).unwrap();
    let Some(PreparedValue::Record { fields, .. }) = prepared.constant(declaration(&prepared, "VALUE")) else {
        panic!("expected Packet")
    };
    assert_eq!(fields.as_ref(), &[PreparedValue::Array([].into())]);
    refused(
        &source.replace("[Part;0]", "[Part;1]"),
        "Packet{..}",
        PreparationReason::OmittedFieldZeroNeedsProof,
        "the omitted field's zero value requires proof of the data predicate",
    );
}

#[test]
fn an_omitted_nominal_needs_a_complete_shape_before_producing_any_zero() {
    let source = "data Part [copy]{low:f32; high:f64;} data Packet [copy]{part:Part;} const VALUE:Packet=Packet{..};";
    let progress = TypingProgress::Complete(typed_packages::typed("", source));
    let PreparationAnswers::Frozen { constants, .. } = evaluate(&progress).unwrap() else {
        panic!("expected frozen constants")
    };
    let TypingProgress::Complete(trees) = progress else { unreachable!() };
    let id = &trees
        .constants
        .iter()
        .find(|c| c.declaration.path == "VALUE")
        .unwrap()
        .declaration;
    let PreparedValue::Record { fields, .. } = &constants[id] else {
        panic!("expected Packet")
    };
    let [PreparedValue::Record { fields, .. }] = fields.as_ref() else {
        panic!("expected omitted Part")
    };
    assert_eq!(fields[0].float(), Some(PreparedFloat::Binary32(0)));
    assert_eq!(fields[1].float(), Some(PreparedFloat::Binary64(0)));

    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    let part = facts
        .static_data_shapes
        .keys()
        .find(|ty| matches!(ty, ValueType::Nominal(owner) if owner.path == "Part"))
        .unwrap()
        .clone();
    facts.static_data_shapes.remove(&part).unwrap();
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts, 0));
    let refusal = evaluate(&changed).unwrap_err();
    spec_corpus::assert_preparation_cause!(
        refusal,
        RefusalCause::Preparation(PreparationReason::ConstantTypeWithoutFieldShape),
        "client",
        "client.omg",
        range(source, "Packet{..}"),
        "static evaluation cannot admit this invocation: the complete constant type has no finite typed field shape",
    );
}
