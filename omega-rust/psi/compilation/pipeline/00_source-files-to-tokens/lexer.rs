//! Lex loaded sources into ordered token streams, preserving source spans and errors.
//! `lex_sources` owns the batch; `Lexer::fill_tokens` scans one source and publishes
//! tokens. Token dispatch leads into `literals` for numeric and quoted-byte rules.

#![forbid(unsafe_code)]

mod literals {
    mod numbers;
    pub(super) mod quoted_bytes;
}

use arena::Arena;
use diagnostics::Diagnostic;
use literals::quoted_bytes::decode_string_literal;
use rayon::prelude::*;
use source::{LoadedSource, LoadedSources, SourcePosition, Span};
use std::str::Chars;
use tokens::{
    CommentKind, KeywordKind, LexedSource, LexedSources, PunctuationKind, TokenKind, TokenStream,
};

pub fn lex_sources(mut sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let loaded_sources = sources.sources.span_mut_or_empty(sources.batch);
    let source_count = loaded_sources.len();
    let mut lexed_sources = Arena::with_capacity(source_count);
    let results: Vec<Result<LexedSource, Diagnostic>> = if source_count <= 1 {
        loaded_sources.iter_mut().map(lex_source).collect()
    } else {
        execution_admission::within_budget(|| {
            loaded_sources.par_iter_mut().map(lex_source).collect()
        })
    };
    let lexed = results
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| vec![error])?;

    let batch = lexed_sources.insert_many(lexed);

    Ok(LexedSources {
        sources: lexed_sources,
        batch,
    })
}

fn lex_source(loaded_source: &mut LoadedSource) -> Result<LexedSource, Diagnostic> {
    let tokens = Lexer::new(loaded_source.source.as_ref())
        .tokenize()
        .map_err(|error| {
            let position = SourcePosition::of(loaded_source.source.as_ref(), error.span.start);
            Diagnostic::error(format!(
                "{}:{}:{}: {}",
                loaded_source.path.display(),
                position.line,
                position.column,
                error.message
            ))
        })?;

    Ok(LexedSource {
        source_id: loaded_source.source_id,
        path: std::mem::take(&mut loaded_source.path),
        source: loaded_source.source.clone(),
        origin: loaded_source.origin,
        tokens: tokens.retain_source(loaded_source.source.clone()),
        scope: loaded_source.scope,
    })
}

pub struct Lexer<'source> {
    source: &'source str,
    chars: SourceCursor<'source>,
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
            chars: SourceCursor::new(source),
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

        while let Some((next_index, next)) = self.chars.peek() {
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
        self.chars.next();
        // Leave LF for whitespace scanning so every source byte retains a span.
        self.chars.skip_until(b'\n');
        LexedToken {
            kind: TokenKind::Comment(CommentKind::Line),
            span: Span::new(start, self.chars.offset()),
        }
    }

    fn lex_block_comment(&mut self, start: usize) -> Result<LexedToken, LexError> {
        self.chars.next();
        let mut depth = 1usize;

        loop {
            // Only delimiters can change nesting; UTF-8 payload bytes stay opaque.
            self.chars.skip_until_either(b'/', b'*');
            let Some((index, character)) = self.chars.next() else {
                break;
            };
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

        while let Some((next_index, next)) = self.chars.peek() {
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
        let Some((lexeme, kind)) = PunctuationKind::matching_prefix(rest) else {
            return Err(LexError::outside_lexical_profile(Span::new(
                start,
                start + first.len_utf8(),
            )));
        };

        for _ in 1..lexeme.len() {
            self.chars.next();
        }

        Ok(LexedToken {
            kind: TokenKind::Punctuation(kind),
            span: Span::new(start, start + lexeme.len()),
        })
    }

    pub(crate) fn peek_character(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, character)| character)
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

pub(crate) fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

pub(crate) fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn is_syntactic_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}

const OUTSIDE_LEXICAL_PROFILE_MESSAGE: &str = "spelling is outside the current language profile";

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

#[derive(Clone)]
struct SourceCursor<'source> {
    source_length: usize,
    chars: Chars<'source>,
}

impl<'source> SourceCursor<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            source_length: source.len(),
            chars: source.chars(),
        }
    }

    fn offset(&self) -> usize {
        self.source_length - self.chars.as_str().len()
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars
            .clone()
            .next()
            .map(|character| (self.offset(), character))
    }

    // Callers search ASCII delimiters, whose byte positions are UTF-8 boundaries.
    fn skip_until(&mut self, byte: u8) {
        debug_assert!(byte.is_ascii());
        let remaining = self.chars.as_str();
        let offset = memchr::memchr(byte, remaining.as_bytes()).unwrap_or(remaining.len());
        self.chars = remaining[offset..].chars();
    }

    fn skip_until_either(&mut self, first: u8, second: u8) {
        debug_assert!(first.is_ascii() && second.is_ascii());
        let remaining = self.chars.as_str();
        let offset =
            memchr::memchr2(first, second, remaining.as_bytes()).unwrap_or(remaining.len());
        self.chars = remaining[offset..].chars();
    }
}

impl Iterator for SourceCursor<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.offset();
        self.chars.next().map(|character| (offset, character))
    }
}
