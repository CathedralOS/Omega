use crate::diagnostics::parse_error::ParseError;
use crate::diagnostics::render_diagnostic as diagnostics;
use crate::input::delimited::find_top_level_punctuation;
use crate::input::literals::{parse_integer_literal, validate_float_literal};
use arena::{Handle, HandleSpan};
use numerics::literals::IntegerLiteral;
use source::{SourceId, SourceSpan, Span};
use syntax_trees::FloatSpelling;
use syntax_trees::identifier::Identifier;
use tokens::{KeywordKind, PunctuationKind, Token, TokenKind, TokenStream, TokenText};

pub(crate) type ParseResult<'tokens, 'source, T> = Result<(T, Input<'tokens, 'source>), ParseError>;

/// The most levels of `(`/`[` nesting the recursive-descent parser walks into
/// before it rejects the input as too deeply nested. The parser recurses once
/// per level at each of its two choke points (`parse_expression_handle_in` and
/// `parse_type_reference_handle`), so unbounded nesting overflows the native
/// stack on pathological-but-parseable input (e.g. `((((...))))`). This bound
/// converts that crash into a clean diagnostic; it sits far above any nesting a
/// real program reaches, yet far below the depth that would exhaust the large
/// stack the pipeline runs on (see `compile`), so the guard -- not a crash --
/// is always what fires first.
pub(crate) const MAX_NESTING_DEPTH: u16 = 1024;

