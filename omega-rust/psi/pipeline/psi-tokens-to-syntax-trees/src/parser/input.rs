use crate::parse_error::ParseError;
use crate::parser::diagnostics;
use crate::parser::input::delimited::find_top_level_punctuation;
use crate::parser::input::literals::{parse_integer_literal, validate_float_literal};
use psi_arena::{Handle, HandleSpan};
use psi_numerics::literals::IntegerLiteral;
use psi_source::{SourceId, SourceSpan, SourceText, Span};
use psi_syntax_trees::identifier::Identifier;
use psi_tokens::{KeywordKind, PunctuationKind, Token, TokenKind, TokenText};

mod delimited;
mod literals;

pub(super) type ParseResult<'tokens, 'source, T> = Result<(T, Input<'tokens, 'source>), ParseError>;

/// The most levels of `(`/`[` nesting the recursive-descent parser walks into
/// before it rejects the input as too deeply nested. The parser recurses once
/// per level at each of its two choke points (`parse_expression_handle_in` and
/// `parse_type_reference_handle`), so unbounded nesting overflows the native
/// stack on pathological-but-parseable input (e.g. `((((...))))`). This bound
/// converts that crash into a clean diagnostic; it sits far above any nesting a
/// real program reaches, yet far below the depth that would exhaust the large
/// stack the pipeline runs on (see `compile`), so the guard -- not a crash --
/// is always what fires first.
pub(super) const MAX_NESTING_DEPTH: u16 = 1024;

#[derive(Clone, Copy)]
pub(super) struct Input<'tokens, 'source> {
    pub(super) source_id: SourceId,
    pub(super) tokens: &'tokens [Token<'source>],
    /// Current `(`/`[` nesting depth, carried by value through the parse so the
    /// choke points can bound it. Reconstructions that merely ADVANCE the token
    /// cursor (`advanced`) preserve it; a fresh `new` (top-level item / guard
    /// boundary) resets it to 0, giving each independent construct its own budget.
    depth: u16,
    /// The second `>` from a lexed `>>` while parsing nested generic types.
    /// Expression parsing still sees `>>` as one shift operator; only an
    /// explicit request to consume `>` splits the token contextually.
    pending_greater: bool,
}

