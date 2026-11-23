//! What the parser receives from the lexer's rules, observed through `parse_file`.
//!
//! Each test lexes a controlled source with the real lexer, parses it with the real parser and
//! compares the tree's leaves exactly: the innermost node holding each meaningful token, the
//! token's kind, its spelling and its byte range. Where a rule hides bytes from the parser, the
//! tree is also compared with the tree of the same source written plainly.
//!
//! L0 rules with no comparison here, and why:
//!
//! - R2 (tiling), R4 (words) and R16 (no `>>`) already have parser observations: the tree check in
//!   `golden_fixtures.rs`, the path-expression leaf at COPY's initializer, and `right_shift.omg`.
//! - R9 (a radix prefix needs a digit of its radix) and R14 (no raw literal form) only remove
//!   inputs. Every source they decide is refused by the lexer and never reaches the parser; what
//!   they still accept is an ordinary radix integer (compared under R8) or a lone `r` word (R4).
//! - R13 (codepoint escapes and raw line breaks in quotes) and R17 (other ASCII characters) are
//!   refusals only. No token list exists for the parser to consume.

use omega_lexer::{IntegerRadix, Punctuation, TokenKind, lex};
use omega_parser::{NodeKind, ParseErrorKind, TreeEvent, parse_file};

/// The innermost node, the token kind, the spelling and the byte range of one tree leaf.
type Leaf<'source> = (NodeKind, TokenKind, &'source str, usize, usize);

struct ParsedSource<'source> {
    leaves: Vec<Leaf<'source>>,
    meaningful_token_indices: Vec<u32>,
    events: Vec<TreeEvent>,
}

fn parse(source: &str) -> ParsedSource<'_> {
    let source_bytes = source.as_bytes();
    let tokens = lex(source_bytes).unwrap_or_else(|error| panic!("{source:?} must lex: {error}"));
    let tree = parse_file(source_bytes, &tokens).unwrap_or_else(|error| panic!("{source:?} must parse: {error}"));
    let mut open_nodes = Vec::new();
    let mut token_indices = tree.get_meaningful_token_indices().iter();
    let mut leaves = Vec::new();

    for event in tree.get_events() {
        match *event {
            TreeEvent::Open(kind) => open_nodes.push(kind),
            TreeEvent::Close => {
                open_nodes.pop();
            }
            TreeEvent::Token => {
                let token_index = *token_indices.next().expect("one token index per token event");
                let token = tokens[token_index as usize];
                let span = token.get_span();
                let (start, end) = (span.get_start_offset(), span.get_end_offset());
                // A span that splits a character cannot be sliced as text, so this also checks byte offsets.
                let spelling = source.get(start..end).expect("a leaf span lies on character boundaries");
                let node = *open_nodes.last().expect("every token sits inside a node");

                leaves.push((node, token.get_kind(), spelling, start, end));
            }
        }
    }

    assert_eq!(token_indices.next(), None, "{source:?}: a token index without a token event");

    ParsedSource {
        leaves,
        meaningful_token_indices: tree.get_meaningful_token_indices().to_vec(),
        events: tree.get_events().to_vec(),
    }
}

const WORD: TokenKind = TokenKind::Word;
const QUOTED: TokenKind = TokenKind::Quoted;

const fn punctuation(spelling: Punctuation) -> TokenKind {
    TokenKind::Punctuation(spelling)
}

const fn integer(
    radix: IntegerRadix,
    suffix_start_offset: usize,
) -> TokenKind {
    TokenKind::Integer { radix, suffix_start_offset }
}

const fn float(suffix_start_offset: usize) -> TokenKind {
    TokenKind::Float { suffix_start_offset }
}

const EQUAL: TokenKind = punctuation(Punctuation::Equal);
const SEMICOLON: TokenKind = punctuation(Punctuation::Semicolon);
const PLUS: TokenKind = punctuation(Punctuation::Plus);

