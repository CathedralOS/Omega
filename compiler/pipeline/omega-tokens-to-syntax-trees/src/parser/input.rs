use crate::parse_error::ParseError;
use omega_core::source::{SourceId, SourceSpan, SourceText};
use omega_syntax_trees::identifier::{Identifier, IdentifierPath};
use omega_tokens::{KeywordKind, PunctuationKind, Token, TokenKind};

pub(super) type ParseResult<'tokens, 'source, T> =
    Result<(T, Input<'tokens, 'source>), ParseError>;

#[derive(Clone, Copy)]
pub(super) struct Input<'tokens, 'source> {
    pub(super) source_id: SourceId,
    pub(super) tokens: &'tokens [Token<'source>],
}

impl<'tokens, 'source> Input<'tokens, 'source> {
    pub(super) fn new(source_id: SourceId, tokens: &'tokens [Token<'source>]) -> Self {
        Self { source_id, tokens }
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
        if token.is_integer_literal() {
            let value = token.lexeme.as_str().parse::<i64>().map_err(|_| {
                ParseError::at_source_span("invalid integer literal", self.source_span(token))
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
        if token.is_float_literal() {
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
