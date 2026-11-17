//! Validate quoted-byte escapes before decoding runs into the stream-owned byte buffer.

use source::Span;
use tokens::TokenKind;

use crate::LexError;

use crate::{LexedToken, Lexer};

pub(crate) fn decode_string_literal(raw: &str, bytes: &mut Vec<u8>) {
    let contents = &raw.as_bytes()[1..raw.len() - 1];
    bytes.reserve(contents.len());
    let mut rest = contents;

    while let Some(escape) = rest.iter().position(|&byte| byte == b'\\') {
        bytes.extend_from_slice(&rest[..escape]);
        let (decoded, spelled_length) = match rest[escape + 1] {
            b'"' => (b'"', 2),
            b'\\' => (b'\\', 2),
            b'n' => (b'\n', 2),
            b'r' => (b'\r', 2),
            b't' => (b'\t', 2),
            b'0' => (b'\0', 2),
            b'x' => {
                let high = hex_digit_value(rest[escape + 2]);
                let low = hex_digit_value(rest[escape + 3]);
                ((high << 4) | low, 4)
            }
            other => {
                unreachable!("validated string literal contains escape byte {other:#04x}")
            }
        };
        bytes.push(decoded);
        rest = &rest[escape + spelled_length..];
    }
    bytes.extend_from_slice(rest);
}

fn hex_digit_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        b'A'..=b'F' => digit - b'A' + 10,
        other => unreachable!("validated hex escape contains byte {other:#04x}"),
    }
}

impl<'source> Lexer<'source> {
    pub(crate) fn lex_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_cooked_string_end(start)?;
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
