use source::Span;
use tokens::TokenKind;

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

    fn lex_cooked_string_end(&mut self, start: usize) -> Result<usize, LexError> {
        while let Some((index, character)) = self.chars.next() {
            if character == '"' {
                return Ok(index + character.len_utf8());
            }

            if matches!(character, '\r' | '\n') {
                return Err(LexError::outside_lexical_profile(Span::new(
                    index,
                    index + character.len_utf8(),
                )));
            }

            if character == '\\' {
                let Some((escape_index, escaped)) = self.chars.next() else {
                    return Err(LexError::new(
                        "unterminated string escape",
                        Span::new(start, self.source.len()),
                    ));
                };

                match escaped {
                    '"' | '\\' | 'n' | 'r' | 't' | '0' => {}
                    'x' => self.consume_hex_escape(start)?,
                    'u' => {
                        return Err(LexError::outside_lexical_profile(Span::new(
                            index,
                            escape_index + escaped.len_utf8(),
                        )));
                    }
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
}
