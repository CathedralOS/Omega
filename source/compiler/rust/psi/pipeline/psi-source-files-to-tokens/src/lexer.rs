use std::iter::Peekable;
use std::str::CharIndices;

use crate::LexError;
use crate::lexer::strings::decode_string_literal;
use psi_source::Span;
use psi_tokens::{
    CommentKind, KeywordKind, PunctuationKind, Token, TokenKind, TokenStream, TokenText,
};
use unicode_ident::{is_xid_continue, is_xid_start};

mod numbers;
mod strings;

pub struct Lexer<'source> {
    source: &'source str,
    chars: Peekable<CharIndices<'source>>,
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

    pub fn tokenize(mut self) -> Result<TokenStream<'source>, LexError> {
        let estimated_token_count = self.source.len().saturating_div(4).max(16);
        let mut tokens = Vec::with_capacity(estimated_token_count);

        while let Some(token) = self.lex_next_token()? {
            tokens.push(self.build_token(token)?);
        }

        Ok(TokenStream::new(tokens))
    }

    fn lex_next_token(&mut self) -> Result<Option<LexedToken>, LexError> {
        let Some((start, character)) = self.chars.next() else {
            return Ok(None);
        };

        let token = if character.is_whitespace() {
            self.lex_whitespace(start, character)
        } else if character == '/' && self.peek_character() == Some('/') {
            self.lex_line_comment(start)
        } else if character == '/' && self.peek_character() == Some('*') {
            self.lex_block_comment(start)?
        } else if character == 'r' && matches!(self.peek_character(), Some('"') | Some('#')) {
            self.lex_raw_string_token(start)?
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
            if !next.is_whitespace() {
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
            return Err(LexError::new(
                format!("unsupported punctuation `{first}`"),
                Span::new(start, start + first.len_utf8()),
            ));
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
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || is_xid_start(character)
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || is_xid_continue(character)
}

#[cfg(test)]
mod tests;
