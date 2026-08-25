use psi_source_files_to_tokens::Lexer;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

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
        "expected one of `use`, `export`, `data`, `domain`, `abi`, `machine`, `target`, `capability`, `library`, `measure`, `host`, `module`, `operator`, `package`, `platform`, `pub`, `trait`, `boundary operator`, `boundary data`, `boundary trait`, found punctuation `{`"
    );
}

#[test]
fn wire_data_form_reports_retirement() {
    let message = parse_error_message("wire data Save { 1: seed: u64; }");
    assert!(
        message.contains("`wire data` is retired"),
        "expected the retirement guidance, got: {message}"
    );
}

#[test]
fn enum_keyword_reports_retirement() {
    let message = parse_error_message("enum Direction { North, South }");
    assert_eq!(
        message,
        "`enum` is retired; spell alternatives as `case` members of a `data` declaration"
    );
}

#[test]
fn case_payload_field_requires_name_and_type() {
    // Payload fields are NAMED: a bare type with no `name:` is a parse error.
    let message = parse_error_message("data Command { case None; case Say(String); }");
    assert_eq!(message, "expected `:`, found punctuation `)`");
}

#[test]
fn bare_variant_member_spelling_is_retired() {
    // The pre-`case` bare `Name;` variant member is no longer accepted.
    let message = parse_error_message("data Direction { North; South; }");
    assert_eq!(
        message,
        "expected `:` after data field `North` (alternatives are spelled `case North;`)"
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
        "expected one of `pub entry`, `entry`, `state`, found keyword `let`"
    );
}

#[test]
fn raw_bytes_remain_rejected_in_utf16_text_sugar() {
    let message = parse_error_message(r#"machine emit() { utf16"\x80" }"#);
    assert_eq!(
        message,
        "raw byte string literal requires the terminal byte-sequence lowering path"
    );
}
