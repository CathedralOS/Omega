use psi_source::Span;
use psi_tokens::TokenKind;

use crate::LexError;

use super::LexedToken;

mod decode;

pub(in crate::lexer) use decode::decode_string_literal;

impl<'source> super::Lexer<'source> {
    pub(super) fn lex_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_cooked_string_end(start)?;
        decode_string_literal(&self.source[start..end], start)?;
        Ok(LexedToken {
            kind: TokenKind::StringLiteral,
            span: Span::new(start, end),
        })
    }

    pub(super) fn lex_raw_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_raw_string_end(start)?;
        decode_string_literal(&self.source[start..end], start)?;
        Ok(LexedToken {
            kind: TokenKind::StringLiteral,
            span: Span::new(start, end),
        })
    }

    fn lex_cooked_string_end(&mut self, start: usize) -> Result<usize, LexError> {
        while let Some((index, character)) = self.chars.next() {
            if character == '"' {
                return Ok(index + character.len_utf8());
            }

            if character == '\\' {
                let Some((escape_index, escaped)) = self.chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(start, self.source.len()),
                    ));
                };

                match escaped {
                    '"' | '\'' | '\\' | 'n' | 'r' | 't' | '0' => {}
                    'x' => self.consume_hex_escape(start)?,
                    'u' => self.consume_unicode_escape(start, escape_index)?,
                    other => {
                        return Err(LexError::new(
                            format!("unsupported escape sequence `\\{other}`"),
                            Span::new(index, escape_index + other.len_utf8()),
                        ));
                    }
                }
            }
        }

        Err(LexError::new(
            "unterminated string literal",
            Span::new(start, self.source.len()),
        ))
    }

    fn lex_raw_string_end(&mut self, start: usize) -> Result<usize, LexError> {
        let mut hash_count = 0usize;

        while self.peek_character() == Some('#') {
            self.chars.next();
            hash_count += 1;
        }

        if self.peek_character() != Some('"') {
            return Err(LexError::new(
                "invalid raw string delimiter",
                Span::new(start, self.source.len().min(start + 2 + hash_count)),
            ));
        }
        self.chars.next();

        while let Some((index, character)) = self.chars.next() {
            if character != '"' {
                continue;
            }

            let mut clone = self.chars.clone();
            let mut matched = true;
            let mut end = index + character.len_utf8();

            for _ in 0..hash_count {
                match clone.next() {
                    Some((hash_index, '#')) => {
                        end = hash_index + '#'.len_utf8();
                    }
                    _ => {
                        matched = false;
                        break;
                    }
                }
            }

            if matched {
                for _ in 0..hash_count {
                    self.chars.next();
                }
                return Ok(end);
            }
        }

        Err(LexError::new(
            "unterminated raw string literal",
            Span::new(start, self.source.len()),
        ))
    }

    fn consume_hex_escape(&mut self, string_start: usize) -> Result<(), LexError> {
        for _ in 0..2 {
            let Some((digit_index, digit)) = self.chars.next() else {
                return Err(LexError::new(
                    "unterminated hex escape",
                    Span::new(string_start, self.source.len()),
                ));
            };
            if !digit.is_ascii_hexdigit() {
                return Err(LexError::new(
                    "invalid hex escape digit",
                    Span::new(digit_index, digit_index + digit.len_utf8()),
                ));
            }
        }

        Ok(())
    }

    fn consume_unicode_escape(
        &mut self,
        string_start: usize,
        escape_index: usize,
    ) -> Result<(), LexError> {
        let Some((brace_index, brace)) = self.chars.next() else {
            return Err(LexError::new(
                "unterminated unicode escape",
                Span::new(string_start, self.source.len()),
            ));
        };
        if brace != '{' {
            return Err(LexError::new(
                "expected `{` after `\\u`",
                Span::new(brace_index, brace_index + brace.len_utf8()),
            ));
        }

        let mut saw_digit = false;
        while let Some((digit_index, digit)) = self.chars.next() {
            if digit == '}' {
                return if saw_digit {
                    Ok(())
                } else {
                    Err(LexError::new(
                        "empty unicode escape",
                        Span::new(escape_index, digit_index + digit.len_utf8()),
                    ))
                };
            }

            if !digit.is_ascii_hexdigit() {
                return Err(LexError::new(
                    "invalid unicode escape digit",
                    Span::new(digit_index, digit_index + digit.len_utf8()),
                ));
            }

            saw_digit = true;
        }

        Err(LexError::new(
            "unterminated unicode escape",
            Span::new(string_start, self.source.len()),
        ))
    }
}
