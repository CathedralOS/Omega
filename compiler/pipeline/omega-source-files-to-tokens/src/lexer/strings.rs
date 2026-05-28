use std::str::CharIndices;

use omega_core::Span;
use omega_tokens::TokenKind;

use crate::LexError;

use super::LexedToken;

impl<'source> super::Lexer<'source> {
    pub(super) fn lex_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_cooked_string_end(start)?;
        self.decode_string_literal(&self.source[start..end], start)?;
        Ok(LexedToken {
            kind: TokenKind::StringLiteral,
            span: Span::new(start, end),
        })
    }

    pub(super) fn lex_raw_string_token(&mut self, start: usize) -> Result<LexedToken, LexError> {
        let end = self.lex_raw_string_end(start)?;
        self.decode_string_literal(&self.source[start..end], start)?;
        Ok(LexedToken {
            kind: TokenKind::StringLiteral,
            span: Span::new(start, end),
        })
    }

    pub(super) fn decode_string_literal(
        &self,
        raw: &str,
        source_start: usize,
    ) -> Result<String, LexError> {
        if raw.starts_with('r') {
            return Ok(self.decode_raw_string_literal(raw));
        }

        self.decode_cooked_string_literal(raw, source_start)
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

    fn decode_cooked_string_literal(
        &self,
        raw: &str,
        source_start: usize,
    ) -> Result<String, LexError> {
        let mut lexeme = String::new();
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
                    '"' => lexeme.push('"'),
                    '\'' => lexeme.push('\''),
                    '\\' => lexeme.push('\\'),
                    'n' => lexeme.push('\n'),
                    'r' => lexeme.push('\r'),
                    't' => lexeme.push('\t'),
                    '0' => lexeme.push('\0'),
                    'x' => lexeme.push(self.decode_hex_escape(&mut chars, source_start + index)?),
                    'u' => {
                        lexeme.push(self.decode_unicode_escape(&mut chars, source_start + index)?)
                    }
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
                lexeme.push(character);
            }
        }

        Err(LexError::new(
            "unterminated string literal",
            Span::new(source_start, source_start + raw.len()),
        ))
    }

    fn decode_raw_string_literal(&self, raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut hash_count = 0usize;

        while 1 + hash_count < bytes.len() && bytes[1 + hash_count] == b'#' {
            hash_count += 1;
        }

        let content_start = 1 + hash_count + 1;
        let content_end = raw.len() - hash_count - 1;
        raw[content_start..content_end].to_owned()
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

    fn decode_hex_escape(
        &self,
        chars: &mut CharIndices<'_>,
        source_index: usize,
    ) -> Result<char, LexError> {
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

        Ok(value as char)
    }

    fn decode_unicode_escape(
        &self,
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
}