/// R1. Two-, three- and four-byte characters before later tokens: the parser still finds `const`
/// by its byte offset, and every leaf it hands on slices the source as text.
#[test]
fn utf8_literal_and_comment_bytes_keep_later_leaves_at_their_byte_offsets() {
    let source = "const ok = \"\u{E9}\u{65E5}\u{1F600}\"; /* \u{1F600} */ const next = ok;";
    let parsed = parse(source);
    let observed = (parsed.leaves, parsed.meaningful_token_indices);
    let expected = (
        vec![
            (NodeKind::Const, WORD, "const", 0, 5),
            (NodeKind::Path, WORD, "ok", 6, 8),
            (NodeKind::Const, EQUAL, "=", 9, 10),
            (NodeKind::Literal, QUOTED, "\"\u{E9}\u{65E5}\u{1F600}\"", 11, 22),
            (NodeKind::Const, SEMICOLON, ";", 22, 23),
            (NodeKind::Const, WORD, "const", 35, 40),
            (NodeKind::Path, WORD, "next", 41, 45),
            (NodeKind::Const, EQUAL, "=", 46, 47),
            (NodeKind::PathExpression, WORD, "ok", 48, 50),
            (NodeKind::Const, SEMICOLON, ";", 50, 51),
        ],
        vec![0, 2, 4, 6, 7, 11, 13, 15, 17, 18],
    );

    spec_corpus::observe_handoff!("valid-utf-8/consumer", assert_eq!(observed, expected));
}

/// R3. Tab, CR, LF and space each separate tokens and none reaches the tree: the tree equals the
/// tree of the same declaration written with single spaces.
#[test]
fn every_whitespace_byte_is_skipped_like_a_single_space() {
    let source = "const\ta\r\n= b\n;";
    let parsed = parse(source);
    let observed = (parsed.leaves, parsed.meaningful_token_indices, parsed.events);
    let expected = (
        vec![
            (NodeKind::Const, WORD, "const", 0, 5),
            (NodeKind::Path, WORD, "a", 6, 7),
            (NodeKind::Const, EQUAL, "=", 9, 10),
            (NodeKind::PathExpression, WORD, "b", 11, 12),
            (NodeKind::Const, SEMICOLON, ";", 13, 14),
        ],
        vec![0, 2, 4, 6, 8],
        parse("const a = b;").events,
    );

    spec_corpus::observe_handoff!("whitespace-bytes/consumer", assert_eq!(observed, expected));
}

/// R5. Non-ASCII bytes in a line comment, a block comment and a quoted literal: only the quoted
/// literal becomes a leaf, and every word the parser decides on is ASCII.
#[test]
fn non_ascii_bytes_reach_the_tree_only_inside_a_quoted_literal() {
    let source = "// \u{E9} line\nconst label = /* \u{FC} */ \"na\u{EF}ve\";";
    let parsed = parse(source);
    let observed = (parsed.leaves, parsed.meaningful_token_indices);
    let expected = (
        vec![
            (NodeKind::Const, WORD, "const", 11, 16),
            (NodeKind::Path, WORD, "label", 17, 22),
            (NodeKind::Const, EQUAL, "=", 23, 24),
            (NodeKind::Literal, QUOTED, "\"na\u{EF}ve\"", 34, 42),
            (NodeKind::Const, SEMICOLON, ";", 42, 43),
        ],
        vec![2, 4, 6, 10, 11],
    );

    spec_corpus::observe_handoff!("non-ascii-placement/consumer", assert_eq!(observed, expected));
}

/// R6. `; const z = q` after `//` is comment text, so the declaration ends at the `;` on the next line.
#[test]
fn a_line_comment_hides_the_rest_of_its_line_from_the_parser() {
    let source = "const a = b // ; const z = q\n;";
    let parsed = parse(source);
    let observed = (parsed.leaves, parsed.meaningful_token_indices, parsed.events);
    let expected = (
        vec![
            (NodeKind::Const, WORD, "const", 0, 5),
            (NodeKind::Path, WORD, "a", 6, 7),
            (NodeKind::Const, EQUAL, "=", 8, 9),
            (NodeKind::PathExpression, WORD, "b", 10, 11),
            (NodeKind::Const, SEMICOLON, ";", 29, 30),
        ],
        vec![0, 2, 4, 6, 10],
        parse("const a = b;").events,
    );

    spec_corpus::observe_handoff!("line-comments/consumer", assert_eq!(observed, expected));
}

