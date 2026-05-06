use std::iter::Peekable;
use std::str::CharIndices;

use crate::lexer::{LexError, Span, Token, TokenKind};

pub struct Lexer<'source> {
    source: &'source str,
    chars: Peekable<CharIndices<'source>>,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while let Some((start, character)) = self.chars.next() {
            if character.is_whitespace() {
                continue;
            }

            if character == '/' && self.peek_character() == Some('/') {
                self.skip_line_comment();
                continue;
            }

            if character.is_ascii_alphabetic() || character == '_' {
                tokens.push(self.lex_identifier(start, character));
                continue;
            }

            if character.is_ascii_digit() {
                tokens.push(self.lex_integer(start, character));
                continue;
            }

            if character == '"' {
                tokens.push(self.lex_string(start)?);
                continue;
            }

            tokens.push(self.lex_symbol(start, character));
        }

        Ok(tokens)
    }

    fn skip_line_comment(&mut self) {
        for (_, character) in self.chars.by_ref() {
            if character == '\n' {
                break;
            }
        }
    }

    fn lex_identifier(&mut self, start: usize, first: char) -> Token {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        Token {
            kind: TokenKind::Identifier,
            lexeme: self.source[start..end].to_owned(),
            span: Span::new(start, end),
        }
    }

    fn lex_integer(&mut self, start: usize, first: char) -> Token {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next.is_ascii_digit() {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        Token {
            kind: TokenKind::Integer,
            lexeme: self.source[start..end].to_owned(),
            span: Span::new(start, end),
        }
    }

    fn lex_string(&mut self, start: usize) -> Result<Token, LexError> {
        let mut lexeme = String::new();

        while let Some((index, character)) = self.chars.next() {
            if character == '"' {
                return Ok(Token {
                    kind: TokenKind::String,
                    lexeme,
                    span: Span::new(start, index + character.len_utf8()),
                });
            }

            if character == '\\' {
                let Some((_, escaped)) = self.chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(start, self.source.len()),
                    ));
                };

                match escaped {
                    '"' => lexeme.push('"'),
                    '\\' => lexeme.push('\\'),
                    'n' => lexeme.push('\n'),
                    't' => lexeme.push('\t'),
                    other => {
                        return Err(LexError::new(
                            format!("unsupported escape sequence `\\{other}`"),
                            Span::new(index, index + other.len_utf8()),
                        ));
                    }
                }
            } else {
                lexeme.push(character);
            }
        }

        Err(LexError::new(
            "unterminated string literal",
            Span::new(start, self.source.len()),
        ))
    }

    fn lex_symbol(&mut self, start: usize, first: char) -> Token {
        let mut end = start + first.len_utf8();

        if (first == ':' && self.peek_character() == Some(':'))
            || (first == '-' && self.peek_character() == Some('>'))
        {
            if let Some((next_index, next)) = self.chars.next() {
                end = next_index + next.len_utf8();
            }
        }

        Token {
            kind: TokenKind::Symbol,
            lexeme: self.source[start..end].to_owned(),
            span: Span::new(start, end),
        }
    }

    fn peek_character(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, character)| *character)
    }
}
