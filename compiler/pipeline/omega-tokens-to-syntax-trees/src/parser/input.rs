use crate::parse_error::ParseError;
use omega_core::source::{SourceId, SourceSpan, SourceText};
use omega_syntax_trees::identifier::{Identifier, IdentifierPath};
use omega_tokens::{
    FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase, PunctuationKind, Token,
    TokenKind,
};

pub(super) type ParseResult<'tokens, 'source, T> =
    Result<(T, Input<'tokens, 'source>), ParseError>;

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
            None => Err(self.error_here("unexpected EOF")),
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
            Err(ParseError::at_source_span(
                format!("expected `{label}`"),
                self.source_span(token),
            ))
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
            Err(ParseError::at_source_span(
                format!("expected `{label}`"),
                self.source_span(token),
            ))
        }
    }

    pub(super) fn take_contextual(self, name: &str) -> Result<Self, ParseError> {
        let (token, rest) = self.expect_token()?;
        if matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
            && token.lexeme.as_str() == name
        {
            Ok(rest)
        } else {
            Err(ParseError::at_source_span(
                format!("expected `{name}`"),
                self.source_span(token),
            ))
        }
    }

    pub(super) fn take_identifier(self) -> Result<(Identifier, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_)) {
            Ok((
                Identifier::new(token.lexeme.as_str(), self.source_span(token)),
                rest,
            ))
        } else {
            Err(ParseError::at_source_span(
                "expected identifier",
                self.source_span(token),
            ))
        }
    }

    pub(super) fn take_integer(self) -> Result<(i64, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let value = parse_integer_literal(token.lexeme.as_str(), kind).map_err(|message| {
                ParseError::at_source_span(message, self.source_span(token))
            })?;
            Ok((value, rest))
        } else {
            Err(ParseError::at_source_span(
                "expected integer literal",
                self.source_span(token),
            ))
        }
    }

    pub(super) fn take_string(self) -> Result<(String, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.is_string_literal() {
            Ok((token.lexeme.as_str().trim_matches('"').to_owned(), rest))
        } else {
            Err(ParseError::at_source_span(
                "expected string literal",
                self.source_span(token),
            ))
        }
    }

    pub(super) fn take_float_text(self) -> Result<(SourceText, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.float_literal_kind() {
            validate_float_literal(kind).map_err(|message| {
                ParseError::at_source_span(message, self.source_span(token))
            })?;
            Ok((SourceText::new(token.lexeme.as_str(), self.source_span(token)), rest))
        } else {
            Err(ParseError::at_source_span(
                "expected float literal",
                self.source_span(token),
            ))
        }
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
        self.tokens
            .first()
            .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_)))
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

fn parse_integer_literal(
    text: &str,
    kind: IntegerLiteralKind,
) -> Result<i64, &'static str> {
    if kind.empty_digits {
        return Err("invalid integer literal");
    }
    if kind.has_suffix {
        return Err("integer literal suffixes are not supported yet");
    }

    let (radix, body) = match kind.base {
        NumericBase::Binary => (2, text.strip_prefix("0b").or_else(|| text.strip_prefix("0B"))),
        NumericBase::Octal => (8, text.strip_prefix("0o").or_else(|| text.strip_prefix("0O"))),
        NumericBase::Decimal => (10, Some(text)),
        NumericBase::Hexadecimal => {
            (16, text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")))
        }
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

pub(super) fn parse_path<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, IdentifierPath> {
    let (first, mut rest) = input.take_identifier()?;
    let mut members = vec![first];

    while rest.at_punctuation(PunctuationKind::ColonColon) {
        rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = rest.take_identifier()?;
        members.push(member);
        rest = next;
    }

    Ok((IdentifierPath::from(members), rest))
}
