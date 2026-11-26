use crate::errors::expected as diagnostics;
use crate::errors::parse_error::ParseError;
use crate::literals::numbers::{parse_integer_literal, validate_float_literal};
use crate::token_input::delimiters::find_top_level_punctuation;
use arena::{Handle, HandleSpan};
use language_core::ReferenceAccess;
use numeric_literals::IntegerLiteral;
use source_text::{SourceId, SourceSpan, Span};
use syntax_trees::FloatSpelling;
use syntax_trees::identifier::Identifier;
use tokens::{KeywordKind, PunctuationKind, Token, TokenKind, TokenStream};

pub(crate) type ParseResult<'tokens, 'source, T> = Result<(T, Input<'tokens, 'source>), ParseError>;

pub(crate) const MAX_NESTING_DEPTH: u16 = 1024;

#[derive(Clone, Copy)]
pub(crate) struct Input<'tokens, 'source> {
    pub(crate) source_id: SourceId,
    pub(crate) tokens: &'tokens [Token],
    stream: &'tokens TokenStream<'source>,
    pub(crate) file_qualifiers: Option<&'tokens super::file_qualifiers::FileQualifiers<'tokens>>,
    source_owner: Handle<syntax_trees::RetainedSource>,
    depth: u16,
    /// Set while parsing a constant initializer or a constant integer
    /// expression. Those are folded before any operand is typed, so
    /// a suffixed policy operator rejects there at its own token.
    constant_position: bool,
}

impl<'tokens, 'source> Input<'tokens, 'source> {
    pub(crate) fn new(
        source_id: SourceId,
        stream: &'tokens TokenStream<'source>,
        syntax_trees: &mut syntax_trees::SyntaxTrees,
    ) -> Self {
        let source_owner = syntax_trees.text.retain(stream.shared_source());
        Self {
            source_id,
            source_owner,
            file_qualifiers: None,
            stream,
            tokens: skip_trivia(stream.as_slice()),
            depth: 0,
            constant_position: false,
        }
    }

    fn advanced(&self, tokens: &'tokens [Token]) -> Self {
        Self {
            tokens: skip_trivia(tokens),
            ..*self
        }
    }

    pub(crate) fn restarted(self, tokens: &'tokens [Token]) -> Self {
        Self {
            tokens: skip_trivia(tokens),
            depth: 0,
            ..self
        }
    }

