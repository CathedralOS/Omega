//! The escape rules of a quoted literal, shared by the scanner that validates and the decoder.

/// Why an escape sequence is not one of the seven the spec allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EscapeProblem {
    /// The source ended right after the backslash.
    EndOfSource,
    /// `\u`: quoted literals are bytes, and the spec rejects codepoint escapes.
    Codepoint,
    /// `\x` not followed by two hexadecimal digits.
    InvalidHex,
    /// A backslash followed by anything else.
    Unknown,
}

/// Reads one escape sequence. `escape_bytes` must start at the backslash.
///
/// Returns the byte the escape denotes and how many source bytes spell it. Both the scanner and
/// `decode_quoted_bytes` call this, so there is one table of escapes and they cannot drift apart.
pub(crate) fn read_escape(escape_bytes: &[u8]) -> Result<(u8, usize), EscapeProblem> {
    let Some(&escape_letter) = escape_bytes.get(1) else {
        return Err(EscapeProblem::EndOfSource);
    };

    match escape_letter {
        b'n' => Ok((b'\n', 2)),
        b'r' => Ok((b'\r', 2)),
        b't' => Ok((b'\t', 2)),
        b'0' => Ok((0, 2)),
        b'\\' => Ok((b'\\', 2)),
        b'"' => Ok((b'"', 2)),
        b'x' => {
            let high_digit = escape_bytes.get(2).copied().and_then(get_hex_digit_value);
            let low_digit = escape_bytes.get(3).copied().and_then(get_hex_digit_value);
            match (high_digit, low_digit) {
                (Some(high_digit), Some(low_digit)) => Ok(((high_digit << 4) | low_digit, 4)),
                _ => Err(EscapeProblem::InvalidHex),
            }
        }
        b'u' => Err(EscapeProblem::Codepoint),
        _ => Err(EscapeProblem::Unknown),
    }
}

/// Computes the bytes a quoted literal denotes from its spelling, quotes included.
///
/// Every byte between the quotes is copied as written, except escape sequences, which become the
/// byte they name. Returns None if `quoted_spelling` is not a well-formed quoted literal, which
/// cannot happen for the spelling of a `TokenKind::Quoted` token.
pub fn decode_quoted_bytes(quoted_spelling: &[u8]) -> Option<Vec<u8>> {
    let body_bytes = quoted_spelling.strip_prefix(b"\"")?.strip_suffix(b"\"")?;
    let mut decoded_bytes = Vec::with_capacity(body_bytes.len());
    let mut body_offset = 0;

    while let Some(&body_byte) = body_bytes.get(body_offset) {
        match body_byte {
            b'\\' => {
                let (decoded_byte, spelled_byte_count) =
                    read_escape(&body_bytes[body_offset..]).ok()?;
                decoded_bytes.push(decoded_byte);
                body_offset += spelled_byte_count;
            }
            // An unescaped quote or line break inside the body means this was never one literal.
            b'"' | b'\n' | b'\r' => return None,
            _ => {
                decoded_bytes.push(body_byte);
                body_offset += 1;
            }
        }
    }

    Some(decoded_bytes)
}

fn get_hex_digit_value(digit_byte: u8) -> Option<u8> {
    match digit_byte {
        b'0'..=b'9' => Some(digit_byte - b'0'),
        b'a'..=b'f' => Some(digit_byte - b'a' + 10),
        b'A'..=b'F' => Some(digit_byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::decode_quoted_bytes;

    #[test]
    fn decodes_every_escape_the_spec_lists() {
        assert_eq!(
            decode_quoted_bytes(br#""a\n\r\t\0\\\"\x41\xfF""#),
            Some(vec![b'a', 0x0A, 0x0D, 0x09, 0x00, 0x5C, 0x22, 0x41, 0xFF])
        );
    }

    #[test]
    fn copies_non_ascii_bytes_without_interpreting_them() {
        assert_eq!(
            decode_quoted_bytes("\"h\u{e9}\"".as_bytes()),
            Some(vec![b'h', 0xC3, 0xA9])
        );
    }

    #[test]
    fn refuses_spellings_that_are_not_one_literal() {
        assert_eq!(decode_quoted_bytes(b"abc"), None);
        assert_eq!(decode_quoted_bytes(b"\"abc"), None);
        assert_eq!(decode_quoted_bytes(br#""a"b""#), None);
        assert_eq!(decode_quoted_bytes(br#""\q""#), None);
        assert_eq!(decode_quoted_bytes(br#""\x4""#), None);
        assert_eq!(decode_quoted_bytes(b"\""), None);
    }
}
