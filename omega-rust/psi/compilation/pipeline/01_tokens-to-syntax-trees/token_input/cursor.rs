use crate::errors::expected as diagnostics;
use crate::errors::parse_error::ParseError;
use crate::literals::numbers::{parse_integer_literal, validate_float_literal};
use crate::token_input::delimiters::find_top_level_punctuation;
use arena::{Handle, HandleSpan};
use language_core::ReferenceAccess;
use numerics::literals::IntegerLiteral;
use source::{SourceId, SourceSpan, Span};
use syntax_trees::FloatSpelling;
use syntax_trees::identifier::Identifier;
use tokens::{KeywordKind, PunctuationKind, Token, TokenKind, TokenStream, TokenText};

pub(crate) type ParseResult<'tokens, 'source, T> = Result<(T, Input<'tokens, 'source>), ParseError>;

pub(crate) const MAX_NESTING_DEPTH: u16 = 1024;

#[derive(Clone, Copy)]
pub(crate) struct Input<'tokens, 'source> {
    pub(crate) source_id: SourceId,
    pub(crate) tokens: &'tokens [Token],
    stream: &'tokens TokenStream<'source>,
    source_owner: Handle<syntax_trees::RetainedSource>,
    depth: u16,
    pending_greater: bool,
    /// Set while parsing a constant initializer or a constant integer
    /// expression. Those are folded before any operand is typed, so
    /// a suffixed policy operator rejects there at its own token.
    constant_position: bool,
}

impl<'tokens, 'source> Input<'tokens, 'source> {
    pub(crate) fn retain_source(stream: &TokenStream<'_>) -> std::sync::Arc<str> {
        stream
            .source_owner()
            .cloned()
            .unwrap_or_else(|| std::sync::Arc::from(stream.source_text()))
    }

    pub(crate) fn new(
        source_id: SourceId,
        stream: &'tokens TokenStream<'source>,
        retained_source: &std::sync::Arc<str>,
        syntax_trees: &mut syntax_trees::SyntaxTrees,
    ) -> Self {
        let source_owner = syntax_trees.text.retain(retained_source.clone());
        Self {
            source_id,
            source_owner,
            stream,
            tokens: skip_non_semantic_tokens(stream.as_slice()),
            depth: 0,
            pending_greater: false,
            constant_position: false,
        }
    }

    fn advanced(&self, tokens: &'tokens [Token]) -> Self {
        Self {
            source_id: self.source_id,
            stream: self.stream,
            source_owner: self.source_owner,
            tokens: skip_non_semantic_tokens(tokens),
            depth: self.depth,
            pending_greater: self.pending_greater,
            constant_position: self.constant_position,
        }
    }

    pub(crate) fn restarted(self, tokens: &'tokens [Token]) -> Self {
        Self {
            tokens: skip_non_semantic_tokens(tokens),
            depth: 0,
            pending_greater: false,
            ..self
        }
    }

    pub(crate) fn resume_tokens(&mut self, tokens: &'tokens [Token]) {
        *self = self.advanced(tokens);
    }

