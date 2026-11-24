//! Validate quoted-byte escapes before decoding runs into the stream-owned byte buffer.

use source_text::Span;
use tokens::TokenKind;

use crate::{ByteScanner, LexError, LexedToken};

pub(crate) fn decode_string_literal(raw: &str, bytes: &mut Vec<u8>) {
    let end = raw.len() - 1;
    bytes.reserve(end - 1);
    let mut offset = 1;
    while let Some(relative) = memchr::memchr(b'\\', &raw.as_bytes()[offset..end]) {
        let escape = offset + relative;
        bytes.extend_from_slice(&raw.as_bytes()[offset..escape]);
        let (decoded, next) =
            read_escape(raw, escape, 0).expect("valid string token contains a checked escape");
        bytes.push(decoded);
        offset = next;
    }
    bytes.extend_from_slice(&raw.as_bytes()[offset..end]);
}

impl ByteScanner<'_> {
    pub(crate) fn scan_quoted(&mut self, start: usize) -> Result<LexedToken, LexError> {
        while let Some(byte) = self.peek() {
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(LexedToken {
                        kind: TokenKind::StringLiteral,
                        span: Span::new(start, self.offset),
                    });
                }
                b'\r' | b'\n' => {
                    return Err(LexError::outside_lexical_profile(Span::new(
                        self.offset,
                        self.offset + 1,
                    )));
                }
                b'\\' => {
                    let (_, next) = read_escape(self.source, self.offset, start)?;
                    self.offset = next;
                }
                _ => self.offset += 1,
            }
        }
        Err(LexError::new(
            "unterminated string literal",
            Span::new(start, self.offset),
        ))
    }
}

/// Returns the decoded byte and the offset of the next source byte.
fn read_escape(source: &str, slash: usize, string_start: usize) -> Result<(u8, usize), LexError> {
    let bytes = source.as_bytes();
    let Some(&escaped) = bytes.get(slash + 1) else {
        return Err(LexError::new(
            "unterminated string escape",
            Span::new(string_start, bytes.len()),
        ));
    };
    let byte = match escaped {
        b'"' | b'\\' => escaped,
        b'n' => b'\n',
        b'r' => b'\r',
        b't' => b'\t',
        b'0' => 0,
        b'x' => {
            let mut value = 0;
            for offset in slash + 2..slash + 4 {
                let Some(&digit) = bytes.get(offset) else {
                    return Err(LexError::new(
                        "unterminated hex escape",
                        Span::new(string_start, bytes.len()),
                    ));
                };
                let nibble = match digit {
                    b'0'..=b'9' => digit - b'0',
                    b'a'..=b'f' => digit - b'a' + 10,
                    b'A'..=b'F' => digit - b'A' + 10,
                    _ => {
                        let character = source[offset..].chars().next().unwrap();
                        return Err(LexError::new(
                            "invalid hex escape digit",
                            Span::new(offset, offset + character.len_utf8()),
                        ));
                    }
                };
                value = (value << 4) | nibble;
            }
            return Ok((value, slash + 4));
        }
        b'u' => {
            return Err(LexError::outside_lexical_profile(Span::new(
                slash,
                slash + 2,
            )));
        }
        _ => {
            let character = source[slash + 1..].chars().next().unwrap();
            return Err(LexError::new(
                format!("unsupported escape sequence `\\{character}`"),
                Span::new(slash, slash + 1 + character.len_utf8()),
            ));
        }
    };
    Ok((byte, slash + 2))
}
