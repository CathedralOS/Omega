use crate::parse_error::ParseError;
use crate::parser::diagnostics;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::source::{SourceId, SourceSpan, SourceText};
use omega_syntax_trees::identifier::Identifier;
use omega_tokens::{
    FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase, PunctuationKind, Token,
    TokenKind,
};

pub(super) type ParseResult<'tokens, 'source, T> = Result<(T, Input<'tokens, 'source>), ParseError>;

#[derive(Clone, Copy)]
pub(super) struct Input<'tokens, 'source> {
    pub(super) source_id: SourceId,
    pub(super) tokens: &'tokens [Token<'source>],
}

impl<'tokens, 'source> Input<'tokens, 'source> {
    pub(super) fn new(source_id: SourceId, tokens: &'tokens [Token<'source>]) -> Self {
        Self {
            source_id,
            tokens: skip_non_semantic_tokens(tokens),
        }
    }

    pub(super) fn source_span(&self, token: &Token<'_>) -> SourceSpan {
        SourceSpan::new(self.source_id, token.span)
    }

    pub(super) fn error_here(&self, message: impl Into<String>) -> ParseError {
        let source_span = self
            .tokens
            .first()
            .map(|token| self.source_span(token))
            .unwrap_or_default();
        ParseError::at_source_span(message, source_span)
    }

    pub(super) fn expect_token(self) -> Result<(&'tokens Token<'source>, Self), ParseError> {
        match self.tokens.split_first() {
            Some((token, rest)) => Ok((token, Self::new(self.source_id, rest))),
            None => Err(diagnostics::unexpected_eof(self, "token")),
        }
    }

    pub(super) fn take_keyword(
        self,
        keyword: KeywordKind,
        label: &str,
    ) -> Result<Self, ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.keyword() == Some(keyword) {
            Ok(rest)
        } else {
            Err(diagnostics::expected(self, token, format!("`{label}`")))
        }
    }

    pub(super) fn take_punctuation(
        self,
        punctuation: PunctuationKind,
        label: &str,
    ) -> Result<Self, ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.punctuation() == Some(punctuation) {
            Ok(rest)
        } else {
            Err(diagnostics::expected(self, token, format!("`{label}`")))
        }
    }

    pub(super) fn take_contextual(self, name: &str) -> Result<Self, ParseError> {
        let (token, rest) = self.expect_token()?;
        if matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
            && token.lexeme.as_str() == name
        {
            Ok(rest)
        } else {
            Err(diagnostics::expected(self, token, format!("`{name}`")))
        }
    }

    pub(super) fn take_identifier(self) -> Result<(Identifier, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if is_identifier_token(token) {
            Ok((
                Identifier::new(token.lexeme.as_str(), self.source_span(token)),
                rest,
            ))
        } else {
            Err(diagnostics::expected(self, token, "identifier"))
        }
    }

    pub(super) fn take_integer(self) -> Result<(i64, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let value = parse_integer_literal(token.lexeme.as_str(), kind)
                .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
            Ok((value, rest))
        } else {
            Err(diagnostics::expected(self, token, "integer literal"))
        }
    }

    pub(super) fn take_string(self) -> Result<(String, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.is_string_literal() {
            Ok((token.lexeme.as_str().trim_matches('"').to_owned(), rest))
        } else {
            Err(diagnostics::expected(self, token, "string literal"))
        }
    }

    pub(super) fn take_float_text(self) -> Result<(SourceText, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.float_literal_kind() {
            validate_float_literal(kind)
                .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
            Ok((
                SourceText::new(token.lexeme.as_str(), self.source_span(token)),
                rest,
            ))
        } else {
            Err(diagnostics::expected(self, token, "float literal"))
        }
    }

    pub(super) fn expected_one_of_here(self, expected: &[&str]) -> ParseError {
        diagnostics::expected_one_of_here(self, expected)
    }

    pub(super) fn at_keyword(&self, keyword: KeywordKind) -> bool {
        self.tokens.first().and_then(Token::keyword) == Some(keyword)
    }

    pub(super) fn at_punctuation(&self, punctuation: PunctuationKind) -> bool {
        self.tokens.first().and_then(Token::punctuation) == Some(punctuation)
    }

    pub(super) fn at_contextual(&self, name: &str) -> bool {
        self.tokens.first().is_some_and(|token| {
            matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && token.lexeme.as_str() == name
        })
    }

    pub(super) fn at_name_like(&self) -> bool {
        self.tokens.first().is_some_and(is_identifier_token)
    }

    pub(super) fn split_at_top_level_punctuation(
        self,
        delimiter: PunctuationKind,
        message: &str,
    ) -> Result<(Self, Self), ParseError> {
        let split_index = self
            .find_top_level_punctuation(delimiter)
            .ok_or_else(|| self.error_here(message))?;
        let (prefix_tokens, rest_tokens) = self.tokens.split_at(split_index);
        Ok((
            Self::new(self.source_id, prefix_tokens),
            Self::new(self.source_id, rest_tokens),
        ))
    }

    pub(super) fn skip_braced_block(self) -> Result<(usize, Self), ParseError> {
        let mut input = self.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let mut depth = 1usize;
        let mut token_count = 0usize;

        while depth > 0 {
            let (token, rest) = input.expect_token()?;
            input = rest;

            match token.punctuation() {
                Some(PunctuationKind::LeftBrace) => depth += 1,
                Some(PunctuationKind::RightBrace) => depth -= 1,
                _ => {}
            }

            if depth > 0 {
                token_count += 1;
            }
        }

        Ok((token_count, input))
    }

    pub(super) fn skip_parenthesized_tokens_after_open(self) -> Result<(usize, Self), ParseError> {
        let mut input = self;
        let mut depth = 1usize;
        let mut token_count = 0usize;

        while depth > 0 {
            let (token, rest) = input.expect_token()?;
            input = rest;

            match token.punctuation() {
                Some(PunctuationKind::LeftParen) => depth += 1,
                Some(PunctuationKind::RightParen) => depth -= 1,
                _ => {}
            }

            if depth > 0 {
                token_count += 1;
            }
        }

        Ok((token_count, input))
    }

    pub(super) fn skip_bracketed_block(self) -> Result<(usize, Self), ParseError> {
        let mut input = self.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let mut depth = 1usize;
        let mut token_count = 0usize;

        while depth > 0 {
            let (token, rest) = input.expect_token()?;
            input = rest;

            match token.punctuation() {
                Some(PunctuationKind::LeftBracket) => depth += 1,
                Some(PunctuationKind::RightBracket) => depth -= 1,
                _ => {}
            }

            if depth > 0 {
                token_count += 1;
            }
        }

        Ok((token_count, input))
    }

    fn find_top_level_punctuation(self, delimiter: PunctuationKind) -> Option<usize> {
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut brace_depth = 0usize;

        for (index, token) in self.tokens.iter().enumerate() {
            match token.punctuation() {
                Some(PunctuationKind::LeftParen) => paren_depth += 1,
                Some(PunctuationKind::RightParen) => paren_depth = paren_depth.saturating_sub(1),
                Some(PunctuationKind::LeftBracket) => bracket_depth += 1,
                Some(PunctuationKind::RightBracket) => {
                    bracket_depth = bracket_depth.saturating_sub(1)
                }
                Some(PunctuationKind::LeftBrace) => {
                    if delimiter == PunctuationKind::LeftBrace
                        && paren_depth == 0
                        && bracket_depth == 0
                        && brace_depth == 0
                    {
                        return Some(index);
                    }
                    brace_depth += 1;
                }
                Some(PunctuationKind::RightBrace) => brace_depth = brace_depth.saturating_sub(1),
                Some(punctuation)
                    if punctuation == delimiter
                        && paren_depth == 0
                        && bracket_depth == 0
                        && brace_depth == 0 =>
                {
                    return Some(index);
                }
                _ => {}
            }
        }

        None
    }
}

