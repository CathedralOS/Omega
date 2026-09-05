use std::iter::Peekable;
use std::str::CharIndices;

use crate::LexError;
use crate::lexer::strings::decode_string_literal;
use source::Span;
use tokens::{CommentKind, KeywordKind, PunctuationKind, Token, TokenKind, TokenStream, TokenText};

mod numbers;
mod strings;

pub struct Lexer<'source> {
    source: &'source str,
    chars: Peekable<CharIndices<'source>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokenization<'source> {
    pub tokens: TokenStream<'source>,
    pub diagnostic: Option<LexError>,
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
            chars: source.char_indices().peekable(),
        }
    }

    pub fn tokenize(self) -> Result<TokenStream<'source>, LexError> {
        let observation = self.tokenize_with_diagnostic();
        match observation.diagnostic {
            Some(error) => Err(error),
            None => Ok(observation.tokens),
        }
    }

    /// Retains the successfully completed token prefix when lexical analysis
    /// rejects. This is observation data only; callers must still treat a
    /// present diagnostic as rejection rather than a usable token stream.
    pub fn tokenize_with_diagnostic(mut self) -> Tokenization<'source> {
        let estimated_token_count = self.source.len().saturating_div(4).max(16);
        let mut tokens = Vec::with_capacity(estimated_token_count);

        loop {
            match self.lex_next_token() {
                Ok(Some(token)) => match self.build_token(token) {
                    Ok(token) => tokens.push(token),
                    Err(error) => {
                        return Tokenization {
                            tokens: TokenStream::new(tokens),
                            diagnostic: Some(error),
                        };
                    }
                },
                Ok(None) => {
                    return Tokenization {
                        tokens: TokenStream::new(tokens),
                        diagnostic: None,
                    };
                }
                Err(error) => {
                    return Tokenization {
                        tokens: TokenStream::new(tokens),
                        diagnostic: Some(error),
                    };
                }
            }
        }
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

    fn build_token(&self, token: LexedToken) -> Result<Token<'source>, LexError> {
        let lexeme = match token.kind {
            TokenKind::StringLiteral => {
                let raw = &self.source[token.span.start..token.span.end];
                let value = decode_string_literal(raw, token.span.start)?;
                TokenText::owned_bytes(value)
            }
            _ => TokenText::source(&self.source[token.span.start..token.span.end]),
        };

        Ok(Token {
            kind: token.kind,
            lexeme,
            span: token.span,
        })
    }

    fn lex_whitespace(&mut self, start: usize, first: char) -> LexedToken {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
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
        let mut end = start + '/'.len_utf8();
        self.chars.next();
        end += '/'.len_utf8();

        for (_, character) in self.chars.by_ref() {
            if character == '\n' {
                break;
            }
            end += character.len_utf8();
        }

        LexedToken {
            kind: TokenKind::Comment(CommentKind::Line),
            span: Span::new(start, end),
        }
    }

    fn lex_block_comment(&mut self, start: usize) -> Result<LexedToken, LexError> {
        self.chars.next();
        let mut depth = 1usize;

        while let Some((index, character)) = self.chars.next() {
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

        while let Some((next_index, next)) = self.chars.peek().copied() {
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
        let Some((lexeme, kind)) = PunctuationKind::ordered_lexemes()
            .iter()
            .copied()
            .find(|(lexeme, _)| self.source[start..].starts_with(lexeme))
        else {
            return Err(LexError::outside_lexical_profile(Span::new(
                start,
                start + first.len_utf8(),
            )));
        };

        let mut consumed = 1usize;
        while consumed < lexeme.chars().count() {
            self.chars.next();
            consumed += 1;
        }

        Ok(LexedToken {
            kind: TokenKind::Punctuation(kind),
            span: Span::new(start, start + lexeme.len()),
        })
    }

    fn peek_character(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, character)| *character)
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

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn is_syntactic_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}

#[cfg(test)]
mod tests;
