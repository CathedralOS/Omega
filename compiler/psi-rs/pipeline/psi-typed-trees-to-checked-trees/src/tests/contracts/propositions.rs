use super::*;

#[test]
fn proposition_type_arguments_instantiate_value_parameter_types() {
    let source = r#"
        proposition typed<T>(value: T);
        data Main { value: i32; }

        machine Main::run(&mut self)
        requires typed<i32>(self.value)
        {
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("the concrete type argument should instantiate the proposition value signature");
}

#[test]
fn proposition_type_arguments_reject_mismatched_value_arguments() {
    let source = r#"
        proposition typed<T>(value: T);
        data Main { value: bool; }

        machine Main::run(&mut self)
        requires typed<i32>(self.value)
        {
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a bool value cannot satisfy a proposition parameter instantiated as i32");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "proposition `typed` argument 1 does not match parameter `value` type `i32`",
            )
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
