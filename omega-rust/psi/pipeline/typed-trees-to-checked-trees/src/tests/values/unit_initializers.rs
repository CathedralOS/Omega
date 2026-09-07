use super::{lower_typed_trees, typed_trees};

#[test]
fn explicit_unit_locals_reject_value_initializers() {
    for initializer in [
        "7",
        "true",
        "1.5",
        "\"text\"",
        "number",
        "number + 1",
        "values",
        "values[0]",
        "&number",
        "Record { value: 7 }",
        "value()",
    ] {
        let source = format!(
            "data Record {{ value: u64; }}
            machine value() -> u64 {{ 7 }}
            machine main() -> u64 {{
                let number: u64 = 7;
                let values: [u64; 2] = [7, 8];
                let result: () = {initializer};
                0
            }}"
        );
        let diagnostics = lower_typed_trees(typed_trees(&source))
            .err()
            .unwrap_or_else(|| panic!("Unit must not store {initializer}"));
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("local `result`")
                    && diagnostic.message.contains("explicit Unit type")
            }),
            "{initializer}: {diagnostics:#?}"
        );
    }
}

#[test]
fn unit_calls_and_explicit_result_discard_remain_valid() {
    for source in [
        "machine touch(value: &mut u64) { value = 7; }
        machine main() -> u64 {
            let mut value: u64 = 0;
            touch(&mut value);
            value
        }",
        "machine value() -> u64 { 7 } machine main() { _ = value(); }",
        "machine main() { let unused: (); }",
    ] {
        lower_typed_trees(typed_trees(source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn destructuring_retains_generated_unit_inference_sentinels() {
    let source = "data Record { value: u64; }
        machine read(record: &Record) -> u64 {
            let { value as selected } = record;
            selected
        }";
    let typed = typed_trees(source);
    let inferred_unit = typed
        .machines()
        .iter()
        .flat_map(|machine| typed.machine_states(machine))
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .any(|statement| {
            matches!(statement,
            typed_trees::statement::StatementNode::LocalData(local)
                if local.type_is_inferred && matches!(
                    typed.type_reference_table.type_reference(local.type_reference),
                    typed_trees::types::TypeReferenceNode::Unit))
        });
    assert!(
        inferred_unit,
        "the generated marker retains its untyped origin"
    );
    lower_typed_trees(typed).expect("generated inference markers are not authored Unit stores");
}
