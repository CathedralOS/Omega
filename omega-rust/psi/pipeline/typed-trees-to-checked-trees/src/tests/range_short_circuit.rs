use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

fn rejects_index(source: &str) {
    let diagnostics = check(source).expect_err("unproved index must reject");
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
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
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

#[test]
fn selected_conjunction_retains_both_signed_index_bounds() {
    check(
        "machine inspect(bytes: &[u8], index: i64) -> bool {
            0 <= index && index < bytes.len && bytes[index] == 128
        }",
    )
    .unwrap();
    rejects_index(
        "machine inspect(bytes: &[u8], index: i64) -> bool {
            index < bytes.len && bytes[index] == 128
        }",
    );
}

#[test]
fn argument_guards_do_not_escape_to_the_next_argument_or_statement() {
    check(
        "boundary trait Output { machine flag(value: bool) reaches Output; }
        machine inspect(bytes: &[u8]) reaches Output {
            Output::flag(bytes.len > 0 && bytes[0] == 128);
        }",
    )
    .unwrap();
    rejects_index(
        "machine consume(guarded: bool, byte: u8) {}
        machine inspect(bytes: &[u8]) {
            consume(bytes.len > 0 && bytes[0] == 128, bytes[0]);
        }",
    );
    rejects_index(
        "machine inspect(bytes: &[u8]) -> u8 {
            let guarded: bool = bytes.len > 0 && bytes[0] == 128;
            bytes[0]
        }",
    );
}

#[test]
fn possible_right_operand_writes_invalidate_incoming_index_facts() {
    for condition in [
        "enabled && replace(&mut index)",
        "enabled || replace(&mut index)",
    ] {
        rejects_index(&format!(
            "machine replace(index: &mut u64) -> bool {{ index = 255; true }}
            machine inspect(bytes: &[u8], mut index: u64, enabled: bool) -> u8
            requires index < bytes.len;
            {{
                let result: bool = {condition};
                bytes[index]
            }}"
        ));
    }
}

#[test]
fn mutating_guard_cannot_replay_an_earlier_comparison() {
    rejects_index(
        "machine replace(index: &mut u64) -> bool { index = 255; true }
        machine inspect(bytes: &[u8], mut index: u64) -> bool {
            (index < bytes.len && replace(&mut index)) && bytes[index] == 128
        }",
    );
}

#[test]
fn disjoint_right_operand_writes_preserve_existing_index_facts() {
    check(
        "machine replace(value: &mut u64) -> bool { value = 255; true }
        machine inspect(bytes: &[u8], index: u64, enabled: bool) -> u8
        requires index < bytes.len;
        {
            let mut other: u64 = 0;
            let result: bool = enabled && replace(&mut other);
            bytes[index]
        }",
    )
    .unwrap();
}