/// R7. The first `*/` closes only the inner comment; `const z =` before the matching `*/` stays hidden.
#[test]
fn a_nested_block_comment_hides_everything_to_its_matching_close() {
    let source = "const a = /* outer /* inner */ const z = */ b;";
    let parsed = parse(source);
    let observed = (parsed.leaves, parsed.meaningful_token_indices, parsed.events);
    let expected = (
        vec![
            (NodeKind::Const, WORD, "const", 0, 5),
            (NodeKind::Path, WORD, "a", 6, 7),
            (NodeKind::Const, EQUAL, "=", 8, 9),
            (NodeKind::PathExpression, WORD, "b", 44, 45),
            (NodeKind::Const, SEMICOLON, ";", 45, 46),
        ],
        vec![0, 2, 4, 8, 9],
        parse("const a = b;").events,
    );

    spec_corpus::observe_handoff!("nested-block-comments/consumer", assert_eq!(observed, expected));
}

/// R8. Every radix, with `_` among its digits, is one integer token: the parser accepts it as a
/// stable number, which must be an integer, and as one literal operand.
#[test]
fn a_lone_raw_prefix_is_an_ordinary_name_on_both_sides_of_a_binding() {
    let leaves: Vec<_> = parse("const r = r;").leaves.into_iter().filter(|leaf| leaf.2 == "r").collect();
    spec_corpus::observe_handoff!(
        "raw-prefix/ordinary-name",
        assert_eq!(leaves, [(NodeKind::Path, WORD, "r", 6, 7), (NodeKind::PathExpression, WORD, "r", 10, 11)])
    );
}

#[test]
fn radix_integers_are_single_stable_numbers_and_literals() {
    let source = "data Packet {\n    #0x1_0 tag: u8;\n    #0b1_1 size: u8;\n}\nconst mask = 1_000 + 0o7_7;";
    let hash = punctuation(Punctuation::Hash);
    let colon = punctuation(Punctuation::Colon);
    let observed = parse(source).leaves;
    let expected = vec![
        (NodeKind::Data, WORD, "data", 0, 4),
        (NodeKind::Data, WORD, "Packet", 5, 11),
        (NodeKind::Data, punctuation(Punctuation::LeftBrace), "{", 12, 13),
        (NodeKind::StableNumber, hash, "#", 18, 19),
        (NodeKind::StableNumber, integer(IntegerRadix::Hexadecimal, 24), "0x1_0", 19, 24),
        (NodeKind::Field, WORD, "tag", 25, 28),
        (NodeKind::Field, colon, ":", 28, 29),
        (NodeKind::PathType, WORD, "u8", 30, 32),
        (NodeKind::Field, SEMICOLON, ";", 32, 33),
        (NodeKind::StableNumber, hash, "#", 38, 39),
        (NodeKind::StableNumber, integer(IntegerRadix::Binary, 44), "0b1_1", 39, 44),
        (NodeKind::Field, WORD, "size", 45, 49),
        (NodeKind::Field, colon, ":", 49, 50),
        (NodeKind::PathType, WORD, "u8", 51, 53),
        (NodeKind::Field, SEMICOLON, ";", 53, 54),
        (NodeKind::Data, punctuation(Punctuation::RightBrace), "}", 55, 56),
        (NodeKind::Const, WORD, "const", 57, 62),
        (NodeKind::Path, WORD, "mask", 63, 67),
        (NodeKind::Const, EQUAL, "=", 68, 69),
        (NodeKind::Literal, integer(IntegerRadix::Decimal, 75), "1_000", 70, 75),
        (NodeKind::Binary, PLUS, "+", 76, 77),
        (NodeKind::Literal, integer(IntegerRadix::Octal, 83), "0o7_7", 78, 83),
        (NodeKind::Const, SEMICOLON, ";", 83, 84),
    ];

    spec_corpus::observe_handoff!("integer-spellings/consumer", assert_eq!(observed, expected));
}

