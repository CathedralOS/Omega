use super::checks::check_program;

#[test]
fn rejects_mutation_of_source_retained_by_aggregate_helper_call_leaf() {
    let source = r#"
        data View {
            body: &mut i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise(source: &mut i32) {
            let held: View = View { body: identity(source) };
            write(source);
            write(held.body);
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("the aggregate leaf must retain the helper call's selected input loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("while local borrow `held` is still active"),
        "expected the retained aggregate loan conflict, got:\n{combined}"
    );
}

#[test]
fn accepts_unrelated_mutation_beside_aggregate_helper_call_leaf() {
    let source = r#"
        data View {
            body: &mut i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise(first: &mut i32, second: &mut i32) {
            let held: View = View { body: identity(first) };
            write(second);
            write(held.body);
        }
    "#;

    check_program(source)
        .expect("the helper-linked aggregate loan must not capture an unrelated source");
}
