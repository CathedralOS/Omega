use crate::tests::front_end::typed_program;

#[test]
fn closed_type_not_satisfying_property_bound_is_rejected_by_validation() {
    let typed = typed_program(
        r#"
        data Math {}
        data LinearValue [linear] { value: i32; }

        boundary operator Math::same<Value [copy]>(left: Value, right: Value) -> bool;
        machine compare(left: LinearValue, right: LinearValue) -> bool {
            Math::same(left, right)
        }
        "#,
    );
    let diagnostics = validation::validate_program(&typed)
        .expect_err("LinearValue must not satisfy the operator's [copy] bound");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("Value [copy]")
            && diagnostic.message.contains("LinearValue")
            && diagnostic.message.contains("does not satisfy `[copy]`")
    }));
}

#[test]
fn explicit_bounded_type_argument_must_equal_operand_inference() {
    let typed = typed_program(
        r#"
        data Math {}
        data Actual [copy] { value: i32; }
        data Other [copy] { value: i32; }

        boundary operator Math::same<Value [copy]>(left: Value, right: Value) -> bool;
        machine compare(left: Actual, right: Actual) -> bool {
            Math::same<Other>(left, right)
        }
        "#,
    );
    let diagnostics = validation::validate_program(&typed)
        .expect_err("the explicit bounded type must agree with operand inference");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not equal the type inferred from its operands")
    }));
}
