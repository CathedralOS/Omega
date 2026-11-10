/*
Token stream cursor and literal/delimiter scanning (`tokens`), syntax-arena
table accessors and contiguous handle-span staging (`storage`), and `ParseError`
diagnostic rendering (`diagnostics`).
*/

pub(crate) mod diagnostics {
    pub mod parse_error {
        use source::SourceSpan;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ParseError {
            pub message: String,
            pub source_span: SourceSpan,
        }

        impl ParseError {
            pub fn new(message: impl Into<String>) -> Self {
                Self {
                    message: message.into(),
                    source_span: SourceSpan::default(),
                }
            }

            pub fn at_source_span(message: impl Into<String>, source_span: SourceSpan) -> Self {
                Self {
                    message: message.into(),
                    source_span,
                }
            }
        }
    }
    pub(crate) mod render_diagnostic {
        use crate::diagnostics::parse_error::ParseError;
        use crate::tokens::token_cursor::Input;
        use tokens::{CommentKind, Token, TokenKind};

        fn expected_at(
            input: Input<'_, '_>,
            token: Option<&Token>,
            expected: &[&str],
        ) -> ParseError {
            let expected = match expected {
                [] => "unexpected token".to_owned(),
                [only] => format!("expected {only}"),
                _ => format!("expected one of {}", expected.join(", ")),
            };

            match token {
                Some(token) => ParseError::at_source_span(
                    format!("{expected}, found {}", describe_token(input, token)),
                    input.source_span(token),
                ),
                None => {
                    ParseError::at_source_span(format!("{expected}, found EOF"), Default::default())
                }
            }
        }

        pub(crate) fn unexpected_eof(
            input: Input<'_, '_>,
            expected: impl Into<String>,
        ) -> ParseError {
            let expected = expected.into();
            expected_at(input, None, &[expected.as_str()])
        }

        pub(crate) fn expected(
            input: Input<'_, '_>,
            token: &Token,
            expected: impl Into<String>,
        ) -> ParseError {
            let expected = expected.into();
            expected_at(input, Some(token), &[expected.as_str()])
        }

        pub(crate) fn expected_one_of_here(input: Input<'_, '_>, expected: &[&str]) -> ParseError {
            expected_at(input, input.tokens.first(), expected)
        }

        fn describe_token(input: Input<'_, '_>, token: &Token) -> String {
            match token.kind {
                TokenKind::Identifier => format!("identifier `{}`", input.text(token).as_str()),
                TokenKind::NumericLiteral(_) => {
                    format!("numeric literal `{}`", input.text(token).as_str())
                }
                TokenKind::StringLiteral => "string literal".to_owned(),
                TokenKind::Keyword(keyword) => format!("keyword `{}`", keyword.lexeme()),
                TokenKind::Punctuation(punctuation) => {
                    format!("punctuation `{}`", punctuation.lexeme())
                }
                TokenKind::Whitespace => "whitespace".to_owned(),
                TokenKind::Comment(CommentKind::Line) => "line comment".to_owned(),
                TokenKind::Comment(CommentKind::Block) => "block comment".to_owned(),
            }
        }
    }
}

pub(crate) mod storage {
    pub(crate) mod handle_spans {
        use arena::{Handle, HandleSpan};

        pub(crate) struct HandleSpanBuilder<T> {
            start: Handle<T>,
            count: u32,
        }

        impl<T> HandleSpanBuilder<T> {
            pub(crate) fn new() -> Self {
                Self {
                    start: Handle::invalid(),
                    count: 0,
                }
            }

            pub(crate) fn push(&mut self, handle: Handle<T>) {
                if self.count == 0 {
                    self.start = handle;
                }
                self.count = self
                    .count
                    .checked_add(1)
                    .expect("handle span count overflow");
            }

            pub(crate) fn push_span(&mut self, span: HandleSpan<T>) {
                if span.is_empty() {
                    return;
                }
                if self.count == 0 {
                    self.start = span.start();
                }
                self.count = self
                    .count
                    .checked_add(span.count())
                    .expect("handle span count overflow");
            }

            pub(crate) fn is_empty(&self) -> bool {
                self.count == 0
            }

            pub(crate) fn finish(self) -> HandleSpan<T> {
                HandleSpan::from_parts_or_empty(self.start, self.count)
            }
        }
    }
    pub(crate) mod tables {
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::ExpressionTable;
        use syntax_trees::item::ItemTable;
        use syntax_trees::statement::StatementTable;
        use syntax_trees::types::TypeReferenceTable;

