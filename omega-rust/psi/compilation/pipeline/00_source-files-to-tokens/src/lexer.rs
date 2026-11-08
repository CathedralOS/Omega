/*
Single-source lexical scanner (`Lexer::tokenize` and `Lexer::tokenize_reusing`),
numeric and string literal rules (`numbers`, `strings`), infallible string-byte
decoding (`strings::decode`), and `LexError` diagnostics.
*/

use std::iter::Peekable;
use std::str::CharIndices;

use crate::LexError;
use crate::lexer::strings::decode_string_literal;
use source::Span;
use tokens::{CommentKind, KeywordKind, PunctuationKind, TokenKind, TokenStream};

pub mod lex_error {
    use source::Span;

    pub(crate) const OUTSIDE_LEXICAL_PROFILE_MESSAGE: &str =
        "spelling is outside the current language profile";

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LexError {
        pub message: String,
        pub span: Span,
    }

    impl LexError {
        pub fn new(message: impl Into<String>, span: Span) -> Self {
            Self {
                message: message.into(),
                span,
            }
        }

        pub(crate) fn outside_lexical_profile(span: Span) -> Self {
            Self::new(OUTSIDE_LEXICAL_PROFILE_MESSAGE, span)
        }
    }
}
pub(crate) mod numbers {
    use source::Span;
    use tokens::{
        FloatLiteralKind, IntegerLiteralKind, NumericBase, NumericLiteralKind, TokenKind,
    };

    use super::{LexedToken, is_identifier_continue, is_identifier_start};

