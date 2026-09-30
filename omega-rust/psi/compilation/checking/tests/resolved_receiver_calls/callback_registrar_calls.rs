//! A qualified registrar call (`Registrar::install<Provider::call>(1, 2)`)
//! selects a boundary requirement generic over `machine` binders. Its
//! `native callback ... from Handler` destination has no runtime argument:
//! the call binds it through the static machine argument for `Handler`,
//! while every runtime formal keeps its ordinary arity, type and result-use
//! obligations.

const REGISTRAR: &str = "
    boundary trait Procedure { machine call(message: u64) -> u64; }
    boundary trait Registrar {
        machine install<machine Handler>(
            kind: u64,
            native callback procedure from Handler,
            module: u64
        )
        where machine Handler satisfies Procedure::call;
        machine register<machine Handler>(
            kind: u64,
            native callback procedure from Handler
        ) -> u64
        where machine Handler satisfies Procedure::call;
        machine misbound<machine Handler>(
            kind: u64,
            native callback procedure from Missing
        )
        where machine Handler satisfies Procedure::call;
    }
    data Provider {}
    machine Provider::call(message: u64) -> u64 satisfies Procedure::call { message }
";

fn registrar_program(body: &str) -> String {
    format!("{REGISTRAR} machine main() reaches Registrar {{ {body} }}")
}

fn diagnostics(body: &str) -> Vec<String> {
    match validation::validate_program(&crate::front_end::typed_program(&registrar_program(body))) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect(),
    }
}

fn rejects(body: &str, fragment: &str) {
    let messages = diagnostics(body);
    assert!(
        messages.iter().any(|message| message.contains(fragment)),
        "{body}: expected {fragment:?}: {messages:#?}"
    );
}

#[test]
fn qualified_registrar_binds_its_callback_to_the_machine_argument() {
    for body in [
        "Registrar::install<Provider::call>(1u64, 2u64);",
        "_ = Registrar::register<Provider::call>(1u64);",
    ] {
        assert_eq!(diagnostics(body), Vec::<String>::new(), "{body}");
    }
}

#[test]
fn qualified_registrar_keeps_runtime_argument_and_result_obligations() {
    for (body, fragment) in [
        (
            "Registrar::install<Provider::call>(1u64);",
            "expects 2 argument(s), got 1",
        ),
        (
            "Registrar::install<Provider::call>(1u64, 2u64, 3u64);",
            "expects 2 argument(s), got 3",
        ),
        ("Registrar::install<Provider::call>(1u64, true);", "u64"),
        (
            "Registrar::register<Provider::call>(1u64);",
            "discards its non-unit",
        ),
    ] {
        rejects(body, fragment);
    }
}

#[test]
fn callback_destination_requires_its_binders_selection() {
    rejects(
        "Registrar::install(1u64, 2u64);",
        "supplies no static machine argument for native callback `procedure` from `Handler`",
    );
    rejects(
        "Registrar::misbound<Provider::call>(1u64);",
        "native callback `procedure` of `misbound` names `Missing`",
    );
}

#[test]
fn qualified_calls_with_type_binders_still_reject_without_substitution() {
    // Validation has no instantiation for a `Type` formal; the selected
    // declaration is refused rather than admitted unvalidated.
    let program = crate::front_end::typed_program(
        "boundary trait Sink { machine record<T>(value: T); }
         machine main() reaches Sink { Sink::record<u64>(1u64); }",
    );
    let diagnostics = validation::validate_program(&program).expect_err("type binder rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("qualified call `record` has no resolved callable declaration")),
        "{diagnostics:#?}"
    );
}
