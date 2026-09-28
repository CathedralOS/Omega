use super::checks::check_program;

#[test]
fn accepts_unrelated_source_mutation_beside_cast_aggregate_loan() {
    let source = aggregate_cast_source(
        "let inner: View = make_view(source);\n             let held: Outer = Outer { view: inner as View };",
        "write(other); write(held.view.body);",
    );

    check_program(&source).expect("a cast aggregate loan must not capture an unrelated source");
}

fn aggregate_cast_source(initializer: &str, body: &str) -> String {
    format!(
        r#"
        data View {{
            body: &mut i32;
        }}

        data Outer {{
            view: View;
        }}

        machine make_view(source: &mut i32) -> View {{
            let result: View = View {{ body: source }};
            transition {{ _ -> result }}
        }}

        machine write(value: &mut i32) {{
            value = 1;
        }}

        machine exercise(source: &mut i32, other: &mut i32) {{
            {initializer}
            {body}
        }}
        "#
    )
}
