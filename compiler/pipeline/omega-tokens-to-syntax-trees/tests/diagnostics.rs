use omega_source_files_to_tokens::Lexer;
use omega_tokens_to_syntax_trees::parse_syntax_trees;

fn parse_error_message(source: &str) -> String {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenization should succeed for diagnostics test");
    parse_syntax_trees(&tokens)
        .expect_err("parse should fail for diagnostics test")
        .message
}

#[test]
fn top_level_item_error_lists_expected_items() {
    let message = parse_error_message("{");
    assert_eq!(
        message,
        "expected one of `use`, `export`, `data`, `domain`, `enum`, `machine`, `target`, `trust`, `capability`, `invariant`, `library`, `platform`, `trait`, `boundary trait`, found punctuation `{`"
    );
}

#[test]
fn eof_reports_expected_identifier() {
    let message = parse_error_message("use ");
    assert_eq!(message, "expected token, found EOF");
}

#[test]
fn machine_item_error_lists_expected_members() {
    let message = parse_error_message("machine main { entry() {} let value: i32; }");
    assert_eq!(
        message,
        "expected one of `pub entry`, `entry`, `state`, `invariant`, found keyword `let`"
    );
}