        pub fn exprs(syntax_trees: &mut SyntaxTrees) -> &mut ExpressionTable {
            &mut syntax_trees.expressions
        }

        pub fn exprs_of(syntax_trees: &SyntaxTrees) -> &ExpressionTable {
            &syntax_trees.expressions
        }

        pub fn items(syntax_trees: &mut SyntaxTrees) -> &mut ItemTable {
            &mut syntax_trees.items
        }

        pub fn items_of(syntax_trees: &SyntaxTrees) -> &ItemTable {
            &syntax_trees.items
        }

        pub fn stmts(syntax_trees: &mut SyntaxTrees) -> &mut StatementTable {
            &mut syntax_trees.statements
        }

        pub fn stmts_of(syntax_trees: &SyntaxTrees) -> &StatementTable {
            &syntax_trees.statements
        }

        pub fn type_refs(syntax_trees: &mut SyntaxTrees) -> &mut TypeReferenceTable {
            &mut syntax_trees.type_references
        }

        pub fn type_refs_of(syntax_trees: &SyntaxTrees) -> &TypeReferenceTable {
            &syntax_trees.type_references
        }
    }
}

pub(crate) mod tokens {
    pub(crate) mod delimited {
        use crate::tokens::token_cursor::Input;
        use tokens::PunctuationKind;

        pub(crate) fn find_top_level_punctuation(
            input: Input<'_, '_>,
            delimiter: PunctuationKind,
        ) -> Option<usize> {
            let mut paren_depth = 0usize;
            let mut bracket_depth = 0usize;
            let mut brace_depth = 0usize;

            for (index, token) in input.tokens.iter().enumerate() {
                match token.punctuation() {
                    Some(PunctuationKind::LeftParen) => paren_depth += 1,
                    Some(PunctuationKind::RightParen) => {
                        paren_depth = paren_depth.saturating_sub(1)
                    }
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
                    Some(PunctuationKind::RightBrace) => {
                        brace_depth = brace_depth.saturating_sub(1)
                    }
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
    pub(crate) mod literals {
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
        use tokens::{FloatLiteralKind, IntegerLiteralKind, NumericBase};

        pub(crate) fn parse_integer_literal(
            text: &str,
            kind: IntegerLiteralKind,
        ) -> Result<IntegerLiteral, &'static str> {
            if kind.empty_digits {
                return Err("invalid integer literal");
            }

            let (radix, body) = match kind.base {
                NumericBase::Binary => (
                    IntegerRadix::Binary,
                    text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")),
                ),
                NumericBase::Octal => (
                    IntegerRadix::Octal,
                    text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")),
                ),
                NumericBase::Decimal => (IntegerRadix::Decimal, Some(text)),
                NumericBase::Hexadecimal => (
                    IntegerRadix::Hexadecimal,
                    text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")),
                ),
            };

            let body = body.ok_or("invalid integer literal")?;
            let (body, landed_type) = if kind.has_suffix {
                strip_integer_suffix(body)?
            } else {
                (body, None)
            };
            let literal = IntegerLiteral::from_parts(false, radix, body)?;
            Ok(match landed_type {
                Some(landed_type) => literal.with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                }),
                None => literal,
            })
        }

        pub(crate) fn validate_float_literal(
            text: &str,
            kind: FloatLiteralKind,
        ) -> Result<(), &'static str> {
            if kind.empty_exponent {
                return Err("invalid float literal");
            }
            if text.ends_with("real") || text.ends_with("Real") {
                return Err(
                    "the `real` literal suffix is retired; use an explicit f32/f64 format or a core Real embedding machine",
                );
            }
            if kind.has_suffix {
                strip_float_suffix(text)?;
            }

            Ok(())
        }

        fn strip_integer_suffix(
            text: &str,
        ) -> Result<(&str, Option<LandedIntegerType>), &'static str> {
            for (suffix, landed_type) in INTEGER_SUFFIXES {
                if let Some(digits) = text.strip_suffix(suffix) {
                    return Ok((digits, *landed_type));
                }
            }

            Err("unknown integer literal suffix")
        }

        fn strip_float_suffix(text: &str) -> Result<&str, &'static str> {
            for suffix in FLOAT_SUFFIXES {
                if let Some(digits) = text.strip_suffix(suffix) {
                    return Ok(digits);
                }
            }

