use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "stale state argument accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("index")),
                "expected an index rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn readonly_and_disjoint_calls_preserve_named_state_indices() {
    for call in [
        "inspect(index);",
        "set_index(&mut unrelated);",
        "let ignored: u64 = inspect_value(index);",
        "let ignored: u64 = set_index_value(&mut unrelated);",
    ] {
        check(
            &format!(
                r#"
            machine inspect(index: u64) {{}}
            machine inspect_value(index: u64) -> u64 {{ index }}
            machine set_index(index: &mut u64) {{ index = 255; }}
            machine set_index_value(index: &mut u64) -> u64 {{ index = 255; 0 }}
            machine main() -> u64 {{
                let values: [u64; 2] = [10, 20];
                let index: u64 = 0;
                let mut unrelated: u64 = 0;
                {call}
                transition {{ _ -> read(values, index) }}
                state read(values: [u64; 2], index: u64) -> u64 {{ values[index] }}
            }}
        "#
            ),
            true,
        );
    }
}
