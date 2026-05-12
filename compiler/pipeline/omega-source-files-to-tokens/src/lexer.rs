use std::iter::Peekable;
use std::str::CharIndices;

use crate::LexError;
use omega_core::Span;
use omega_tokens::{
    CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase,
    NumericLiteralKind, PunctuationKind, Token, TokenKind, TokenStream, TokenText,
    WhitespaceKind,
};
use unicode_ident::{is_xid_continue, is_xid_start};

pub struct Lexer<'source> {
    source: &'source str,
    chars: Peekable<CharIndices<'source>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LexedToken {
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

    pub fn tokenize(mut self) -> Result<TokenStream<'source>, LexError> {
        let mut tokens = Vec::new();

        while let Some(token) = self.lex_next_token()? {
            tokens.push(self.build_token(token)?);
        }

        Ok(TokenStream::new(tokens))
    }

    fn lex_next_token(&mut self) -> Result<Option<LexedToken>, LexError> {
        let Some((start, character)) = self.chars.next() else {
            return Ok(None);
        };

        let token = if character.is_whitespace() {
            self.lex_whitespace(start, character)
        } else if character == '/' && self.peek_character() == Some('/') {
            self.lex_line_comment(start)
        } else if character == '/' && self.peek_character() == Some('*') {
            self.lex_block_comment(start)?
        } else if character == 'r' && matches!(self.peek_character(), Some('"') | Some('#')) {
            self.lex_raw_string_token(start)?
        } else if is_identifier_start(character) {
            self.lex_identifier_or_keyword(start, character)
        } else if character.is_ascii_digit()
            || (character == '.'
                && self.peek_character()
                    .is_some_and(|next| next.is_ascii_digit()))
        {
            self.lex_number(start, character)
        } else if character == '"' {
            self.lex_string_token(start)?
        } else {
            self.lex_punctuation(start, character)?
        };

        Ok(Some(token))
    }

    fn build_token(&self, token: LexedToken) -> Result<Token<'source>, LexError> {
        let lexeme = match token.kind {
            TokenKind::StringLiteral => {
                let raw = &self.source[token.span.start..token.span.end];
                let value = self.decode_string_literal(raw, token.span.start)?;
                TokenText::owned(value)
            }
            _ => TokenText::source(&self.source[token.span.start..token.span.end]),
        };

        Ok(Token {
            kind: token.kind,
            lexeme,
            span: token.span,
        })
    }

    fn lex_whitespace(&mut self, start: usize, first: char) -> LexedToken {
        let mut end = start + first.len_utf8();
        let mut saw_newline = first == '\n' || first == '\r';
        let mut saw_other = !saw_newline;

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if !next.is_whitespace() {
                break;
            }

            saw_newline |= next == '\n' || next == '\r';
            saw_other |= next != '\n' && next != '\r';
            end = next_index + next.len_utf8();
            self.chars.next();
        }

        let kind = match (saw_newline, saw_other) {
            (true, false) => WhitespaceKind::Newline,
            (false, true) => WhitespaceKind::Space,
            _ => WhitespaceKind::Mixed,
        };

        LexedToken {
            kind: TokenKind::Whitespace(kind),
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

    fn lex_number(&mut self, start: usize, first: char) -> LexedToken {
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
                    integer_kind.empty_digits = !self.consume_digits_with_underscores(&mut end, 2);
                }
                Some('o') | Some('O') => {
                    integer_kind.base = NumericBase::Octal;
                    end = self.consume_base_prefix(end);
                    integer_kind.empty_digits = !self.consume_digits_with_underscores(&mut end, 8);
                }
                Some('x') | Some('X') => {
                    integer_kind.base = NumericBase::Hexadecimal;
                    end = self.consume_base_prefix(end);
                    integer_kind.empty_digits = !self.consume_digits_with_underscores(&mut end, 16);
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
                || next_after_dot.is_some_and(|next| !is_identifier_start(next))
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

    fn lex_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_cooked_string_end(start)?;
        self.decode_string_literal(&self.source[start..end], start)?;
        Ok(LexedToken {
            kind: TokenKind::StringLiteral,
            span: Span::new(start, end),
        })
    }

    fn lex_raw_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_raw_string_end(start)?;
        self.decode_string_literal(&self.source[start..end], start)?;
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

            if character == '\\' {
                let Some((escape_index, escaped)) = self.chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(start, self.source.len()),
                    ));
                };

                match escaped {
                    '"' | '\'' | '\\' | 'n' | 'r' | 't' | '0' => {}
                    'x' => self.consume_hex_escape(start)?,
                    'u' => self.consume_unicode_escape(start, escape_index)?,
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

    fn lex_raw_string_end(&mut self, start: usize) -> Result<usize, LexError> {
        let mut hash_count = 0usize;

        while self.peek_character() == Some('#') {
            self.chars.next();
            hash_count += 1;
        }

        if self.peek_character() != Some('"') {
            return Err(LexError::new(
                "invalid raw string delimiter",
                Span::new(start, self.source.len().min(start + 2 + hash_count)),
            ));
        }
        self.chars.next();

        while let Some((index, character)) = self.chars.next() {
            if character != '"' {
                continue;
            }

            let mut clone = self.chars.clone();
            let mut matched = true;
            let mut end = index + character.len_utf8();

            for _ in 0..hash_count {
                match clone.next() {
                    Some((hash_index, '#')) => {
                        end = hash_index + '#'.len_utf8();
                    }
                    _ => {
                        matched = false;
                        break;
                    }
                }
            }

            if matched {
                for _ in 0..hash_count {
                    self.chars.next();
                }
                return Ok(end);
            }
        }

        Err(LexError::new(
            "unterminated raw string literal",
            Span::new(start, self.source.len()),
        ))
    }

    fn decode_string_literal(&self, raw: &str, source_start: usize) -> Result<String, LexError> {
        if raw.starts_with('r') {
            return Ok(self.decode_raw_string_literal(raw));
        }

        self.decode_cooked_string_literal(raw, source_start)
    }

    fn decode_cooked_string_literal(
        &self,
        raw: &str,
        source_start: usize,
    ) -> Result<String, LexError> {
        let mut lexeme = String::new();
        let mut chars = raw.char_indices();

        let Some((_, opening_quote)) = chars.next() else {
            return Ok(lexeme);
        };
        debug_assert_eq!(opening_quote, '"');

        while let Some((index, character)) = chars.next() {
            if character == '"' {
                return Ok(lexeme);
            }

            if character == '\\' {
                let Some((escape_index, escaped)) = chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(source_start, source_start + raw.len()),
                    ));
                };

                match escaped {
                    '"' => lexeme.push('"'),
                    '\'' => lexeme.push('\''),
                    '\\' => lexeme.push('\\'),
                    'n' => lexeme.push('\n'),
                    'r' => lexeme.push('\r'),
                    't' => lexeme.push('\t'),
                    '0' => lexeme.push('\0'),
                    'x' => lexeme.push(self.decode_hex_escape(&mut chars, source_start + index)?),
                    'u' => {
                        lexeme.push(self.decode_unicode_escape(&mut chars, source_start + index)?)
                    }
                    other => {
                        return Err(LexError::new(
                            format!("unsupported escape sequence `\\{other}`"),
                            Span::new(
                                source_start + index,
                                source_start + escape_index + other.len_utf8(),
                            ),
                        ));
                    }
                }
            } else {
                lexeme.push(character);
            }
        }

        Err(LexError::new(
            "unterminated string literal",
            Span::new(source_start, source_start + raw.len()),
        ))
    }

    fn decode_raw_string_literal(&self, raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut hash_count = 0usize;

        while 1 + hash_count < bytes.len() && bytes[1 + hash_count] == b'#' {
            hash_count += 1;
        }

        let content_start = 1 + hash_count + 1;
        let content_end = raw.len() - hash_count - 1;
        raw[content_start..content_end].to_owned()
    }

    fn lex_punctuation(&mut self, start: usize, first: char) -> Result<LexedToken, LexError> {
        let Some((lexeme, kind)) = PunctuationKind::ordered_lexemes()
            .iter()
            .copied()
            .find(|(lexeme, _)| self.source[start..].starts_with(lexeme))
        else {
            return Err(LexError::new(
                format!("unsupported punctuation `{first}`"),
                Span::new(start, start + first.len_utf8()),
            ));
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

    fn consume_unicode_escape(
        &mut self,
        string_start: usize,
        escape_index: usize,
    ) -> Result<(), LexError> {
        let Some((brace_index, brace)) = self.chars.next() else {
            return Err(LexError::new(
                "unterminated unicode escape",
                Span::new(string_start, self.source.len()),
            ));
        };
        if brace != '{' {
            return Err(LexError::new(
                "expected `{` after `\\u`",
                Span::new(brace_index, brace_index + brace.len_utf8()),
            ));
        }

        let mut saw_digit = false;
        while let Some((digit_index, digit)) = self.chars.next() {
            if digit == '}' {
                return if saw_digit {
                    Ok(())
                } else {
                    Err(LexError::new(
                        "empty unicode escape",
                        Span::new(escape_index, digit_index + digit.len_utf8()),
                    ))
                };
            }

            if !digit.is_ascii_hexdigit() {
                return Err(LexError::new(
                    "invalid unicode escape digit",
                    Span::new(digit_index, digit_index + digit.len_utf8()),
                ));
            }

            saw_digit = true;
        }

        Err(LexError::new(
            "unterminated unicode escape",
            Span::new(string_start, self.source.len()),
        ))
    }

    fn decode_hex_escape(
        &self,
        chars: &mut CharIndices<'_>,
        source_index: usize,
    ) -> Result<char, LexError> {
        let mut value = 0u8;
        for _ in 0..2 {
            let Some((offset, digit)) = chars.next() else {
                return Err(LexError::new(
                    "unterminated hex escape",
                    Span::new(source_index, source_index + 2),
                ));
            };
            let Some(parsed) = digit.to_digit(16) else {
                return Err(LexError::new(
                    "invalid hex escape digit",
                    Span::new(source_index + offset, source_index + offset + digit.len_utf8()),
                ));
            };
            value = (value << 4) | parsed as u8;
        }

        Ok(value as char)
    }

    fn decode_unicode_escape(
        &self,
        chars: &mut CharIndices<'_>,
        source_index: usize,
    ) -> Result<char, LexError> {
        let Some((brace_offset, brace)) = chars.next() else {
            return Err(LexError::new(
                "unterminated unicode escape",
                Span::new(source_index, source_index + 2),
            ));
        };
        if brace != '{' {
            return Err(LexError::new(
                "expected `{` after `\\u`",
                Span::new(
                    source_index + brace_offset,
                    source_index + brace_offset + brace.len_utf8(),
                ),
            ));
        }

        let mut digits = String::new();
        while let Some((offset, digit)) = chars.next() {
            if digit == '}' {
                if digits.is_empty() {
                    return Err(LexError::new(
                        "empty unicode escape",
                        Span::new(source_index, source_index + offset + digit.len_utf8()),
                    ));
                }

                let value = u32::from_str_radix(&digits, 16).map_err(|_| {
                    LexError::new(
                        "invalid unicode escape value",
                        Span::new(source_index, source_index + offset + digit.len_utf8()),
                    )
                })?;

                return char::from_u32(value).ok_or_else(|| {
                    LexError::new(
                        "unicode escape is not a valid scalar value",
                        Span::new(source_index, source_index + offset + digit.len_utf8()),
                    )
                });
            }

            if !digit.is_ascii_hexdigit() {
                return Err(LexError::new(
                    "invalid unicode escape digit",
                    Span::new(source_index + offset, source_index + offset + digit.len_utf8()),
                ));
            }

            digits.push(digit);
        }

        Err(LexError::new(
            "unterminated unicode escape",
            Span::new(source_index, source_index + 2 + digits.len()),
        ))
    }
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || is_xid_start(character)
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || is_xid_continue(character)
}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use omega_tokens::{
        CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase,
        NumericLiteralKind, PunctuationKind, TokenKind,
    };

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
    fn tokenizes_keywords_and_identifiers_distinctly() {
        assert_eq!(
            semantic_kinds("machine game entry self true false custom"),
            vec![
                TokenKind::Keyword(KeywordKind::Machine),
                TokenKind::Identifier,
                TokenKind::Keyword(KeywordKind::Entry),
                TokenKind::Keyword(KeywordKind::SelfValue),
                TokenKind::Keyword(KeywordKind::True),
                TokenKind::Keyword(KeywordKind::False),
                TokenKind::Identifier,
            ]
        );
    }

    #[test]
    fn tokenizes_unicode_identifiers() {
        let tokens = Lexer::new("变量 café μέτρο")
            .tokenize()
            .expect("tokenization should succeed");

        let semantic: Vec<_> = tokens
            .iter()
            .filter(|token| !token.is_non_semantic())
            .collect();

        assert_eq!(semantic.len(), 3);
        assert!(semantic.iter().all(|token| token.kind == TokenKind::Identifier));
        assert_eq!(semantic[0].lexeme.as_str(), "变量");
        assert_eq!(semantic[1].lexeme.as_str(), "café");
        assert_eq!(semantic[2].lexeme.as_str(), "μέτρο");
    }

    #[test]
    fn tokenizes_multi_character_punctuation() {
        assert_eq!(
            semantic_kinds(":: -> == != << <= >> >= && ||"),
            vec![
                TokenKind::Punctuation(PunctuationKind::ColonColon),
                TokenKind::Punctuation(PunctuationKind::Arrow),
                TokenKind::Punctuation(PunctuationKind::EqualEqual),
                TokenKind::Punctuation(PunctuationKind::ExclamationEqual),
                TokenKind::Punctuation(PunctuationKind::LessLess),
                TokenKind::Punctuation(PunctuationKind::LessEqual),
                TokenKind::Punctuation(PunctuationKind::GreaterGreater),
                TokenKind::Punctuation(PunctuationKind::GreaterEqual),
                TokenKind::Punctuation(PunctuationKind::AndAnd),
                TokenKind::Punctuation(PunctuationKind::PipePipe),
            ]
        );
    }

    #[test]
    fn preserves_line_comments_and_whitespace() {
        let tokens = Lexer::new("let // comment\n value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert!(matches!(tokens[1].kind, TokenKind::Whitespace(_)));
        assert!(matches!(tokens[2].kind, TokenKind::Comment(CommentKind::Line)));
        assert!(matches!(tokens[3].kind, TokenKind::Whitespace(_)));
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens[4].lexeme.as_str(), "value");
    }

    #[test]
    fn preserves_nested_block_comments() {
        let tokens = Lexer::new("let /* outer /* inner */ still outer */ value")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::Let));
        assert!(matches!(tokens[1].kind, TokenKind::Whitespace(_)));
        assert!(matches!(tokens[2].kind, TokenKind::Comment(CommentKind::Block)));
        assert!(matches!(tokens[3].kind, TokenKind::Whitespace(_)));
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens[4].lexeme.as_str(), "value");
    }

    #[test]
    fn errors_on_unterminated_block_comment() {
        let error = Lexer::new("let /* comment")
            .tokenize()
            .expect_err("tokenization should fail");

        assert_eq!(error.message, "unterminated block comment");
    }

    #[test]
    fn tokenizes_cooked_and_raw_strings() {
        let cooked = Lexer::new("\"line\\nvalue\"")
            .tokenize()
            .expect("tokenization should succeed");
        let raw = Lexer::new("r#\"line\\nvalue\"#")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(cooked[0].kind, TokenKind::StringLiteral);
        assert_eq!(cooked[0].lexeme.as_str(), "line\nvalue");
        assert_eq!(raw[0].kind, TokenKind::StringLiteral);
        assert_eq!(raw[0].lexeme.as_str(), "line\\nvalue");
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
}
