//! Cases whose input cannot be written as a readable fixture file: control bytes, invalid UTF-8,
//! invisible characters and exact line endings. Each test names the contract rule it covers.

mod support;

use omega_lexer::{LexErrorKind, SourceSpan, TokenKind, decode_quoted_bytes, lex, lex_into};

fn expect_error(
    source_bytes: &[u8],
    expected_kind: LexErrorKind,
    expected_span: SourceSpan,
) {
    match lex(source_bytes) {
        Ok(tokens) => {
            panic!("expected {expected_kind:?} but the source was accepted as {tokens:?}")
        }
        Err(lex_error) => {
            assert_eq!(lex_error.get_kind(), expected_kind, "wrong error kind for {source_bytes:?}");
            assert_eq!(lex_error.get_span(), expected_span, "wrong error span for {source_bytes:?}");
        }
    }
}

fn expect_kinds(
    source_bytes: &[u8],
    expected_kinds: &[TokenKind],
) {
    let tokens = lex(source_bytes).unwrap_or_else(|lex_error| panic!("{source_bytes:?} was rejected: {lex_error}"));

    assert_eq!(support::describe_tiling_problem(source_bytes, &tokens), None);

    let actual_kinds: Vec<TokenKind> = tokens.iter().map(|token| token.get_kind()).collect();

    assert_eq!(actual_kinds, expected_kinds, "wrong token kinds for {source_bytes:?}");
}

// R1

#[test]
fn invalid_utf8_rejects() {
    expect_error(b"let a\xFF = 1;", LexErrorKind::InvalidUtf8, SourceSpan::new(5, 6));
}

#[test]
fn invalid_utf8_inside_a_comment_still_rejects() {
    // Comments may hold any characters, but the file as a whole must still be UTF-8.
    expect_error(b"// \xC3\x28", LexErrorKind::InvalidUtf8, SourceSpan::new(3, 4));
}

#[test]
fn utf8_cut_off_at_end_of_file_rejects() {
    expect_error(b"// \xE6\x97", LexErrorKind::InvalidUtf8, SourceSpan::new(3, 5));
}

// R2

#[test]
fn empty_source_has_no_tokens() {
    expect_kinds(b"", &[]);
}

#[test]
fn lex_into_reuses_the_list_and_matches_lex() {
    let mut reused_tokens = Vec::new();

    for source_bytes in [
        &b"let a = 1; // first"[..],
        b"",
        b"x",
        b"machine main() { return 0x10u8 +! 2; }",
    ] {
        lex_into(source_bytes, &mut reused_tokens).expect("Expected the source to lex into the reused list.");

        assert_eq!(reused_tokens, lex(source_bytes).expect("Expected the source to lex."));
    }
}

#[test]
fn lex_into_leaves_the_list_empty_on_a_refusal() {
    let mut reused_tokens = lex(b"left over from an earlier file").expect("Expected the earlier file to lex.");
    let unknown_character_error = lex_into(b"let a = ?;", &mut reused_tokens).expect_err("Expected `?` to be refused, since it is not Omega syntax.");

    assert_eq!(unknown_character_error.get_kind(), LexErrorKind::UnknownCharacter);
    assert!(reused_tokens.is_empty(), "tokens scanned before the error must not be left behind");

    let utf8_error = lex_into(b"ok \xFF", &mut reused_tokens).expect_err("Expected invalid UTF-8 to be refused.");

    assert_eq!(utf8_error.get_kind(), LexErrorKind::InvalidUtf8);
    assert!(reused_tokens.is_empty());
}

// R3

#[test]
fn all_four_whitespace_bytes_form_one_token() {
    expect_kinds(b" \t\r\n", &[TokenKind::Whitespace]);
}

#[test]
fn crlf_line_endings_lex() {
    expect_kinds(
        b"a\r\nb\r\n",
        &[
            TokenKind::Word,
            TokenKind::Whitespace,
            TokenKind::Word,
            TokenKind::Whitespace,
        ],
    );
}

#[test]
fn form_feed_rejects() {
    expect_error(b"a\x0Cb", LexErrorKind::UnknownCharacter, SourceSpan::new(1, 2));
}

#[test]
fn vertical_tab_rejects() {
    expect_error(b"a\x0Bb", LexErrorKind::UnknownCharacter, SourceSpan::new(1, 2));
}

#[test]
fn no_break_space_rejects() {
    // U+00A0 is two bytes, C2 A0. The spec says Unicode whitespace other than the four ASCII bytes rejects.
    expect_error("a\u{A0}b".as_bytes(), LexErrorKind::NonAsciiOutsideCommentOrQuoted, SourceSpan::new(1, 3));
}

#[test]
fn byte_order_mark_rejects() {
    // U+FEFF is three bytes. It is non-ASCII outside a comment or literal, so no special rule is needed.
    expect_error("\u{FEFF}let".as_bytes(), LexErrorKind::NonAsciiOutsideCommentOrQuoted, SourceSpan::new(0, 3));
}

// R5

#[test]
fn four_byte_character_outside_a_comment_reports_its_full_width() {
    expect_error("a \u{1F600}".as_bytes(), LexErrorKind::NonAsciiOutsideCommentOrQuoted, SourceSpan::new(2, 6));
}