    pub(crate) fn spelling(self, token: &Token) -> &'tokens str {
        self.stream.spelling(token)
    }

    // The cursor starts on a grammar token. Trivia may separate every later pair.
    #[inline]
    fn peek_semantic(&self, index: usize) -> Option<&'tokens Token> {
        let mut tokens = self.tokens;
        for _ in 0..index {
            tokens = skip_trivia(tokens.get(1..)?);
        }
        tokens.first()
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

    pub(crate) fn name_until(self, rest: Self) -> Identifier {
        Identifier::source_backed(self.source_owner, self.source_span_until(rest))
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
            .find(|token| !token.is_trivia())
            .expect("a consumed source span requires one semantic token");
        let last = consumed
            .iter()
            .rev()
            .find(|token| !token.is_trivia())
            .expect("a consumed source span requires one semantic token");
        SourceSpan::new(self.source_id, Span::new(first.span.start, last.span.end))
    }

    /// Where a source ends. An error about a token the source never
    /// supplied points here instead of at the first byte of the file.
    pub(crate) fn end_of_source_span(&self) -> SourceSpan {
        let end = self.stream.source_text().len();
        SourceSpan::new(self.source_id, Span::new(end, end))
    }

    pub(crate) fn error_here(&self, message: impl Into<String>) -> ParseError {
        let source_span = self
            .tokens
            .first()
            .map(|token| self.source_span(token))
            .unwrap_or_else(|| self.end_of_source_span());
        ParseError::at_source_span(message, source_span)
    }

    pub(crate) fn expect_token(self) -> Result<(&'tokens Token, Self), ParseError> {
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

    /// Separates consecutive match or transition arms and leaves the closing
    /// brace for the enclosing dispatch, with or without a trailing comma.
    pub(crate) fn after_dispatch_arm(self) -> Result<Self, ParseError> {
        if self.at_punctuation(PunctuationKind::RightBrace) {
            return Ok(self);
        }
        let Some(next) = self.take_comma()? else {
            return Err(self.error_here("expected `,` between arms or `}` after the final arm"));
        };
        if next.at_punctuation(PunctuationKind::Comma) {
            return Err(next.error_here("expected an arm or `}` after `,`"));
        }
        Ok(next)
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
            && self.spelling(token) == name
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

    /// The `..` that ends a data pattern, kept as the two bytes it spans so
    /// later stages can point at it.
    pub(crate) fn take_rest_marker(self) -> Result<(Identifier, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if token.punctuation() == Some(PunctuationKind::DotDot) {
            let span = self.source_span(token);
            Ok((Identifier::source_backed(self.source_owner, span), rest))
        } else {
            Err(diagnostics::expected(self, token, "`..`"))
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
        let literal = parse_integer_literal(self.spelling(token), kind)
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
        if let Some(bytes) = self.stream.literal_bytes(token) {
            let value = std::str::from_utf8(&bytes).map_err(|_| {
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

    pub(crate) fn take_string_bytes(self) -> Result<(source_text::SourceBytes, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(bytes) = self.stream.literal_bytes(token) {
            Ok((bytes, rest))
        } else {
            Err(diagnostics::expected(self, token, "string literal"))
        }
    }

    pub(crate) fn take_float_text(self) -> Result<(FloatSpelling, Self), ParseError> {
        let (token, rest) = self.expect_token()?;
        if let Some(kind) = token.float_literal_kind() {
            validate_float_literal(self.spelling(token), kind)
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
        self.tokens.first().and_then(Token::punctuation) == Some(punctuation)
    }

    /// A right shift is two `>` with no byte between them; `> >` is not one
    /// (wiki/spec/language/expressions.md). Returns the span of the pair and
    /// the cursor after it.
    pub(crate) fn take_right_shift(self) -> Option<(SourceSpan, Self)> {
        let [first, second, rest @ ..] = self.tokens else {
            return None;
        };
        let touching = first.punctuation() == Some(PunctuationKind::Greater)
            && second.punctuation() == Some(PunctuationKind::Greater)
            && first.span.end == second.span.start;
        touching.then(|| {
            (
                SourceSpan::new(self.source_id, Span::new(first.span.start, second.span.end)),
                self.advanced(rest),
            )
        })
    }

    pub(crate) fn at_expression_arguments(&self) -> bool {
        self.at_punctuation(PunctuationKind::ColonColon)
            && self.peek_semantic(1).and_then(Token::punctuation) == Some(PunctuationKind::Less)
    }

    /// The cursor after the `>` that closes the static argument list opened
    /// by the `<` here, found without parsing the list. A parser that tries
    /// a list and rewinds when no `(` or `::member` follows re-reads every
    /// nested list on each attempt, which is exponential in the nesting; a
    /// caller asks this first and parses only a list whose end has the
    /// expected follower.
    ///
    /// Outside parentheses, brackets and braces, a `<` in the list only opens
    /// a nested list and a `>` only closes one: a value argument is an
    /// arithmetic chain with no comparison, and there is no `>>` token. So
    /// counting them finds the end; a block such as a `match` arm list is a
    /// group like parentheses. `None` when the list does not close before a
    /// `;` outside every group, an unmatched closer, or the end.
    pub(crate) fn after_static_argument_list(&self) -> Option<Self> {
        if !self.at_punctuation(PunctuationKind::Less) {
            return None;
        }
        let mut angles = 0usize;
        let mut groups = 0usize;
        for (offset, token) in self.tokens.iter().enumerate() {
            let Some(punctuation) = token.punctuation() else {
                continue;
            };
            match punctuation {
                PunctuationKind::LeftParen
                | PunctuationKind::LeftBracket
                | PunctuationKind::LeftBrace => groups += 1,
                PunctuationKind::RightParen
                | PunctuationKind::RightBracket
                | PunctuationKind::RightBrace => groups = groups.checked_sub(1)?,
                PunctuationKind::Semicolon if groups == 0 => return None,
                PunctuationKind::Less if groups == 0 => angles += 1,
                PunctuationKind::Greater if groups == 0 => {
                    angles -= 1;
                    if angles == 0 {
                        return Some(self.advanced(&self.tokens[offset + 1..]));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// A file qualifier can only begin a name. The lexer keeps `||` and
    /// `+|` as compound tokens before the parser sees this prefix.
    pub(crate) fn at_path_head(&self) -> bool {
        self.at_name_like() || self.at_punctuation(PunctuationKind::Pipe)
    }

    pub(crate) fn take_path_head(self) -> Result<(Identifier, Self), ParseError> {
        if !self.at_punctuation(PunctuationKind::Pipe) {
            return self.take_identifier();
        }
        let after_bar = self.take_punctuation(PunctuationKind::Pipe, "|")?;
        let (alias, rest) = after_bar.take_identifier()?;
        let rest = rest.take_punctuation(PunctuationKind::Pipe, "|")?;
        let rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (declaration, rest) = rest.take_identifier()?;
        if let Some(uses) = self.file_qualifiers {
            uses.record(self.source_span_until(rest).span.start, after_bar.spelling(&after_bar.tokens[0]));
        }
        let name = format!("|{}|::{}", alias.as_str(&self), declaration.as_str(&self));
        Ok((Identifier::new(name, self.source_span_until(rest)), rest))
    }

    pub(crate) fn at_path_member(&self) -> bool {
        self.at_punctuation(PunctuationKind::ColonColon) && !self.at_expression_arguments()
    }

    pub(crate) fn take_path_separator(self) -> Result<Self, ParseError> {
        self.take_punctuation(PunctuationKind::ColonColon, "::")
    }

    pub(crate) fn at_contextual(&self, name: &str) -> bool {
        self.tokens.first().is_some_and(|token| {
            matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && self.spelling(token) == name
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
        let Some(next) = self.peek_semantic(1) else {
            return false;
        };
        if next.kind == TokenKind::Keyword(KeywordKind::As)
            || (next.kind == TokenKind::Identifier && self.spelling(next) == "in")
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

    pub(crate) fn take_binding_mutability(self) -> Result<(bool, Self), ParseError> {
        if self.at_contextual("mutable")
            && self.peek_semantic(1).is_some_and(|token| {
                matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                    || token.punctuation() == Some(PunctuationKind::At)
            })
        {
            Ok((true, self.take_contextual("mutable")?))
        } else {
            Ok((false, self))
        }
    }

    pub(crate) fn take_reference_access(self) -> Result<(ReferenceAccess, Self), ParseError> {
        if self.at_contextual("mutable") {
            Ok((ReferenceAccess::Mutable, self.take_contextual("mutable")?))
        } else if self.at_contextual("write") {
            Ok((ReferenceAccess::WriteOnly, self.take_contextual("write")?))
        } else {
            Ok((ReferenceAccess::Shared, self))
        }
    }

    pub(crate) fn at_contextual_then_string(&self, name: &str) -> bool {
        self.peek_semantic(0).is_some_and(|token| {
            token.kind == TokenKind::Identifier && self.spelling(token) == name
        }) && self.peek_semantic(1).is_some_and(Token::is_string_literal)
    }

    pub(crate) fn at_identifier_then_contextual(&self, name: &str) -> bool {
        self.peek_semantic(0)
            .is_some_and(|first| first.kind == TokenKind::Identifier)
            && self.peek_semantic(1).is_some_and(|token| {
                matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                    && self.spelling(token) == name
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
        let Some(after_brace) = self.tokens.get(1..).map(skip_trivia) else {
            return false;
        };
        let Some((word, rest)) = after_brace.split_first() else {
            return false;
        };
        matches!(word.kind, TokenKind::Identifier | TokenKind::Keyword(_))
            && matches!(self.spelling(word), "decreases" | "increases")
            && skip_trivia(rest)
                .first()
                .is_some_and(is_identifier_token_for_parser)
    }

    pub(crate) fn at_contextual_then_name(&self, name: &str) -> bool {
        self.at_contextual(name)
            && self
                .peek_semantic(1)
                .is_some_and(is_identifier_token_for_parser)
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
        self.peek_semantic(0).is_some_and(|first| {
            matches!(first.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && self.spelling(first) == "stack"
        }) && self
            .peek_semantic(1)
            .is_some_and(|integer| integer.integer_literal_kind().is_some())
            && self.peek_semantic(2).is_some_and(|token| {
                matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                    && self.spelling(token) == "bytes"
            })
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
        TokenKind::Keyword(keyword) => keyword != KeywordKind::SelfValue,
        _ => false,
    }
}

fn skip_trivia(tokens: &[Token]) -> &[Token] {
    let mut index = 0usize;
    while index < tokens.len() && tokens[index].is_trivia() {
        index += 1;
    }
    &tokens[index..]
}

pub(crate) fn parse_path_handle_span<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    mut append_member: impl FnMut(Identifier) -> Handle<Identifier>,
) -> ParseResult<'tokens, 'source, HandleSpan<Identifier>> {
    let (first, mut rest) = input.take_path_head()?;
    let mut path = arena::HandleSpan::empty();
    path.push_contiguous(append_member(first));

    while rest.at_path_member() {
        rest = rest.take_path_separator()?;
        let (member, next) = rest.take_identifier()?;
        path.push_contiguous(append_member(member));
        rest = next;
    }

    Ok((path, rest))
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
#[path = "../../tests/unit/retained_source.rs"]
mod storage_tests;