    pub(crate) fn text(self, token: &Token) -> TokenText<'tokens> {
        self.stream.text(token)
    }

    fn peek(&self, index: usize) -> Option<&'tokens Token> {
        if index == 0 {
            return self.tokens.first();
        }
        skip_non_semantic_tokens(self.tokens.get(1..)?).get(index - 1)
    }

    pub(crate) fn depth(&self) -> u16 {
        self.depth
    }

    pub(crate) fn deepen(self) -> Result<Self, ParseError> {
        if self.depth >= MAX_NESTING_DEPTH {
            return Err(self.error_here(format!(
                "expression or type nesting is too deep (exceeds the maximum of {MAX_NESTING_DEPTH} levels)"
            )));
        }
        Ok(Self {
            depth: self.depth + 1,
            ..self
        })
    }

    pub(crate) fn with_depth(self, depth: u16) -> Self {
        Self { depth, ..self }
    }

    pub(crate) fn constant_position(&self) -> bool {
        self.constant_position
    }

    pub(crate) fn with_constant_position(self, constant_position: bool) -> Self {
        Self {
            constant_position,
            ..self
        }
    }

    pub(crate) fn source_span(&self, token: &Token) -> SourceSpan {
        SourceSpan::new(self.source_id, token.span)
    }

    pub(crate) fn current_source_span(&self) -> SourceSpan {
        self.tokens
            .first()
            .map(|token| self.source_span(token))
            .expect("a current source span requires one semantic token")
    }

    pub(crate) fn source_span_until(self, rest: Self) -> SourceSpan {
        debug_assert_eq!(self.source_id, rest.source_id);
        let consumed_count = self
            .tokens
            .len()
            .checked_sub(rest.tokens.len())
            .expect("the remaining parser cursor must descend from the starting cursor");
        let consumed = &self.tokens[..consumed_count];
        let first = consumed
            .iter()
            .find(|token| !token.is_non_semantic())
            .expect("a consumed source span requires one semantic token");
        let last = consumed
            .iter()
            .rev()
            .find(|token| !token.is_non_semantic())
            .expect("a consumed source span requires one semantic token");
        SourceSpan::new(self.source_id, Span::new(first.span.start, last.span.end))
    }

    pub(crate) fn error_here(&self, message: impl Into<String>) -> ParseError {
        let source_span = self
            .tokens
            .first()
            .map(|token| self.source_span(token))
            .unwrap_or_default();
        ParseError::at_source_span(message, source_span)
    }

    pub(crate) fn expect_token(self) -> Result<(&'tokens Token, Self), ParseError> {
        if self.pending_greater {
            return Err(self.error_here("expected `>` before the next token"));
        }
        match self.tokens.split_first() {
            Some((token, rest)) => Ok((token, self.advanced(rest))),
            None => Err(diagnostics::unexpected_eof(self, "token")),
        }
    }

    pub(crate) fn take_keyword(
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

    pub(crate) fn take_punctuation(
        self,
        punctuation: PunctuationKind,
        label: &str,
    ) -> Result<Self, ParseError> {
        if self.pending_greater {
            if punctuation == PunctuationKind::Greater {
                return Ok(Self {
                    pending_greater: false,
                    ..self
                });
            }
            return Err(self.error_here(format!("expected `{label}`")));
        }
        if punctuation == PunctuationKind::Greater
            && self.tokens.first().and_then(Token::punctuation)
                == Some(PunctuationKind::GreaterGreater)
        {
            let (_, rest) = self.expect_token()?;
            return Ok(Self {
                pending_greater: true,
                ..rest
            });
        }
        let (token, rest) = self.expect_token()?;
        if token.punctuation() == Some(punctuation) {
            Ok(rest)
        } else {
            Err(diagnostics::expected(self, token, format!("`{label}`")))
        }
    }

    pub(crate) fn take_comma(self) -> Result<Option<Self>, ParseError> {
        if self.at_punctuation(PunctuationKind::Comma) {
            self.take_punctuation(PunctuationKind::Comma, ",").map(Some)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn leading_paren_group_has_top_level_comma(&self) -> bool {
        let mut paren = 0usize;
        let mut bracket = 0usize;
        let mut brace = 0usize;
        for token in self.tokens {
            match token.punctuation() {
                Some(PunctuationKind::LeftParen) => paren += 1,
                Some(PunctuationKind::RightParen) => {
                    paren = paren.saturating_sub(1);
                    if paren == 0 {
                        return false;
                    }
                }
                Some(PunctuationKind::LeftBracket) => bracket += 1,
                Some(PunctuationKind::RightBracket) => bracket = bracket.saturating_sub(1),
                Some(PunctuationKind::LeftBrace) => brace += 1,
                Some(PunctuationKind::RightBrace) => brace = brace.saturating_sub(1),
                Some(PunctuationKind::Comma) if paren == 1 && bracket == 0 && brace == 0 => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn take_contextual(self, name: &str) -> Result<Self, ParseError> {
        let (token, rest) = self.expect_token()?;
        if matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
            && self.text(token).as_str() == name
        {
            Ok(rest)
        } else {
            Err(diagnostics::expected(self, token, format!("`{name}`")))
        }
    }

    pub(crate) fn take_identifier(self) -> Result<(Identifier, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if is_identifier_token_for_parser(token) {
            let span = self.source_span(token);
            Ok((Identifier::source_backed(self.source_owner, span), rest))
        } else {
            Err(diagnostics::expected(self, token, "identifier"))
        }
    }

    pub(crate) fn take_optional_label(self) -> Result<(Option<Identifier>, Self), ParseError> {
        if let Ok((binding, after_binding)) = self.take_identifier()
            && after_binding.at_punctuation(PunctuationKind::Colon)
        {
            Ok((
                Some(binding),
                after_binding.take_punctuation(PunctuationKind::Colon, ":")?,
            ))
        } else {
            Ok((None, self))
        }
    }

    pub(crate) fn take_integer_literal(self) -> Result<(IntegerLiteral, Self), ParseError> {
        self.take_parsed_integer("integer literal")
    }

    fn take_parsed_integer(self, expected: &str) -> Result<(IntegerLiteral, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        let Some(kind) = token.integer_literal_kind() else {
            return Err(diagnostics::expected(self, token, expected));
        };
        let literal = parse_integer_literal(self.text(token).as_str(), kind)
            .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
        Ok((literal, rest))
    }

    pub(crate) fn take_integer(self) -> Result<(i64, Self), ParseError> {
        let (literal, rest) = self.take_parsed_integer("integer literal")?;
        let value = literal.value_i64().ok_or_else(|| {
            ParseError::at_source_span(
                "integer literal exceeds i64; this position needs a parse-time number",
                self.current_source_span(),
            )
        })?;
        Ok((value, rest))
    }

    pub(crate) fn take_identity(self) -> Result<(u64, Self), ParseError> {
        let (literal, rest) = self.take_parsed_integer("stable identity")?;
        let value = literal.value_u64().ok_or_else(|| {
            ParseError::at_source_span(
                "stable identity must be a nonnegative u64 value",
                self.current_source_span(),
            )
        })?;
        Ok((value, rest))
    }

    pub(crate) fn take_string(self) -> Result<(String, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.is_string_literal() {
            let bytes = self.text(token).as_bytes();
            let value = std::str::from_utf8(bytes).map_err(|_| {
                ParseError::at_source_span(
                    "raw byte string literal requires the terminal byte-sequence lowering path",
                    self.source_span(token),
                )
            })?;
            Ok((value.to_owned(), rest))
        } else {
            Err(diagnostics::expected(self, token, "string literal"))
        }
    }

    pub(crate) fn take_string_bytes(self) -> Result<(source::SourceBytes, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.is_string_literal() {
            Ok((self.stream.literal_bytes(token), rest))
        } else {
            Err(diagnostics::expected(self, token, "string literal"))
        }
    }

    pub(crate) fn take_float_text(self) -> Result<(FloatSpelling, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.float_literal_kind() {
            validate_float_literal(self.text(token).as_str(), kind)
                .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
            Ok((
                FloatSpelling::source_backed(self.source_owner, self.source_span(token)),
                rest,
            ))
        } else {
            Err(diagnostics::expected(self, token, "float literal"))
        }
    }

    pub(crate) fn expected_one_of_here(self, expected: &[&str]) -> ParseError {
        diagnostics::expected_one_of_here(self, expected)
    }

    pub(crate) fn at_keyword(&self, keyword: KeywordKind) -> bool {
        self.tokens.first().and_then(Token::keyword) == Some(keyword)
    }

    pub(crate) fn at_punctuation(&self, punctuation: PunctuationKind) -> bool {
        if self.pending_greater {
            return punctuation == PunctuationKind::Greater;
        }
        let actual = self.tokens.first().and_then(Token::punctuation);
        actual == Some(punctuation)
            || (punctuation == PunctuationKind::Greater
                && actual == Some(PunctuationKind::GreaterGreater))
    }

    pub(crate) fn at_contextual(&self, name: &str) -> bool {
        self.tokens.first().is_some_and(|token| {
            matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && self.text(token).as_str() == name
        })
    }

    pub(crate) fn at_contract_continuation_keyword(&self) -> bool {
        (self.at_contextual("terminates")
            || self.at_contextual("decreases")
            || self.at_contextual("reaches")
            || self.at_contextual("effects")
            || self.at_contextual("invokes")
            || self.at_contextual("suspends")
            || self.at_contextual("blocks")
            || self.at_contextual("crashes")
            || self.at_contextual("boundary"))
            && !self.clause_word_is_an_operand()
    }

    /// These clause words are not reserved, so a parameter may carry
    /// one of them and appear as the subject of a proposition:
    /// `requires boundary <= 4`. No clause is followed by an infix or
    /// projection operator, so that next token tells the two apart.
    /// The one clause that is, the installation bound
    /// `reaches <= Bound`, keeps its clause reading.
    pub(crate) fn clause_word_is_an_operand(&self) -> bool {
        let Some(next) = self.peek(1) else {
            return false;
        };
        if next.kind == TokenKind::Keyword(KeywordKind::As)
            || (next.kind == TokenKind::Identifier && self.text(next).as_str() == "in")
        {
            return true;
        }
        let Some(punctuation) = next.punctuation() else {
            return false;
        };
        if punctuation == PunctuationKind::LessEqual && self.at_contextual("reaches") {
            return false;
        }
        matches!(
            punctuation,
            PunctuationKind::Dot
                | PunctuationKind::LeftBracket
                | PunctuationKind::Comma
                | PunctuationKind::EqualEqual
                | PunctuationKind::ExclamationEqual
                | PunctuationKind::Less
                | PunctuationKind::LessEqual
                | PunctuationKind::Greater
                | PunctuationKind::GreaterEqual
                | PunctuationKind::Plus
                | PunctuationKind::Minus
                | PunctuationKind::Asterisk
                | PunctuationKind::Slash
                | PunctuationKind::Percent
                | PunctuationKind::AndAnd
                | PunctuationKind::PipePipe
        )
    }

    pub(crate) fn take_reference_access(self) -> Result<(ReferenceAccess, Self), ParseError> {
        if self.at_contextual("mut") {
            Ok((ReferenceAccess::Mutable, self.take_contextual("mut")?))
        } else if self.at_contextual("write") {
            Ok((ReferenceAccess::WriteOnly, self.take_contextual("write")?))
        } else {
            Ok((ReferenceAccess::Shared, self))
        }
    }

    pub(crate) fn at_contextual_then_string(&self, name: &str) -> bool {
        self.peek(0).is_some_and(|token| {
            token.kind == TokenKind::Identifier && self.text(token).as_str() == name
        }) && self.peek(1).is_some_and(Token::is_string_literal)
    }

    pub(crate) fn at_identifier_then_contextual(&self, name: &str) -> bool {
        self.peek(0)
            .is_some_and(|first| first.kind == TokenKind::Identifier)
            && self.peek(1).is_some_and(|token| {
                matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                    && self.text(token).as_str() == name
            })
    }

    /// Whether `{` opens the retired `terminates { decreases ...; }`
    /// ranking block rather than a body: its first statement would be
    /// `decreases` or `increases` followed by a subject, which no body
    /// statement can start with.
    pub(crate) fn at_retired_ranking_block(&self) -> bool {
        if !self.at_punctuation(PunctuationKind::LeftBrace) {
            return false;
        }
        let Some(after_brace) = self.tokens.get(1..).map(skip_non_semantic_tokens) else {
            return false;
        };
        let Some((word, rest)) = after_brace.split_first() else {
            return false;
        };
        matches!(word.kind, TokenKind::Identifier | TokenKind::Keyword(_))
            && matches!(self.text(word).as_str(), "decreases" | "increases")
            && skip_non_semantic_tokens(rest)
                .first()
                .is_some_and(is_identifier_token_for_parser)
    }

    pub(crate) fn at_contextual_then_name(&self, name: &str) -> bool {
        self.at_contextual(name) && self.peek(1).is_some_and(is_identifier_token_for_parser)
    }

    pub(crate) fn at_name_like(&self) -> bool {
        self.tokens
            .first()
            .is_some_and(is_identifier_token_for_parser)
    }

    pub(crate) fn at_integer(&self) -> bool {
        self.tokens
            .first()
            .is_some_and(|token| token.integer_literal_kind().is_some())
    }

    pub(crate) fn at_same_stack_demand(&self) -> bool {
        self.peek(0).is_some_and(|first| {
            matches!(first.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && self.text(first).as_str() == "stack"
        }) && self
            .peek(1)
            .is_some_and(|integer| integer.integer_literal_kind().is_some())
            && self.peek(2).is_some_and(|token| {
                matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                    && self.text(token).as_str() == "bytes"
            })
    }

    pub(crate) fn has_newline_before(self, later: Self) -> bool {
        let skipped_count = self.tokens.len().saturating_sub(later.tokens.len());
        let mut index = skipped_count;
        while index > 0 {
            let token = &self.tokens[index - 1];
            if !token.is_non_semantic() {
                return false;
            }
            if self.text(token).as_str().contains('\n') {
                return true;
            }
            index -= 1;
        }

        false
    }

    pub(crate) fn split_at_top_level_punctuation(
        self,
        delimiter: PunctuationKind,
        message: &str,
    ) -> Result<(Self, Self), ParseError> {
        let split_index =
            find_top_level_punctuation(self, delimiter).ok_or_else(|| self.error_here(message))?;
        let (prefix_tokens, rest_tokens) = self.tokens.split_at(split_index);
        Ok((self.advanced(prefix_tokens), self.advanced(rest_tokens)))
    }
}

pub(crate) fn is_identifier_token_for_parser(token: &Token) -> bool {
    match token.kind {
        TokenKind::Identifier => true,
        TokenKind::Keyword(keyword) => !keyword.is_strict_identifier_keyword(),
        _ => false,
    }
}

fn skip_non_semantic_tokens(tokens: &[Token]) -> &[Token] {
    let mut index = 0usize;
    while index < tokens.len() && tokens[index].is_non_semantic() {
        index += 1;
    }
    &tokens[index..]
}

pub(crate) fn parse_path_handle_span<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    mut append_member: impl FnMut(Identifier) -> Handle<Identifier>,
) -> ParseResult<'tokens, 'source, HandleSpan<Identifier>> {
    let (first, mut rest) = input.take_identifier()?;
    let mut path = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    path.push(append_member(first));

    while rest.at_punctuation(PunctuationKind::ColonColon) {
        rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = rest.take_identifier()?;
        path.push(append_member(member));
        rest = next;
    }

    Ok((path.finish(), rest))
}

impl syntax_trees::SyntaxTextContext for Input<'_, '_> {
    fn source_text(&self, source: Handle<syntax_trees::RetainedSource>) -> &str {
        assert_eq!(
            source, self.source_owner,
            "identifier belongs to a different parser source"
        );
        self.stream.source_text()
    }
}

#[cfg(test)]
#[path = "../tests/unit/retained_source.rs"]
mod storage_tests;
