use validation::validate_program;

fn validate(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let typed = crate::front_end::typed_program_result(source)?;
    validate_program(&typed)
}

#[test]
fn accepts_one_exact_trait_requirement_identity() {
    validate(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        boundary trait WindowProcedure {
            machine call(value: i32) -> i32;
        }
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call> {}
        "#,
    )
    .expect("exact trait requirement identity should validate");
}

#[test]
fn accepts_a_free_machine_identity_without_imposing_a_callable_shape() {
    validate(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        machine callback_without_a_shared_schema(value: u64, tag: i32) {}
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<callback_without_a_shared_schema> {}
        "#,
    )
    .expect("a declaration identity slot must not invent a callable contract");
}

#[test]
fn rejects_a_runtime_type_in_a_machine_identity_slot() {
    let diagnostics = validate(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<WndClassLayout> {}
        "#,
    )
    .expect_err("a data type is not a machine declaration identity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("expected one exact")
            && diagnostic.message.contains("Trait::requirement")
    }));
}

#[test]
fn signature_free_requirement_identity_rejects_an_overload_family() {
    let diagnostics = validate(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        boundary trait WindowProcedure {
            machine call(value: i32) -> i32;
            machine call(value: u64) -> u64;
        }
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call> {}
        "#,
    )
    .expect_err("signature-free identity must not select an overload");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("signature-free references reject overloads")
    }));
}
