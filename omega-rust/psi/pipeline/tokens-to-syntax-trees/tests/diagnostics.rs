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
fn top_level_item_error_lists_expected_items() {
    let message = parse_error_message("{");
    assert_eq!(
        message,
        "expected one of `use`, `data`, `domain`, `abi`, `machine`, `capability`, `let`, `library`, `measure`, `host`, `module`, `operator`, `package`, `platform`, `pub`, `trait`, `boundary let`, `boundary operator`, `boundary requirement`, `boundary data`, `boundary trait`, found punctuation `{`"
    );
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
fn increases_remains_an_ordinary_identifier_outside_a_retired_clause() {
    let tokens = Lexer::new("machine increases(increases: u64) -> u64 { increases }")
        .tokenize()
        .expect("tokenize ordinary contextual name");
    parse_syntax_trees(&tokens).expect("retirement guidance must not reserve an identifier");
}

/// `abd983fe9d` removed the `enum` token and the `wire data` migration-only
/// rejection branch along with their fixtures, so neither spelling is known to
/// the grammar any more. What is left to pin is that they are ABSENT rather
/// than special: each is an ordinary identifier the top-level item parser does
/// not accept, reported by the same expected-items list every other unknown
/// item gets, with no retirement branch left to maintain. Their own retirement
/// guidance is deliberately gone and must not come back.
#[test]
fn retired_declaration_spellings_are_absent_rather_than_special_cased() {
    for (source, spelling) in [
        ("enum Direction { North, South }", "enum"),
        ("wire data Save { 1: seed: u64; }", "wire"),
    ] {
        let message = parse_error_message(source);
        assert_eq!(
            message,
            format!(
                "expected one of `use`, `data`, `domain`, `abi`, `machine`, `capability`, `let`, `library`, `measure`, `host`, `module`, `operator`, `package`, `platform`, `pub`, `trait`, `boundary let`, `boundary operator`, `boundary requirement`, `boundary data`, `boundary trait`, found identifier `{spelling}`"
            ),
            "{spelling} must reject as an ordinary unknown item"
        );
        assert!(
            !message.contains("retired"),
            "{spelling} keeps no retirement branch: {message}"
        );
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

/// A tokenless requirement still parses, and a semicolon head is not mistaken
/// for a token.
#[test]
fn a_tokenless_trait_requirement_is_unaffected() {
    let tokens = Lexer::new("trait Ranked { machine before(left: Self, right: Self) -> bool; }")
        .tokenize()
        .expect("tokenize");
    parse_syntax_trees(&tokens).expect("a tokenless trait requirement parses");
}

/// A domain's token signature is an ordinary top-level declaration, and the
/// `machine` head already carried its token -- with a body and bodyless alike.
/// Pinned alongside the trait head above so the pair that OPERATOR-MACHINE-SUPPLY
/// named together stays spelled the same way.
#[test]
fn a_domain_token_signature_takes_the_ordinary_machine_head() {
    for source in [
        "domain u32::Meters;\nmachine < Meters::before(a: u32 in Meters, b: u32 in Meters) -> bool { true }",
        "domain u32::Meters;\nboundary machine < Meters::before(a: u32 in Meters, b: u32 in Meters) -> bool;",
        "domain u32::Meters;\nboundary operator < Meters::before(a: u32 in Meters, b: u32 in Meters) -> bool;",
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        parse_syntax_trees(&tokens).expect("a domain token signature parses");
    }
}
