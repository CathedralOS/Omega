use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool, rejection: &str) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "stale entry assumption accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn replacing_a_slice_descriptor_retires_its_old_extent() {
    for (replacement, accepted) in [("other", false), ("items[0..1]", true)] {
        check(
            &format!(
                r#"
            machine main(mut items: &[u64], other: &[u64]) -> u64
            requires items.len > 0
            {{
                items = {replacement};
                transition {{ _ -> read(items) }}
                state read(items: &[u64]) -> u64 {{ items[0] }}
            }}
        "#
            ),
            accepted,
            "index",
        );
    }
}