/// R10. A fraction, an exponent with either sign, or both make one float literal; `0..5` has no
/// digit after its first `.`, so the parser receives two integers around `..` and builds a range.
#[test]
fn floats_are_single_literals_and_a_dot_without_a_digit_leaves_a_range() {
    let source = "const f = 1.5 + 2e-3 + 6.02e+23;\nconst s = v[0..5];";
    let observed = parse(source).leaves;
    let expected = vec![
        (NodeKind::Const, WORD, "const", 0, 5),
        (NodeKind::Path, WORD, "f", 6, 7),
        (NodeKind::Const, EQUAL, "=", 8, 9),
        (NodeKind::Literal, float(13), "1.5", 10, 13),
        (NodeKind::Binary, PLUS, "+", 14, 15),
        (NodeKind::Literal, float(20), "2e-3", 16, 20),
        (NodeKind::Binary, PLUS, "+", 21, 22),
        (NodeKind::Literal, float(31), "6.02e+23", 23, 31),
        (NodeKind::Const, SEMICOLON, ";", 31, 32),
        (NodeKind::Const, WORD, "const", 33, 38),
        (NodeKind::Path, WORD, "s", 39, 40),
        (NodeKind::Const, EQUAL, "=", 41, 42),
        (NodeKind::PathExpression, WORD, "v", 43, 44),
        (NodeKind::Index, punctuation(Punctuation::LeftBracket), "[", 44, 45),
        (NodeKind::Literal, integer(IntegerRadix::Decimal, 46), "0", 45, 46),
        (NodeKind::Range, punctuation(Punctuation::DotDot), "..", 46, 48),
        (NodeKind::Literal, integer(IntegerRadix::Decimal, 49), "5", 48, 49),
        (NodeKind::Index, punctuation(Punctuation::RightBracket), "]", 49, 50),
        (NodeKind::Const, SEMICOLON, ";", 50, 51),
    ];

    spec_corpus::observe_handoff!("float-spellings/consumer", assert_eq!(observed, expected));
}

/// R10. The parser tells the lexer's float from an integer: `1.5` cannot be a stable number.
#[test]
fn a_float_is_refused_where_the_parser_needs_an_integer() {
    let source = "data Packet {\n    #1.5 tag: u8;\n}";
    let source_bytes = source.as_bytes();
    let tokens = lex(source_bytes).expect("the float lexes");
    let error = parse_file(source_bytes, &tokens).expect_err("a float is not a stable number");
    let observed = (
        error.get_kind(),
        error.get_expected(),
        error.get_span().get_start_offset(),
        error.get_span().get_end_offset(),
    );

    spec_corpus::observe_handoff!(
        "float-spellings/consumer-2",
        assert_eq!(observed, (ParseErrorKind::UnexpectedToken, "a stable number", 19, 22))
    );
}

/// R11. Every accepted suffix form stays inside its number, so each operand is one literal leaf and
/// no suffix reaches the parser as a word.
#[test]
fn a_numeric_suffix_stays_inside_its_literal() {
    let source = "const n = 7u8 + 3i + 9u + 12i64 + 1.5f32 + 2e3f64;";
    let observed = parse(source).leaves;
    let expected = vec![
        (NodeKind::Const, WORD, "const", 0, 5),
        (NodeKind::Path, WORD, "n", 6, 7),
        (NodeKind::Const, EQUAL, "=", 8, 9),
        (NodeKind::Literal, integer(IntegerRadix::Decimal, 11), "7u8", 10, 13),
        (NodeKind::Binary, PLUS, "+", 14, 15),
        (NodeKind::Literal, integer(IntegerRadix::Decimal, 17), "3i", 16, 18),
        (NodeKind::Binary, PLUS, "+", 19, 20),
        (NodeKind::Literal, integer(IntegerRadix::Decimal, 22), "9u", 21, 23),
        (NodeKind::Binary, PLUS, "+", 24, 25),
        (NodeKind::Literal, integer(IntegerRadix::Decimal, 28), "12i64", 26, 31),
        (NodeKind::Binary, PLUS, "+", 32, 33),
        (NodeKind::Literal, float(37), "1.5f32", 34, 40),
        (NodeKind::Binary, PLUS, "+", 41, 42),
        (NodeKind::Literal, float(46), "2e3f64", 43, 49),
        (NodeKind::Const, SEMICOLON, ";", 49, 50),
    ];

    spec_corpus::observe_handoff!("number-suffixes/consumer", assert_eq!(observed, expected));
}

