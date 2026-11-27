//! Validate and decode quoted-byte literals in one pass into the stream-owned byte buffer.

use source_text::Span;

use crate::{ByteScanner, LexError};

impl ByteScanner<'_> {
    pub(crate) fn scan_quoted(
        &mut self,
        start: usize,
        out: &mut Vec<u8>,
    ) -> Result<Span, LexError> {
        let bytes = self.source.as_bytes();
        while self.offset < bytes.len() {
            let rest = &bytes[self.offset..];
            let next = memchr::memchr3(b'"', b'\\', b'\n', rest).unwrap_or(rest.len());
            let chunk = &rest[..next];
            if let Some(cr) = memchr::memchr(b'\r', chunk) {
                let pos = self.offset + cr;
                return Err(LexError::outside_lexical_profile(Span::new(pos, pos + 1)));
            }
            out.extend_from_slice(chunk);
            self.offset += next;
            let Some(&delim) = bytes.get(self.offset) else {
                break;
            };
            match delim {
                b'"' => {
                    self.offset += 1;
                    return Ok(Span::new(start, self.offset));
                }
                b'\n' => {
                    return Err(LexError::outside_lexical_profile(Span::new(
                        self.offset,
                        self.offset + 1,
                    )));
                }
                b'\\' => {
                    let (decoded, after) = read_escape(self.source, self.offset, start)?;
                    out.push(decoded);
                    self.offset = after;
                }
                _ => unreachable!(),
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
