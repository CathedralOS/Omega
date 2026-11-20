use super::resolve;
use omega_preparation::{CheckedPreparationEvidence, PreparationAnswers, prepare_typing_pass_with_cause};
use omega_typer::{StaticValue, StaticValueDemand, TypingEnvironment, TypingProgress, ValueType, type_source_package_for_preparation};

const PROGRAM: &str = r"use omega\language\core;
        machine width()->u64 {8u64}
        machine wide()->u64 {16u64}
        domain u<width()>::Ready;
        domain u<wide()>::Ready;
        data Mark { prefix:u8; value:u8 in u8::Ready; }
        machine pick()->[u8 in u8::Ready;1] {[1u8 as +u8::Ready]}
        machine skipped()->bool {let v=pick(); true}
        machine enabled()->bool {true || skipped()}
        data Flag<const B:bool> [copy] {}
        machine consume(x:Flag<enabled()>, bytes:[u8;core\size_of::<Mark>()]){}";

#[test]
fn dependent_calls_and_record_layout_wait_for_the_selected_domain() {
    let code = PROGRAM;
    let package = resolve(code);
    let initial = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap();
    let TypingProgress::Pending(first) = &initial else { panic!("the carrier widths are needed") };
    let trees = first.trees();
    assert!(!trees.signatures.iter().any(|signature| signature.machine.path == "pick"));
    assert!(!trees.record_fields.keys().any(|owner| owner.path == "Mark"));
    assert!(!trees.calls.iter().any(|call| call.machine.path == "pick"));

    // The layout depends on the complete field list. The static call depends on
    // pick's signature even though evaluation will short-circuit before calling it.
    let first_pass = prepare_typing_pass_with_cause(&initial, &[], None, CheckedPreparationEvidence::default()).unwrap();
    let PreparationAnswers::Pending(answers) = first_pass.answers else { panic!("only the two carrier widths can run") };
    assert_eq!(answers.len(), 2);
    for (call, value) in [("width()", 8u64), ("wide()", 16)] {
        let start = code.find(&format!("u<{call}>")).unwrap() + 2;
        let demand = first.demands().iter().find(|demand| demand.request().expression.span.get_start_offset() == start).unwrap();
        assert_eq!(demand.request().expression.span.slice(code.as_bytes()), Some(call.as_bytes()));
        assert!(answers.contains(&(demand.clone(), StaticValue::Integer(value.into()))));
    }
    assert_eq!(first.demands().len(), answers.len());
    let evaluated: Vec<_> = answers.into_iter().map(|(demand, value)| {
        let StaticValueDemand::Closed(request) = demand else { panic!("the width calls are closed") };
        (request, value)
    }).collect();
    let environment = TypingEnvironment { evaluated_indices: &evaluated, ..Default::default() };
    let resumed = type_source_package_for_preparation(&package, &environment).unwrap();
    let TypingProgress::Pending(second) = &resumed else { panic!("the layout and enabled() still need answers") };
    let trees = second.trees();
    let (_, fields) = trees.record_fields.iter().find(|(owner, _)| owner.path == "Mark").unwrap();
    let [prefix, value] = fields.as_slice() else { panic!("both fields are required") };
    assert_eq!((prefix.name(), value.name()), ("prefix", "value"));
    let [domain] = value.ty.domains() else { panic!("the value field must retain its selected domain") };
    assert_eq!(trees.domain_carriers[&domain.declaration], ValueType::Integer { signed: false, width: Some(8) });
    let pick = trees.signatures.iter().find(|signature| signature.machine.path == "pick").unwrap();
    assert_eq!(pick.result, ValueType::Array(Box::new(value.ty.clone()), 1));
    assert!(trees.calls.iter().any(|call| call.machine == pick.machine && call.result == pick.result));

    let second_pass = prepare_typing_pass_with_cause(&resumed, &[], None, CheckedPreparationEvidence::default()).unwrap();
    let PreparationAnswers::OpenTarget { demands, answers } = second_pass.answers else { panic!("only the layout needs a target") };
    let [open] = demands.as_slice() else { panic!("one open layout request") };
    assert_eq!(open.demand.request().expression.span.slice(code.as_bytes()), Some(br"core\size_of::<Mark>()".as_slice()));
    let [(completed, answer)] = answers.as_slice() else { panic!("the independent call must complete") };
    assert_eq!(completed.request().expression.span.slice(code.as_bytes()), Some(b"enabled()".as_slice()));
    assert_eq!(answer, &StaticValue::Bool(true));
}

#[test]
fn a_width_depending_on_the_same_domain_selection_waits_while_the_other_width_runs() {
    let code = PROGRAM.replace("width()->u64 {8u64}", "width()->u64 {let v=enabled(); 8u64}");
    let package = resolve(&code);
    let initial = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap();
    let TypingProgress::Pending(first) = &initial else { panic!("wide() remains independent") };
    let [ready] = first.demands() else { panic!("only the independent width can run") };
    assert_eq!(ready.request().expression.span.slice(code.as_bytes()), Some(b"wide()".as_slice()));
    let pass = prepare_typing_pass_with_cause(&initial, &[], None, CheckedPreparationEvidence::default()).unwrap();
    let PreparationAnswers::Pending(answers) = pass.answers else { panic!("the layout depends on the width cycle") };
    assert_eq!(answers, [(ready.clone(), StaticValue::Integer(16u64.into()))]);
    let Err(omega_preparation::PreparationFailure::ResumedTyping(error)) = omega_preparation::prepare_source_package_diagnosed(&package) else {
        panic!("the remaining width needs its own selection to type");
    };
    let start = code.find("u<width()>").unwrap() + 2;
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "main.omg"));
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (start, start + "width()".len()));
    assert_eq!(error.message, "static evaluation is blocked by a typing dependency cycle");
}