    impl<'source> super::Lexer<'source> {
        pub(crate) fn lex_number(&mut self, start: usize, first: char) -> LexedToken {
            if first == '.' {
                return self.lex_leading_dot_float(start);
            }

            let mut end = start + first.len_utf8();
            let mut integer_kind = IntegerLiteralKind::default();
            let mut float_kind = FloatLiteralKind::default();

            if first == '0' {
                match self.peek_character() {
                    Some('b') | Some('B') => {
                        integer_kind.base = NumericBase::Binary;
                        end = self.consume_base_prefix(end);
                        integer_kind.empty_digits =
                            !self.consume_digits_with_underscores(&mut end, 2);
                    }
                    Some('o') | Some('O') => {
                        integer_kind.base = NumericBase::Octal;
                        end = self.consume_base_prefix(end);
                        integer_kind.empty_digits =
                            !self.consume_digits_with_underscores(&mut end, 8);
                    }
                    Some('x') | Some('X') => {
                        integer_kind.base = NumericBase::Hexadecimal;
                        end = self.consume_base_prefix(end);
                        integer_kind.empty_digits =
                            !self.consume_digits_with_underscores(&mut end, 16);
                    }
                    _ => {
                        self.consume_digits_with_underscores(&mut end, 10);
                    }
                }
            } else {
                self.consume_digits_with_underscores(&mut end, 10);
            }

            let mut is_float = false;
            if integer_kind.base == NumericBase::Decimal && self.peek_character() == Some('.') {
                let next_after_dot = self.peek_nth_character(1);
                if next_after_dot.is_some_and(|next| next.is_ascii_digit())
                    || next_after_dot.is_none()
                    || next_after_dot.is_some_and(|next| next != '.' && !is_identifier_start(next))
                {
                    is_float = true;
                    if let Some((dot_index, dot)) = self.chars.next() {
                        end = dot_index + dot.len_utf8();
                    }
                    self.consume_digits_with_underscores(&mut end, 10);
                }
            }

            if integer_kind.base == NumericBase::Decimal
                && matches!(self.peek_character(), Some('e') | Some('E'))
            {
                is_float = true;
                float_kind.has_exponent = true;
                if let Some((exp_index, exp)) = self.chars.next() {
                    end = exp_index + exp.len_utf8();
                }

                if matches!(self.peek_character(), Some('+') | Some('-'))
                    && let Some((sign_index, sign)) = self.chars.next()
                {
                    end = sign_index + sign.len_utf8();
                }

                float_kind.empty_exponent = !self.consume_digits_with_underscores(&mut end, 10);
            }

            let has_suffix = self.consume_literal_suffix(&mut end);

            LexedToken {
                kind: if is_float {
                    TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                        has_exponent: float_kind.has_exponent,
                        empty_exponent: float_kind.empty_exponent,
                        has_suffix,
                    }))
                } else {
                    TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                        base: integer_kind.base,
                        empty_digits: integer_kind.empty_digits,
                        has_suffix,
                    }))
                },
                span: Span::new(start, end),
            }
        }

        fn lex_leading_dot_float(&mut self, start: usize) -> LexedToken {
            let mut end = start + '.'.len_utf8();
            let mut float_kind = FloatLiteralKind::default();

            self.consume_digits_with_underscores(&mut end, 10);

            if matches!(self.peek_character(), Some('e') | Some('E')) {
                float_kind.has_exponent = true;
                if let Some((exp_index, exp)) = self.chars.next() {
                    end = exp_index + exp.len_utf8();
                }

                if matches!(self.peek_character(), Some('+') | Some('-'))
                    && let Some((sign_index, sign)) = self.chars.next()
                {
                    end = sign_index + sign.len_utf8();
                }

                float_kind.empty_exponent = !self.consume_digits_with_underscores(&mut end, 10);
            }

            float_kind.has_suffix = self.consume_literal_suffix(&mut end);

            LexedToken {
                kind: TokenKind::NumericLiteral(NumericLiteralKind::Float(float_kind)),
                span: Span::new(start, end),
            }
        }

        fn peek_nth_character(&self, offset: usize) -> Option<char> {
            let mut clone = self.chars.clone();
            for _ in 0..offset {
                clone.next()?;
            }
            clone.peek().map(|(_, character)| *character)
        }

        fn consume_base_prefix(&mut self, current_end: usize) -> usize {
            if let Some((prefix_index, prefix)) = self.chars.next() {
                prefix_index + prefix.len_utf8()
            } else {
                current_end
            }
        }

        fn consume_digits_with_underscores(&mut self, end: &mut usize, radix: u32) -> bool {
            let mut saw_digit = false;

            while let Some((next_index, next)) = self.chars.peek().copied() {
                if next == '_' {
                    *end = next_index + next.len_utf8();
                    self.chars.next();
                    continue;
                }

                if next.is_digit(radix) {
                    saw_digit = true;
                    *end = next_index + next.len_utf8();
                    self.chars.next();
                    continue;
                }

                break;
            }

            saw_digit
        }

        fn consume_literal_suffix(&mut self, end: &mut usize) -> bool {
            let Some(next) = self.peek_character() else {
                return false;
            };
            if !is_identifier_start(next) {
                return false;
            }

            let mut has_suffix = false;
            while let Some((next_index, next)) = self.chars.peek().copied() {
                if !is_identifier_continue(next) {
                    break;
                }

                has_suffix = true;
                *end = next_index + next.len_utf8();
                self.chars.next();
            }

            has_suffix
        }
    }
}
pub(crate) mod strings {
    use source::Span;
    use tokens::TokenKind;

    use crate::LexError;

    use super::LexedToken;

    pub(crate) mod decode {
        pub(crate) fn decode_string_literal(raw: &str, bytes: &mut Vec<u8>) {
            let contents = &raw.as_bytes()[1..raw.len() - 1];
            bytes.reserve(contents.len());
            let mut rest = contents;

            while let Some(escape) = rest.iter().position(|&byte| byte == b'\\') {
                bytes.extend_from_slice(&rest[..escape]);
                let (decoded, spelled_length) = match rest[escape + 1] {
                    b'"' => (b'"', 2),
                    b'\\' => (b'\\', 2),
                    b'n' => (b'\n', 2),
                    b'r' => (b'\r', 2),
                    b't' => (b'\t', 2),
                    b'0' => (b'\0', 2),
                    b'x' => {
                        let high = hex_digit_value(rest[escape + 2]);
                        let low = hex_digit_value(rest[escape + 3]);
                        ((high << 4) | low, 4)
                    }
                    other => {
                        unreachable!("validated string literal contains escape byte {other:#04x}")
                    }
                };
                bytes.push(decoded);
                rest = &rest[escape + spelled_length..];
            }
            bytes.extend_from_slice(rest);
        }