fn is_identifier_token(token: &Token<'_>) -> bool {
    match token.kind {
        TokenKind::Identifier => true,
        TokenKind::Keyword(keyword) => !keyword.is_strict_identifier_keyword(),
        _ => false,
    }
}

fn skip_non_semantic_tokens<'tokens, 'source>(
    tokens: &'tokens [Token<'source>],
) -> &'tokens [Token<'source>] {
    let mut index = 0usize;
    while index < tokens.len() && tokens[index].is_non_semantic() {
        index += 1;
    }
    &tokens[index..]
}

fn parse_integer_literal(text: &str, kind: IntegerLiteralKind) -> Result<i64, &'static str> {
    if kind.empty_digits {
        return Err("invalid integer literal");
    }
    if kind.has_suffix {
        return Err("integer literal suffixes are not supported yet");
    }

    let (radix, body) = match kind.base {
        NumericBase::Binary => (
            2,
            text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")),
        ),
        NumericBase::Octal => (
            8,
            text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")),
        ),
        NumericBase::Decimal => (10, Some(text)),
        NumericBase::Hexadecimal => (
            16,
            text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")),
        ),
    };

    let body = body.ok_or("invalid integer literal")?;
    let normalized: String = body.chars().filter(|character| *character != '_').collect();
    i64::from_str_radix(&normalized, radix).map_err(|_| "invalid integer literal")
}

fn validate_float_literal(kind: FloatLiteralKind) -> Result<(), &'static str> {
    if kind.empty_exponent {
        return Err("invalid float literal");
    }
    if kind.has_suffix {
        return Err("float literal suffixes are not supported yet");
    }

    Ok(())
}

pub(super) fn parse_path_handle_span<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    mut append_member: impl FnMut(Identifier) -> Handle<Identifier>,
) -> ParseResult<'tokens, 'source, HandleSpan<Identifier>> {
    let (first, mut rest) = input.take_identifier()?;
    let start = append_member(first);
    let mut count = 1u32;

    while rest.at_punctuation(PunctuationKind::ColonColon) {
        rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = rest.take_identifier()?;
        append_member(member);
        count = count
            .checked_add(1)
            .expect("identifier path member span count overflow");
        rest = next;
    }

    Ok((HandleSpan::from_parts(start, count), rest))
}
