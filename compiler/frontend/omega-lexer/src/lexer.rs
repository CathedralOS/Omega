use std::iter::Peekable;
use std::str::CharIndices;

use crate::{LexError, Span, Token, TokenKind, TokenText};

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

    pub fn tokenize(mut self) -> Result<Vec<Token<'source>>, LexError> {
        let mut tokens = Vec::new();
        let source = self.source;

        while let Some((start, character)) = self.chars.next() {
            if character.is_whitespace() {
                continue;
            }

            if character == '/' && self.peek_character() == Some('/') {
                self.skip_line_comment();
                continue;
            }

            if character.is_ascii_alphabetic() || character == '_' {
                let end = self.lex_identifier_end(start, character);
                tokens.push(Token {
                    kind: TokenKind::Identifier,
                    lexeme: TokenText::source(&source[start..end]),
                    span: Span::new(start, end),
                });
                continue;
            }

            if character.is_ascii_digit() {
                let (kind, end) = self.lex_number(start, character);
                tokens.push(Token {
                    kind,
                    lexeme: TokenText::source(&source[start..end]),
                    span: Span::new(start, end),
                });
                continue;
            }

            if character == '"' {
                let (lexeme, span) = self.lex_string(start)?;
                tokens.push(Token {
                    kind: TokenKind::String,
                    lexeme: TokenText::owned(lexeme),
                    span,
                });
                continue;
            }

            let end = self.lex_symbol_end(start, character);
            tokens.push(Token {
                kind: TokenKind::Symbol,
                lexeme: TokenText::source(&source[start..end]),
                span: Span::new(start, end),
            });
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

    fn lex_identifier_end(&mut self, start: usize, first: char) -> usize {
        let mut end = start + first.len_utf8();

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        end
    }

    fn lex_number(&mut self, start: usize, first: char) -> (TokenKind, usize) {
        let mut end = start + first.len_utf8();
        let mut is_float = false;

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next.is_ascii_digit() {
                end = next_index + next.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        if self.peek_character() == Some('.') {
            let mut clone = self.chars.clone();
            clone.next();

            if matches!(clone.peek(), Some((_, character)) if character.is_ascii_digit()) {
                is_float = true;

                if let Some((dot_index, dot)) = self.chars.next() {
                    end = dot_index + dot.len_utf8();
                }

                while let Some((next_index, next)) = self.chars.peek().copied() {
                    if next.is_ascii_digit() {
                        end = next_index + next.len_utf8();
                        self.chars.next();
                    } else {
                        break;
                    }
                }

                if self.peek_character() == Some('f') {
                    if let Some((suffix_index, suffix)) = self.chars.next() {
                        end = suffix_index + suffix.len_utf8();
                    }
                }
            }
        }

        (
            if is_float {
                TokenKind::Float
            } else {
                TokenKind::Integer
            },
            end,
        )
    }

    fn lex_string(&mut self, start: usize) -> Result<(String, Span), LexError> {
        let mut lexeme = String::new();

        while let Some((index, character)) = self.chars.next() {
            if character == '"' {
                return Ok((lexeme, Span::new(start, index + character.len_utf8())));
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

    fn lex_symbol_end(&mut self, start: usize, first: char) -> usize {
        let mut end = start + first.len_utf8();

        if matches!(
            (first, self.peek_character()),
            (':', Some(':'))
                | ('-', Some('>'))
                | ('=', Some('='))
                | ('!', Some('='))
                | ('<', Some('='))
                | ('>', Some('='))
                | ('&', Some('&'))
                | ('|', Some('|'))
        ) {
            if let Some((next_index, next)) = self.chars.next() {
                end = next_index + next.len_utf8();
            }
        }

        end
    }

    fn peek_character(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, character)| *character)
    }
}