            Err("unknown float literal suffix")
        }

        const INTEGER_SUFFIXES: &[(&str, Option<LandedIntegerType>)] = &[
            ("isize", None),
            ("usize", None),
            ("nat", None),
            ("Nat", None),
            ("i8", Some(LandedIntegerType::I8)),
            ("i16", Some(LandedIntegerType::I16)),
            ("i32", Some(LandedIntegerType::I32)),
            ("i64", Some(LandedIntegerType::I64)),
            ("u8", Some(LandedIntegerType::U8)),
            ("u16", Some(LandedIntegerType::U16)),
            ("u32", Some(LandedIntegerType::U32)),
            ("u64", Some(LandedIntegerType::U64)),
        ];

        const FLOAT_SUFFIXES: &[&str] = &["f32", "f64"];

        #[cfg(test)]
        mod tests {
            use super::{FloatLiteralKind, validate_float_literal};

            #[test]
            fn real_float_suffix_is_retired_with_a_directed_diagnostic() {
                let error = validate_float_literal(
                    "3.0real",
                    FloatLiteralKind {
                        has_suffix: true,
                        ..FloatLiteralKind::default()
                    },
                )
                .expect_err("Real is an ordinary core carrier, not a literal landing");

                assert!(error.contains("`real` literal suffix is retired"));
            }
        }
    }
    pub(crate) mod paths {
        use source::{SourceSpan, Span};
        use syntax_trees::identifier::Identifier;

        pub(crate) fn join_path_identifier(
            syntax_trees: &syntax_trees::SyntaxTrees,
            members: &[Identifier],
        ) -> Identifier {
            let mut name = String::new();

            for (index, member) in members.iter().enumerate() {
                if index > 0 {
                    name.push_str("::");
                }

                name.push_str(member.as_str(syntax_trees));
            }

            let first = members
                .first()
                .expect("joined machine path should contain a name")
                .source_span();
            let last = members
                .last()
                .expect("joined machine path should contain a name")
                .source_span();
            debug_assert_eq!(first.source_id, last.source_id);
            Identifier::new(
                name,
                SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end)),
            )
        }

        #[cfg(test)]
        mod tests {}
    }
    pub(crate) mod token_cursor {
        use crate::diagnostics::parse_error::ParseError;
        use crate::diagnostics::render_diagnostic as diagnostics;
        use crate::tokens::delimited::find_top_level_punctuation;
        use crate::tokens::literals::{parse_integer_literal, validate_float_literal};
        use arena::{Handle, HandleSpan};
        use language_core::ReferenceAccess;
        use numerics::literals::IntegerLiteral;
        use source::{SourceId, SourceSpan, Span};
        use syntax_trees::FloatSpelling;
        use syntax_trees::identifier::Identifier;
        use tokens::{KeywordKind, PunctuationKind, Token, TokenKind, TokenStream, TokenText};

        pub(crate) type ParseResult<'tokens, 'source, T> =
            Result<(T, Input<'tokens, 'source>), ParseError>;

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
                let consumed_count =
                    self.tokens.len().checked_sub(rest.tokens.len()).expect(
                        "the remaining parser cursor must descend from the starting cursor",
                    );
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
                        Some(PunctuationKind::Comma)
                            if paren == 1 && bracket == 0 && brace == 0 =>
                        {
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

            pub(crate) fn take_optional_label(
                self,
            ) -> Result<(Option<Identifier>, Self), ParseError> {
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

            fn take_parsed_integer(
                self,
                expected: &str,
            ) -> Result<(IntegerLiteral, Self), ParseError> {
                let (token, rest) = self.expect_token()?;
                let Some(kind) = token.integer_literal_kind() else {
                    return Err(diagnostics::expected(self, token, expected));
                };
                let literal =
                    parse_integer_literal(self.text(token).as_str(), kind).map_err(|message| {
                        ParseError::at_source_span(message, self.source_span(token))
                    })?;
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

            pub(crate) fn take_string_bytes(
                self,
            ) -> Result<(source::SourceBytes, Self), ParseError> {
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
                    validate_float_literal(self.text(token).as_str(), kind).map_err(|message| {
                        ParseError::at_source_span(message, self.source_span(token))
                    })?;
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

            pub(crate) fn take_reference_access(
                self,
            ) -> Result<(ReferenceAccess, Self), ParseError> {
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
                let split_index = find_top_level_punctuation(self, delimiter)
                    .ok_or_else(|| self.error_here(message))?;
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
            let mut path = crate::storage::handle_spans::HandleSpanBuilder::new();
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
    }
}
