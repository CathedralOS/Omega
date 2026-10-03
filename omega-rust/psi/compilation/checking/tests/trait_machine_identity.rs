use validation::validate_program;

fn validate(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let typed = crate::front_end::typed_program_result(source)?;
    validate_program(&typed)
}

#[test]
fn exact_satisfaction_infers_self_from_operands_not_machine_namespace() {
    validate(
        r#"
        trait Order { machine before(left: Self, right: Self) -> bool; }
        data Algorithms {}
        machine Algorithms::before(left: i32, right: i32) -> bool
        satisfies Order::before { left < right }
        "#,
    )
    .expect("a qualified exact satisfier's namespace is not its operand type");
}

#[test]
fn exact_satisfaction_requires_one_carrier_for_repeated_self() {
    let diagnostics = validate(
        r#"
        trait Order { machine before(left: Self, right: Self) -> bool; }
        data Algorithms {}
        machine Algorithms::before(left: i32, right: u32) -> bool
        satisfies Order::before { false }
        "#,
    )
    .expect_err("qualification cannot waive repeated Self correspondence");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("parameter `right`")
            && diagnostic.message.contains("does not satisfy trait")
    }));
}

#[test]
fn whole_conformance_still_requires_its_declared_subject() {
    let diagnostics = validate(
        r#"
        trait Reset { machine reset(value: &mut Self); }
        data Counter { value: i32; }
        data Other { value: i32; }
        machine Counter::reset(value: &mut Other) satisfies Reset::reset {
            value.value = 0;
        }
        CounterReset: Counter satisfies Reset;
        "#,
    )
    .expect_err("an exact satisfaction edge does not establish a whole conformance");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not satisfy trait `Reset`")
    }));
}

#[test]
fn qualified_exact_satisfaction_preserves_reference_access() {
    let diagnostics = validate(
        r#"
        trait Inspect { machine inspect(value: &Self); }
        data Algorithms {}
        machine Algorithms::inspect(value: &mut i32) satisfies Inspect::inspect {}
        "#,
    )
    .expect_err("an implementation cannot demand exclusive access from a shared requirement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not satisfy trait `Inspect`")
    }));
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