#[derive(Clone, Copy)]
pub(crate) struct Input<'tokens, 'source> {
    pub(crate) source_id: SourceId,
    pub(crate) tokens: &'tokens [Token],
    stream: &'tokens TokenStream<'source>,
    source_owner: Handle<syntax_trees::RetainedSource>,
    /// Current `(`/`[` nesting depth, carried by value through the parse so the
    /// choke points can bound it. Reconstructions that merely ADVANCE the token
    /// cursor (`advanced`) preserve it; a fresh `new` or `restarted` (top-level item / guard
    /// boundary) resets it to 0, giving each independent construct its own budget.
    depth: u16,
    /// The second `>` from a lexed `>>` while parsing nested generic types.
    /// Expression parsing still sees `>>` as one shift operator; only an
    /// explicit request to consume `>` splits the token contextually.
    pending_greater: bool,
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
        }
    }

    /// Reconstruct the cursor over `tokens` while PRESERVING the nesting depth.
    /// Use for every reconstruction that only advances within the same construct
    /// (token consumption, splits) so accumulated depth is not lost.
    fn advanced(&self, tokens: &'tokens [Token]) -> Self {
        Self {
            source_id: self.source_id,
            stream: self.stream,
            source_owner: self.source_owner,
            tokens: skip_non_semantic_tokens(tokens),
            depth: self.depth,
            pending_greater: self.pending_greater,
        }
    }

    /// Start an independent construct in the same stream, resetting its budget.
    pub(crate) fn restarted(self, tokens: &'tokens [Token]) -> Self {
        Self {
            tokens: skip_non_semantic_tokens(tokens),
            depth: 0,
            pending_greater: false,
            ..self
        }
    }

    pub(crate) fn text(self, token: &Token) -> TokenText<'tokens> {
        self.stream.text(token)
    }

    pub(crate) fn depth(&self) -> u16 {
        self.depth
    }

    /// Enter one more level of nesting, rejecting input that exceeds
    /// [`MAX_NESTING_DEPTH`] before it can overflow the parser's stack. Called at
    /// the recursion choke points; pair with [`Self::with_depth`] to restore the
    /// outer depth on exit so sibling expressions do not accumulate.
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

    pub(crate) fn source_span(&self, token: &Token) -> SourceSpan {
        SourceSpan::new(self.source_id, token.span)
    }

    pub(crate) fn current_source_span(&self) -> SourceSpan {
        self.tokens
            .first()
            .map(|token| self.source_span(token))
            .expect("a current source span requires one semantic token")
    }

    /// Exact semantic-token extent consumed between this cursor and `rest`.
    /// Leading/trailing trivia is excluded even though cursor advancement may
    /// have skipped it.
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

    /// The cursor after a consumed `,` when one heads a comma list's next
    /// element, or `None` when the list ended at this boundary. Comma list
    /// loops answer this one question before continuing or breaking.
    pub(crate) fn take_comma(self) -> Result<Option<Self>, ParseError> {
        if self.at_punctuation(PunctuationKind::Comma) {
            self.take_punctuation(PunctuationKind::Comma, ",").map(Some)
        } else {
            Ok(None)
        }
    }

    /// True when `self` begins with a `(` whose matching `)` group contains a
    /// TOP-LEVEL comma -- i.e. a real tuple/pattern list `(a, b)`, as opposed to
    /// a single parenthesized expression `(a + b)`. Scans only within the leading
    /// group (stops at its matching `)`); a comma nested in a further `(`/`[`/`{`
    /// does not count. Used to disambiguate a transition guard SUBJECT starting
    /// with `(`: a top-level comma means a tuple of subjects, no comma means a
    /// parenthesized expression to route through the general expression parser.
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
            Ok((self.identifier_from_token(token), rest))
        } else {
            Err(diagnostics::expected(self, token, "identifier"))
        }
    }

    /// An optional `identifier:` label prefix; leaves the cursor unchanged
    /// when the upcoming tokens are not a labeled prefix.
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

    /// An integer literal as its ANONYMOUS payload (D14) -- the expression
    /// path, where the literal's type comes from a later use.
    pub(crate) fn take_integer_literal(self) -> Result<(IntegerLiteral, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let literal = parse_integer_literal(self.text(token).as_str(), kind)
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
    pub(crate) fn take_integer(self) -> Result<(i64, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let literal = parse_integer_literal(self.text(token).as_str(), kind)
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
    pub(crate) fn take_identity(self) -> Result<(u64, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.integer_literal_kind() {
            let literal = parse_integer_literal(self.text(token).as_str(), kind)
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

    /// True at any keyword that continues a machine signature contract —
    /// `terminates`/`decreases`/`reaches`/`effects`/`invokes`/`suspends`/
    /// `blocks`/`crashes`/`boundary`. Consumes nothing.
    pub(crate) fn at_contract_continuation_keyword(&self) -> bool {
        self.at_contextual("terminates")
            || self.at_contextual("decreases")
            || self.at_contextual("reaches")
            || self.at_contextual("effects")
            || self.at_contextual("invokes")
            || self.at_contextual("suspends")
            || self.at_contextual("blocks")
            || self.at_contextual("crashes")
            || self.at_contextual("boundary")
    }

    /// The reference-access modifier after `&`: `mut` for Mutable, `write`
    /// for WriteOnly, otherwise Shared (consuming nothing).
    pub(crate) fn take_reference_access(
        self,
    ) -> Result<(language_core::ReferenceAccess, Self), ParseError> {
        if self.at_contextual("mut") {
            Ok((
                language_core::ReferenceAccess::Mutable,
                self.take_contextual("mut")?,
            ))
        } else if self.at_contextual("write") {
            Ok((
                language_core::ReferenceAccess::WriteOnly,
                self.take_contextual("write")?,
            ))
        } else {
            Ok((language_core::ReferenceAccess::Shared, self))
        }
    }

    /// True when the input leads with a bare identifier IMMEDIATELY followed by
    /// the contextual keyword `name` -- a 2-token peek for identifier-led items
    /// like `<target> machine Path(..)`, which have no leading keyword to
    /// dispatch on. Consumes nothing.
    /// True when the input leads with the contextual keyword `name` IMMEDIATELY
    /// followed (past trivia) by a string literal -- the prefixed-string shape
    /// (`utf16"..."`). A bare identifier followed by a string literal is never
    /// otherwise valid, so the peek cannot shadow user code. Consumes nothing.
    pub(crate) fn at_contextual_then_string(&self, name: &str) -> bool {
        let Some((first, rest)) = self.tokens.split_first() else {
            return false;
        };
        if !(matches!(first.kind, TokenKind::Identifier) && self.text(first).as_str() == name) {
            return false;
        }
        skip_non_semantic_tokens(rest)
            .first()
            .is_some_and(|token| token.kind == TokenKind::StringLiteral)
    }

    pub(crate) fn at_identifier_then_contextual(&self, name: &str) -> bool {
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
                && self.text(token).as_str() == name
        })
    }

    /// A contextual prefix such as `move value` applies only to a following
    /// name; `move(...)` stays an ordinary call of a declaration named `move`.
    pub(crate) fn at_contextual_then_name(&self, name: &str) -> bool {
        self.at_contextual(name)
            && skip_non_semantic_tokens(&self.tokens[1..])
                .first()
                .is_some_and(is_identifier_token_for_parser)
    }

    pub(crate) fn at_name_like(&self) -> bool {
        self.tokens
            .first()
            .is_some_and(is_identifier_token_for_parser)
    }

    /// Whether the next token is an integer literal (the identity-number
    /// payload of a stable member identity, `#N name: Type;`.
    pub(crate) fn at_integer(&self) -> bool {
        self.tokens
            .first()
            .is_some_and(|token| token.integer_literal_kind().is_some())
    }

    /// True when the input leads with the complete same-stack demand clause
    /// `stack <integer> bytes`. The bare contextual word `stack` also begins
    /// ordinary contract propositions whose subject happens to be named
    /// `stack` (`requires stack.count < 4 {`), so the clause is recognized only
    /// by its full shape. The peek cannot shadow user code: `stack <integer>
    /// bytes` is not otherwise a valid start of a proposition. Consumes nothing.
    pub(crate) fn at_same_stack_demand(&self) -> bool {
        let Some((first, rest)) = self.tokens.split_first() else {
            return false;
        };
        if !(matches!(first.kind, TokenKind::Identifier | TokenKind::Keyword(_))
            && self.text(first).as_str() == "stack")
        {
            return false;
        }
        let Some((integer, rest)) = skip_non_semantic_tokens(rest).split_first() else {
            return false;
        };
        if integer.integer_literal_kind().is_none() {
            return false;
        }
        skip_non_semantic_tokens(rest).first().is_some_and(|token| {
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

    fn identifier_from_token(&self, token: &Token) -> Identifier {
        let source_span = self.source_span(token);
        Identifier::source_backed(self.source_owner, source_span)
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
    let mut path = crate::handle_spans::HandleSpanBuilder::new();
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
mod storage_tests {
    use super::*;
    use source_files_to_tokens::Lexer;
    use std::sync::Arc;

    #[test]
    fn escaping_text_and_literal_bytes_outlive_token_storage() {
        let source: Arc<str> = Arc::from(r#"name 1.5 "\xff""#);
        let tokens = Lexer::new(&source)
            .tokenize()
            .unwrap()
            .retain_source(source.clone());
        let mut syntax = syntax_trees::SyntaxTrees::new(SourceId(77));
        let input = Input::new(SourceId(77), &tokens, &source, &mut syntax);
        let (name, rest) = input.take_identifier().unwrap();
        let (float, rest) = rest.take_float_text().unwrap();
        let decoded_pointer = tokens.text(tokens.last().unwrap()).as_bytes().as_ptr();
        let (bytes, rest) = rest.take_string_bytes().unwrap();
        assert_eq!(bytes.as_ptr(), decoded_pointer);
        assert!(rest.tokens.is_empty());
        drop(tokens);
        drop(source);
        assert_eq!(name.as_str(&syntax), "name");
        assert_eq!(name.source_span().source_id, SourceId(77));
        assert_eq!(float.as_str(&syntax), "1.5");
        assert_eq!(bytes.as_ref(), &[255]);
    }
}
