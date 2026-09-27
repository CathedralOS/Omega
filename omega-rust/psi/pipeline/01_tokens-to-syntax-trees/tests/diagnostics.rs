use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;

fn parse_error_message(source: &str) -> String {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenization should succeed for diagnostics test");
    parse_syntax_trees(&tokens)
        .expect_err("parse should fail for diagnostics test")
        .message
}

#[test]
fn retired_ranking_clauses_report_their_replacements() {
    for (clause, retired, replacement) in [
        (
            "terminates { decreases remaining; }",
            "`terminates { decreases ...; }` block form is retired",
            "terminates by",
        ),
        (
            "decreases remaining;",
            "standalone `decreases` clause is retired",
            "terminates by",
        ),
        (
            "increases remaining;",
            "standalone `increases` clause is retired",
            "Nat::IncreasingTo(<bound>)",
        ),
    ] {
        let message = parse_error_message(&format!("machine walk(remaining: u64) {clause} {{ }}"));
        assert!(message.contains(retired), "{message}");
        assert!(message.contains(replacement), "{message}");
    }
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
    let message = parse_error_message("machine main { state ready() {} let value: i32; }");
    assert_eq!(message, "expected `state`, found keyword `let`");
}

#[test]
fn raw_bytes_remain_rejected_in_utf16_text_sugar() {
    let message = parse_error_message(r#"machine emit() { utf16"\x80" }"#);
    assert_eq!(
        message,
        "raw byte string literal requires the terminal byte-sequence lowering path"
    );
}

/// `expressions.md`'s executable-supply table spells a token-bearing
/// requirement as the ordinary `machine + Name(...)`, and a concrete crowned
/// declaration already uses that grammar. A trait requirement now accepts it
/// too, so the token is not reachable only behind the `operator` introducer
/// the board is retiring. The introducer keeps working: this is additive.
#[test]
fn a_trait_requirement_carries_its_token_on_the_ordinary_machine_head() {
    for source in [
        "trait Ranked { machine < before(left: Self, right: Self) -> bool; }",
        "trait Ranked { operator < before(left: Self, right: Self) -> bool; }",
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        parse_syntax_trees(&tokens).expect("a token-bearing trait requirement parses");
    }
}