impl<'tokens, 'source> Input<'tokens, 'source> {
    pub(super) fn new(source_id: SourceId, tokens: &'tokens [Token<'source>]) -> Self {
        Self {
            source_id,
            tokens: skip_non_semantic_tokens(tokens),
            depth: 0,
            pending_greater: false,
        }
    }

    /// Reconstruct the cursor over `tokens` while PRESERVING the nesting depth.
    /// Use for every reconstruction that only advances within the same construct
    /// (token consumption, splits) so accumulated depth is not lost.
    fn advanced(&self, tokens: &'tokens [Token<'source>]) -> Self {
        Self {
            source_id: self.source_id,
            tokens: skip_non_semantic_tokens(tokens),
            depth: self.depth,
            pending_greater: self.pending_greater,
        }
    }

    pub(super) fn depth(&self) -> u16 {
        self.depth
    }

    /// Enter one more level of nesting, rejecting input that exceeds
    /// [`MAX_NESTING_DEPTH`] before it can overflow the parser's stack. Called at
    /// the recursion choke points; pair with [`Self::with_depth`] to restore the
    /// outer depth on exit so sibling expressions do not accumulate.
    pub(super) fn deepen(self) -> Result<Self, ParseError> {
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

    pub(super) fn with_depth(self, depth: u16) -> Self {
        Self { depth, ..self }
    }

    pub(super) fn source_span(&self, token: &Token<'_>) -> SourceSpan {
        SourceSpan::new(self.source_id, token.span)
    }

    pub(super) fn current_source_span(&self) -> SourceSpan {
        self.tokens
            .first()
            .map(|token| self.source_span(token))
            .expect("a current source span requires one semantic token")
    }

    /// Exact semantic-token extent consumed between this cursor and `rest`.
    /// Leading/trailing trivia is excluded even though cursor advancement may
    /// have skipped it.
    pub(super) fn source_span_until(self, rest: Self) -> SourceSpan {
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

    pub(super) fn error_here(&self, message: impl Into<String>) -> ParseError {
        let source_span = self
            .tokens
            .first()
            .map(|token| self.source_span(token))
            .unwrap_or_default();
        ParseError::at_source_span(message, source_span)
    }

    pub(super) fn expect_token(self) -> Result<(&'tokens Token<'source>, Self), ParseError> {
        if self.pending_greater {
            return Err(self.error_here("expected `>` before the next token"));
        }
        match self.tokens.split_first() {
            Some((token, rest)) => Ok((token, self.advanced(rest))),
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

    /// True when `self` begins with a `(` whose matching `)` group contains a
    /// TOP-LEVEL comma -- i.e. a real tuple/pattern list `(a, b)`, as opposed to
    /// a single parenthesized expression `(a + b)`. Scans only within the leading
    /// group (stops at its matching `)`); a comma nested in a further `(`/`[`/`{`
    /// does not count. Used to disambiguate a transition guard SUBJECT starting
    /// with `(`: a top-level comma means a tuple of subjects, no comma means a
    /// parenthesized expression to route through the general expression parser.
    pub(super) fn leading_paren_group_has_top_level_comma(&self) -> bool {
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
        if is_identifier_token_for_parser(token) {
            Ok((self.identifier_from_token(token), rest))
        } else {
            Err(diagnostics::expected(self, token, "identifier"))
        }
    }

    /// An integer literal as its ANONYMOUS payload (D14) -- the expression
    /// path, where the literal's type comes from a later use.
    pub(super) fn take_integer_literal(self) -> Result<(IntegerLiteral, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let literal = parse_integer_literal(token.lexeme.as_str(), kind)
                .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
            Ok((literal, rest))
        } else {
            Err(diagnostics::expected(self, token, "integer literal"))
        }
    }

    /// An integer literal at a STRUCTURAL position that needs a number at
    /// parse time (array lengths, wire field numbers, range-form bounds).
    /// These keep an explicit i64 ceiling with a loud
    /// error -- they are not value-binding uses, so D14's fit-at-use does not
    /// apply.
    pub(super) fn take_integer(self) -> Result<(i64, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let literal = parse_integer_literal(token.lexeme.as_str(), kind)
                .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
            let value = literal.value_i64().ok_or_else(|| {
                ParseError::at_source_span(
                    "integer literal exceeds i64; this position needs a parse-time number",
                    self.source_span(token),
                )
            })?;
            Ok((value, rest))
        } else {
            Err(diagnostics::expected(self, token, "integer literal"))
        }
    }

    /// A nonnegative structural identity carried through the complete `u64`
    /// range. Unlike signed structural integers, stable schema identities are
    /// opaque unsigned keys rather than arithmetic values.
    pub(super) fn take_identity(self) -> Result<(u64, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let literal = parse_integer_literal(token.lexeme.as_str(), kind)
                .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
            let value = literal.value_u64().ok_or_else(|| {
                ParseError::at_source_span(
                    "stable identity must be a nonnegative u64 value",
                    self.source_span(token),
                )
            })?;
            Ok((value, rest))
        } else {
            Err(diagnostics::expected(self, token, "stable identity"))
        }
    }

    pub(super) fn take_string(self) -> Result<(String, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.is_string_literal() {
            let bytes = token.lexeme.as_bytes();
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

    pub(super) fn take_string_bytes(self) -> Result<(std::sync::Arc<[u8]>, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.is_string_literal() {
            Ok((std::sync::Arc::from(token.lexeme.as_bytes()), rest))
        } else {
            Err(diagnostics::expected(self, token, "string literal"))
        }
    }

    pub(super) fn take_float_text(self) -> Result<(SourceText, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.float_literal_kind() {
            validate_float_literal(token.lexeme.as_str(), kind)
                .map_err(|message| ParseError::at_source_span(message, self.source_span(token)))?;
            Ok((self.source_text_from_token(token), rest))
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
        if self.pending_greater {
            return punctuation == PunctuationKind::Greater;
        }
        let actual = self.tokens.first().and_then(Token::punctuation);
        actual == Some(punctuation)
            || (punctuation == PunctuationKind::Greater
                && actual == Some(PunctuationKind::GreaterGreater))
    }

    pub(super) fn at_contextual(&self, name: &str) -> bool {
        self.tokens.first().is_some_and(|token| {
            matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && token.lexeme.as_str() == name
        })
    }

    /// True when the input leads with a bare identifier IMMEDIATELY followed by
    /// the contextual keyword `name` -- a 2-token peek for identifier-led items
    /// like `<target> machine Path(..)`, which have no leading keyword to
    /// dispatch on. Consumes nothing.
    /// True when the input leads with the contextual keyword `name` IMMEDIATELY
    /// followed (past trivia) by a string literal -- the prefixed-string shape
    /// (`utf16"..."`). A bare identifier followed by a string literal is never
    /// otherwise valid, so the peek cannot shadow user code. Consumes nothing.
    pub(super) fn at_contextual_then_string(&self, name: &str) -> bool {
        let Some((first, rest)) = self.tokens.split_first() else {
            return false;
        };
        if !(matches!(first.kind, TokenKind::Identifier) && first.lexeme.as_str() == name) {
            return false;
        }
        skip_non_semantic_tokens(rest)
            .first()
            .is_some_and(|token| token.kind == TokenKind::StringLiteral)
    }

    pub(super) fn at_identifier_then_contextual(&self, name: &str) -> bool {
        // `self.tokens[0]` is semantic (leading trivia was skipped in `new`), but
        // trivia between it and the next token is retained, so skip past it before
        // peeking the SECOND semantic token.
        let Some((first, rest)) = self.tokens.split_first() else {
            return false;
        };
        if first.kind != TokenKind::Identifier {
            return false;
        }
        skip_non_semantic_tokens(rest).first().is_some_and(|token| {
            matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && token.lexeme.as_str() == name
        })
    }

    pub(super) fn at_name_like(&self) -> bool {
        self.tokens
            .first()
            .is_some_and(is_identifier_token_for_parser)
    }

    /// Whether the next token is an integer literal (the identity-number
    /// payload of a stable member identity, `#N name: Type;`.
    pub(super) fn at_integer(&self) -> bool {
        self.tokens
            .first()
            .is_some_and(|token| token.integer_literal_kind().is_some())
    }

    pub(super) fn has_newline_before(self, later: Self) -> bool {
        let skipped_count = self.tokens.len().saturating_sub(later.tokens.len());
        let mut index = skipped_count;
        while index > 0 {
            let token = &self.tokens[index - 1];
            if !token.is_non_semantic() {
                return false;
            }
            if token.lexeme.as_str().contains('\n') {
                return true;
            }
            index -= 1;
        }

        false
    }

    fn identifier_from_token(&self, token: &Token<'_>) -> Identifier {
        let source_span = self.source_span(token);
        match &token.lexeme {
            TokenText::Shared { source, .. } => {
                Identifier::source_backed(source.clone(), source_span)
            }
            _ => Identifier::new(token.lexeme.as_str(), source_span),
        }
    }

    fn source_text_from_token(&self, token: &Token<'_>) -> SourceText {
        let source_span = self.source_span(token);
        match &token.lexeme {
            TokenText::Shared { source, .. } => {
                SourceText::source_backed(source.clone(), source_span)
            }
            _ => SourceText::new(token.lexeme.as_str(), source_span),
        }
    }

    pub(super) fn split_at_top_level_punctuation(
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

pub(super) fn is_identifier_token_for_parser(token: &Token<'_>) -> bool {
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