#[test]
fn non_ascii_is_allowed_in_comments_and_quoted_literals() {
    expect_kinds(
        "/* \u{65E5}\u{672C} */ \"\u{1F600}\" // \u{E9}".as_bytes(),
        &[
            TokenKind::BlockComment,
            TokenKind::Whitespace,
            TokenKind::Quoted,
            TokenKind::Whitespace,
            TokenKind::LineComment,
        ],
    );
}

// R6

#[test]
fn line_comment_stops_before_the_line_feed() {
    let source_bytes = b"// note\nx";
    let tokens = lex(source_bytes).expect("Expected the source to lex.");

    assert_eq!(tokens[0].get_kind(), TokenKind::LineComment);
    assert_eq!(tokens[0].get_span(), SourceSpan::new(0, 7));
    assert_eq!(tokens[1].get_kind(), TokenKind::Whitespace);
}

#[test]
fn line_comment_at_end_of_file_needs_no_line_feed() {
    expect_kinds(b"x // end", &[TokenKind::Word, TokenKind::Whitespace, TokenKind::LineComment]);
}

#[test]
fn line_comment_with_crlf_keeps_the_carriage_return() {
    // The comment runs to the line feed, so a CRLF file's carriage return is part of the comment.
    let source_bytes = b"// note\r\nx";
    let tokens = lex(source_bytes).expect("Expected the source to lex.");

    assert_eq!(tokens[0].get_span(), SourceSpan::new(0, 8));
}

// R12 and R13

#[test]
fn carriage_return_in_quoted_rejects() {
    expect_error(b"\"a\rb\"", LexErrorKind::NewlineInQuoted, SourceSpan::new(2, 3));
}

#[test]
fn raw_tab_in_quoted_is_allowed() {
    let source_bytes = b"\"a\tb\"";

    expect_kinds(source_bytes, &[TokenKind::Quoted]);
    assert_eq!(decode_quoted_bytes(source_bytes), Some(vec![b'a', b'\t', b'b']));
}

#[test]
fn backslash_at_end_of_file_is_an_unterminated_literal() {
    expect_error(b"\"abc\\", LexErrorKind::UnterminatedQuoted, SourceSpan::new(0, 5));
}

#[test]
fn unknown_escape_of_a_wide_character_reports_the_whole_character() {
    expect_error("\"\\\u{E9}\"".as_bytes(), LexErrorKind::UnknownEscape, SourceSpan::new(1, 4));
}

#[test]
fn hex_escape_cut_off_by_the_closing_quote_rejects() {
    expect_error(b"\"\\x4\"", LexErrorKind::InvalidHexEscape, SourceSpan::new(1, 3));
}

#[test]
fn hex_escape_at_end_of_file_rejects() {
    expect_error(b"\"\\x", LexErrorKind::InvalidHexEscape, SourceSpan::new(1, 3));
}

// Cases taken from Omega V1's own lexer tests, to confirm the two lexers agree where the contract does.

#[test]
fn other_unicode_whitespace_rejects() {
    // U+2028 LINE SEPARATOR and U+3000 IDEOGRAPHIC SPACE are three bytes each.
    expect_error("a\u{2028}b".as_bytes(), LexErrorKind::NonAsciiOutsideCommentOrQuoted, SourceSpan::new(1, 4));
    expect_error("a\u{3000}b".as_bytes(), LexErrorKind::NonAsciiOutsideCommentOrQuoted, SourceSpan::new(1, 4));
}

#[test]
fn non_ascii_directly_after_a_number_rejects_at_the_character() {
    expect_error("1\u{e9}".as_bytes(), LexErrorKind::NonAsciiOutsideCommentOrQuoted, SourceSpan::new(1, 3));
}

#[test]
fn raw_prefix_fragments_without_a_quote_are_ordinary_tokens() {
    let source_bytes = b"r#1 r###candidate";
    let tokens = lex(source_bytes).expect("Expected an `r` followed by `#` but no quote to lex, since it is not a raw literal.");
    let spellings: Vec<&[u8]> = tokens
        .iter()
        .filter(|token| !token.get_kind().is_trivia())
        .filter_map(|token| token.get_span().slice(source_bytes))
        .collect();

    assert_eq!(spellings, [&b"r"[..], b"#", b"1", b"r", b"#", b"#", b"#", b"candidate"]);
}

#[test]
fn raw_prefix_with_many_hashes_rejects_through_the_quote() {
    expect_error(b"r##\"raw\"##", LexErrorKind::RawLiteralPrefix, SourceSpan::new(0, 4));
}

#[test]
fn block_comment_closes_on_the_last_star_of_a_run() {
    expect_kinds(b"/* tail **/x", &[TokenKind::BlockComment, TokenKind::Word]);
}

#[test]
fn slash_star_slash_does_not_close_itself() {
    // The `*` that opens the comment cannot also be the `*` that closes it.
    expect_error(b"/*/", LexErrorKind::UnterminatedBlockComment, SourceSpan::new(0, 3));
}

// R17

#[test]
fn nul_byte_rejects() {
    expect_error(b"a\0b", LexErrorKind::UnknownCharacter, SourceSpan::new(1, 2));
}
