//! Lex loaded sources into ordered token streams, preserving source spans and errors.
//! `lex_file` and `lex_unit_reusing` scan one source into reusable buffers; token
//! dispatch leads into `literals` for numeric and quoted-byte rules.

#![forbid(unsafe_code)]

mod literals {
    mod numbers;
    mod quoted_bytes;
}

use diagnostics::Diagnostic;
use source_files::LoadedSource;
use source_text::{SourcePosition, Span};
use tokens::{CommentKind, KeywordKind, LexedSource, PunctuationKind, TokenKind, TokenStream};

/// Move this file's identity and path into its tokens. A streaming caller
/// recycles the buffers after the parser has consumed the file.
pub fn lex_file(
    loaded_source: &mut LoadedSource,
    scratch: &mut tokens::TokenScratch,
) -> Result<LexedSource, LexError> {
    let tokens = lex_unit_reusing(&loaded_source.source, scratch).map_err(|mut error| {
        let position = SourcePosition::of(&loaded_source.source, error.span.start);
        error.location = Some((loaded_source.path.clone(), position));
        error
    })?;

    Ok(LexedSource {
        source_id: loaded_source.source_id,
        path: std::mem::take(&mut loaded_source.path),
        origin: loaded_source.origin,
        tokens: tokens.retain_source(loaded_source.source.clone()),
        scope: loaded_source.scope,
    })
}

/// Scans one admitted source using the current page's token buffers.
pub fn lex_unit_reusing<'source>(
    source: &'source str,
    scratch: &mut tokens::TokenScratch,
) -> Result<TokenStream<'source>, LexError> {
    let capacity = source.len().saturating_div(4).max(16);
    let mut tokens = TokenStream::with_scratch(source, capacity, scratch);
    match ByteScanner::new(source).fill_tokens(&mut tokens) {
        Ok(()) => Ok(tokens),
        Err(error) => {
            tokens.recycle(scratch);
            Err(error)
        }
    }
}

/// Source loading has already admitted UTF-8. Syntax delimiters are ASCII, so
/// the scanner can skip comment and literal contents using byte offsets.
struct ByteScanner<'source> {
    source: &'source str,
    offset: usize,
}

impl<'source> ByteScanner<'source> {
    fn new(source: &'source str) -> Self {
        Self { source, offset: 0 }
    }

    fn bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    fn peek(&self) -> Option<u8> {
        self.peek_ahead(0)
    }

    fn peek_ahead(&self, distance: usize) -> Option<u8> {
        self.bytes().get(self.offset + distance).copied()
    }

    fn fill_tokens(mut self, tokens: &mut TokenStream<'source>) -> Result<(), LexError> {
        while let Some(first) = self.peek() {
            let start = self.offset;
            self.offset += 1;
            if first == b'"' {
                tokens.try_push_decoded(|out| self.scan_quoted(start, out))?;
            } else {
                let kind = self.scan_token(start, first)?;
                tokens.push_source(kind, Span::new(start, self.offset));
            }
        }
        Ok(())
    }

    fn scan_token(&mut self, start: usize, first: u8) -> Result<TokenKind, LexError> {
        let kind = match first {
            b' ' | b'\t' | b'\r' | b'\n' => {
                while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                    self.offset += 1;
                }
                TokenKind::Whitespace
            }
            b'/' if self.peek() == Some(b'/') => {
                // Leave the line feed for the next whitespace token.
                self.offset += 1;
                let rest = &self.bytes()[self.offset..];
                self.offset += memchr::memchr(b'\n', rest).unwrap_or(rest.len());
                TokenKind::Comment(CommentKind::Line)
            }
            b'/' if self.peek() == Some(b'*') => {
                self.offset += 1;
                self.scan_block_comment(start)?;
                TokenKind::Comment(CommentKind::Block)
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                if first == b'r'
                    && let Some(end) = self.raw_quote_end()
                {
                    return Err(LexError::outside_lexical_profile(Span::new(start, end)));
                }
                while self.peek().is_some_and(is_identifier_continue) {
                    self.offset += 1;
                }
                KeywordKind::from_lexeme(&self.source[start..self.offset])
                    .map(TokenKind::Keyword)
                    .unwrap_or(TokenKind::Identifier)
            }
            b'0'..=b'9' => self.scan_number(first),
            b'.' if self.peek().is_some_and(|next| next.is_ascii_digit()) => {
                self.scan_number(first)
            }
            _ => {
                let Some((lexeme, kind)) = PunctuationKind::matching_prefix(&self.bytes()[start..])
                else {
                    let character = self.source[start..].chars().next().unwrap();
                    return Err(LexError::outside_lexical_profile(Span::new(
                        start,
                        start + character.len_utf8(),
                    )));
                };
                self.offset = start + lexeme.len();
                TokenKind::Punctuation(kind)
            }
        };
        Ok(kind)
    }

    fn scan_block_comment(&mut self, start: usize) -> Result<(), LexError> {
        let mut depth = 1usize;
        while let Some(distance) = memchr::memchr2(b'/', b'*', &self.bytes()[self.offset..]) {
            self.offset += distance;
            let delimiter = self.bytes()[self.offset];
            self.offset += 1;
            match (delimiter, self.peek()) {
                (b'/', Some(b'*')) => {
                    depth += 1;
                    self.offset += 1;
                }
                (b'*', Some(b'/')) => {
                    depth -= 1;
                    self.offset += 1;
                    if depth == 0 {
                        return Ok(());
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

    fn raw_quote_end(&self) -> Option<usize> {
        let mut end = self.offset;
        while self.bytes().get(end) == Some(&b'#') {
            end += 1;
        }
        (self.bytes().get(end) == Some(&b'"')).then_some(end + 1)
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

const OUTSIDE_LEXICAL_PROFILE_MESSAGE: &str = "spelling is outside the current language profile";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
    location: Option<(std::path::PathBuf, SourcePosition)>,
}

impl LexError {
    fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            location: None,
        }
    }

    pub(crate) fn outside_lexical_profile(span: Span) -> Self {
        Self::new(OUTSIDE_LEXICAL_PROFILE_MESSAGE, span)
    }
}

impl LexError {
    /// Formats the source location captured by the stage with its error message.
    pub fn into_diagnostic(self) -> Diagnostic {
        match self.location {
            Some((path, position)) => Diagnostic::error(format!(
                "{}:{}:{}: {}",
                path.display(),
                position.line,
                position.column,
                self.message
            )),
            None => Diagnostic::error(self.message),
        }
    }
}
