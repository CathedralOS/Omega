use crate::tests::front_end::checked_program_result;

fn rejects_index(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("unproved index must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("index")),
        "{diagnostics:#?}"
    );
}

#[test]
fn right_operand_uses_only_its_selected_guard_polarity() {
    for expression in [
        "bytes.len > 0 && bytes[0] == 128",
        "bytes.len == 0 || bytes[0] == 128",
        "bytes.len <= 0 || bytes[0] == 128",
        "bytes.len > 0 && (bytes[0] == 128 || bytes[0] == 0)",
    ] {
        let source = format!("machine inspect(bytes: &[u8]) -> bool {{ {expression} }}");
        checked_program_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
    for expression in [
        "bytes.len > 0 || bytes[0] == 128",
        "bytes.len == 0 && bytes[0] == 128",
        "bytes[0] == 128 && bytes.len > 0",
        "other.len > 0 && bytes[0] == 128",
    ] {
        rejects_index(&format!(
            "machine inspect(bytes: &[u8], other: &[u8]) -> bool {{ {expression} }}"
        ));
    }
}
