use super::*;

#[test]
fn a_machine_reference_waits_for_its_selected_signature() {
    let declarations = "machine width()->u64{8u64} machine wide()->u64{16u64}
        domain u<width()>::Ready; domain u<wide()>::Ready; data Erase<T> = u8;
        data F=machine(x:u8)->u8 reaches none crashes none;";
    for (body, site) in [
        ("machine grab(){let f:F=selected;}", "selected"),
        ("machine apply<const H:F>(x:u8)->u8{H(x)} machine grab()->u8{apply::<selected>(0u8)}", "apply::<selected>(0u8)"),
    ] {
        // Forming the type argument needs the widths even when the alias erases it.
        let valid = format!("{declarations} machine selected(x:Erase<u8 in u8::Ready>)->u8 reaches none crashes none{{0u8}} {body}");
        prepare(&valid).unwrap();
        let invalid = valid.replace("Erase<u8 in u8::Ready>", "u8 in u8::Ready");
        let package = resolve(&invalid);
        let failure = omega_preparation::prepare_source_package_diagnosed(&package).unwrap_err();
        let omega_preparation::PreparationFailure::ResumedTyping(error) = failure else { panic!("expected resumed typing") };
        assert_eq!(error.message, "selected machine has incompatible callable signature");
        assert_eq!(error.span.get_start_offset(), invalid.rfind(site).unwrap());
        assert_eq!(error.span.slice(invalid.as_bytes()), Some(site.as_bytes()));
    }
}

#[test]
fn a_width_call_can_rule_out_an_overload_before_its_domain_is_selected() {
    for (actual, parameter) in [("", ""), ("true", "flag:bool"), ("1u8", "n:u8")] {
        let source = format!(
            "machine width()->u64{{measure({actual})}} machine wide()->u64{{16u64}}
             domain u<width()>::Ready; domain u<wide()>::Ready;
             machine measure({parameter})->u64{{8u64}} machine measure(x:u8 in u8::Ready)->u64{{15u64}}
             machine read(x:u8 in u8::Ready){{}}"
        );
        let package = resolve(&source);
        let TypingProgress::Pending(pending) = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap() else {
            panic!("domain declarations need evaluated widths")
        };
        let selected = pending.trees().calls.iter().find(|call| call.machine.path == "measure").expect("width has a ready call");
        let signature = pending.trees().signatures.iter().find(|s| s.machine == selected.machine).unwrap();
        assert_eq!(signature.parameters.len(), usize::from(!parameter.is_empty()));
        assert!(signature.parameters.iter().all(|(_, ty)| ty.domains().is_empty()));
        let product = prepare_source_package(&package).unwrap();
        let read = product.typed().signatures.iter().find(|s| s.machine.path == "read").unwrap();
        assert_eq!(read.parameters[0].1.domains().len(), 1);
    }
}

#[test]
fn nested_width_calls_use_the_operand_context_shared_by_the_overloads() {
    for (arguments, ready, unfinished) in [
        ("companion(), true", "first:bool, second:bool", "first:bool, second:u8 in u8::Ready"),
        ("true, companion()", "first:bool, second:bool", "first:u8 in u8::Ready, second:bool"),
        ("companion()", "flag:bool", "value:u8 in u8::Ready"),
    ] {
        let source = format!(
            "machine companion()->bool{{true}} machine width()->u64{{measure({arguments})}} machine wide()->u64{{16u64}}
             domain u<width()>::Ready; domain u<wide()>::Ready;
             machine measure({ready})->u64{{8u64}} machine measure({unfinished})->u64{{16u64}}
             machine read(x:u8 in u8::Ready){{}}"
        );
        let product = prepare(&source).unwrap();
        let selected = product.typed().calls.iter().find(|call| call.machine.path == "measure").unwrap();
        let signature = product.typed().signatures.iter().find(|s| s.machine == selected.machine).unwrap();
        assert!(signature.parameters.iter().all(|(_, ty)| ty == &ValueType::Bool));
    }
}

#[test]
fn an_operand_with_a_known_wrong_type_reports_the_call_error_before_width_evaluation() {
    let source = "machine width()->u64{measure(true)} machine wide()->u64{16u64}
        domain u<width()>::Ready; domain u<wide()>::Ready; machine measure(x:u8 in u8::Ready)->u64{8u64}";
    let package = resolve(source);
    let failure = omega_preparation::prepare_source_package_diagnosed(&package).unwrap_err();
    let omega_preparation::PreparationFailure::Typing(error) = failure else { panic!("expected initial typing") };
    assert_eq!(error.message, "no matching overload for measure(bool)");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"measure".as_slice()));
    assert_eq!(error.span.get_start_offset(), source.find("measure").unwrap());
}

#[test]
fn a_width_call_that_needs_its_own_domain_reports_the_dependency_cycle() {
    let source = "machine width()->u64{measure()} domain u<width()>::Ready; machine measure()->u64{_ = 1u8 in u8::Ready; 8u64}";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.message, "static evaluation is blocked by a typing dependency cycle");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"width()".as_slice()));
}