        fn hex_digit_value(digit: u8) -> u8 {
            match digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                b'A'..=b'F' => digit - b'A' + 10,
                other => unreachable!("validated hex escape contains byte {other:#04x}"),
            }
        }
    }

    pub(crate) use decode::decode_string_literal;

    impl<'source> super::Lexer<'source> {
        pub(crate) fn lex_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
            let end = self.lex_cooked_string_end(start)?;
            Ok(LexedToken {
                kind: TokenKind::StringLiteral,
                span: Span::new(start, end),
            })
        }

        fn lex_cooked_string_end(&mut self, start: usize) -> Result<usize, LexError> {
            while let Some((index, character)) = self.chars.next() {
                if character == '"' {
                    return Ok(index + character.len_utf8());
                }

                if matches!(character, '\r' | '\n') {
                    return Err(LexError::outside_lexical_profile(Span::new(
                        index,
                        index + character.len_utf8(),
                    )));
                }

                if character == '\\' {
                    let Some((escape_index, escaped)) = self.chars.next() else {
                        return Err(LexError::new(
                            "unterminated string escape",
                            Span::new(start, self.source.len()),
                        ));
                    };

                    match escaped {
                        '"' | '\\' | 'n' | 'r' | 't' | '0' => {}
                        'x' => self.consume_hex_escape(start)?,
                        'u' => {
                            return Err(LexError::outside_lexical_profile(Span::new(
                                index,
                                escape_index + escaped.len_utf8(),
                            )));
                        }
                        other => {
                            return Err(LexError::new(
                                format!("unsupported escape sequence `\\{other}`"),
                                Span::new(index, escape_index + other.len_utf8()),
                            ));
                        }
                    }
                }
            }

            Err(LexError::new(
                "unterminated string literal",
                Span::new(start, self.source.len()),
            ))
        }

        fn consume_hex_escape(&mut self, string_start: usize) -> Result<(), LexError> {
            for _ in 0..2 {
                let Some((digit_index, digit)) = self.chars.next() else {
                    return Err(LexError::new(
                        "unterminated hex escape",
                        Span::new(string_start, self.source.len()),
                    ));
                };
                if !digit.is_ascii_hexdigit() {
                    return Err(LexError::new(
                        "invalid hex escape digit",
                        Span::new(digit_index, digit_index + digit.len_utf8()),
                    ));
                }
            }

            Ok(())
        }
    }
}

