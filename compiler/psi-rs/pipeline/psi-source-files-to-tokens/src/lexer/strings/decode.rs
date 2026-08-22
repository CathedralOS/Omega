use std::str::CharIndices;

use psi_source::Span;

use crate::LexError;

pub(in crate::lexer) fn decode_string_literal(
    raw: &str,
    source_start: usize,
) -> Result<Vec<u8>, LexError> {
    if raw.starts_with('r') {
        return Ok(decode_raw_string_literal(raw));
    }

    decode_cooked_string_literal(raw, source_start)
}

fn decode_cooked_string_literal(raw: &str, source_start: usize) -> Result<Vec<u8>, LexError> {
    let mut lexeme = Vec::new();
    let mut chars = raw.char_indices();

    let Some((_, opening_quote)) = chars.next() else {
        return Ok(lexeme);
    };
    debug_assert_eq!(opening_quote, '"');

    while let Some((index, character)) = chars.next() {
        if character == '"' {
            return Ok(lexeme);
        }

        if character == '\\' {
            let Some((escape_index, escaped)) = chars.next() else {
                return Err(LexError::new(
                    "unterminated string escape",
                    Span::new(source_start, source_start + raw.len()),
                ));
            };

            match escaped {
                '"' => lexeme.push(b'"'),
                '\'' => lexeme.push(b'\''),
                '\\' => lexeme.push(b'\\'),
                'n' => lexeme.push(b'\n'),
                'r' => lexeme.push(b'\r'),
                't' => lexeme.push(b'\t'),
                '0' => lexeme.push(b'\0'),
                'x' => lexeme.push(decode_hex_escape(&mut chars, source_start + index)?),
                'u' => push_character(
                    &mut lexeme,
                    decode_unicode_escape(&mut chars, source_start + index)?,
                ),
                other => {
                    return Err(LexError::new(
                        format!("unsupported escape sequence `\\{other}`"),
                        Span::new(
                            source_start + index,
                            source_start + escape_index + other.len_utf8(),
                        ),
                    ));
                }
            }
        } else {
            push_character(&mut lexeme, character);
        }
    }

    Err(LexError::new(
        "unterminated string literal",
        Span::new(source_start, source_start + raw.len()),
    ))
}

fn decode_raw_string_literal(raw: &str) -> Vec<u8> {
    let bytes = raw.as_bytes();
    let mut hash_count = 0usize;

    while 1 + hash_count < bytes.len() && bytes[1 + hash_count] == b'#' {
        hash_count += 1;
    }

    let content_start = 1 + hash_count + 1;
    let content_end = raw.len() - hash_count - 1;
    raw[content_start..content_end].as_bytes().to_vec()
}

fn decode_hex_escape(chars: &mut CharIndices<'_>, source_index: usize) -> Result<u8, LexError> {
    let mut value = 0u8;
    for _ in 0..2 {
        let Some((offset, digit)) = chars.next() else {
            return Err(LexError::new(
                "unterminated hex escape",
                Span::new(source_index, source_index + 2),
            ));
        };
        let Some(parsed) = digit.to_digit(16) else {
            return Err(LexError::new(
                "invalid hex escape digit",
                Span::new(
                    source_index + offset,
                    source_index + offset + digit.len_utf8(),
                ),
            ));
        };
        value = (value << 4) | parsed as u8;
    }

    Ok(value)
}

fn push_character(bytes: &mut Vec<u8>, character: char) {
    let mut encoded = [0u8; 4];
    bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
}

fn decode_unicode_escape(
    chars: &mut CharIndices<'_>,
    source_index: usize,
) -> Result<char, LexError> {
    let Some((brace_offset, brace)) = chars.next() else {
        return Err(LexError::new(
            "unterminated unicode escape",
            Span::new(source_index, source_index + 2),
        ));
    };
    if brace != '{' {
        return Err(LexError::new(
            "expected `{` after `\\u`",
            Span::new(
                source_index + brace_offset,
                source_index + brace_offset + brace.len_utf8(),
            ),
        ));
    }

    let mut digits = String::new();
    while let Some((offset, digit)) = chars.next() {
        if digit == '}' {
            if digits.is_empty() {
                return Err(LexError::new(
                    "empty unicode escape",
                    Span::new(source_index, source_index + offset + digit.len_utf8()),
                ));
            }

            let value = u32::from_str_radix(&digits, 16).map_err(|_| {
                LexError::new(
                    "invalid unicode escape value",
                    Span::new(source_index, source_index + offset + digit.len_utf8()),
                )
            })?;

            return char::from_u32(value).ok_or_else(|| {
                LexError::new(
                    "unicode escape is not a valid scalar value",
                    Span::new(source_index, source_index + offset + digit.len_utf8()),
                )
            });
        }

        if !digit.is_ascii_hexdigit() {
            return Err(LexError::new(
                "invalid unicode escape digit",
                Span::new(
                    source_index + offset,
                    source_index + offset + digit.len_utf8(),
                ),
            ));
        }

        digits.push(digit);
    }

    Err(LexError::new(
        "unterminated unicode escape",
        Span::new(source_index, source_index + 2 + digits.len()),
    ))
}