/// R12. An escaped quote does not end the literal, so the `;` inside it is not the declaration's
/// end; `\x41`, `\\`, `\t` and `\0` stay inside the second literal.
#[test]
fn escaped_quotes_keep_a_quoted_literal_one_leaf() {
    let source = r#"const q = "say \"hi\"; done" + "\x41\\\t\0";"#;
    let observed = parse(source).leaves;
    let expected = vec![
        (NodeKind::Const, WORD, "const", 0, 5),
        (NodeKind::Path, WORD, "q", 6, 7),
        (NodeKind::Const, EQUAL, "=", 8, 9),
        (NodeKind::Literal, QUOTED, r#""say \"hi\"; done""#, 10, 28),
        (NodeKind::Binary, PLUS, "+", 29, 30),
        (NodeKind::Literal, QUOTED, r#""\x41\\\t\0""#, 31, 43),
        (NodeKind::Const, SEMICOLON, ";", 43, 44),
    ];

    spec_corpus::observe_handoff!("quoted-literals/consumer", assert_eq!(observed, expected));
}

/// R15. `+!`, `&&`, `::`, `<=` and `+%` each reach the parser as one spelling, so `a +!b` is a
/// trapping add; with a space, `a + !b` is an add of a negation.
#[test]
fn the_parser_receives_the_longest_punctuation_spelling() {
    let source = "const p = a +!b && c::d <= e +% f;\nconst q = a + !b;";
    let observed = parse(source).leaves;
    let expected = vec![
        (NodeKind::Const, WORD, "const", 0, 5),
        (NodeKind::Path, WORD, "p", 6, 7),
        (NodeKind::Const, EQUAL, "=", 8, 9),
        (NodeKind::PathExpression, WORD, "a", 10, 11),
        (NodeKind::Binary, punctuation(Punctuation::PlusTrapping), "+!", 12, 14),
        (NodeKind::PathExpression, WORD, "b", 14, 15),
        (NodeKind::Binary, punctuation(Punctuation::AmpersandAmpersand), "&&", 16, 18),
        (NodeKind::PathExpression, WORD, "c", 19, 20),
        (NodeKind::PathExpression, punctuation(Punctuation::ColonColon), "::", 20, 22),
        (NodeKind::PathExpression, WORD, "d", 22, 23),
        (NodeKind::Binary, punctuation(Punctuation::LessEqual), "<=", 24, 26),
        (NodeKind::PathExpression, WORD, "e", 27, 28),
        (NodeKind::Binary, punctuation(Punctuation::PlusWrapping), "+%", 29, 31),
        (NodeKind::PathExpression, WORD, "f", 32, 33),
        (NodeKind::Const, SEMICOLON, ";", 33, 34),
        (NodeKind::Const, WORD, "const", 35, 40),
        (NodeKind::Path, WORD, "q", 41, 42),
        (NodeKind::Const, EQUAL, "=", 43, 44),
        (NodeKind::PathExpression, WORD, "a", 45, 46),
        (NodeKind::Binary, PLUS, "+", 47, 48),
        (NodeKind::Unary, punctuation(Punctuation::Exclamation), "!", 49, 50),
        (NodeKind::PathExpression, WORD, "b", 50, 51),
        (NodeKind::Const, SEMICOLON, ";", 51, 52),
    ];

    spec_corpus::observe_handoff!("longest-punctuation/consumer", assert_eq!(observed, expected));
}
