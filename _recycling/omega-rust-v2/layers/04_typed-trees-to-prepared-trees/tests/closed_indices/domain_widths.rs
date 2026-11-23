use super::*;

const WIDTHS: &str = "machine width()->u64{8u64} machine wide()->u64{16u64}";
const HEADS: &str = "domain u<width()>::Ready; domain u<wide()>::Ready;";

#[test]
fn a_sum_waits_for_every_case_before_publishing_its_shape() {
    use omega_preparation::{CheckedPreparationEvidence, PreparationAnswers, prepare_typing_pass_with_cause};
    use omega_typer::StaticValue;
    let code = "machine width()->u64{8u64} domain u<width()>::Ready;
        data Choice [copy] {case Empty; case Item(value:u8 in u8::Ready);}
        const VALUE:Choice=Choice::Empty;";
    let source = resolve(code);
    let first = type_source_package_for_preparation(&source, &TypingEnvironment::default()).unwrap();
    let TypingProgress::Pending(pending) = &first else { panic!("the payload needs its width") };
    let choice = pending.trees().constants.iter().find(|c| c.declaration.path == "VALUE").unwrap().initializer.carrier.clone();
    assert!(!pending.trees().static_data_shapes.contains_key(&choice));
    let pass = prepare_typing_pass_with_cause(&first, &[], None, CheckedPreparationEvidence::default()).unwrap();
    let PreparationAnswers::Pending(answers) = pass.answers else { panic!("only the width can execute") };
    let [(StaticValueDemand::Closed(request), value)] = answers.as_slice() else { panic!("one closed width") };
    assert_eq!(request.expression.span.slice(code.as_bytes()), Some(b"width()".as_slice()));
    assert_eq!(value, &StaticValue::Integer(8.into()));
    let evaluated = [(request.clone(), value.clone())];
    let environment = TypingEnvironment { evaluated_indices: &evaluated, ..Default::default() };
    let TypingProgress::Complete(trees) = type_source_package_for_preparation(&source, &environment).unwrap() else { panic!("both cases are ready") };
    let shape = &trees.static_data_shapes[&choice];
    let [(empty, no_fields), (item, fields)] = shape.cases.as_slice() else { panic!("the sum must include both cases") };
    assert_eq!(empty.path, "Choice::Empty");
    assert!(no_fields.is_empty());
    assert_eq!(item.path, "Choice::Item");
    let [value] = fields.as_slice() else { panic!("one Item field") };
    let [domain] = value.domains() else { panic!("the qualified field retains its domain") };
    assert_eq!(&trees.domain_carriers[&domain.declaration], value.carrier());
}

#[test]
fn aliases_publish_the_selected_domain_only_after_typing_receives_the_widths() {
    let code = format!("{WIDTHS} {HEADS}
        data Box<T>{{value:T;}}
        domain u8::Selected=u8::Ready;
        data Mark=Box<u8 in u8::Selected>;
        machine read(x:Mark)->u8 in u8::Ready{{x.value}}");
    let package = resolve(&code);
    let TypingProgress::Pending(first) = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap() else {
        panic!("the aliases depend on a computed width");
    };
    assert!(first.trees().type_alias_targets.keys().all(|id| id.path != "Mark"));
    let prepared = prepare_source_package(&package).unwrap();
    let trees = prepared.typed();
    let read = trees.signatures.iter().find(|s| s.machine.path == "read").unwrap();
    let (head, arguments) = read.parameters[0].1.nominal_parts().unwrap();
    assert_eq!(head.path, "Box");
    assert_eq!(arguments, std::slice::from_ref(&read.result));
    let [atom] = read.result.domains() else { panic!("expected the underlying Ready domain") };
    assert_eq!(trees.domain_carriers[&atom.declaration], ValueType::Integer { signed: false, width: Some(8) });
}

#[test]
fn equal_evaluated_widths_refuse_the_second_domain_head() {
    use omega_preparation::{PreparationFailure, prepare_source_package_diagnosed};
    let code = format!("{} {HEADS} machine read(x:u8 in u8::Ready){{}}", WIDTHS.replace("16u64", "8u64"));
    let package = resolve(&code);
    let Err(PreparationFailure::ResumedTyping(error)) = prepare_source_package_diagnosed(&package) else {
        panic!("typing must check the two declaration identities after evaluation");
    };
    assert_eq!(error.message, "duplicate domain u8::Ready");
    assert_eq!(error.span.slice(code.as_bytes()), Some(b"u<wide()>::Ready".as_slice()));
}

#[test]
fn a_computed_domain_mismatch_is_reported_by_typing_after_evaluation() {
    use omega_preparation::{PreparationFailure, prepare_source_package_diagnosed};

    let code = "machine width()->u64{16u64} domain u<width()>::Ready; machine read(x:u8 in u8::Ready){}";
    let package = resolve(code);
    let TypingProgress::Pending(pending) = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap() else {
        panic!("typing needs the declared domain's width");
    };
    let [demand] = pending.demands() else { panic!("only the domain width is missing") };
    assert_eq!(demand.request().expression.span.slice(code.as_bytes()), Some(b"width()".as_slice()));
    let Err(PreparationFailure::ResumedTyping(error)) = prepare_source_package_diagnosed(&package) else {
        panic!("the domain carrier differs only after the width is known");
    };
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "main.omg"));
    assert_eq!(error.span.slice(code.as_bytes()), Some(b"u8::Ready".as_slice()));
    assert_eq!(error.message, "no visible domain on the written carrier");
}

#[test]
fn computed_domain_carriers_select_by_the_evaluated_width() {
    let declarations = "machine width()->u64{8u64} machine wide()->u64{16u64}";
    let narrow = "domain u<width()>::Ready;";
    let wide = "domain u<wide()>::Ready;";
    let selected = |domains: &str, carrier: &str| {
        let code = format!("{declarations} {domains} machine read(x:{carrier} in {carrier}::Ready){{}}");
        let source = resolve(&code);
        let TypingProgress::Pending(pending) = type_source_package_for_preparation(&source, &TypingEnvironment::default()).unwrap() else {
            panic!("typing needs the widths before it can select a domain");
        };
        let mut occurrences: Vec<_> = pending.demands().iter().map(|demand| {
            demand.request().expression.span.slice(code.as_bytes()).unwrap()
        }).collect();
        occurrences.sort_unstable();
        let mut expected = Vec::new();
        if domains.contains(wide) { expected.push(b"wide()".as_slice()); }
        if domains.contains(narrow) { expected.push(b"width()".as_slice()); }
        assert_eq!(occurrences, expected);
        let prepared = prepare_source_package(&source).unwrap();
        let read = prepared.typed().signatures.iter().find(|s| s.machine.path == "read").unwrap();
        let [selected] = read.parameters[0].1.domains() else { panic!("one selected domain") };
        selected.declaration.clone()
    };
    let narrow_id = selected(narrow, "u8");
    let wide_id = selected(wide, "u16");
    assert_ne!(narrow_id, wide_id);
    for both in [format!("{narrow} {wide}"), format!("{wide} {narrow}")] {
        assert_eq!(selected(&both, "u8"), narrow_id);
        assert_eq!(selected(&both, "u16"), wide_id);
    }
}