pub struct Lexer<'source> {
    source: &'source str,
    chars: Peekable<CharIndices<'source>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LexedToken {
    kind: TokenKind,
    span: Span,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
        }
    }

    pub fn tokenize(self) -> Result<TokenStream<'source>, LexError> {
        let capacity = self.source.len().saturating_div(4).max(16);
        let mut tokens = TokenStream::with_capacity(self.source, capacity);
        self.fill_tokens(&mut tokens)?;
        Ok(tokens)
    }

    pub fn tokenize_reusing(
        self,
        scratch: &mut tokens::TokenScratch,
    ) -> Result<TokenStream<'source>, LexError> {
        let capacity = self.source.len().saturating_div(4).max(16);
        let mut tokens = TokenStream::with_scratch(self.source, capacity, scratch);
        match self.fill_tokens(&mut tokens) {
            Ok(()) => Ok(tokens),
            Err(error) => {
                tokens.recycle(scratch);
                Err(error)
            }
        }
    }

    fn fill_tokens(mut self, tokens: &mut TokenStream<'source>) -> Result<(), LexError> {
        while let Some(token) = self.lex_next_token()? {
            if token.kind == TokenKind::StringLiteral {
                let spelling = &self.source[token.span.start..token.span.end];
                tokens.push_decoded(token.span, |bytes| decode_string_literal(spelling, bytes));
            } else {
                tokens.push_source(token.kind, token.span);
            }
        }
        Ok(())
    }

    fn lex_next_token(&mut self) -> Result<Option<LexedToken>, LexError> {
        let Some((start, character)) = self.chars.next() else {
            return Ok(None);
        };
        let raw_string_prefix_end = (character == 'r')
            .then(|| self.raw_string_prefix_end(start))
            .flatten();

        let token = if is_syntactic_whitespace(character) {
            self.lex_whitespace(start, character)
        } else if character == '/' && self.peek_character() == Some('/') {
            self.lex_line_comment(start)
        } else if character == '/' && self.peek_character() == Some('*') {
            self.lex_block_comment(start)?
        } else if let Some(prefix_end) = raw_string_prefix_end {
            return Err(LexError::outside_lexical_profile(Span::new(
                start, prefix_end,
            )));
        } else if is_identifier_start(character) {
            self.lex_identifier_or_keyword(start, character)
        } else if character.is_ascii_digit()
            || (character == '.'
                && self
                    .peek_character()
                    .is_some_and(|next| next.is_ascii_digit()))
        {
            self.lex_number(start, character)
        } else if character == '"' {
            self.lex_string_token(start)?
        } else if !character.is_ascii() {
            return Err(LexError::outside_lexical_profile(Span::new(
                start,
                start + character.len_utf8(),
            )));
        } else {
            self.lex_punctuation(start, character)?
        };

        Ok(Some(token))
    }

    fn lex_whitespace(&mut self, start: usize, first: char) -> LexedToken {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if !is_syntactic_whitespace(next) {
                break;
            }

            end = next_index + next.len_utf8();
            self.chars.next();
        }

        LexedToken {
            kind: TokenKind::Whitespace,
            span: Span::new(start, end),
        }
    }

    fn lex_line_comment(&mut self, start: usize) -> LexedToken {
        let mut end = start + '/'.len_utf8();
        self.chars.next();
        end += '/'.len_utf8();

        for (_, character) in self.chars.by_ref() {
            if character == '\n' {
                break;
            }
            end += character.len_utf8();
        }

        LexedToken {
            kind: TokenKind::Comment(CommentKind::Line),
            span: Span::new(start, end),
        }
    }

    fn lex_block_comment(&mut self, start: usize) -> Result<LexedToken, LexError> {
        self.chars.next();
        let mut depth = 1usize;

        while let Some((index, character)) = self.chars.next() {
            match (character, self.peek_character()) {
                ('/', Some('*')) => {
                    self.chars.next();
                    depth += 1;
                }
                ('*', Some('/')) => {
                    self.chars.next();
                    depth -= 1;
                    if depth == 0 {
                        return Ok(LexedToken {
                            kind: TokenKind::Comment(CommentKind::Block),
                            span: Span::new(start, index + 2),
                        });
                    }
                }
                _ => {}
            }
        }

        Err(LexError::new(
            "unterminated block comment",
            Span::new(start, self.source.len()),
        ))
    }

    fn lex_identifier_or_keyword(&mut self, start: usize, first: char) -> LexedToken {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if is_identifier_continue(next) {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let lexeme = &self.source[start..end];
        let kind = KeywordKind::from_lexeme(lexeme)
            .map(TokenKind::Keyword)
            .unwrap_or(TokenKind::Identifier);

        LexedToken {
            kind,
            span: Span::new(start, end),
        }
    }

    fn lex_punctuation(&mut self, start: usize, first: char) -> Result<LexedToken, LexError> {
        let rest = &self.source.as_bytes()[start..];
        let Some((lexeme, kind)) =
            PunctuationKind::ordered_lexemes()
                .iter()
                .copied()
                .find(|(lexeme, _)| {
                    lexeme.as_bytes().first() == rest.first() && rest.starts_with(lexeme.as_bytes())
                })
        else {
            return Err(LexError::outside_lexical_profile(Span::new(
                start,
                start + first.len_utf8(),
            )));
        };

        let mut consumed = 1usize;
        while consumed < lexeme.chars().count() {
            self.chars.next();
            consumed += 1;
        }

        Ok(LexedToken {
            kind: TokenKind::Punctuation(kind),
            span: Span::new(start, start + lexeme.len()),
        })
    }

    fn peek_character(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, character)| *character)
    }

    fn raw_string_prefix_end(&self, start: usize) -> Option<usize> {
        let mut cursor = start + 1;
        if self.source.as_bytes().get(cursor) == Some(&b'"') {
            return Some(cursor + 1);
        }
        if self.source.as_bytes().get(cursor) != Some(&b'#') {
            return None;
        }
        while self.source.as_bytes().get(cursor) == Some(&b'#') {
            cursor += 1;
        }
        (self.source.as_bytes().get(cursor) == Some(&b'"')).then_some(cursor + 1)
    }
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn is_syntactic_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Lexer;
    use tokens::{
        CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase,
        NumericLiteralKind, TokenKind,
    };

    use crate::lexer::lex_error::OUTSIDE_LEXICAL_PROFILE_MESSAGE;

    fn semantic_kinds(source: &str) -> Vec<TokenKind> {
        Lexer::new(source)
            .tokenize()
            .expect("tokenization should succeed")
            .iter()
            .filter(|token| !token.is_non_semantic())
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn tokenizes_retired_invariant_word_as_identifier() {
        assert_eq!(semantic_kinds("invariant"), vec![TokenKind::Identifier]);
    }

    #[test]
    fn accepts_exact_ascii_identifier_grammar() {
        let tokens = Lexer::new("_ alpha Z9 snake_case")
            .tokenize()
            .expect("ASCII identifiers should tokenize");
        assert!(
            tokens
                .iter()
                .filter(|token| !token.is_non_semantic())
                .all(|token| token.kind == TokenKind::Identifier)
        );
    }

    #[test]
    fn rejects_non_ascii_identifier_spelling() {
        assert_profile_rejection("变量", 0, "变".len());
        assert_profile_rejection("café", 3, 5);
        assert_profile_rejection("μέτρο", 0, "μ".len());
        assert_profile_rejection("1é", 1, 3);
    }

    #[test]
    fn preserves_line_comments_and_whitespace() {
        let tokens = Lexer::new("let // comment\n value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert_eq!(tokens[1].kind, TokenKind::Whitespace);
        assert!(matches!(
            tokens[2].kind,
            TokenKind::Comment(CommentKind::Line)
        ));
        assert_eq!(tokens[3].kind, TokenKind::Whitespace);
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens.text(&tokens[4]).as_str(), "value");
    }

    #[test]
    fn accepts_only_the_closed_syntactic_whitespace_set() {
        let tokens = Lexer::new("a \t\r\nb")
            .tokenize()
            .expect("space, tab, CR, and LF should tokenize");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[1].kind, TokenKind::Whitespace);
        assert_eq!(tokens.text(&tokens[1]).as_str(), " \t\r\n");

        for source in [
            "a\u{000b}b",
            "a\u{000c}b",
            "a\u{00a0}b",
            "a\u{2028}b",
            "a\u{3000}b",
        ] {
            let character = source[1..].chars().next().expect("test character");
            assert_profile_rejection(source, 1, 1 + character.len_utf8());
        }
    }

    #[test]
    fn preserves_nested_block_comments() {
        let tokens = Lexer::new("let /* outer /* inner */ still outer */ value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert_eq!(tokens[1].kind, TokenKind::Whitespace);
        assert!(matches!(
            tokens[2].kind,
            TokenKind::Comment(CommentKind::Block)
        ));
        assert_eq!(tokens[3].kind, TokenKind::Whitespace);
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens.text(&tokens[4]).as_str(), "value");
    }

    #[test]
    fn preserves_non_ascii_comment_and_literal_body_bytes() {
        let source = "// café 变量\n/* μέτρο */ \"café😀\"";
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("non-ASCII payload bytes should remain legal");

        assert_eq!(
            tokens.text(&tokens[0]).as_bytes(),
            "// café 变量".as_bytes()
        );
        assert_eq!(tokens.text(&tokens[1]).as_bytes(), "/* μέτρο */".as_bytes());
        assert_eq!(tokens[3].kind, TokenKind::StringLiteral);
        assert_eq!(tokens.text(&tokens[3]).as_bytes(), "café😀".as_bytes());
    }

    #[test]
    fn errors_on_unterminated_block_comment() {
        let error = Lexer::new("let /* comment")
            .tokenize()
            .expect_err("tokenization should fail");

        assert_eq!(error.message, "unterminated block comment");
    }

    #[test]
    fn tokenizes_cooked_byte_strings() {
        let cooked = Lexer::new("\"line\\nvalue\"")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(cooked[0].kind, TokenKind::StringLiteral);
        assert_eq!(cooked.text(&cooked[0]).as_str(), "line\nvalue");
    }

    #[test]
    fn string_decoding_copies_text_runs_between_every_escape_form() {
        let tokens = Lexer::new(r#""" "héllo \"q\" \\ \t\r\0\x4A\x4a ünd""#)
            .tokenize()
            .expect("tokenization should succeed");
        let literals: Vec<_> = tokens
            .iter()
            .filter(|token| token.is_string_literal())
            .map(|token| tokens.text(token).as_bytes().to_vec())
            .collect();

        assert_eq!(
            literals,
            [
                Vec::new(),
                "héllo \"q\" \\ \t\r\0JJ ünd".as_bytes().to_vec()
            ]
        );
    }

    #[test]
    fn hex_escapes_decode_to_exact_bytes() {
        let mut source = String::from("\"");
        for byte in u8::MIN..=u8::MAX {
            source.push_str(&format!("\\x{byte:02x}"));
        }
        source.push('"');

        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenization should succeed");
        let expected: Vec<_> = (u8::MIN..=u8::MAX).collect();

        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens.text(&tokens[0]).as_bytes(), expected);
        assert_eq!(tokens.text(&tokens[0]).try_as_str(), None);
    }

    #[test]
    fn rejects_retired_codepoint_escapes_and_raw_strings() {
        for source in [r#""\u{1f600}""#, r#""\u""#, r#""\u{}""#] {
            assert_profile_rejection(source, 1, 3);
        }

        assert_profile_rejection("r\"raw\"", 0, 2);
        assert_profile_rejection("r##\"raw\"##", 0, 4);
    }

    #[test]
    fn raw_prefix_fragments_remain_ordinary_ascii_tokens() {
        let tokens = Lexer::new("r#1 r###candidate")
            .tokenize()
            .expect("incomplete raw candidates are not reserved V1 syntax");
        assert_eq!(
            tokens
                .iter()
                .filter(|token| !token.is_non_semantic())
                .map(|token| tokens.text(token).as_str())
                .collect::<Vec<_>>(),
            ["r", "#", "1", "r", "#", "#", "#", "candidate"]
        );
    }

    #[test]
    fn rejects_raw_newlines_in_quoted_literals() {
        assert_profile_rejection("\"line\nvalue\"", 5, 6);
        assert_profile_rejection("\"line\rvalue\"", 5, 6);
    }

    #[test]
    fn rejects_malformed_hex_escapes() {
        for (source, expected) in [
            (r#""\xG0""#, "invalid hex escape digit"),
            (r#""\x0""#, "invalid hex escape digit"),
            (r#""\x"#, "unterminated hex escape"),
        ] {
            let error = Lexer::new(source)
                .tokenize()
                .expect_err("tokenization should fail");
            assert_eq!(error.message, expected, "source: {source:?}");
        }
    }

    #[test]
    fn tokenizes_integer_and_float_literals_with_metadata() {
        let kinds = semantic_kinds("42 3.14 1. .5 1e10 0xff 0b1010 1_000");

        assert_eq!(
            kinds,
            vec![
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                    base: NumericBase::Decimal,
                    empty_digits: false,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                    has_exponent: false,
                    empty_exponent: false,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                    has_exponent: false,
                    empty_exponent: false,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                    has_exponent: false,
                    empty_exponent: false,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                    has_exponent: true,
                    empty_exponent: false,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                    base: NumericBase::Hexadecimal,
                    empty_digits: false,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                    base: NumericBase::Binary,
                    empty_digits: false,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                    base: NumericBase::Decimal,
                    empty_digits: false,
                    has_suffix: false,
                })),
            ]
        );
    }

    #[test]
    fn captures_numeric_suffixes_and_empty_parts() {
        let kinds = semantic_kinds("123abc 1e 0x");

        assert_eq!(
            kinds,
            vec![
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                    base: NumericBase::Decimal,
                    empty_digits: false,
                    has_suffix: true,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                    has_exponent: true,
                    empty_exponent: true,
                    has_suffix: false,
                })),
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                    base: NumericBase::Hexadecimal,
                    empty_digits: true,
                    has_suffix: false,
                })),
            ]
        );
    }

    fn assert_profile_rejection(source: &str, expected_start: usize, expected_end: usize) {
        let error = Lexer::new(source)
            .tokenize()
            .expect_err("spelling outside Lexical Profile V1 should reject");
        assert_eq!(
            error.message, OUTSIDE_LEXICAL_PROFILE_MESSAGE,
            "source: {source:?}"
        );
        assert_eq!(error.span.start, expected_start, "source: {source:?}");
        assert_eq!(error.span.end, expected_end, "source: {source:?}");
    }
}
